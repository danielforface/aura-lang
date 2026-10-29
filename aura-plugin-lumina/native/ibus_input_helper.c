/* Linux IBus native engine process. This is a REAL libibus client:
 * all P/C/H events below come from an IBusInputContext signal attached
 * to a live daemon engine, not from an injected test stub.
 * Protocol: stdout READY; input F1,F0 or K<TAB>keysym; output
 * P<TAB>hexUTF8<TAB>Unicode-scalar-caret, C<TAB>hexUTF8,
 * H, then R<TAB>handled, or A (focus acknowledgement).
 * The Rust side validates every line and applies strict timeouts/bounds.
 */
#include <ibus.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>
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
    ibus_input_context_set_capabilities(ic,IBUS_CAP_FOCUS | IBUS_CAP_PREEDIT_TEXT);
    ibus_input_context_set_engine(ic,argv[1]);
    pump_ms(350);
    IBusEngineDesc *initial=ibus_input_context_get_engine(ic);
    fprintf(stderr,"LUMINA_IBUS_ACTUAL_ENGINE %s\n",
        initial ? ibus_engine_desc_get_name(initial) : "(none)");
    puts("READY");
    char *line=NULL;size_t cap=0;
    while(getline(&line,&cap,stdin)!=-1){
        size_t n=strlen(line);
        if(n>128){puts("FATAL input_too_long");break;}
        if(!strncmp(line,"F1",2)){
            ibus_input_context_focus_in(ic);
            ibus_input_context_property_activate(ic,"InputMode",PROP_STATE_CHECKED);
            pump_ms(200);
            IBusEngineDesc *focused=ibus_input_context_get_engine(ic);
            fprintf(stderr,"LUMINA_IBUS_FOCUSED_ENGINE %s\n",
                focused ? ibus_engine_desc_get_name(focused) : "(none)");
            puts("A");continue;
        }
        if(!strncmp(line,"F0",2)){
            ibus_input_context_focus_out(ic);
            ibus_input_context_reset(ic);
            pump_ms(40);puts("A");continue;
        }
        if(!strncmp(line,"K\t",2)){
            char *end=NULL;
            unsigned long key=strtoul(line+2,&end,10);
            if(end==line+2 || (end && *end!='\n') || key>0x10FFFFUL){
                puts("R\t0");continue;
            }
            gboolean handled=ibus_input_context_process_key_event(ic,(guint32)key,0,0);
            fprintf(stderr,"LUMINA_IBUS_PROCESS_KEY key=%lu handled=%d\n",key,handled);
            // ProcessKeyEvent replies synchronously, but the input-context
            // signals are received by the GLib main loop.
            pump_ms(25);
            printf("R\t%d\n",!!handled);continue;
        }
        puts("FATAL invalid_command");break;
    }
    free(line);
    ibus_input_context_focus_out(ic);
    g_object_unref(ic);
    g_object_unref(bus);
    return 0;
}
