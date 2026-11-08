#ifndef AURA_XRAY_ENGINE_H
#define AURA_XRAY_ENGINE_H
#include <stdint.h>
#include <stdio.h>

#define XR_TRACK_LIMIT 4096
#define XR_BOARD_LIMIT 128

/* Win32-originated observation. This platform-neutral engine does no OS I/O. */
typedef struct {
    uint32_t pid;
    uint64_t create_ticks;
    uint64_t working_set_kib;
    uint64_t read_bytes;
    uint64_t write_bytes;
    uint64_t cpu_ticks_100ns;
    unsigned char has_identity, has_memory, has_io, has_cpu;
    const char *name;  /* supplied only when process names are explicitly enabled */
} XRObservation;

typedef struct {
    const char *quality;   /* baseline, observed, partial, no_identity, pid_reused, counter_reset, unavailable, overflow */
    uint32_t interval_ms;
    uint32_t cpu_millicores;
    uint32_t read_kib_s;
    uint32_t write_kib_s;
    int64_t memory_delta_kib;
    uint32_t activity_score;
    unsigned char has_cpu, has_read, has_write, has_memory_delta, has_score;
} XRMetrics;

typedef struct {
    uint32_t pid;
    uint64_t create_ticks, working_set_kib, read_bytes, write_bytes, cpu_ticks_100ns;
    unsigned char has_identity, has_memory, has_io, has_cpu;
} XRHistory;

typedef struct {
    uint32_t pid;
    uint64_t create_ticks;
    uint32_t peak_score, peak_cpu_millicores, peak_read_kib_s, peak_write_kib_s;
    uint32_t peak_memory_growth_mib, last_memory_mib, samples, peak_sample;
    char name[144];
} XRLeader;

typedef struct {
    XRHistory prev[XR_TRACK_LIMIT];
    XRHistory curr[XR_TRACK_LIMIT];
    unsigned prev_count, curr_count, sample_id;
    uint64_t prev_ms, current_ms;
    XRLeader leaders[XR_BOARD_LIMIT];
    unsigned leader_count;
    uint64_t records, derived_records, baseline_records, missing_identity;
    uint64_t reused_pids, counter_resets, overflow_events, track_overflows;
    uint64_t ended_processes;
    uint32_t sample_derived[31], sample_peak_score[31];
    unsigned total_samples;
} XRAnalyzer;

XRAnalyzer *xr_new(void);
void xr_free(XRAnalyzer *a);
void xr_begin_sample(XRAnalyzer *a, unsigned sample, uint64_t monotonic_ms);
XRMetrics xr_observe(XRAnalyzer *a, const XRObservation *o);
void xr_end_sample(XRAnalyzer *a);
int xr_write_html(FILE *out, const XRAnalyzer *a, const char *build_sha,
                  unsigned seconds, uint64_t elapsed_ms, uint64_t open_denied,
                  uint64_t open_failed, int include_names);
/* Real native Aura function calls are executed as part of this deterministic oracle. */
int xr_self_test_engine(void);
#endif
