/*
 * AURA X-RAY XR-02 portable process-history and evidence presentation engine.
 * Windows API access lives only in win32_host.c. Domain normalization, rate
 * calculation and activity scoring are compiled from analysis.aura.
 */
#include "xr02_engine.h"
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <inttypes.h>

/* Native Aura ABI; missing generated cells cause the Windows linker to fail. */
extern uint32_t xray_read_kib_per_sec(uint32_t delta_kib, uint32_t elapsed_ms);
extern uint32_t xray_cpu_millicores(uint32_t delta_cpu_ms, uint32_t elapsed_ms);
extern uint32_t xray_activity_score(uint32_t cpu_milli, uint32_t read_kib_s,
                                    uint32_t write_kib_s, uint32_t memory_growth_mib);
extern uint32_t xray_working_set_mib(uint32_t working_set_kib);

/* Products by 1000 must fit u32 for current Aura uint32 ABI. */
#define XR_SAFE_RATE_DELTA 4294967u
#define XR_SAFE_INTERVAL 4294967u

XRAnalyzer *xr_new(void) { return (XRAnalyzer *)calloc(1, sizeof(XRAnalyzer)); }
void xr_free(XRAnalyzer *a) { free(a); }

void xr_begin_sample(XRAnalyzer *a, unsigned sample, uint64_t monotonic_ms) {
    if (!a) return;
    a->sample_id = sample;
    a->current_ms = monotonic_ms;
    a->curr_count = 0;
    if (sample > a->total_samples) a->total_samples = sample;
}

static const XRHistory *xr_previous(const XRAnalyzer *a, uint32_t pid) {
    for (unsigned i=0; i<a->prev_count; ++i)
        if (a->prev[i].pid == pid) return &a->prev[i];
    return NULL;
}

static void xr_leader_update(XRAnalyzer *a, const XRObservation *o, const XRMetrics *m) {
    if (!m->has_score || !o->has_identity) return;
    unsigned pos=XR_BOARD_LIMIT;
    for (unsigned i=0; i<a->leader_count; ++i) {
        if (a->leaders[i].pid==o->pid &&
            a->leaders[i].create_ticks==o->create_ticks) { pos=i; break; }
    }
    if (pos==XR_BOARD_LIMIT) {
        if (a->leader_count<XR_BOARD_LIMIT) pos=a->leader_count++;
        else {
            unsigned min_idx=0;
            for (unsigned i=1; i<a->leader_count; ++i)
                if (a->leaders[i].peak_score<a->leaders[min_idx].peak_score)
                    min_idx=i;
            if (m->activity_score<=a->leaders[min_idx].peak_score) return;
            pos=min_idx;
        }
        memset(&a->leaders[pos],0,sizeof(a->leaders[pos]));
        a->leaders[pos].pid=o->pid;
        a->leaders[pos].create_ticks=o->create_ticks;
    }
    XRLeader *b=&a->leaders[pos];
    ++b->samples;
    if (o->has_memory && o->working_set_kib<=UINT32_MAX)
        b->last_memory_mib=xray_working_set_mib((uint32_t)o->working_set_kib);
    if (m->cpu_millicores>b->peak_cpu_millicores) b->peak_cpu_millicores=m->cpu_millicores;
    if (m->read_kib_s>b->peak_read_kib_s) b->peak_read_kib_s=m->read_kib_s;
    if (m->write_kib_s>b->peak_write_kib_s) b->peak_write_kib_s=m->write_kib_s;
    if (m->has_memory_delta && m->memory_delta_kib>0) {
        uint64_t growth=(uint64_t)m->memory_delta_kib/1024u;
        if (growth>b->peak_memory_growth_mib)
            b->peak_memory_growth_mib=(uint32_t)(growth>UINT32_MAX?UINT32_MAX:growth);
    }
    if (m->activity_score>=b->peak_score) {
        b->peak_score=m->activity_score;
        b->peak_sample=a->sample_id;
    }
    if (o->name && o->name[0]) {
        size_t n=strlen(o->name);
        if (n>=sizeof(b->name)) n=sizeof(b->name)-1;
        memcpy(b->name,o->name,n);
        b->name[n]=0;
    }
}

XRMetrics xr_observe(XRAnalyzer *a, const XRObservation *o) {
    XRMetrics m;
    memset(&m,0,sizeof(m));
    m.quality="baseline";
    if (!a || !o) { m.quality="unavailable"; return m; }
    ++a->records;

    if (a->curr_count>=XR_TRACK_LIMIT) {
        ++a->track_overflows;
        m.quality="overflow";
        return m;
    }
    XRHistory *dst=&a->curr[a->curr_count++];
    dst->pid=o->pid;
    dst->create_ticks=o->create_ticks;
    dst->working_set_kib=o->working_set_kib;
    dst->read_bytes=o->read_bytes;
    dst->write_bytes=o->write_bytes;
    dst->cpu_ticks_100ns=o->cpu_ticks_100ns;
    dst->has_identity=o->has_identity;
    dst->has_memory=o->has_memory;
    dst->has_io=o->has_io;
    dst->has_cpu=o->has_cpu;

    if (!o->has_identity) {
        ++a->missing_identity; m.quality="no_identity"; return m;
    }
    const XRHistory *p=xr_previous(a,o->pid);
    if (!p) { ++a->baseline_records; return m; }
    if (!p->has_identity) { m.quality="no_identity"; return m; }
    if (p->create_ticks!=o->create_ticks) {
        ++a->reused_pids; m.quality="pid_reused"; return m;
    }
    if (a->current_ms<=a->prev_ms ||
        a->current_ms-a->prev_ms>XR_SAFE_INTERVAL) {
        m.quality="unavailable"; return m;
    }
    m.interval_ms=(uint32_t)(a->current_ms-a->prev_ms);
    int reset=0, overflow=0;
    if (o->has_memory && p->has_memory &&
        o->working_set_kib<=UINT32_MAX && p->working_set_kib<=UINT32_MAX) {
        m.memory_delta_kib=(int64_t)o->working_set_kib-(int64_t)p->working_set_kib;
        m.has_memory_delta=1;
    }
    if (o->has_cpu && p->has_cpu) {
        if (o->cpu_ticks_100ns<p->cpu_ticks_100ns) reset=1;
        else {
            uint64_t delta_ms=(o->cpu_ticks_100ns-p->cpu_ticks_100ns)/10000u;
            if (delta_ms>XR_SAFE_RATE_DELTA) overflow=1;
            else {
                m.cpu_millicores=xray_cpu_millicores((uint32_t)delta_ms,m.interval_ms);
                m.has_cpu=1;
            }
        }
    }
    if (o->has_io && p->has_io) {
        if (o->read_bytes<p->read_bytes || o->write_bytes<p->write_bytes)
            reset=1;
        else {
            uint64_t dr=(o->read_bytes-p->read_bytes)/1024u;
            uint64_t dw=(o->write_bytes-p->write_bytes)/1024u;
            if (dr>XR_SAFE_RATE_DELTA || dw>XR_SAFE_RATE_DELTA) overflow=1;
            else {
                m.read_kib_s=xray_read_kib_per_sec((uint32_t)dr,m.interval_ms);
                m.write_kib_s=xray_read_kib_per_sec((uint32_t)dw,m.interval_ms);
                m.has_read=1;
                m.has_write=1;
            }
        }
    }
    if (reset) { ++a->counter_resets; m.quality="counter_reset"; }
    else if (overflow) { ++a->overflow_events; m.quality="overflow"; }
    else if (m.has_cpu && m.has_read && m.has_write && m.has_memory_delta)
        m.quality="observed";
    else if (m.has_cpu || m.has_read || m.has_write || m.has_memory_delta)
        m.quality="partial";
    else m.quality="unavailable";

    /* Only score complete comparable observations. Unknown != zero. */
    if (!reset && !overflow && m.has_cpu && m.has_read && m.has_write &&
        m.has_memory_delta) {
        uint64_t growth=m.memory_delta_kib>0?(uint64_t)m.memory_delta_kib/1024u:0u;
        if (growth>16384u) growth=16384u;
        m.activity_score=xray_activity_score(m.cpu_millicores,m.read_kib_s,
                                               m.write_kib_s,(uint32_t)growth);
        m.has_score=1;
        ++a->derived_records;
        if (a->sample_id<=30) {
            ++a->sample_derived[a->sample_id];
            if (m.activity_score>a->sample_peak_score[a->sample_id])
                a->sample_peak_score[a->sample_id]=m.activity_score;
        }
        xr_leader_update(a,o,&m);
    }
    return m;
}

void xr_end_sample(XRAnalyzer *a) {
    if (!a) return;
    /* Track exited processes by stable identity, not by PID alone. */
    for (unsigned i=0; i<a->prev_count; ++i) {
        if (!a->prev[i].has_identity) continue;
        int found=0;
        for (unsigned j=0; j<a->curr_count; ++j) {
            if (a->prev[i].pid==a->curr[j].pid &&
                a->curr[j].has_identity &&
                a->prev[i].create_ticks==a->curr[j].create_ticks) {
                found=1; break;
            }
        }
        if (!found) ++a->ended_processes;
    }
    XRHistory *tmp=(XRHistory*)malloc(sizeof(a->prev));
    if (!tmp) {
        /* Fail closed for deltas if memory exhausted: no stale state reused. */
        a->prev_count=0; a->prev_ms=0; ++a->track_overflows; return;
    }
    memcpy(tmp,a->prev,sizeof(a->prev));
    memcpy(a->prev,a->curr,sizeof(a->curr));
    memcpy(a->curr,tmp,sizeof(a->curr));
    free(tmp);
    a->prev_count=a->curr_count;
    a->curr_count=0;
    a->prev_ms=a->current_ms;
}

static void html_escape(FILE *f, const char *s) {
    for (const unsigned char *p=(const unsigned char*)s; *p; ++p) {
        if (*p=='&') fputs("&amp;",f);
        else if (*p=='<') fputs("&lt;",f);
        else if (*p=='>') fputs("&gt;",f);
        else if (*p=='"') fputs("&quot;",f);
        else if (*p=='\'') fputs("&#39;",f);
        else if (*p>=32) fputc(*p,f);
    }
}

static int compare_leaders(const void *x, const void *y) {
    const XRLeader *a=x,*b=y;
    if (a->peak_score>b->peak_score) return -1;
    if (a->peak_score<b->peak_score) return 1;
    if (a->pid<b->pid) return -1;
    return a->pid>b->pid?1:0;
}

int xr_write_html(FILE *out, const XRAnalyzer *a, const char *sha,
                  unsigned seconds, uint64_t elapsed_ms, uint64_t denied,
                  uint64_t failed, int include_names) {
    if (!out || !a) return 0;
    fputs("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">"
          "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">"
          "<meta http-equiv=\"Content-Security-Policy\" "
          "content=\"default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; base-uri 'none'\">"
          "<title>AURA X-RAY / Local telemetry</title>"
          "<style>"
          ":root{color-scheme:dark;--bg:#070b14;--surface:#101827;--border:#27354a;--fg:#eff7ff;--muted:#9db1c9;--cyan:#60e4fa;--gold:#ffc977}"
          "*{box-sizing:border-box}body{margin:0;background:radial-gradient(ellipse at top right,#1a2846 0,#070b14 62%);font-family:system-ui,Segoe UI,sans-serif;color:var(--fg);line-height:1.5}"
          "main{max-width:1280px;margin:auto;padding:45px 28px 80px}header{border-bottom:1px solid var(--border);padding-bottom:28px;margin-bottom:28px}"
          ".eyebrow{color:var(--cyan);font-size:11px;letter-spacing:.29em;font-weight:750}.title{font-size:clamp(34px,7vw,73px);letter-spacing:-.07em;margin:4px 0;line-height:1.04}"
          "p{color:var(--muted)}.tiny{font-size:12px}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:12px;margin:24px 0}"
          ".card{background:linear-gradient(150deg,#152239,#0e1625);border:1px solid var(--border);border-radius:14px;padding:19px}.value{font-size:31px;font-weight:700;font-variant-numeric:tabular-nums}"
          ".label{color:var(--muted);font-size:11px;text-transform:uppercase;letter-spacing:.11em}"
          "section{margin:45px 0}h2{font-size:22px;letter-spacing:-.03em}.chart{display:flex;align-items:flex-end;gap:6px;height:128px;background:#0d1523;padding:12px;border:1px solid var(--border);border-radius:12px}"
          ".bar{flex:1;min-width:8px;background:linear-gradient(#65edfa,#1c638d);border-radius:4px 4px 0 0;position:relative}"
          ".bar:hover{background:var(--gold)}.bar b{position:absolute;bottom:-23px;left:30%;font-size:10px;color:var(--muted);font-weight:400}"
          ".toolbar{display:flex;gap:12px;flex-wrap:wrap;align-items:center}input{background:#111c2c;border:1px solid #3d526e;padding:12px 14px;border-radius:9px;color:#fff;min-width:225px}"
          ".tablewrap{overflow-x:auto;border:1px solid var(--border);border-radius:12px;margin-top:18px}table{width:100%;border-collapse:collapse;font-variant-numeric:tabular-nums;font-size:13px;min-width:800px}"
          "th,td{padding:13px;text-align:right;border-bottom:1px solid #203047}th:first-child,td:first-child{text-align:left}th{background:#111e30;color:var(--muted);font-size:11px;text-transform:uppercase;letter-spacing:.06em}"
          "tr:hover{background:#15253b}.score{font-weight:750;color:var(--cyan)}.foot{margin-top:40px;border-top:1px solid var(--border);padding-top:20px;color:var(--muted);font-size:12px}"
          "</style></head><body><main><header><div class=\"eyebrow\">AURA / SYSTEMS RESEARCH / LOCAL EVIDENCE</div>"
          "<h1 class=\"title\">X-RAY <span style=\"color:var(--cyan)\">/ 02</span></h1>"
          "<p>Process telemetry reconstructed from real Windows API observations. "
          "This is a read-only, offline report; activity scores are descriptive heuristics, not diagnoses or proofs.</p>"
          "<div class=\"tiny\">Built from source SHA: <code>",out);
    html_escape(out,sha);
    fputs("</code></div></header><div class=\"grid\">",out);
    fprintf(out,"<div class=\"card\"><div class=\"label\">Capture window</div><div class=\"value\">%.2f s</div><div class=\"tiny\">Requested %u s</div></div>",
            (double)elapsed_ms/1000.,seconds);
    fprintf(out,"<div class=\"card\"><div class=\"label\">Observations</div><div class=\"value\">%llu</div></div>",
            (unsigned long long)a->records);
    fprintf(out,"<div class=\"card\"><div class=\"label\">Complete rate vectors</div><div class=\"value\">%llu</div></div>",
            (unsigned long long)a->derived_records);
    fprintf(out,"<div class=\"card\"><div class=\"label\">Access denied</div><div class=\"value\">%llu</div></div>",
            (unsigned long long)denied);
    fprintf(out,"<div class=\"card\"><div class=\"label\">Process open failures</div><div class=\"value\">%llu</div></div>",
            (unsigned long long)failed);
    fprintf(out,"<div class=\"card\"><div class=\"label\">PID replacements observed</div><div class=\"value\">%llu</div></div></div>",
            (unsigned long long)a->reused_pids);
    fputs("<section><div class=\"eyebrow\">01 / ACTIVITY TIMELINE</div>"
          "<h2>Highest heuristic activity score per sample</h2>"
          "<p>Sample 1 is intentionally a baseline with no derived rates. "
          "No sample is silently treated as zero when process identity or counters are missing.</p>"
          "<div class=\"chart\" aria-label=\"Peak activity by sample\">",out);
    uint32_t maxv=1;
    for(unsigned i=1;i<=a->total_samples && i<=30;i++)
        if(a->sample_peak_score[i]>maxv)maxv=a->sample_peak_score[i];
    for(unsigned i=1;i<=a->total_samples && i<=30;i++) {
        uint32_t value=a->sample_peak_score[i];
        unsigned pct=(unsigned)((uint64_t)value*100u/maxv);
        if(pct<3)pct=3;
        fprintf(out,"<div class=\"bar\" title=\"Sample %u: score %u, valid vectors %u\" style=\"height:%u%%\"><b>%u</b></div>",
                i,value,a->sample_derived[i],pct,i);
    }
    fputs("</div></section><section><div class=\"eyebrow\">02 / PROCESS PRESSURE</div>"
          "<h2>Processes ranked by peak measured activity</h2>"
          "<p>CPU is reported in <b>millicores</b>: 1,000 = one fully utilized logical CPU. "
          "Read/write values are KiB/s derived from cumulative byte counter deltas. "
          "Scores are heuristic comparison aids, never automatic fault attribution.</p>"
          "<div class=\"toolbar\"><input id=\"filter\" type=\"search\" placeholder=\"Filter by PID or name\" "
          "aria-label=\"Filter process rows\"><span id=\"count\" class=\"tiny\"></span></div>"
          "<div class=\"tablewrap\"><table id=\"processes\"><thead><tr>"
          "<th>Process</th><th>PID</th><th>Peak score</th><th>CPU (milli)</th>"
          "<th>Read KiB/s</th><th>Write KiB/s</th><th>Growth MiB</th>"
          "<th>Last MiB</th><th>Peak sample</th></tr></thead><tbody>",out);
    XRLeader sorted[XR_BOARD_LIMIT];
    memcpy(sorted,a->leaders,a->leader_count*sizeof(XRLeader));
    qsort(sorted,a->leader_count,sizeof(XRLeader),compare_leaders);
    for(unsigned i=0;i<a->leader_count;i++) {
        const XRLeader *b=&sorted[i];
        fputs("<tr><td>",out);
        if(include_names && b->name[0])html_escape(out,b->name);
        else fputs("<span style=\"color:var(--muted)\">redacted</span>",out);
        fprintf(out,"</td><td>%u</td><td class=\"score\">%u</td><td>%u</td>"
                    "<td>%u</td><td>%u</td><td>%u</td><td>%u</td><td>%u</td></tr>",
                b->pid,b->peak_score,b->peak_cpu_millicores,
                b->peak_read_kib_s,b->peak_write_kib_s,
                b->peak_memory_growth_mib,b->last_memory_mib,b->peak_sample);
    }
    fputs("</tbody></table></div></section><section><div class=\"eyebrow\">03 / MEASUREMENT INTEGRITY</div>"
          "<h2>Uncertainty ledger</h2><div class=\"grid\">",out);
    fprintf(out,"<div class=\"card\"><div class=\"label\">Baseline records</div><div class=\"value\">%llu</div></div>",
            (unsigned long long)a->baseline_records);
    fprintf(out,"<div class=\"card\"><div class=\"label\">Unknown identities</div><div class=\"value\">%llu</div></div>",
            (unsigned long long)a->missing_identity);
    fprintf(out,"<div class=\"card\"><div class=\"label\">Counter resets</div><div class=\"value\">%llu</div></div>",
            (unsigned long long)a->counter_resets);
    fprintf(out,"<div class=\"card\"><div class=\"label\">History capacity misses</div><div class=\"value\">%llu</div></div></div>",
            (unsigned long long)a->track_overflows);
    fputs("<p class=\"tiny\">Limitations: no ETW, no root-cause proof, no attribution of shared "
          "memory, no guarantee of collecting protected processes. A process disappearing "
          "from an observed snapshot is not necessarily a confirmed exit. "
          "Use process creation time to guard against PID reuse.</p></section>"
          "<div class=\"foot\">NO NETWORK · NO EXTERNAL ASSETS · NO TELEMETRY UPLOAD · "
          "SOURCE: WINDOWS TOOLHELP32 / PSAPI / GETPROCESSTIMES · AURA BACKEND C<br>"
          "All process names remain redacted by default. Report is an offline snapshot.</div>"
          "<script>'use strict';const input=document.getElementById('filter');"
          "const rows=[...document.querySelectorAll('#processes tbody tr')];"
          "const n=document.getElementById('count');"
          "function show(){const q=input.value.trim().toLowerCase();"
          "let shown=0;for(const row of rows){const visible=row.textContent.toLowerCase().includes(q);"
          "row.hidden=!visible;if(visible)shown++;}n.textContent=shown+' of '+rows.length+' ranked processes';}"
          "input.addEventListener('input',show);show();</script>"
          "</main></body></html>",out);
    return !ferror(out);
}

/* Controlled fault-injection oracle, run through genuine Aura symbols. */
int xr_self_test_engine(void) {
    XRAnalyzer *a=xr_new();
    if(!a) return 0;
    XRObservation o;
    memset(&o,0,sizeof(o));
    o.pid=700; o.create_ticks=123456;
    o.has_identity=o.has_memory=o.has_io=o.has_cpu=1;
    o.working_set_kib=4096; o.read_bytes=1024; o.write_bytes=2048;
    o.cpu_ticks_100ns=100000;
    xr_begin_sample(a,1,10000);
    XRMetrics m=xr_observe(a,&o);
    if(strcmp(m.quality,"baseline")!=0 || m.has_score){xr_free(a);return 0;}
    xr_end_sample(a);
    o.working_set_kib+=4096;
    o.read_bytes+=8u*1024u*1024u;
    o.write_bytes+=4u*1024u*1024u;
    o.cpu_ticks_100ns+=500u*10000u;
    xr_begin_sample(a,2,11000);
    m=xr_observe(a,&o);
    /* CPU 500 milli, read 8192 KiB/s, write 4096 KiB/s,
     * growth 4 MiB -> Aura score = 5+4+2+8 = 19 */
    if(!m.has_score || m.cpu_millicores!=500 || m.read_kib_s!=8192 ||
       m.write_kib_s!=4096 || m.memory_delta_kib!=4096 ||
       m.activity_score!=19 || strcmp(m.quality,"observed")!=0){
        xr_free(a);return 0;
    }
    xr_end_sample(a);
    /* Reset must make rate unknown instead of wrapping an unsigned counter. */
    o.read_bytes=1;
    xr_begin_sample(a,3,12000);
    m=xr_observe(a,&o);
    if(m.has_score || strcmp(m.quality,"counter_reset")!=0){xr_free(a);return 0;}
    xr_end_sample(a);
    /* Reused PID with new creation time must never inherit old CPU/IO. */
    o.create_ticks++;
    xr_begin_sample(a,4,13000);
    m=xr_observe(a,&o);
    if(m.has_score || strcmp(m.quality,"pid_reused")!=0){xr_free(a);return 0;}
    xr_end_sample(a);
    /* Identity unavailable must never be interpreted as matching PID. */
    o.has_identity=0;
    xr_begin_sample(a,5,14000);
    m=xr_observe(a,&o);
    if(m.has_score || strcmp(m.quality,"no_identity")!=0){xr_free(a);return 0;}
    xr_end_sample(a);
    /* Remaining observed history remains bounded and non-overwriting. */
    if(a->derived_records!=1 || a->counter_resets!=1 ||
       a->reused_pids!=1 || a->missing_identity!=1){xr_free(a);return 0;}
    xr_free(a);
    return 1;
}
