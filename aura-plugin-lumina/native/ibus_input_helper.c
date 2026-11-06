/* Linux IBus native engine process. This is a REAL libibus client:
 * all P/C/H events below come from an IBusInputContext signal attached
 * to a live daemon engine, not from an injected test stub.
 * Protocol: stdout READY; input F1,F0 or K<TAB>keysym<TAB>mods<TAB>released<TAB>code; output
 * P<TAB>hexUTF8<TAB>Unicode-scalar-caret, C<TAB>hexUTF8,
 * H, then R<TAB>handled, or A (focus acknowledgement).\n * G<TAB>screen_x<TAB>screen_y<TAB>width<TAB>height sets genuine IBus cursor rectangle, A acknowledges.
 * The Rust side validates every line and applies strict timeouts/bounds.
 */
#include <ibus.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <errno.h>
#include <string.h>
#include <unistd.h>
#ifdef LUMINA_IBUS_TEST_STOP_AFTER_DELETE
#include <signal.h>
#endif
/* Phase34 diagnostic only: observational CALL / POST_CALL_PUMP / IDLE
 * phase of REAL libibus callbacks. This is NOT engine transaction provenance.
 * Ordinary production builds omit both the instrumentation and phase logs.
 * Never report committed text, surrounding text or caret data here. */
#ifdef LUMINA_IBUS_TEST_CALLBACK_PHASE
enum p34_callback_phase { P34_IDLE, P34_CALL, P34_PUMP };
static enum p34_callback_phase p34_phase=P34_IDLE;
static unsigned p34_signal_counter=0;
static void p34_signal_observed(char kind){
    const char *phase=p34_phase==P34_CALL ? "CALL" :
        p34_phase==P34_PUMP ? "PUMP" : "IDLE";
    fprintf(stderr,"LUMINA_IBUS_P34_CALLBACK kind=%c phase=%s serial=%u\n",
        kind,phase,++p34_signal_counter);
    fflush(stderr);
}
#else
#define p34_signal_observed(kind) ((void)0)
#endif
/* Bounded, decimal-only field decoder; rejects signs, overflows and junk. */
static int dec_field(char **at, unsigned long *value, int last) {
    char *p=*at, *end=NULL;
    if (*p<'0' || *p>'9') return 0;
    errno=0;
    unsigned long n=strtoul(p,&end,10);
    if (errno==ERANGE || end==p) return 0;
    if (last) {
        if (*end!='\n' && *end!='\0') return 0;
    } else {
        if (*end!='\t') return 0;
        *at=end+1;
    }
    *value=n;
    return 1;
}
static void hex_print(const char *s) {
    if (!s) return;
    for (const unsigned char *p=(const unsigned char*)s; *p; ++p)
        printf("%02x",(unsigned)*p);
}
static void commit_text(IBusInputContext *ctx, IBusText *t, gpointer ud) {
    (void)ctx;(void)ud;
    p34_signal_observed('C');
    printf("C\t"); hex_print(t ? t->text : ""); printf("\n"); fflush(stdout);
}
static void update_preedit(IBusInputContext *ctx,IBusText *t,
    guint cursor,gboolean visible,gpointer ud) {
    (void)ctx;(void)ud;
    if (!visible) {puts("H");fflush(stdout);return;}
    printf("P\t");hex_print(t ? t->text : "");printf("\t%u\n",cursor);
    fflush(stdout);
}
static void hide_preedit(IBusInputContext *ctx,gpointer ud) {
    (void)ctx;(void)ud;puts("H");fflush(stdout);
}
/* All lookup data below originate from the active IBus engine signal.
 * Hard cap: 8 candidates/page, 128 UTF-8 bytes/candidate, 4096-byte frame.
 * Never emit a fabricated candidate or an unbounded external text payload. */
static void hide_lookup(IBusInputContext *ctx,gpointer ud) {
    (void)ctx;(void)ud;puts("L\t0");fflush(stdout);
}
static void update_lookup(IBusInputContext *ctx,IBusLookupTable *table,
    gboolean visible,gpointer ud) {
    (void)ctx;(void)ud;
    if(!visible || !table){hide_lookup(ctx,ud);return;}
    guint total=ibus_lookup_table_get_number_of_candidates(table);
    guint size=ibus_lookup_table_get_page_size(table);
    guint cursor=ibus_lookup_table_get_cursor_pos(table);
    if(total==0 || total>100000 || size==0 || size>16 || cursor>=total) {
        hide_lookup(ctx,ud);return;
    }
    guint first=(cursor/size)*size;
    guint end=first+size;
    if(end>total)end=total;
    /* Keep real engine selection in the visible eight-row window, even
     * when the source IME has a nine/sixteen-candidate page. */
    if(cursor-first>=8)first=cursor-7;
    guint limit=end-first;
    if(limit>8)limit=8;
    /* Reject a page rather than silently truncating unsupported candidate
     * strings or pretending to offer complete candidate selection. */
    for(guint i=0;i<limit;i++){
        IBusText *v=ibus_lookup_table_get_candidate(table,first+i);
        if(!v || !v->text || strlen(v->text)>128 ||
           !g_utf8_validate(v->text,-1,NULL)){hide_lookup(ctx,ud);return;}
    }
    printf("L\t1\t%u\t%u\t%u",cursor,first,total);
    for(guint i=0;i<limit;i++){
        IBusText *v=ibus_lookup_table_get_candidate(table,first+i);
        printf("\t");hex_print(v->text);
    }
    printf("\n");fflush(stdout);
    fprintf(stderr,"LUMINA_IBUS_LIVE_LOOKUP total=%u cursor=%u page_start=%u shown=%u\n",
        total,cursor,first,limit);
}
/* Engine-origin requests are observable but NEVER blindly authoritative.
 * A delete request is delivered as bounded advisory D frame to Rust;
 * phase 22 forbids applying it until transaction/selection proof exists. */
static void request_surrounding(IBusInputContext *ctx,gpointer data) {
    (void)ctx;(void)data;
    fprintf(stderr,"LUMINA_IBUS_REQUIRE_SURROUNDING\n");
    puts("N");fflush(stdout);
}
static void delete_surrounding(IBusInputContext *ctx,gint offset,guint count,gpointer data) {
    (void)ctx;(void)data;
    p34_signal_observed('D');
    fprintf(stderr,"LUMINA_IBUS_DELETE_REQUEST offset=%d count=%u (not applied)\n",
        offset,count);
    printf("D\t%d\t%u\n",offset,count);fflush(stdout);
#ifdef LUMINA_IBUS_TEST_STOP_AFTER_DELETE
    /* Diagnostic-only binary, compiled with an explicit test flag, NOT
     * present in normal installed helpers. Stop AFTER a genuine upstream
     * nonempty delete, BEFORE any later CommitText from the same real key.
     * The isolated Rust test kills the actual daemon while this thread is
     * stopped, then resumes us. No IBus C/D/P signal is injected here. */
    if(offset<0 && count>0){
        fputs("LUMINA_IBUS_P31_GENUINE_D_PAUSED_BEFORE_C\n",stderr);
        fflush(stderr);
        raise(SIGSTOP);
    }
#endif
}
/* The daemon connection itself is a security-sensitive generation.
 * An automatic libibus reconnection must NEVER revive an old context. */
static void daemon_disconnected(IBusBus *bus,gpointer user_data){
    (void)bus;
    gboolean *lost=(gboolean *)user_data;
    *lost=TRUE;
    fputs("LUMINA_IBUS_DAEMON_GENERATION_LOST (no text logged)\n",stderr);
}
static int hex_decode_utf8(char *hex,char *out,size_t capacity) {
    size_t n=strlen(hex);
    if(n%2 || n>2048 || n/2>=capacity) return 0;
    for(size_t i=0;i<n;i+=2){
        unsigned value=0;
        for(unsigned j=0;j<2;j++){
            unsigned char c=(unsigned char)hex[i+j];
            unsigned digit=(c>='0' && c<='9')?c-'0':
               (c>='a' && c<='f')?c-'a'+10:256;
            if(digit>15)return 0;
            value=(value<<4)|digit;
        }
        out[i/2]=(char)value;
        if(!value)return 0;
    }
    out[n/2]='\0';
    return g_utf8_validate(out,-1,NULL);
}
static void pump_ms(unsigned ms){
    for(unsigned i=0;i<ms;i++){
        while(g_main_context_iteration(NULL,FALSE)){}
        g_usleep(1000);
    }
}
int main(int argc,char**argv) {
    setvbuf(stdout,NULL,_IOLBF,0);
    if(argc!=2 || strlen(argv[1])>80) {fputs("FATAL engine_arg\n",stdout);return 2;}
    ibus_init();
    IBusBus *bus=ibus_bus_new();
    if(!bus || !ibus_bus_is_connected(bus)){
        puts("FATAL no_live_ibus_daemon");return 3;
    }
    gboolean daemon_lost=FALSE;
    g_signal_connect(bus,"disconnected",G_CALLBACK(daemon_disconnected),&daemon_lost);
    IBusInputContext *ic=ibus_bus_create_input_context(bus,"lumina-native");
    if(!ic){puts("FATAL context_unavailable");return 4;}
    g_signal_connect(ic,"update-preedit-text",G_CALLBACK(update_preedit),NULL);
    g_signal_connect(ic,"commit-text",G_CALLBACK(commit_text),NULL);
    g_signal_connect(ic,"hide-preedit-text",G_CALLBACK(hide_preedit),NULL);
    g_signal_connect(ic,"update-lookup-table",G_CALLBACK(update_lookup),NULL);
    g_signal_connect(ic,"hide-lookup-table",G_CALLBACK(hide_lookup),NULL);
    g_signal_connect(ic,"require-surrounding-text",G_CALLBACK(request_surrounding),NULL);
    g_signal_connect(ic,"delete-surrounding-text",G_CALLBACK(delete_surrounding),NULL);
    const guint base_caps=IBUS_CAP_FOCUS | IBUS_CAP_PREEDIT_TEXT | IBUS_CAP_LOOKUP_TABLE;
    int surrounding_enabled=0;
    ibus_input_context_set_capabilities(ic,base_caps);
    ibus_input_context_set_engine(ic,argv[1]);
    pump_ms(350);
    if(daemon_lost || !ibus_bus_is_connected(bus)){
        puts("FATAL daemon_generation_lost");return 7;
    }
    IBusEngineDesc *initial=ibus_input_context_get_engine(ic);
    fprintf(stderr,"LUMINA_IBUS_ACTUAL_ENGINE %s\n",
        initial ? ibus_engine_desc_get_name(initial) : "(none)");
    puts("READY");
    /* Fixed input frame: no getline allocation before checking its size. */
    char line[2112];
    int exit_code=0;
#define CHECK_DAEMON_GENERATION() do { \
        if(daemon_lost || !ibus_bus_is_connected(bus)){ \
            puts("FATAL daemon_generation_lost"); \
            exit_code=7;goto helper_cleanup; \
        } \
    } while(0)
    while(fgets(line,sizeof(line),stdin)) {
        if (!strchr(line,'\n')) {
            puts("FATAL input_too_long");break;
        }
        pump_ms(1);
        CHECK_DAEMON_GENERATION();
#ifdef LUMINA_IBUS_TEST_CALLBACK_PHASE
        /* Phase35 separate TEST-ONLY helper: bounded post-R idle GLib drain.
         * No physical key dispatch (NO B/R), no fabricated engine signals,
         * no text/keys/cursor in diagnostic stderr. The normal installed
         * helper does not recognize this command at all. */
        if(!strncmp(line,"I\t",2)){
            char *duration=line+2;
            unsigned long ms=0;
            if(!dec_field(&duration,&ms,1)||ms>200UL){
                puts("FATAL invalid_idle_probe");
                exit_code=6;break;
            }
            fprintf(stderr,"LUMINA_IBUS_P35_IDLE_DRAIN ms=%lu (no content)\n",ms);
            pump_ms((unsigned)ms);
            CHECK_DAEMON_GENERATION();
            puts("A");continue;
        }
#endif
        if(!strcmp(line,"T\n")){
            /* Privacy-neutral read-only health probe forces dispatch of
             * the real daemon-disconnected signal while input is idle. */
            pump_ms(3);
            CHECK_DAEMON_GENERATION(); puts("A");continue;
        }
        if(!strcmp(line,"V0\n") || !strcmp(line,"V1\n")){
            // Clear old text BEFORE changing the external engine capability.
            IBusText *empty=ibus_text_new_from_string("");
            g_object_ref_sink(empty);
            ibus_input_context_set_surrounding_text(ic,empty,0,0);
            g_object_unref(empty);
            int enable=(line[1]=='1');
            ibus_input_context_set_capabilities(ic,base_caps |
                (enable ? IBUS_CAP_SURROUNDING_TEXT : 0));
            ibus_input_context_reset(ic);
            surrounding_enabled=enable;
            pump_ms(20);
            fprintf(stderr,"LUMINA_IBUS_SURROUNDING_POLICY enabled=%d (content redacted)\n",enable);
            CHECK_DAEMON_GENERATION(); puts("A");continue;
        }
        if(!strcmp(line,"F1\n")){
            ibus_input_context_focus_in(ic);
            pump_ms(200);
            // A deterministic opt-in test requests the REAL Hangul switch.
            // The subsequent engine preedit and commit remain engine-origin.
            if (g_getenv("LUMINA_IBUS_TEST_HANGUL_SWITCH") != NULL) {
                gboolean toggled=ibus_input_context_process_key_event(
                    ic,IBUS_KEY_Hangul,0,0);
                fprintf(stderr,"LUMINA_IBUS_HANGUL_SWITCH handled=%d\n",toggled);
                pump_ms(150);
            }
            IBusEngineDesc *focused=ibus_input_context_get_engine(ic);
            const char *actual=focused ? ibus_engine_desc_get_name(focused) : NULL;
            fprintf(stderr,"LUMINA_IBUS_FOCUSED_ENGINE %s\n",
                actual ? actual : "(none)");
            if(!actual || strcmp(actual,argv[1])!=0) {
                puts("FATAL requested_engine_not_active");break;
            }
            CHECK_DAEMON_GENERATION(); puts("A");continue;
        }
        if(!strcmp(line,"F0\n")){
            ibus_input_context_focus_out(ic);
            ibus_input_context_reset(ic);
            IBusText *empty=ibus_text_new_from_string("");
            g_object_ref_sink(empty);
            ibus_input_context_set_surrounding_text(ic,empty,0,0);
            g_object_unref(empty);
            pump_ms(40);CHECK_DAEMON_GENERATION(); puts("A");continue;
        }
        if(!strncmp(line,"S\t",2)){
            if(!surrounding_enabled){
                puts("FATAL surrounding_not_authorized");
                exit_code=6;break;
            }
            char *start=line+2,*cursor=strchr(start,'\t');
            if(!cursor){puts("FATAL invalid_surrounding_frame");break;}
            *cursor++='\0';
            char *anchor=strchr(cursor,'\t');
            if(!anchor){puts("FATAL invalid_surrounding_frame");break;}
            *anchor++='\0';
            unsigned long c,a;
            char *cur_field=cursor,*anchor_field=anchor;
            char utf8[1025];
            if(!dec_field(&cur_field,&c,1)||
               !dec_field(&anchor_field,&a,1)||
               !hex_decode_utf8(start,utf8,sizeof(utf8))) {
                puts("FATAL invalid_surrounding_frame");break;
            }
            glong count=g_utf8_strlen(utf8,-1);
            if(c>(unsigned long)count || a>(unsigned long)count ||
               count>1024 || c>1024 || a>1024){
                puts("FATAL invalid_surrounding_cursor");break;
            }
            IBusText *text=ibus_text_new_from_string(utf8);
            g_object_ref_sink(text);
            ibus_input_context_set_surrounding_text(ic,text,(guint32)c,(guint32)a);
            g_object_unref(text);
            pump_ms(10);
            fprintf(stderr,"LUMINA_IBUS_SURROUNDING_SET bytes=%zu scalars=%ld cursor=%lu anchor=%lu\n",
                strlen(utf8),count,c,a);
            CHECK_DAEMON_GENERATION(); puts("A");continue;
        }
        if(!strncmp(line,"G\t",2)){
            char *cursor=line+2;
            unsigned long x,y,w,h;
            if(!dec_field(&cursor,&x,0) ||
               !dec_field(&cursor,&y,0) ||
               !dec_field(&cursor,&w,0) ||
               !dec_field(&cursor,&h,1) ||
               x>32767UL || y>32767UL || w==0 || h==0 ||
               w>128UL || h>128UL || x+w>32768UL || y+h>32768UL){
                puts("FATAL invalid_cursor_frame");break;
            }
            /* Genuine GTK/IBus screen-coordinate cursor location, not a
             * host-local candidate or fabricated daemon callback. */
            ibus_input_context_set_cursor_location(ic,(gint)x,(gint)y,(gint)w,(gint)h);
            pump_ms(10);
            fprintf(stderr,"LUMINA_IBUS_CURSOR_LOCATION x=%lu y=%lu w=%lu h=%lu\n",
                x,y,w,h);
            CHECK_DAEMON_GENERATION(); puts("A");continue;
        }
        if(!strncmp(line,"K\t",2)){
            char *cursor=line+2;
            unsigned long sym,mod,release,code;
            if(!dec_field(&cursor,&sym,0) ||
               !dec_field(&cursor,&mod,0) ||
               !dec_field(&cursor,&release,0) ||
               !dec_field(&cursor,&code,1) ||
               sym>0x0110ffffUL || mod>15UL || release>1UL || code>65535UL) {
                puts("FATAL invalid_key_frame");break;
            }
            guint state=0;
            if(mod&1) state|=IBUS_SHIFT_MASK;
            if(mod&2) state|=IBUS_CONTROL_MASK;
            if(mod&4) state|=IBUS_MOD1_MASK;
            if(mod&8) state|=IBUS_SUPER_MASK;
            if(release) state|=IBUS_RELEASE_MASK;
            /* Local key-dispatch BEGIN marker. The preceding pump_ms(1)
             * may flush genuine but STALE asynchronous engine signals.
             * B does NOT assert external engine provenance; Rust must drop
             * all P/C/D/L/H/N seen before B for this new key. */
            puts("B");
#ifdef LUMINA_IBUS_TEST_CALLBACK_PHASE
            p34_phase=P34_CALL;
#endif
            gboolean handled=ibus_input_context_process_key_event(
                ic,(guint32)sym,(guint32)code,state);
#ifdef LUMINA_IBUS_TEST_CALLBACK_PHASE
            p34_phase=P34_PUMP;
#endif
            fprintf(stderr,"LUMINA_IBUS_PROCESS_KEY keysym=%lu modifiers=%lu release=%lu code=%lu handled=%d\n",
                sym,mod,release,code,handled);
            pump_ms(25);
#ifdef LUMINA_IBUS_TEST_CALLBACK_PHASE
            p34_phase=P34_IDLE;
#endif
            CHECK_DAEMON_GENERATION();
            printf("R\t%d\n",!!handled);continue;
        }
        puts("FATAL invalid_command");break;
    }
helper_cleanup:
    ibus_input_context_focus_out(ic);
    g_object_unref(ic);
    g_object_unref(bus);
    return exit_code;
}
