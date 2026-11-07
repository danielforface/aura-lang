/*
 * AURA X-RAY XR-03 — evidence-bearing, offline capture explorer and comparison.
 * No OS privileges, no network, no external JS or CSS. All report inputs are
 * parsed as UNTRUSTED JSONL with byte/row/field limits and counter checks.
 * Every score decomposition invokes real Aura-compiled cells.
 */
#define _CRT_SECURE_NO_WARNINGS
#include "xr03_explorer.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <errno.h>
#include <inttypes.h>
#include <limits.h>

extern uint32_t xray_cpu_impact(uint32_t);
extern uint32_t xray_io_impact(uint32_t,uint32_t);
extern uint32_t xray_memory_impact(uint32_t);

#define XR3_MAX_LINES 135000u
#define XR3_MAX_EVENTS 16000u
#define XR3_MAX_FILE 80000000L
#define XR3_MAX_LINE 16384u
#define XR3_MAX_FIELDS 32u

typedef struct { char key[80], val[1200]; unsigned char quoted; } XR3Field;
typedef struct { XR3Field f[XR3_MAX_FIELDS]; unsigned n; } XR3Object;
typedef struct {
    uint32_t sample,pid,score,cpu,read,write,ws_mib;
    uint32_t c_impact,i_impact,m_impact;
    uint64_t creation;
    int64_t delta_kib;
    char name[160], delta_quality[28];
    unsigned char has_score,has_ws;
    uint32_t parent_pid,thread_count;
    unsigned char has_topology,rate_reverified;
} XR3Event;
/* Exact-raw counters are added starting in XR-04 captures. Legacy /2
 * recordings remain replayable, but MUST be labeled score-only audited. */
typedef struct {
    uint32_t pid;
    uint64_t creation,ws_kib,read_bytes,write_bytes,cpu_ticks;
    unsigned char has_id,has_ws,has_io,has_cpu;
} XR3Raw;
typedef struct {
    char sha[48], version[36], privacy[48];
    uint32_t seconds, samples;
    uint64_t elapsed_ms,records,derived,denied,open_failed,errors,unknown,reset,reuse,capacity;
    uint64_t time_ms[31],cpu_sum[31],read_sum[31],write_sum[31],growth_sum[31];
    unsigned counts[31],valid[31],peak[31],quality_missing[31];
    XR3Event *events;
    unsigned kept, truncated, actual_samples;
    uint64_t source_rate_verified;
    unsigned raw_precision;
} XR3Run;

static void xr3_error(char *dst,size_t cap,const char *msg,unsigned line){
    if(cap) (void)snprintf(dst,cap,"capture/2 validation: %s at line %u",msg,line);
}
static void skipspace(const char **p){while(**p==' '||**p=='\t'||**p=='\r'||**p=='\n')++*p;}
static int hexchar(char c){if(c>='0'&&c<='9')return c-'0';if(c>='A'&&c<='F')return c-'A'+10;if(c>='a'&&c<='f')return c-'a'+10;return -1;}
/* Decode top-level JSON string safely; reject controls/surrogate escapes. */
static int jstring(const char **pp,char *dst,size_t cap){
    const unsigned char *p=(const unsigned char *)*pp; size_t n=0;
    if(*p!='"')return 0; ++p;
    while(*p && *p!='"'){
        unsigned char c=*p++;
        if(c=='\\'){
            unsigned char e=*p++;if(!e)return 0;
            if(e=='u'){
                unsigned v=0;
                for(int i=0;i<4;i++){int h=hexchar((char)*p++);if(h<0)return 0;v=v*16u+(unsigned)h;}
                if(v>=0xD800&&v<=0xDFFF)return 0;
                if(v<0x80)c=(unsigned char)v;
                else {
                    /* UTF-8 encoding for direct filenames. */
                    if(v<0x800){
                        if(n+2>=cap)return 0;
                        dst[n++]=(char)(0xC0u|(v>>6));dst[n++]=(char)(0x80u|(v&63u));continue;
                    }
                    if(n+3>=cap)return 0;
                    dst[n++]=(char)(0xE0u|(v>>12));dst[n++]=(char)(0x80u|((v>>6)&63u));dst[n++]=(char)(0x80u|(v&63u));continue;
                }
            }else if(e=='"'||e=='\\'||e=='/')c=e;
            else if(e=='n')c='\n';else if(e=='r')c='\r';else if(e=='t')c='\t';
            else if(e=='b')c='\b';else if(e=='f')c='\f';else return 0;
        }else if(c<0x20)return 0;
        if(c==0 || n+1>=cap)return 0;
        dst[n++]=(char)c;
    }
    if(*p!='"')return 0; ++p;
    dst[n]=0;*pp=(const char*)p;return 1;
}
static int flatjson(const char *line,XR3Object *obj){
    const char *p=line;obj->n=0;skipspace(&p);if(*p++!='{')return 0;
    skipspace(&p);if(*p=='}'){p++;skipspace(&p);return *p==0;}
    for(;;){
        if(obj->n>=XR3_MAX_FIELDS)return 0;
        XR3Field *f=&obj->f[obj->n];
        if(!jstring(&p,f->key,sizeof f->key))return 0;
        for(unsigned i=0;i<obj->n;i++)if(strcmp(f->key,obj->f[i].key)==0)return 0;
        skipspace(&p);if(*p++!=':')return 0;skipspace(&p);
        f->quoted=(*p=='"');
        if(f->quoted){if(!jstring(&p,f->val,sizeof f->val))return 0;}
        else{
            size_t len=0;
            while(*p && *p!=',' && *p!='}' && *p!=' ' && *p!='\r' && *p!='\t' && *p!='\n'){
                if(len+1>=sizeof f->val)return 0;
                f->val[len++]=*p++;
            }
            f->val[len]=0;if(!len)return 0;
            if(strcmp(f->val,"null")!=0 && strcmp(f->val,"true")!=0 &&
               strcmp(f->val,"false")!=0){
                const char *q=f->val;
                if(*q=='-')q++;
                if(!isdigit((unsigned char)*q))return 0;
                for(;*q;q++)if(!isdigit((unsigned char)*q))return 0;
            }
        }
        obj->n++;skipspace(&p);
        if(*p=='}'){p++;skipspace(&p);return *p==0;}
        if(*p++!=',')return 0;skipspace(&p);
    }
}
static const char *field(const XR3Object *o,const char *key){
    for(unsigned i=0;i<o->n;i++)if(strcmp(o->f[i].key,key)==0)return o->f[i].val;
    return NULL;
}
static int u64(const XR3Object *o,const char *key,uint64_t *value){
    const char *p=field(o,key);if(!p||!p[0]||*p=='-'||strcmp(p,"null")==0)return 0;
    for(const char *q=p;*q;q++)if(!isdigit((unsigned char)*q))return 0;
    errno=0;unsigned long long v=strtoull(p,NULL,10);if(errno==ERANGE)return 0;
    *value=(uint64_t)v;return 1;
}
static int i64(const XR3Object *o,const char *key,int64_t *value){
    const char *p=field(o,key);if(!p||!p[0]||strcmp(p,"null")==0)return 0;
    const char *q=p;if(*q=='-')q++;
    if(!*q)return 0;for(;*q;q++)if(!isdigit((unsigned char)*q))return 0;
    errno=0;long long v=strtoll(p,NULL,10);if(errno==ERANGE)return 0;
    *value=(int64_t)v;return 1;
}
static int get32(const XR3Object *o,const char *key,uint32_t *v){
    uint64_t x=0;if(!u64(o,key,&x)||x>UINT32_MAX)return 0;*v=(uint32_t)x;return 1;
}
static int copystr(char *dest,size_t cap,const char *s){
    if(!s)return 0;size_t n=strlen(s);if(n>=cap)return 0;
    memcpy(dest,s,n+1);return 1;
}
static int valid_sha(const char *v){
    if(!v||strlen(v)!=40)return 0;
    for(int i=0;i<40;i++)if(hexchar(v[i])<0)return 0;
    return 1;
}
static int parsefile(const char *path,XR3Run *run,int allow_names,char *error,size_t cap){
    FILE *f=NULL;
    if(fopen_s(&f,path,"rb")!=0||!f){xr3_error(error,cap,"cannot open input",0);return 0;}
    if(fseek(f,0,SEEK_END)!=0){fclose(f);xr3_error(error,cap,"seek failed",0);return 0;}
    long size=ftell(f);
    if(size<20||size>XR3_MAX_FILE||fseek(f,0,SEEK_SET)!=0){
        fclose(f);xr3_error(error,cap,"input size outside bounded budget",0);return 0;
    }
    run->events=(XR3Event*)calloc(XR3_MAX_EVENTS,sizeof(XR3Event));
    if(!run->events){fclose(f);xr3_error(error,cap,"cannot allocate bounded history",0);return 0;}
    unsigned stage=0,line_no=0,active=0,observed=0,vectors=0,peaks=0;
    uint64_t recorded=0,computed=0,readable=0,actual_readable=0;
    XR3Raw *raw_prev=NULL,*raw_curr=NULL;
    unsigned raw_prev_count=0;
    uint64_t prior_ms=0;
    char line[XR3_MAX_LINE];
    int good=1;
    while(fgets(line,sizeof line,f)){
        line_no++;
        if(line_no>XR3_MAX_LINES){xr3_error(error,cap,"too many records",line_no);good=0;break;}
        size_t len=strlen(line);
        if(!len||(line[len-1]!='\n'&&!feof(f))){
            xr3_error(error,cap,"line longer than bounded JSONL limit",line_no);good=0;break;
        }
        XR3Object obj;
        if(!flatjson(line,&obj)){xr3_error(error,cap,"malformed JSON object or duplicate field",line_no);good=0;break;}
        const char *type=field(&obj,"type");
        if(!type){xr3_error(error,cap,"missing record type",line_no);good=0;break;}
        if(strcmp(type,"capture_start")==0){
            const char *schema=field(&obj,"schema"),*sha=field(&obj,"build");
            if(stage||!schema||strcmp(schema,"aura.xray.capture/2")!=0||!valid_sha(sha)||
               !get32(&obj,"seconds",&run->seconds)||run->seconds<1||run->seconds>30||
               !copystr(run->sha,sizeof run->sha,sha)||
               !copystr(run->version,sizeof run->version,field(&obj,"version"))||
               !copystr(run->privacy,sizeof run->privacy,field(&obj,"privacy"))){
                xr3_error(error,cap,"invalid capture start or unsupported schema",line_no);good=0;break;
            }
            run->raw_precision=(strncmp(run->version,"0.4.",4)==0);
            if(run->raw_precision){
                raw_prev=(XR3Raw*)calloc(4096,sizeof(XR3Raw));
                raw_curr=(XR3Raw*)calloc(4096,sizeof(XR3Raw));
                if(!raw_prev||!raw_curr){
                    xr3_error(error,cap,"exact-counter history allocation failed",line_no);
                    good=0;break;
                }
            }
            stage=1;
        }else if(strcmp(type,"sample_start")==0){
            uint32_t s=0;uint64_t ms=0;
            if(stage!=1||active||!get32(&obj,"sample",&s)||
               s!=run->actual_samples+1||s>run->seconds||!u64(&obj,"monotonic_ms",&ms)||
               (s>1&&ms<=run->time_ms[s-1])){
                xr3_error(error,cap,"invalid sample sequencing or clock",line_no);good=0;break;
            }
            run->actual_samples=s;active=s;run->time_ms[s]=ms;observed=0;vectors=0;peaks=0;
        }else if(strcmp(type,"process")==0){
            uint32_t sample=0,pid=0,score=0,cpu=0,read=0,write=0,ws=0;
            uint64_t creation=0;int64_t delta=0;
            const char *dq=field(&obj,"delta_quality");
            (void)u64(&obj,"process_create_filetime",&creation);
            XR3Raw *next_raw=NULL;
            unsigned rate_reverified=0;
            if(run->raw_precision){
                if(!raw_curr){
                    xr3_error(error,cap,"raw counter tracker unavailable",line_no);good=0;break;
                }
                next_raw=&raw_curr[observed];
            }
            if(stage!=1||!active||!get32(&obj,"sample",&sample)||sample!=active||
               !get32(&obj,"pid",&pid)||!dq||strlen(dq)>=28||
               observed>=4096){
                xr3_error(error,cap,"invalid process/sample association or capacity",line_no);good=0;break;
            }
            if(next_raw){
                next_raw->pid=pid;
                next_raw->creation=creation;
                next_raw->has_id=(unsigned char)(creation!=0);
                uint64_t coarse=0;
                next_raw->has_ws=(unsigned char)u64(&obj,"working_set_kib",&next_raw->ws_kib);
                next_raw->has_cpu=(unsigned char)u64(&obj,"cpu_total_100ns",&next_raw->cpu_ticks);
                if(next_raw->has_cpu){
                    if(!u64(&obj,"cpu_total_ms",&coarse)||
                        coarse!=next_raw->cpu_ticks/10000u){
                        xr3_error(error,cap,"exact CPU counter conflicts with coarse CPU",line_no);
                        good=0;break;
                    }
                }else if(field(&obj,"cpu_total_ms")&&
                         strcmp(field(&obj,"cpu_total_ms"),"null")!=0){
                    xr3_error(error,cap,"exact CPU counter missing",line_no);good=0;break;
                }
                next_raw->has_io=(unsigned char)
                    (u64(&obj,"cumulative_io_read_bytes",&next_raw->read_bytes) &&
                     u64(&obj,"cumulative_io_write_bytes",&next_raw->write_bytes));
                if(next_raw->has_io){
                    if(!u64(&obj,"cumulative_io_read_kib",&coarse)||
                       coarse!=next_raw->read_bytes/1024u ||
                       !u64(&obj,"cumulative_io_write_kib",&coarse)||
                       coarse!=next_raw->write_bytes/1024u){
                        xr3_error(error,cap,"exact I/O counters conflict with coarse I/O",line_no);
                        good=0;break;
                    }
                }else if((field(&obj,"cumulative_io_read_kib")&&
                          strcmp(field(&obj,"cumulative_io_read_kib"),"null")!=0)||
                         (field(&obj,"cumulative_io_write_kib")&&
                          strcmp(field(&obj,"cumulative_io_write_kib"),"null")!=0)){
                    xr3_error(error,cap,"exact I/O counters missing",line_no);good=0;break;
                }
                uint32_t topology_field=0;
                if(!get32(&obj,"parent_pid",&topology_field)||
                   !get32(&obj,"thread_count",&topology_field)){
                    xr3_error(error,cap,"topology sample fields missing",line_no);
                    good=0;break;
                }
            }
            observed++;recorded++;
            const char *rawscore=field(&obj,"activity_score");
            if(!rawscore){xr3_error(error,cap,"missing activity score field",line_no);good=0;break;}
            int has_score=strcmp(rawscore,"null")!=0;
            if(has_score){
                if(sample==1||strcmp(dq,"observed")!=0||
                   !get32(&obj,"activity_score",&score)||score>INT_MAX||
                   !get32(&obj,"cpu_millicores",&cpu)||
                   !get32(&obj,"io_read_kib_s",&read)||
                   !get32(&obj,"io_write_kib_s",&write)||
                   !i64(&obj,"memory_delta_kib",&delta)||
                   !u64(&obj,"process_create_filetime",&creation)){
                    xr3_error(error,cap,"scored observation missing comparable evidence",line_no);good=0;break;
                }
                if(run->raw_precision){
                    if(!next_raw || !next_raw->has_id || !next_raw->has_ws ||
                       !next_raw->has_cpu || !next_raw->has_io ||
                       active<=1 || run->time_ms[active]<=prior_ms){
                        xr3_error(error,cap,"complete vector missing raw comparability",line_no);
                        good=0;break;
                    }
                    const XR3Raw *p=NULL;
                    for(unsigned j=0;j<raw_prev_count;j++){
                        if(raw_prev[j].pid==pid){p=&raw_prev[j];break;}
                    }
                    if(!p||!p->has_id||!p->has_ws||!p->has_cpu||!p->has_io||
                       p->creation!=creation ||
                       next_raw->cpu_ticks<p->cpu_ticks||
                       next_raw->read_bytes<p->read_bytes||
                       next_raw->write_bytes<p->write_bytes){
                        xr3_error(error,cap,"counter reset or identity mismatch in scored vector",line_no);
                        good=0;break;
                    }
                    uint64_t interval=run->time_ms[active]-prior_ms;
                    uint64_t d_cpu=(next_raw->cpu_ticks-p->cpu_ticks)/10000u;
                    uint64_t d_r=(next_raw->read_bytes-p->read_bytes)/1024u;
                    uint64_t d_w=(next_raw->write_bytes-p->write_bytes)/1024u;
                    uint32_t reported_interval=0;
                    if(interval>4294967u||d_cpu>4294967u||d_r>4294967u||d_w>4294967u||
                       !get32(&obj,"interval_ms",&reported_interval)||
                       reported_interval!=interval ||
                       (uint32_t)(d_cpu*1000u/interval)!=cpu ||
                       (uint32_t)(d_r*1000u/interval)!=read ||
                       (uint32_t)(d_w*1000u/interval)!=write ||
                       (int64_t)next_raw->ws_kib-(int64_t)p->ws_kib!=delta){
                        xr3_error(error,cap,"exact raw CPU/IO/memory rate mismatch",line_no);
                        good=0;break;
                    }
                    rate_reverified=1;
                    ++run->source_rate_verified;
                }
                uint64_t growth=delta>0?(uint64_t)delta/1024u:0;
                if(growth>16384u)growth=16384u;
                uint32_t c=xray_cpu_impact(cpu);
                uint32_t i=xray_io_impact(read,write);
                uint32_t m=xray_memory_impact((uint32_t)growth);
                if((uint64_t)c+i+m!=score){
                    xr3_error(error,cap,"AURA SCORE ORACLE MISMATCH",line_no);good=0;break;
                }
                vectors++;computed++;
                run->cpu_sum[active]+=cpu;run->read_sum[active]+=read;
                run->write_sum[active]+=write;
                if(delta>0)run->growth_sum[active]+=(uint64_t)delta;
                if(score>peaks)peaks=score;
            }else{
                run->quality_missing[active]++;
                if(sample==1&&strcmp(dq,"baseline")!=0&&strcmp(dq,"no_identity")!=0){
                    xr3_error(error,cap,"first sample contains an unjustified delta status",line_no);good=0;break;
                }
            }
            if(run->kept<XR3_MAX_EVENTS){
                XR3Event *event=&run->events[run->kept++];
                event->sample=sample;event->pid=pid;event->has_score=(unsigned char)has_score;
                event->score=score;event->cpu=cpu;event->read=read;event->write=write;
                event->delta_kib=delta;
                event->creation=creation;
                event->rate_reverified=(unsigned char)rate_reverified;
                uint32_t parent=0,threads=0;
                if(get32(&obj,"parent_pid",&parent) &&
                   get32(&obj,"thread_count",&threads)){
                    event->has_topology=1;event->parent_pid=parent;
                    event->thread_count=threads;
                }
                (void)copystr(event->delta_quality,sizeof event->delta_quality,dq);
                uint64_t workspace=0;
                if(u64(&obj,"working_set_kib",&workspace)){
                    event->has_ws=1;
                    event->ws_mib=(uint32_t)((workspace/1024u)>UINT32_MAX?UINT32_MAX:(workspace/1024u));
                }
                if(has_score){
                    uint32_t growth=(uint32_t)(delta>0?(uint64_t)delta/1024u:0);
                    if(growth>16384u)growth=16384u;
                    event->c_impact=xray_cpu_impact(cpu);
                    event->i_impact=xray_io_impact(read,write);
                    event->m_impact=xray_memory_impact(growth);
                }
                if(allow_names){
                    const char *name=field(&obj,"process_name");
                    if(name){size_t n=strlen(name);if(n>=sizeof event->name)n=sizeof event->name-1;
                        memcpy(event->name,name,n);event->name[n]=0;}
                }
            }else run->truncated++;
            { uint64_t memory_present=0;
              if(u64(&obj,"working_set_kib",&memory_present)) actual_readable++;
            }
        }else if(strcmp(type,"sample_end")==0){
            uint32_t sample=0,processes=0,valid=0,peak=0;
            if(stage!=1||!active||!get32(&obj,"sample",&sample)||sample!=active||
               !get32(&obj,"processes",&processes)||processes!=observed||
               !get32(&obj,"complete_rate_vectors",&valid)||valid!=vectors||
               !get32(&obj,"peak_activity_score",&peak)||peak!=peaks){
                xr3_error(error,cap,"sample count/score aggregate mismatch",line_no);good=0;break;
            }
            run->counts[active]=observed;run->valid[active]=vectors;run->peak[active]=peaks;
            if(run->raw_precision){
                XR3Raw *tmp=raw_prev;raw_prev=raw_curr;raw_curr=tmp;
                raw_prev_count=observed;prior_ms=run->time_ms[active];
            }
            active=0;
        }else if(strcmp(type,"capture_end")==0){
            uint32_t samples=0;
            uint64_t records=0,derived=0;
            if(stage!=1||active||strcmp(field(&obj,"status")?field(&obj,"status"):"","complete")!=0||
               !get32(&obj,"samples",&samples)||samples!=run->seconds||
               samples!=run->actual_samples||
               !u64(&obj,"process_records",&records)||records!=recorded||
               !u64(&obj,"derived_records",&derived)||derived!=computed||
               !u64(&obj,"elapsed_ms",&run->elapsed_ms)||
               run->elapsed_ms<1000u*(uint64_t)samples ||
               !u64(&obj,"readable_memory",&readable)||
               !u64(&obj,"open_denied",&run->denied)||
               !u64(&obj,"open_failed",&run->open_failed)||
               !u64(&obj,"errors",&run->errors)||
               !u64(&obj,"unknown_identity",&run->unknown)||
               !u64(&obj,"counter_resets",&run->reset)||
               !u64(&obj,"pid_reused",&run->reuse)||
               !u64(&obj,"history_capacity_misses",&run->capacity)||
               run->denied>run->open_failed ||
               readable>records || readable!=actual_readable){
                xr3_error(error,cap,"capture summary/elapsed verification failed",line_no);good=0;break;
            }
            run->samples=samples;run->records=records;run->derived=derived;stage=2;
        }else{xr3_error(error,cap,"unexpected record type (no partial silently accepted)",line_no);good=0;break;}
    }
    if(ferror(f)){xr3_error(error,cap,"read error",line_no);good=0;}
    fclose(f);
    free(raw_prev);free(raw_curr);
    if(stage!=2){if(good)xr3_error(error,cap,"capture_end missing or incomplete",line_no);good=0;}
    if(good) {
        /* Validate the previously parsed sample/record summaries; explicit is better
         * than treating successful JSON decoding as evidence of accurate content. */
        uint64_t total=0,valid=0;
        for(unsigned i=1;i<=run->samples;i++){total+=run->counts[i];valid+=run->valid[i];}
        if(total!=run->records||valid!=run->derived){
            xr3_error(error,cap,"cross-sample count mismatch",line_no);good=0;
        }
    }
    return good;
}

static void jsonquote(FILE *f,const char *s){
    fputc('"',f);
    for(const unsigned char *p=(const unsigned char*)s;*p;p++){
        unsigned char c=*p;
        if(c=='"'||c=='\\'){fputc('\\',f);fputc(c,f);}
        else if(c=='<'||c=='>'||c=='&'||c==0x27)
            fprintf(f,"\\u%04x",(unsigned)c);
        else if(c=='\n')fputs("\\n",f);
        else if(c=='\r')fputs("\\r",f);
        else if(c=='\t')fputs("\\t",f);
        else if(c<32)fprintf(f,"\\u%04x",(unsigned)c);
        else fputc(c,f);
    }
    fputc('"',f);
}
static void emitrun(FILE *f,const XR3Run *r){
    fprintf(f,"{\"seconds\":%u,\"elapsed\":%llu,\"records\":%llu,"
              "\"derived\":%llu,\"denied\":%llu,\"failed\":%llu,"
              "\"errors\":%llu,\"unknown\":%llu,\"reuse\":%llu,"
              "\"reset\":%llu,\"capacity\":%llu,\"truncated\":%u,"
              "\"rawPrecision\":%u,\"rateVerified\":%llu,\"build\":",
              r->seconds,(unsigned long long)r->elapsed_ms,
              (unsigned long long)r->records,(unsigned long long)r->derived,
              (unsigned long long)r->denied,(unsigned long long)r->open_failed,
              (unsigned long long)r->errors,(unsigned long long)r->unknown,
              (unsigned long long)r->reuse,(unsigned long long)r->reset,
              (unsigned long long)r->capacity,r->truncated,r->raw_precision,
              (unsigned long long)r->source_rate_verified);
    jsonquote(f,r->sha);
    fputs(",\"samples\":[",f);
    for(unsigned i=1;i<=r->samples;i++){
        if(i>1)fputc(',',f);
        fprintf(f,"[%u,%llu,%llu,%llu,%llu,%u,%u,%u,%u]",
                i,(unsigned long long)r->cpu_sum[i],
                (unsigned long long)r->read_sum[i],(unsigned long long)r->write_sum[i],
                (unsigned long long)r->growth_sum[i],r->peak[i],r->valid[i],
                r->counts[i],r->quality_missing[i]);
    }
    fputs("],\"events\":[",f);
    for(unsigned i=0;i<r->kept;i++){
        const XR3Event *e=&r->events[i];
        if(i)fputc(',',f);
        fprintf(f,"[%u,%u,\"%llu\",%d,%u,%u,%u,%lld,%d,%u,%u,%u,",
                e->sample,e->pid,(unsigned long long)e->creation,
                e->has_score?(int)e->score:-1,
                e->cpu,e->read,e->write,(long long)e->delta_kib,
                e->has_ws?(int)e->ws_mib:-1,
                e->c_impact,e->i_impact,e->m_impact);
        jsonquote(f,e->name);fputc(',',f);jsonquote(f,e->delta_quality);
        fprintf(f,",%d,%d,%u",e->has_topology?(int)e->parent_pid:-1,
                e->has_topology?(int)e->thread_count:-1,
                (unsigned)e->rate_reverified);
        fputc(']',f);
    }
    fputs("]}",f);
}

int xr03_render(FILE *out,const char *candidate_path,const char *baseline_path,
                int include_names,char *error,size_t cap){
    if(!out||!candidate_path){xr3_error(error,cap,"missing output or input",0);return 0;}
    XR3Run *candidate=(XR3Run*)calloc(1,sizeof(XR3Run));
    XR3Run *baseline=baseline_path?(XR3Run*)calloc(1,sizeof(XR3Run)):NULL;
    if(!candidate||(baseline_path&&!baseline)){
        free(candidate);free(baseline);xr3_error(error,cap,"allocation failed",0);return 0;
    }
    int ok=parsefile(candidate_path,candidate,include_names,error,cap);
    if(ok&&baseline)ok=parsefile(baseline_path,baseline,include_names,error,cap);
    if(!ok){free(candidate->events);free(candidate);
        if(baseline){free(baseline->events);free(baseline);}return 0;}
    fputs("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">"
          "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">"
          "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; "
          "style-src 'unsafe-inline';script-src 'unsafe-inline';connect-src 'none';img-src 'none';base-uri 'none'\">"
          "<title>AURA X-RAY / Evidence workbench</title>"
          "<style>"
          ":root{color-scheme:dark;--bg:#080b12;--ink:#e8f1fa;--muted:#90a3b8;--edge:#293447;--panel:#101927;"
          "--accent:#5deed7;--blue:#69aaff;--amber:#f4bc73;--red:#ff8793}*{box-sizing:border-box}"
          "body{margin:0;background:radial-gradient(ellipse 1500px 900px at 80% -10%,#172a41,transparent),var(--bg);color:var(--ink);font:14px/1.55 system-ui,Segoe UI,sans-serif}"
          ".frame{max-width:1480px;margin:auto;padding:26px clamp(14px,3vw,44px) 90px}.row{display:flex;align-items:center;justify-content:space-between;gap:16px;flex-wrap:wrap}"
          "header{padding-bottom:24px;border-bottom:1px solid var(--edge)}.brand{font-size:11px;color:var(--accent);font-weight:800;letter-spacing:.3em}"
          "h1{font-size:clamp(38px,6vw,82px);letter-spacing:-.085em;margin:0;line-height:1}h1 b{color:var(--accent);font-weight:600}"
          "h2{font-size:19px;letter-spacing:-.025em}p,.small{color:var(--muted)}.small{font-size:12px}"
          ".statgrid{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:10px;margin:25px 0}"
          ".stat,.panel{border:1px solid var(--edge);border-radius:14px;background:linear-gradient(145deg,#131e2d,#0d1521);padding:18px}"
          ".k{font-size:11px;color:var(--muted);letter-spacing:.11em;text-transform:uppercase}.v{font-size:32px;font-weight:730;font-variant-numeric:tabular-nums}"
          ".muted{color:var(--muted)}.mono{font-family:ui-monospace,Cascadia Code,Consolas,monospace}"
          ".tag{display:inline-block;border:1px solid #38685f;color:var(--accent);padding:3px 9px;border-radius:20px;font-size:10px;font-weight:750}"
          ".tabs{display:flex;gap:8px;flex-wrap:wrap;margin:22px 0}.tabs button,select,input{background:#152337;color:var(--ink);border:1px solid #3b536b;border-radius:8px;padding:10px 13px;font:inherit}"
          ".tabs button{cursor:pointer}.tabs button.active{background:#234c53;border-color:var(--accent);color:white}"
          ".grid{display:grid;grid-template-columns:minmax(0,1.75fr) minmax(280px,.85fr);gap:16px}@media(max-width:900px){.grid{grid-template-columns:1fr}}"
          ".chart{width:100%;height:275px;display:block}.axis{fill:#7792aa;font-size:10px;font-family:ui-monospace,monospace}"
          ".controls{display:flex;gap:10px;flex-wrap:wrap;margin:14px 0}.controls input{min-width:220px;flex:1}"
          ".scroll{overflow:auto;max-height:500px;border:1px solid var(--edge);border-radius:10px}"
          "table{width:100%;border-collapse:collapse;font-variant-numeric:tabular-nums;min-width:790px;font-size:12px}"
          "th,td{padding:10px 12px;text-align:right;border-bottom:1px solid #223247;white-space:nowrap}"
          "th{position:sticky;top:0;background:#172436;color:var(--muted);z-index:1;font-size:10px;text-transform:uppercase}"
          "th:first-child,td:first-child{text-align:left}tbody tr{cursor:pointer}tbody tr:hover,.sel{background:#203349}"
          ".score{color:var(--accent);font-weight:800}.orange{color:var(--amber)}.line{border-top:1px solid var(--edge);margin:18px 0}"
          ".legend{display:flex;gap:16px;flex-wrap:wrap;color:var(--muted);font-size:11px}.dot{display:inline-block;width:9px;height:9px;border-radius:50%;margin-right:6px;background:var(--accent)}"
          ".heat{display:grid;gap:2px;min-width:550px}.heatcell{min-width:17px;height:22px;border-radius:3px;cursor:pointer;border:1px solid #26354c}"
          ".heatlabel{font:11px ui-monospace,monospace;color:var(--muted);padding:3px;overflow:hidden;white-space:nowrap}"
          ".alert{border-left:2px solid var(--amber);padding:10px 14px;margin:9px 0;background:#192330;border-radius:5px;font-size:12px}"
          ".footer{padding-top:25px;margin-top:40px;border-top:1px solid var(--edge);color:var(--muted);font-size:12px}"
          ".hidden{display:none!important}</style></head><body><div class=\"frame\">"
          "<header><div class=\"row\"><div><div class=\"brand\">AURA / LOCAL EVIDENCE ENGINE / XR-03</div>"
          "<h1>X-RAY <b>workbench</b></h1>"
          "<p>Temporal evidence you can interrogate. Identical sample times for each metric. Source: Windows Toolhelp32 / PSAPI / GetProcessTimes.</p>"
          "</div><div><span class=\"tag\">OFFLINE · VERIFIED INPUT</span><p class=\"small\">NO NETWORK · NO TELEMETRY UPLOAD</p></div></div>"
          "</header><div class=\"statgrid\" id=\"stats\"></div>"
          "<div class=\"tabs\" role=\"tablist\">"
          "<button data-tab=\"timeline\" class=\"active\">01 / Time axis</button>"
          "<button data-tab=\"episodes\">02 / Event evidence</button>"
          "<button data-tab=\"heatmap\">03 / Process matrix</button>"
          "<button data-tab=\"integrity\">04 / Quality &amp; provenance</button>"
          "<button data-tab=\"topology\">05 / Process topology</button>"
          "</div><div class=\"grid\"><main>"
          "<section class=\"panel\" id=\"timeline\"><div class=\"row\"><h2>Multidimensional system timeline</h2>"
          "<select id=\"metric\"><option value=\"cpu\">Sum CPU · millicores</option>"
          "<option value=\"read\">Sum read · KiB/s</option><option value=\"write\">Sum write · KiB/s</option>"
          "<option value=\"growth\">Positive WS growth · KiB</option>"
          "<option value=\"peak\">Peak activity score</option></select></div>"
          "<p class=\"small\">Aggregation is over comparable process observations, NOT total-system resource usage. Working sets may share physical pages.</p>"
          "<svg id=\"trend\" class=\"chart\" viewBox=\"0 0 750 275\" role=\"img\" aria-label=\"Multi-run measured timeline\"></svg>"
          "<div class=\"legend\"><span><i class=\"dot\"></i>Current capture</span>"
          "<span id=\"comparisonLegend\"><i class=\"dot\" style=\"background:var(--blue)\"></i>Baseline capture</span></div>"
          "</section><section class=\"panel\" id=\"episodes\"><h2>Evidence events · synchronized dimensions</h2>"
          "<p class=\"small\">Every row is ONE real sample. CPU, I/O, memory delta and score never mix timestamps. Select a row for a full inspection.</p>"
          "<div class=\"controls\"><input id=\"filter\" type=\"search\" placeholder=\"Filter PID or process name\" aria-label=\"Filter observed events\">"
          "<select id=\"sampleSelect\" aria-label=\"Filter sample\"><option value=\"0\">Every sample</option></select>"
          "<select id=\"runSelect\" aria-label=\"Dataset\"><option value=\"0\">Current</option><option value=\"1\">Baseline</option></select>"
          "</div><div class=\"small\" id=\"found\"></div><div class=\"scroll\"><table><thead><tr>"
          "<th>Process</th><th>Sample</th><th>Score</th><th>CPU milli</th><th>Read KiB/s</th><th>Write KiB/s</th>"
          "<th>WS delta KiB</th><th>WS MiB</th><th>Evidence quality</th></tr></thead><tbody id=\"eventrows\"></tbody></table></div></section>"
          "<section class=\"panel\" id=\"heatmap\"><h2>Identity-aware activity matrix</h2>"
          "<p class=\"small\">Each cell is one process and one sample. Muted cells mean missing evidence; zero score is different.</p>"
          "<div class=\"scroll\" id=\"matrix\"></div></section>"
          "<section class=\"panel\" id=\"integrity\"><h2>Evidence integrity and uncertainty</h2>"
          "<p>Validated capture/2 schema, record ordering, declared sample counts and real Aura score components before rendering this report.</p>"
          "<div class=\"statgrid\" id=\"integrityStats\"></div><p class=\"small\">An absent process in a sample is not proof of an exit. "
          "CPU millicores are per logical core, I/O transfer counters can include non-disk activity, and positive working-set growth "
          "is not proof of a leak. Score is heuristic; no malware, bottleneck or causal diagnosis is asserted.</p>"
          "</section><section class=\"panel\" id=\"topology\"><h2>Process family and thread context</h2>"
          "<p class=\"small\">Parent PID values are OS snapshot hints, NOT verified parent identities. "
          "Thread counts are instantaneous, and a PID alone can be recycled. "
          "Select a row to inspect its actual observation.</p>"
          "<div class=\"scroll\"><table><thead><tr><th>PID / name</th><th>Parent PID hint</th>"
          "<th>Threads</th><th>Sample</th><th>Score</th><th>Raw counter integrity</th>"
          "</tr></thead><tbody id=\"topoRows\"></tbody></table></div></section>"
          "</main><aside><div class=\"panel\"><div class=\"k\">FOCUSED EVIDENCE</div><h2 id=\"focusTitle\">Select an observation</h2>"
          "<div id=\"focusBody\" class=\"small\">Click an event row, matrix cell or a point on the timeline.</div>"
          "<div class=\"line\"></div><div class=\"k\">INVESTIGATION PRINCIPLE</div>"
          "<p class=\"small\">Follow process identity + sample time. Separate OBSERVED raw counters, COMPUTED Aura terms and UNKNOWN values. "
          "No blended peak columns and no hidden imputation.</p></div>"
          "<div class=\"panel\" style=\"margin-top:16px\"><div class=\"k\">CURRENT vs BASELINE</div><div id=\"compareCards\"></div>"
          "<p class=\"small\">Comparisons use aggregate observed distributions, not presumed process identity across captures or causal conclusions.</p>"
          "</div></aside></div>"
          "<div class=\"footer\">AURA ENGINE: nine compiled uint32 cells, including three XR-03 score components. "
          "Input validation is local and fail-closed; no external libraries. SOURCE: WINDOWS · NO NETWORK · NO EXTERNAL ASSETS."
          "<p>Build provenance is embedded in the capture, not silently inferred from this report.</p></div></div>"
          "<script>'use strict';const candidate=",out);
    emitrun(out,candidate);
    fputs(";const baseline=",out);
    if(baseline)emitrun(out,baseline);else fputs("null",out);
    fputs(";const $=id=>document.getElementById(id);const sets=[candidate,baseline].filter(Boolean);"
          "let active=0,chosen=0,focus=null;const n=x=>Number(x||0).toLocaleString('en-US');"
          "const attrs=['sample','pid','creation','score','cpu','read','write','delta','ws','c','i','m','name','quality'];"
          "const value=(v)=>v<0?'unknown':n(v);"
          "function tile(k,v,sub){const el=document.createElement('div');el.className='stat';"
          "const a=document.createElement('div');a.className='k';a.textContent=k;"
          "const b=document.createElement('div');b.className='v';b.textContent=v;"
          "const c=document.createElement('div');c.className='small';c.textContent=sub||'';"
          "el.append(a,b,c);return el;}"
          "function initStats(){const d=$('stats');const s=candidate.samples;d.replaceChildren("
          "tile('Capture window',(candidate.elapsed/1000).toFixed(3)+'s','Requested '+candidate.seconds+' seconds'),"
          "tile('Process observations',n(candidate.records),'Across '+candidate.samples.length+' samples'),"
          "tile('Valid temporal vectors',n(candidate.derived),'Score validated with Aura'),"
          "tile('Exact source rates',candidate.rawPrecision?n(candidate.rateVerified):'legacy',"
          "candidate.rawPrecision?'Recomputed from raw Windows counters':'Score consistency only; raw counters unavailable'),"
          "tile('Process open failures',n(candidate.failed),'Denied '+candidate.denied),"
          "tile('Truncated UI rows',n(candidate.truncated),'All input rows verified'));"
          "$('comparisonLegend').hidden=!baseline;$('runSelect').options[1].disabled=!baseline;"
          "for(const s of candidate.samples){const o=document.createElement('option');o.value=s[0];o.textContent='Sample '+s[0];$('sampleSelect').append(o);}"
          "const i=$('integrityStats');i.replaceChildren(tile('Unknown identities',n(candidate.unknown)),"
          "tile('Counter resets',n(candidate.reset)),tile('PID replacements',n(candidate.reuse)),"
          "tile('Tracked capacity misses',n(candidate.capacity)),tile('Recorded errors',n(candidate.errors)),"
          "tile('Exact raw vectors',candidate.rawPrecision?n(candidate.rateVerified):'N/A',"
          "candidate.rawPrecision?'CPU/IO/memory independently recomputed':'Old XR-02/XR-03 source precision insufficient'));"
          "const c=$('compareCards');if(!baseline){c.textContent='No baseline supplied. Use --compare BASELINE CANDIDATE --report OUTPUT.';return;}"
          "function metric(s,i){return s.samples.reduce((a,v)=>a+v[i],0)}"
          "const fields=[['Observation count',candidate.records,baseline.records],"
          "['Comparable vectors',candidate.derived,baseline.derived],"
          "['Accumulated CPU sample sums',metric(candidate,1),metric(baseline,1)],"
          "['Accumulated read KiB/s sums',metric(candidate,2),metric(baseline,2)],"
          "['Accumulated write KiB/s sums',metric(candidate,3),metric(baseline,3)]];"
          "c.replaceChildren(...fields.map(f=>{const b=document.createElement('div');b.className='alert';"
          "const pct=f[2]===0?'baseline=0':((f[1]-f[2])/f[2]*100).toFixed(1)+'%';"
          "b.textContent=f[0]+': '+n(f[1])+' vs '+n(f[2])+' ('+pct+')';return b;}));}"
          "function metricIdx(){return ({cpu:1,read:2,write:3,growth:4,peak:5})[$('metric').value];}"
          "function draw(){const svg=$('trend');svg.replaceChildren();const idx=metricIdx(),lines=[candidate,baseline].filter(Boolean);"
          "const max=Math.max(1,...lines.flatMap(s=>s.samples.map(x=>x[idx])));"
          "const S=['http:','','www.w3.org','2000','svg'].join('/');function el(tag,attr,text){let x=document.createElementNS(S,tag);"
          "Object.entries(attr).forEach(([k,v])=>x.setAttribute(k,v));if(text!==undefined)x.textContent=text;svg.append(x);return x;}"
          "for(let g=0;g<=4;g++){const y=236-g*49;el('line',{x1:52,y1:y,x2:725,y2:y,stroke:'#293d50'});"
          "el('text',{x:4,y:y+4,class:'axis'},n(Math.round(max*g/4)));}"
          "lines.forEach((r,index)=>{const coords=r.samples.map((s,i)=>[60+i*655/Math.max(1,r.samples.length-1),236-s[idx]*196/max]);"
          "el('polyline',{points:coords.map(p=>p.join(',')).join(' '),fill:'none',stroke:index?'#69aaff':'#5deed7','stroke-width':2.5});"
          "coords.forEach((p,i)=>{const c=el('circle',{cx:p[0],cy:p[1],r:5,fill:index?'#69aaff':'#5deed7',style:'cursor:pointer'});"
          "c.addEventListener('click',()=>{$('sampleSelect').value=r.samples[i][0];$('runSelect').value=index;active=index;renderEvents();});"
          "el('text',{x:p[0]-4,y:259,class:'axis'},String(r.samples[i][0]));});});}"
          "function renderEvents(){active=Number($('runSelect').value)||0;const r=sets[active];if(!r)return;"
          "const q=$('filter').value.trim().toLowerCase(),s=Number($('sampleSelect').value)||0;"
          "const events=r.events.filter(e=>(!s||e[0]===s)&&(!q||String(e[1]).includes(q)||e[12].toLowerCase().includes(q)));"
          "events.sort((a,b)=>b[3]-a[3]||a[0]-b[0]);"
          "$('found').textContent=n(events.length)+' matching events (first 180 shown).';const body=$('eventrows');body.replaceChildren();"
          "for(const e of events.slice(0,180)){const tr=document.createElement('tr');"
          "const columns=[e[12]||'redacted · '+e[1],e[0],value(e[3]),value(e[4]),value(e[5]),value(e[6]),"
          "e[3]<0?'unknown':n(e[7]),value(e[8]),e[13]];"
          "columns.forEach((v,i)=>{const td=document.createElement('td');td.textContent=v;"
          "if(i===2)td.className='score';tr.append(td);});"
          "tr.addEventListener('click',()=>inspect(e,r));body.append(tr);}"
          "if(!focus&&events.length)inspect(events[0],r);}"
          "function inspect(e,r){focus=e;const id=e[1]+':'+e[2];$('focusTitle').textContent=(e[12]||'PID '+e[1])+' / sample '+e[0];"
          "const c=$('focusBody');c.replaceChildren();const lines=["
          "'Identity: PID '+e[1]+' / created '+e[2],"
          "'Score: '+value(e[3])+' (CPU impact '+e[9]+' + IO '+e[10]+' + growth '+e[11]+')',"
          "'CPU: '+value(e[4])+' millicores',"
          "'I/O: '+value(e[5])+' read / '+value(e[6])+' write KiB/s',"
          "'Working set change: '+(e[3]<0?'unknown':n(e[7])+' KiB'),"
          "'Working set now: '+value(e[8])+' MiB',"
          "'Evidence quality: '+e[13],"
          "'COMPUTED FROM OS OBSERVATIONS — not a diagnosis'];"
          "for(const line of lines){const d=document.createElement('div');d.className='alert';d.textContent=line;c.append(d);}"
          "if(e[14]>=0){const line=document.createElement('div');line.className='alert';"
          "line.textContent='Parent PID hint: '+e[14]+' · Threads: '+e[15];c.append(line);}"
          "const validity=document.createElement('div');validity.className='small';"
          "validity.textContent=e[16]?'Exact raw counters revalidated':"
          "'Legacy/partial observation: exact CPU and I/O reconstruction unavailable';c.append(validity);"
          "const history=r.events.filter(x=>x[1]===e[1]&&x[2]===e[2]);"
          "const h=document.createElement('div');h.className='small';h.textContent='Same-identity history: '+history.map(x=>'S'+x[0]+':'+(x[3]<0?'?':x[3])).join(' · ');c.append(h);}"
          "function heat(){const root=$('matrix');root.replaceChildren();const r=sets[active]||candidate;"
          "const ids=new Map();for(const e of r.events){if(e[3]<0)continue;const k=e[1]+':'+e[2];"
          "const prev=ids.get(k);if(!prev||prev[3]<e[3])ids.set(k,e);}"
          "const ranked=[...ids.values()].sort((a,b)=>b[3]-a[3]).slice(0,26);"
          "const grid=document.createElement('div');grid.className='heat';grid.style.gridTemplateColumns='120px repeat('+r.seconds+',minmax(22px,1fr))';"
          "const label=document.createElement('div');label.className='heatlabel';label.textContent='PID / SAMPLE';grid.append(label);"
          "for(let i=1;i<=r.seconds;i++){const x=document.createElement('div');x.className='heatlabel';x.textContent=i;grid.append(x);}"
          "for(const leader of ranked){const title=document.createElement('div');title.className='heatlabel';title.textContent=leader[12]||String(leader[1]);grid.append(title);"
          "for(let s=1;s<=r.seconds;s++){const e=r.events.find(x=>x[0]===s&&x[1]===leader[1]&&x[2]===leader[2]);"
          "const cell=document.createElement('div');cell.className='heatcell';const z=e&&e[3]>=0?e[3]:-1;"
          "cell.style.background=z<0?'#1a2331':z===0?'#1c4550':'hsl('+Math.max(3,185-Math.min(160,z))+' 72% '+(23+Math.min(38,z/12))+'%)';"
          "cell.title='PID '+leader[1]+' / sample '+s+' / score '+(z<0?'unknown':z);"
          "if(e)cell.addEventListener('click',()=>inspect(e,r));grid.append(cell);}}root.append(grid);}"
          "function topology(){const r=sets[active]||candidate;const rows=$('topoRows');rows.replaceChildren();"
          "const e=r.events.filter(x=>x[0]===r.seconds&&x[14]>=0)"
          ".sort((a,b)=>b[15]-a[15]||a[1]-b[1]);"
          "for(const v of e.slice(0,350)){const tr=document.createElement('tr');"
          "const values=[v[12]||'PID '+v[1],v[14],v[15],v[0],v[3]<0?'unknown':v[3],"
          "v[16]?'exact counter audit':'score/legacy'];"
          "for(const x of values){const td=document.createElement('td');td.textContent=x;tr.append(td);}"
          "tr.addEventListener('click',()=>inspect(v,r));rows.append(tr);}"
          "if(!e.length){const tr=document.createElement('tr');const td=document.createElement('td');"
          "td.colSpan=6;td.textContent='Topology unavailable in legacy capture. New XR-04 recordings include parent/thread hints.';"
          "tr.append(td);rows.append(tr);}}"
          "function tab(name){for(const t of ['timeline','episodes','heatmap','integrity','topology'])$(t).classList.toggle('hidden',t!==name);"
          "for(const b of document.querySelectorAll('[data-tab]'))b.classList.toggle('active',b.dataset.tab===name);"
          "if(name==='heatmap')heat();if(name==='topology')topology();}"
          "document.querySelectorAll('[data-tab]').forEach(x=>x.addEventListener('click',()=>tab(x.dataset.tab)));"
          "$('metric').addEventListener('change',draw);$('filter').addEventListener('input',renderEvents);"
          "$('sampleSelect').addEventListener('change',renderEvents);"
          "$('runSelect').addEventListener('change',()=>{active=Number($('runSelect').value)||0;renderEvents();heat();topology();});"
          "initStats();draw();renderEvents();tab('timeline');"
          "</script></body></html>",out);
    int passed=!ferror(out);
    if(!passed)xr3_error(error,cap,"report output write failure",0);
    free(candidate->events);free(candidate);
    if(baseline){free(baseline->events);free(baseline);}
    return passed;
}
