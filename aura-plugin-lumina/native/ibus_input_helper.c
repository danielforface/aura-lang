/* Linux IBus native engine process. This is a REAL libibus client:
 * all P/C/H events below come from an IBusInputContext signal attached
 * to a live daemon engine, not from an injected test stub.
 * Protocol: stdout READY; input F1,F0 or K<TAB>keysym<TAB>mods<TAB>released<TAB>code; output
 * P<TAB>hexUTF8<TAB>Unicode-scalar-caret, C<TAB>hexUTF8,
 * H, then R<TAB>handled, or A (focus acknowledgement).
 * The Rust side validates every line and applies strict timeouts/bounds.
 */
#include <ibus.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <errno.h>
#include <string.h>
#include <unistd.h>
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
    guint limit=total-first;
    if(limit>size)limit=size;
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
    IBusInputContext *ic=ibus_bus_create_input_context(bus,"lumina-native");
    if(!ic){puts("FATAL context_unavailable");return 4;}
    g_signal_connect(ic,"update-preedit-text",G_CALLBACK(update_preedit),NULL);
    g_signal_connect(ic,"commit-text",G_CALLBACK(commit_text),NULL);
    g_signal_connect(ic,"hide-preedit-text",G_CALLBACK(hide_preedit),NULL);
    g_signal_connect(ic,"update-lookup-table",G_CALLBACK(update_lookup),NULL);
    g_signal_connect(ic,"hide-lookup-table",G_CALLBACK(hide_lookup),NULL);
    ibus_input_context_set_capabilities(ic,IBUS_CAP_FOCUS | IBUS_CAP_PREEDIT_TEXT | IBUS_CAP_LOOKUP_TABLE);
    ibus_input_context_set_engine(ic,argv[1]);
    pump_ms(350);
    IBusEngineDesc *initial=ibus_input_context_get_engine(ic);
    fprintf(stderr,"LUMINA_IBUS_ACTUAL_ENGINE %s\n",
        initial ? ibus_engine_desc_get_name(initial) : "(none)");
    puts("READY");
    /* Fixed input frame: no getline allocation before checking its size. */
    char line[129];
    while(fgets(line,sizeof(line),stdin)) {
        if (!strchr(line,'\n')) {
            puts("FATAL input_too_long");break;
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
            puts("A");continue;
        }
        if(!strcmp(line,"F0\n")){
            ibus_input_context_focus_out(ic);
            ibus_input_context_reset(ic);
            pump_ms(40);puts("A");continue;
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
            gboolean handled=ibus_input_context_process_key_event(
                ic,(guint32)sym,(guint32)code,state);
            fprintf(stderr,"LUMINA_IBUS_PROCESS_KEY keysym=%lu modifiers=%lu release=%lu code=%lu handled=%d\n",
                sym,mod,release,code,handled);
            pump_ms(25);
            printf("R\t%d\n",!!handled);continue;
        }
        puts("FATAL invalid_command");break;
    }
    ibus_input_context_focus_out(ic);
    g_object_unref(ic);
    g_object_unref(bus);
    return 0;
}
