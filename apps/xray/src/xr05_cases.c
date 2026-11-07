/* XR-05 Aura-driven investigation cases.
 * C only groups comparable observations by PID/creation and sample sequence.
 * ALL signal thresholds and recovered/streak decisions execute compiled Aura.
 */
#include "xr05_cases.h"
#include <stdlib.h>
#include <string.h>
#include <limits.h>

extern uint32_t xray_burst_gate(uint32_t growth_kib);
extern uint32_t xray_recovery_gate(uint32_t retraction_kib,uint32_t burst_kib);
extern uint32_t xray_cpu_hot_gate(uint32_t cpu_millicores);
extern uint32_t xray_cpu_streak_gate(uint32_t consecutive_samples);
extern uint32_t xray_transfer_surge_gate(uint32_t read_kib_s,uint32_t write_kib_s);

static int by_process_time(const void *va,const void *vb){
    const XR05Observation *a=(const XR05Observation*)va,*b=(const XR05Observation*)vb;
    if(a->pid!=b->pid)return a->pid<b->pid?-1:1;
    if(a->creation!=b->creation)return a->creation<b->creation?-1:1;
    if(a->sample!=b->sample)return a->sample<b->sample?-1:1;
    return 0;
}
static uint32_t cap32(uint64_t v){return (uint32_t)(v>UINT32_MAX?UINT32_MAX:v);}
static void append_case(XR05Casebook *out,XR05Case c){
    if(out->count<XR05_MAX_CASES)out->items[out->count++]=c;
    else ++out->truncated;
}
int xr05_build_cases(const XR05Observation *src,size_t n,XR05Casebook *out){
    if(!out||(!src&&n)||n>XR05_MAX_INPUT)return 0;
    memset(out,0,sizeof *out);
    if(!n)return 1;
    XR05Observation *v=(XR05Observation*)malloc(n*sizeof(*v));
    if(!v)return 0;
    memcpy(v,src,n*sizeof(*v));
    qsort(v,n,sizeof(*v),by_process_time);
    size_t i=0;
    while(i<n){
        size_t end=i+1;
        while(end<n&&v[end].pid==v[i].pid&&v[end].creation==v[i].creation)++end;
        if(!v[i].creation){i=end;continue;}
        /* Memory pulse and observed consecutive-window retraction. */
        for(size_t k=i;k<end;k++){
            const XR05Observation *e=&v[k];
            if(!e->comparable)continue;
            if(e->delta_kib>0){
                uint32_t growth=cap32((uint64_t)e->delta_kib);
                if(xray_burst_gate(growth)>0){
                    XR05Case c={0};
                    c.kind=1;c.pid=e->pid;c.creation=e->creation;
                    c.start=e->sample;c.end=e->sample;c.peak_sample=e->sample;
                    c.peak_value=growth;c.raw_verified=e->rate_reverified;
                    int64_t net=0;uint32_t prior_sample=e->sample;
                    for(size_t j=k+1;j<end&&j<=k+3;j++){
                        const XR05Observation *f=&v[j];
                        if(!f->comparable||f->sample!=prior_sample+1)break;
                        prior_sample=f->sample;
                        if(!f->rate_reverified)c.raw_verified=0;
                        if((f->delta_kib>0&&net>INT64_MAX-f->delta_kib)||
                           (f->delta_kib<0&&net<INT64_MIN-f->delta_kib))break;
                        net+=f->delta_kib;
                        if(net<0&&xray_recovery_gate(cap32((uint64_t)(-net)),growth)>0){
                            c.end=f->sample;
                            c.recovered_kib=cap32((uint64_t)(-net));
                            break;
                        }
                    }
                    append_case(out,c);
                }
            }
            if(xray_transfer_surge_gate(e->read,e->write)>0){
                XR05Case c={0};
                c.kind=3;c.pid=e->pid;c.creation=e->creation;
                c.start=e->sample;c.end=e->sample;c.peak_sample=e->sample;
                c.peak_value=e->read>e->write?e->read:e->write;
                c.raw_verified=e->rate_reverified;
                append_case(out,c);
            }
        }
        /* CPU streak requires adjacent, complete, identity-matched samples. */
        size_t k=i;
        while(k<end){
            if(!v[k].comparable||!xray_cpu_hot_gate(v[k].cpu)){++k;continue;}
            size_t j=k+1;uint32_t peak=v[k].cpu,peak_sample=v[k].sample;
            int verified=v[k].rate_reverified;
            while(j<end&&v[j].comparable&&v[j].sample==v[j-1].sample+1&&
                  xray_cpu_hot_gate(v[j].cpu)>0){
                if(v[j].cpu>peak){peak=v[j].cpu;peak_sample=v[j].sample;}
                if(!v[j].rate_reverified)verified=0;
                ++j;
            }
            if(xray_cpu_streak_gate((uint32_t)(j-k))>0){
                XR05Case c={0};c.kind=2;c.pid=v[k].pid;c.creation=v[k].creation;
                c.start=v[k].sample;c.end=v[j-1].sample;c.peak_sample=peak_sample;
                c.peak_value=peak;c.raw_verified=(uint8_t)verified;
                append_case(out,c);
            }
            k=j;
        }
        i=end;
    }
    free(v);
    return 1;
}

/* Fixtures exercise ACTUAL linked Aura symbols, not mock gates. Includes
 * half-recovery, 3-sample CPU, I/O burst, PID reuse and sample gap rejection. */
int xr05_self_test(void){
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
        {6,43,0,702,0,0,400,0,1,1}
    };
    XR05Casebook out;
    if(!xr05_build_cases(v,sizeof(v)/sizeof(v[0]),&out)||
       out.count!=4||out.truncated)return 0;
    int recovered=0,unrecovered=0,streak=0,io=0;
    for(uint32_t i=0;i<out.count;i++){
        const XR05Case *c=&out.items[i];
        if(c->pid!=42||!c->raw_verified)return 0;
        if(c->kind==1&&c->start==2&&c->end==3&&c->recovered_kib==102400)recovered++;
        if(c->kind==1&&c->start==9&&!c->recovered_kib&&c->creation==200)unrecovered++;
        if(c->kind==2&&c->start==4&&c->end==6&&c->peak_value==1000)streak++;
        if(c->kind==3&&c->start==7&&c->peak_value==20000)io++;
    }
    return recovered==1&&unrecovered==1&&streak==1&&io==1;
}
