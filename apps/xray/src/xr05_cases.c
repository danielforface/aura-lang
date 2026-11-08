/*
 * AURA X-RAY XR-06: evidence event folds over real Aura-generated state cells.
 * No operating-system access, no HTML and no JSON parser in this module.
 *
 * C owns: bounded ordering, stable PID+creation identity, continuity proof,
 * overflow checks, and transport of original observation evidence.
 * AURA owns: CPU/IO recurrence, memory debt/phase, multi-signal intersection,
 * observation grade, and the existing threshold functions.
 */
#include "xr05_cases.h"
#include <stdlib.h>
#include <string.h>
#include <limits.h>

extern uint32_t xray_burst_gate(uint32_t growth_kib);
extern uint32_t xray06_cpu_streak_next(uint32_t prior,uint32_t cpu,uint32_t comparable,uint32_t adjacent);
extern uint32_t xray06_io_streak_next(uint32_t prior,uint32_t read,uint32_t write,uint32_t comparable,uint32_t adjacent);
extern uint32_t xray06_memory_remaining(uint32_t previous,uint32_t growth,uint32_t reduction);
extern uint32_t xray06_memory_phase(uint32_t previous,uint32_t remaining,uint32_t age,uint32_t evidence_ok);
extern uint32_t xray06_signal_dimensions(uint32_t cpu,uint32_t read,uint32_t write,uint32_t growth);
extern uint32_t xray06_evidence_grade(uint32_t comparable,uint32_t raw,uint32_t contiguous);

static int by_identity_time(const void *l,const void *r){
    const XR05Observation *a=(const XR05Observation*)l,*b=(const XR05Observation*)r;
    if(a->pid!=b->pid)return a->pid<b->pid?-1:1;
    if(a->creation!=b->creation)return a->creation<b->creation?-1:1;
    if(a->sample!=b->sample)return a->sample<b->sample?-1:1;
    return 0;
}
static void append(XR05Casebook *book,XR05Case c){
    if(book->count<XR05_MAX_CASES)book->items[book->count++]=c;
    else ++book->truncated;
}
static uint32_t cap_u32(uint64_t value){
    return value>UINT32_MAX?UINT32_MAX:(uint32_t)value;
}
static int magnitude(int64_t signed_kib,uint32_t *positive,uint32_t *negative){
    *positive=0;*negative=0;
    if(signed_kib>0){
        if((uint64_t)signed_kib>UINT32_MAX)return 0;
        *positive=(uint32_t)signed_kib;
    }else if(signed_kib<0){
        if(signed_kib==INT64_MIN || (uint64_t)(-signed_kib)>UINT32_MAX)return 0;
        *negative=(uint32_t)(-signed_kib);
    }
    return 1;
}
static uint8_t grade(uint8_t raw,uint8_t continuity){
    return (uint8_t)xray06_evidence_grade(1,raw?1u:0u,continuity?1u:0u);
}
static void flush_run(XR05Casebook *book,uint32_t kind,uint32_t streak,
                      uint32_t start,uint32_t last,uint32_t peak_sample,uint32_t peak,
                      uint32_t pid,uint64_t creation,uint8_t raw){
    if(!streak || (kind==2 && streak<3))return;
    XR05Case c={0};
    c.kind=kind;c.pid=pid;c.creation=creation;
    c.start=start;c.end=last;c.peak_sample=peak_sample;c.peak_value=peak;
    c.raw_verified=raw;c.grade=grade(raw,1);
    append(book,c);
}
int xr05_build_cases(const XR05Observation *input,size_t n,XR05Casebook *book){
    if(!book || (!input&&n) || n>XR05_MAX_INPUT)return 0;
    memset(book,0,sizeof(*book));
    if(!n)return 1;
    XR05Observation *v=(XR05Observation*)malloc(n*sizeof(*v));
    if(!v)return 0;
    memcpy(v,input,n*sizeof(*v));
    qsort(v,n,sizeof(*v),by_identity_time);
    size_t i=0;
    while(i<n){
        size_t end=i+1;
        while(end<n && v[end].pid==v[i].pid && v[end].creation==v[i].creation)++end;
        if(v[i].creation==0){i=end;continue;}
        /*
         * Phase A: evaluate each complete vector for a memory episode and
         * synchronous multi-signal observation. No C threshold classification.
         */
        for(size_t k=i;k<end;k++){
            const XR05Observation *e=&v[k];
            if(!e->comparable)continue;
            uint32_t pos=0,neg=0;
            if(!magnitude(e->delta_kib,&pos,&neg))continue;
            uint32_t dimensions=xray06_signal_dimensions(e->cpu,e->read,e->write,pos);
            if(dimensions>=2){
                XR05Case c={0};
                c.kind=4;c.pid=e->pid;c.creation=e->creation;
                c.start=e->sample;c.end=e->sample;c.peak_sample=e->sample;
                c.peak_value=dimensions;c.raw_verified=e->rate_reverified;
                c.grade=grade(c.raw_verified,1);
                append(book,c);
            }
            if(!xray_burst_gate(pos))continue;
            XR05Case c={0};
            c.kind=1;c.pid=e->pid;c.creation=e->creation;
            c.start=e->sample;c.end=e->sample;c.peak_sample=e->sample;
            c.peak_value=pos;c.raw_verified=e->rate_reverified;
            c.phase=1;/* OPEN/UNKNOWN until the real Aura phase gate decides. */
            uint32_t remaining=pos/2u;
            uint32_t prior_sample=e->sample;
            int64_t observed_net=0;
            uint8_t unbroken=1;
            for(size_t j=k+1;j<end && j<=k+3;j++){
                const XR05Observation *f=&v[j];
                if(f->sample!=prior_sample+1u || !f->comparable){
                    c.phase=(uint8_t)xray06_memory_phase(c.phase,remaining,0,0);
                    unbroken=0;break;
                }
                prior_sample=f->sample;
                uint32_t growth=0,reduction=0;
                if(!magnitude(f->delta_kib,&growth,&reduction)||
                   growth>=UINT32_MAX-remaining||
                   (f->delta_kib>0 && observed_net>INT64_MAX-f->delta_kib)||
                   (f->delta_kib<0 && observed_net<INT64_MIN-f->delta_kib)){
                    c.phase=(uint8_t)xray06_memory_phase(c.phase,remaining,0,0);
                    unbroken=0;break;
                }
                observed_net+=f->delta_kib; /* audit magnitude only, not decision */
                remaining=xray06_memory_remaining(remaining,growth,reduction);
                c.raw_verified=(uint8_t)(c.raw_verified && f->rate_reverified);
                c.phase=(uint8_t)xray06_memory_phase(c.phase,remaining,(uint32_t)(j-k),1);
                if(c.phase==2){
                    c.end=f->sample;
                    if(observed_net<0)c.recovered_kib=cap_u32((uint64_t)(-observed_net));
                    break;
                }
                if(c.phase==3){ c.end=f->sample; break; }
            }
            /* Missing future samples leave phase OPEN; unobserved != failed. */
            c.grade=grade(c.raw_verified,unbroken);
            append(book,c);
        }
        /*
         * Phase B: CPU and I/O recurrence, both state transitions in Aura.
         * C records the peak and closes a completed adjacent run on a gap or
         * nonqualifying sample; process PID recycling starts a new group.
         */
        uint32_t cpu_state=0,io_state=0,prior_sample=0;
        uint32_t cpu_start=0,io_start=0,cpu_last=0,io_last=0;
        uint32_t cpu_peak=0,io_peak=0,cpu_peak_at=0,io_peak_at=0;
        uint8_t cpu_raw=1,io_raw=1;
        for(size_t k=i;k<end;k++){
            const XR05Observation *e=&v[k];
            uint32_t comparable=e->comparable?1u:0u;
            uint32_t adjacent=(prior_sample!=0 && prior_sample<UINT32_MAX &&
                               e->sample==prior_sample+1u)?1u:0u;
            uint32_t next_cpu=xray06_cpu_streak_next(cpu_state,e->cpu,comparable,adjacent);
            uint32_t next_io=xray06_io_streak_next(io_state,e->read,e->write,comparable,adjacent);
            if(cpu_state && (!adjacent || !next_cpu)){
                flush_run(book,2,cpu_state,cpu_start,cpu_last,cpu_peak_at,cpu_peak,
                          e->pid,e->creation,cpu_raw);
                cpu_state=0;cpu_raw=1;
            }
            if(io_state && (!adjacent || !next_io)){
                flush_run(book,3,io_state,io_start,io_last,io_peak_at,io_peak,
                          e->pid,e->creation,io_raw);
                io_state=0;io_raw=1;
            }
            if(next_cpu){
                if(!cpu_state){cpu_start=e->sample;cpu_peak=0;cpu_raw=1;}
                if(e->cpu>=cpu_peak){cpu_peak=e->cpu;cpu_peak_at=e->sample;}
                cpu_last=e->sample;
                cpu_raw=(uint8_t)(cpu_raw && e->rate_reverified);
            }
            if(next_io){
                if(!io_state){io_start=e->sample;io_peak=0;io_raw=1;}
                uint32_t transfer=e->read>e->write?e->read:e->write;
                if(transfer>=io_peak){io_peak=transfer;io_peak_at=e->sample;}
                io_last=e->sample;
                io_raw=(uint8_t)(io_raw && e->rate_reverified);
            }
            cpu_state=next_cpu;io_state=next_io;prior_sample=e->sample;
        }
        flush_run(book,2,cpu_state,cpu_start,cpu_last,cpu_peak_at,cpu_peak,
                  v[i].pid,v[i].creation,cpu_raw);
        flush_run(book,3,io_state,io_start,io_last,io_peak_at,io_peak,
                  v[i].pid,v[i].creation,io_raw);
        i=end;
    }
    free(v);
    return 1;
}

int xr05_self_test(void){
    /* Real Aura ABI used by every transition. The old XR-05 cases must survive. */
    const XR05Observation v[]={
        {1,42,0,0,0,0,100,0,0,0},
        {2,42,180,500,0,0,100,92160,1,1},
        {3,42,0,100,0,0,100,-102400,1,1},
        {4,42,8,800,0,0,100,0,1,1},
        {5,42,9,900,0,0,100,0,1,1},
        {6,42,10,1000,0,0,100,0,1,1},
        {7,42,9,0,0,20000,100,0,1,1},
        {8,42,0,0,0,0,200,0,0,0},
        {9,42,65,0,0,0,200,33792,1,1},
        {10,42,0,0,0,0,200,-1024,0,0},
        {1,43,0,0,0,0,400,0,0,0},
        {2,43,0,699,16383,0,400,32767,1,1},
        {3,43,0,700,0,0,400,0,1,1},
        {5,43,0,701,0,0,400,0,1,1},
        {6,43,0,702,0,0,400,0,1,1},
        {1,44,0,0,0,0,500,0,0,0},
        {2,44,0,900,20000,0,500,40000,1,1},
        {3,44,0,0,0,0,500,-25000,1,1},
        {1,45,0,0,0,0,600,0,0,0},
        {2,45,0,0,20000,0,600,0,1,1},
        {3,45,0,0,0,17000,600,0,1,1},
        {4,45,0,0,0,0,600,0,1,1},
        {1,46,0,0,0,0,0,0,0,0}
    };
    XR05Casebook book;
    if(!xr05_build_cases(v,sizeof(v)/sizeof(v[0]),&book)||
       book.count!=8 || book.truncated)return 0;
    unsigned pulse=0,cpu=0,transfer=0,compound=0,pending=0;
    for(uint32_t k=0;k<book.count;k++){
        const XR05Case *c=&book.items[k];
        if(c->grade==0)return 0;
        if(c->pid==42 && c->kind==1 && c->start==2 &&
           c->end==3 && c->phase==2 && c->recovered_kib==102400 && c->grade==3)pulse++;
        if(c->pid==42 && c->kind==1 && c->start==9 && c->phase==4 &&
           c->grade==2)pending++;
        if(c->pid==42 && c->kind==2 && c->start==4 && c->end==6 &&
           c->peak_value==1000 && c->grade==3)cpu++;
        if(c->pid==45 && c->kind==3 && c->start==2 && c->end==3 &&
           c->peak_value==20000)transfer++;
        if(c->pid==44 && c->kind==4 && c->start==2 && c->peak_value==3)compound++;
    }
    return pulse==1 && pending==1 && cpu==1 && transfer==1 && compound==1;
}
