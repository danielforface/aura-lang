/*
 * AURA X-RAY XR-01 — unprivileged Win32 observation adapter and CLI.
 * Business metric classification is compiled from apps/xray/src/analysis.aura.
 * This C file is a platform boundary, not a substitute for the Aura analyzer.
 * Local-only, read-only, opt-in persistent output, no network calls.
 */
#define WIN32_LEAN_AND_MEAN
#define _CRT_SECURE_NO_WARNINGS
#include <windows.h>
#include <tlhelp32.h>
#include <psapi.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include "xr02_engine.h"
#include "xr03_explorer.h"

#ifndef XR_BUILD_SHA
#define XR_BUILD_SHA "unknown"
#endif
#define XR_SCHEMA "aura.xray.capture/2"
#define XR_VERSION "0.4.0-xr04"

extern uint32_t xray_memory_quanta_256m(uint32_t working_set_mib);
extern uint32_t xray_io_read_quanta_64m(uint32_t io_read_kib);
extern uint32_t xray_working_set_mib(uint32_t working_set_kib);
extern uint32_t xray_read_kib_per_sec(uint32_t delta_kib, uint32_t elapsed_ms);
extern uint32_t xray_cpu_millicores(uint32_t delta_cpu_ms, uint32_t elapsed_ms);
extern uint32_t xray_activity_score(uint32_t cpu_milli, uint32_t read_kib_s,
                                    uint32_t write_kib_s, uint32_t memory_growth_mib);
extern uint32_t xray_cpu_impact(uint32_t cpu_milli);
extern uint32_t xray_io_impact(uint32_t read_kib_s, uint32_t write_kib_s);
extern uint32_t xray_memory_impact(uint32_t growth_mib);

static void utc_now(char *out, size_t size) {
    SYSTEMTIME t;
    GetSystemTime(&t);
    (void)snprintf(out, size, "%04u-%02u-%02uT%02u:%02u:%02u.%03uZ",
        (unsigned)t.wYear, (unsigned)t.wMonth, (unsigned)t.wDay,
        (unsigned)t.wHour, (unsigned)t.wMinute, (unsigned)t.wSecond,
        (unsigned)t.wMilliseconds);
}

static uint64_t filetime_ticks(FILETIME t) {
    ULARGE_INTEGER x;
    x.LowPart = t.dwLowDateTime;
    x.HighPart = t.dwHighDateTime;
    return x.QuadPart;
}

static void wait_until_monotonic_ms(uint64_t deadline_ms) {
    /* Deadline-based sampling, no busy wait and no UTC wall-clock dependency. */
    for (;;) {
        uint64_t now_ms = GetTickCount64();
        if (now_ms >= deadline_ms) return;
        uint64_t remaining = deadline_ms - now_ms;
        Sleep((DWORD)(remaining > 1000u ? 1000u : remaining));
    }
}

static void json_string(FILE *f, const char *value) {
    fputc('"', f);
    for (const unsigned char *p = (const unsigned char*)value; *p; ++p) {
        switch (*p) {
            case '"': fputs("\\\"", f); break;
            case '\\': fputs("\\\\", f); break;
            case '\n': fputs("\\n", f); break;
            case '\r': fputs("\\r", f); break;
            case '\t': fputs("\\t", f); break;
            default:
                if (*p < 0x20) fprintf(f, "\\u%04x", (unsigned)*p);
                else fputc((int)*p, f);
        }
    }
    fputc('"', f);
}

static void process_name_utf8(const WCHAR *input, char *output, size_t size) {
    if (size == 0) return;
    output[0] = '\0';
    int converted = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, input, -1,
                                        output, (int)size, NULL, NULL);
    if (converted <= 0) strcpy_s(output, size, "<unavailable>");
}

typedef struct {
    unsigned long seen;
    unsigned long readable_memory;
    unsigned long denied;
    unsigned long open_failed;
    unsigned long errors;
} SampleStats;

static int collect_sample(FILE *out, unsigned sample, int include_names, XRAnalyzer *analyzer, SampleStats *stats) {
    HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snapshot == INVALID_HANDLE_VALUE) return 0;

    PROCESSENTRY32W entry;
    ZeroMemory(&entry, sizeof entry);
    entry.dwSize = sizeof entry;
    if (!Process32FirstW(snapshot, &entry)) {
        CloseHandle(snapshot);
        return 0;
    }

    char at[40];
    utc_now(at, sizeof at);
    uint64_t monotonic_ms = GetTickCount64();
    xr_begin_sample(analyzer, sample, monotonic_ms);
    fprintf(out, "{\"type\":\"sample_start\",\"sample\":%u,\"utc\":", sample);
    json_string(out, at);
    fprintf(out, ",\"monotonic_ms\":%llu}\n", (unsigned long long)monotonic_ms);
    do {
        ++stats->seen;
        DWORD pid = entry.th32ProcessID;
        DWORD err = 0;
        uint64_t create_ticks = 0;
        uint64_t read_kib = 0;
        uint64_t working_set_kib = 0;
        uint64_t read_bytes = 0, write_bytes = 0, cpu_ticks = 0;
        int have_memory = 0;
        int have_io = 0;
        int have_creation = 0, have_cpu = 0;

        /* Windows 10/11 GetProcessMemoryInfo, GetProcessIoCounters and GetProcessTimes
         * only need PROCESS_QUERY_LIMITED_INFORMATION. Never ask for VM_READ. */
        HANDLE p = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
        if (p != NULL) {
            PROCESS_MEMORY_COUNTERS pm;
            ZeroMemory(&pm, sizeof pm);
            pm.cb = sizeof pm;
            if (GetProcessMemoryInfo(p, &pm, sizeof pm)) {
                working_set_kib = ((uint64_t)pm.WorkingSetSize) / 1024u;
                have_memory = working_set_kib <= UINT32_MAX;
                if (have_memory) ++stats->readable_memory;
                else ++stats->errors;
            } else {
                err = GetLastError();
            }

            IO_COUNTERS io;
            ZeroMemory(&io, sizeof io);
            if (GetProcessIoCounters(p, &io)) {
                read_bytes = io.ReadTransferCount;
                write_bytes = io.WriteTransferCount;
                read_kib = read_bytes / 1024u;
                have_io = read_kib <= UINT32_MAX;
            } else if (!err) {
                err = GetLastError();
            }

            FILETIME creation, exit_time, kernel, user;
            if (GetProcessTimes(p, &creation, &exit_time, &kernel, &user)) {
                create_ticks = filetime_ticks(creation);
                have_creation = 1;
                uint64_t kt = filetime_ticks(kernel);
                uint64_t ut = filetime_ticks(user);
                if (kt <= UINT64_MAX - ut) {
                    cpu_ticks = kt + ut;
                    have_cpu = 1;
                }
            } else if (!err) {
                err = GetLastError();
            }
            CloseHandle(p);
        } else {
            err = GetLastError();
            ++stats->open_failed;
            if (err == ERROR_ACCESS_DENIED) ++stats->denied;
        }

        char utf8[MAX_PATH * 4];
        utf8[0] = '\0';
        if (include_names) process_name_utf8(entry.szExeFile, utf8, sizeof utf8);
        XRObservation observation = {0};
        observation.pid = (uint32_t)pid;
        observation.create_ticks = create_ticks;
        observation.working_set_kib = working_set_kib;
        observation.read_bytes = read_bytes;
        observation.write_bytes = write_bytes;
        observation.cpu_ticks_100ns = cpu_ticks;
        observation.has_identity = (unsigned char)have_creation;
        observation.has_memory = (unsigned char)have_memory;
        observation.has_io = (unsigned char)have_io;
        observation.has_cpu = (unsigned char)have_cpu;
        observation.name = include_names ? utf8 : NULL;
        XRMetrics metrics = xr_observe(analyzer, &observation);

        fprintf(out, "{\"type\":\"process\",\"sample\":%u,\"pid\":%lu",
            sample, (unsigned long)pid);
        if (include_names) {
            fputs(",\"process_name\":", out);
            json_string(out, utf8);
        }
        /* Snapshot-parent PID is a hint; never claim parent identity without
         * verifying its own creation FILETIME. Thread count is instantaneous. */
        fprintf(out, ",\"parent_pid\":%lu,\"thread_count\":%lu",
                (unsigned long)entry.th32ParentProcessID,
                (unsigned long)entry.cntThreads);

        fputs(",\"process_create_filetime\":", out);
        if (have_creation) fprintf(out, "%llu", (unsigned long long)create_ticks);
        else fputs("null", out);

        fputs(",\"working_set_kib\":", out);
        if (have_memory) fprintf(out, "%llu", (unsigned long long)working_set_kib);
        else fputs("null", out);

        fputs(",\"working_set_mib\":", out);
        if (have_memory) {
            uint32_t mib = xray_working_set_mib((uint32_t)working_set_kib);
            fprintf(out, "%u", mib);
        } else fputs("null", out);

        fputs(",\"memory_quanta_256m\":", out);
        if (have_memory) {
            uint32_t mib = xray_working_set_mib((uint32_t)working_set_kib);
            fprintf(out, "%u", xray_memory_quanta_256m(mib));
        } else fputs("null", out);

        fputs(",\"cumulative_io_read_kib\":", out);
        if (have_io) fprintf(out, "%llu", (unsigned long long)read_kib);
        else fputs("null", out);

        fputs(",\"cumulative_io_read_quanta_64m\":", out);
        if (have_io) fprintf(out, "%u", xray_io_read_quanta_64m((uint32_t)read_kib));
        else fputs("null", out);

        fputs(",\"cumulative_io_write_kib\":", out);
        if (have_io) fprintf(out, "%llu", (unsigned long long)(write_bytes/1024u));
        else fputs("null", out);
        fputs(",\"cpu_total_ms\":", out);
        if (have_cpu) fprintf(out, "%llu", (unsigned long long)(cpu_ticks/10000u));
        else fputs("null", out);
        /* XR-04 exact counter provenance. Required for version 0.4 capture,
         * absent in legacy XR-02/03 recordings; no schema/2 break. */
        fputs(",\"cpu_total_100ns\":", out);
        if (have_cpu) fprintf(out, "%llu", (unsigned long long)cpu_ticks);
        else fputs("null", out);
        fputs(",\"cumulative_io_read_bytes\":", out);
        if (have_io) fprintf(out, "%llu", (unsigned long long)read_bytes);
        else fputs("null", out);
        fputs(",\"cumulative_io_write_bytes\":", out);
        if (have_io) fprintf(out, "%llu", (unsigned long long)write_bytes);
        else fputs("null", out);
        fputs(",\"delta_quality\":", out);
        json_string(out, metrics.quality);
        fputs(",\"interval_ms\":", out);
        if (metrics.interval_ms) fprintf(out, "%u", metrics.interval_ms);
        else fputs("null", out);
        fputs(",\"cpu_millicores\":", out);
        if (metrics.has_cpu) fprintf(out, "%u", metrics.cpu_millicores);
        else fputs("null", out);
        fputs(",\"io_read_kib_s\":", out);
        if (metrics.has_read) fprintf(out, "%u", metrics.read_kib_s);
        else fputs("null", out);
        fputs(",\"io_write_kib_s\":", out);
        if (metrics.has_write) fprintf(out, "%u", metrics.write_kib_s);
        else fputs("null", out);
        fputs(",\"memory_delta_kib\":", out);
        if (metrics.has_memory_delta) fprintf(out, "%lld", (long long)metrics.memory_delta_kib);
        else fputs("null", out);
        fputs(",\"activity_score\":", out);
        if (metrics.has_score) fprintf(out, "%u", metrics.activity_score);
        else fputs("null", out);
        fputs(",\"evidence_class\":\"COMPUTED_FROM_OS_OBSERVATIONS\"", out);

        if (!have_memory || !have_io || !have_creation || !have_cpu) {
            fputs(",\"quality\":\"partial\"", out);
            if (err) fprintf(out, ",\"win32_error\":%lu", (unsigned long)err);
        } else {
            fputs(",\"quality\":\"observed\"", out);
        }
        fputs("}\n", out);
    } while (Process32NextW(snapshot, &entry));

    DWORD last = GetLastError();
    CloseHandle(snapshot);
    if (last != ERROR_NO_MORE_FILES) {
        ++stats->errors;
        return 0;
    }
    xr_end_sample(analyzer);
    fprintf(out, "{\"type\":\"sample_end\",\"sample\":%u,\"processes\":%lu,"
                 "\"readable_memory\":%lu,\"open_denied\":%lu,\"open_failed\":%lu,\"errors\":%lu,"
                 "\"complete_rate_vectors\":%u,\"peak_activity_score\":%u}\n",
                 sample, stats->seen, stats->readable_memory, stats->denied, stats->open_failed, stats->errors,
                 sample <= 30 ? analyzer->sample_derived[sample] : 0u,
                 sample <= 30 ? analyzer->sample_peak_score[sample] : 0u);
    return 1;
}

static int self_test(void) {
    static const struct {uint32_t mib, expected;} memory_tests[] = {
        {0,0}, {255,0}, {256,1}, {511,1}, {1024,4}, {2048,8}
    };
    for (size_t i = 0; i < sizeof(memory_tests)/sizeof(memory_tests[0]); ++i) {
        uint32_t observed = xray_memory_quanta_256m(memory_tests[i].mib);
        if (observed != memory_tests[i].expected) {
            fprintf(stderr, "AURA CORE CONTRACT FAIL: case %zu; got %u expected %u\n",
                i, observed, memory_tests[i].expected);
            return 0;
        }
    }
    if (xray_io_read_quanta_64m(65535) != 0 ||
        xray_io_read_quanta_64m(65536) != 1 ||
        xray_io_read_quanta_64m(131072) != 2 ||
        xray_working_set_mib(1023) != 0 ||
        xray_working_set_mib(1024) != 1 ||
        xray_working_set_mib(4096) != 4) {
        fputs("AURA CORE CONTRACT FAIL: conversion or IO quanta\n", stderr);
        return 0;
    }
    if (xray_read_kib_per_sec(1024,1000) != 1024 ||
        xray_read_kib_per_sec(8192,2000) != 4096 ||
        xray_cpu_millicores(500,1000) != 500 ||
        xray_cpu_millicores(2000,1000) != 2000 ||
        xray_activity_score(500,8192,4096,4) != 19 ||
        xray_activity_score(0,0,0,0) != 0) {
        fputs("AURA CORE CONTRACT FAIL: temporal normalization or score\n",stderr);
        return 0;
    }
    if (!xr_self_test_engine()) {
        fputs("XRAY TEMPORAL CONTRACT FAIL: identity/counter/reset fixture\n",stderr);
        return 0;
    }
    if (xray_cpu_impact(1625)!=16 || xray_cpu_impact(999)!=9 ||
        xray_io_impact(4096,2048)!=3 || xray_io_impact(0,0)!=0 ||
        xray_memory_impact(306)!=612 || xray_memory_impact(0)!=0) {
        fputs("XRAY EVIDENCE CONTRACT FAIL: decomposed Aura score components\n",stderr);
        return 0;
    }
    puts("AURA_CORE_CONTRACT_PASS 24 vectors; source=analysis.aura; backend=c");
    puts("XRAY_TEMPORAL_CONTRACT_PASS baseline, rates, reset, PID reuse, unknown identity");
    return 1;
}

static void usage(FILE *out) {
    fputs("AURA X-RAY XR-03 -- Multidimensional Evidence Workbench\n"
          "Usage:\n"
          "  AuraXRay.exe --help\n"
          "  AuraXRay.exe --version\n"
          "  AuraXRay.exe --self-test\n"
          "  AuraXRay.exe --capture --seconds N --out FILE [--report REPORT.html] [--include-names]\n"
          "  AuraXRay.exe --replay CAPTURE.jsonl --report WORKBENCH.html [--include-names]\n"
          "  AuraXRay.exe --compare BASELINE.jsonl CANDIDATE.jsonl --report DIFF.html [--include-names]\n"
          "Default is read-only, local-only, unprivileged, with no network.\n"
          "Process names are excluded unless --include-names is requested.\n"
          "--seconds N schedules N samples across a full N-second window.\n"
          "XR-04: exact 64-bit CPU/IO evidence, parent-PID hints and thread counts.\n"
          "Reconstructs source deltas when raw counters are available; legacy /2 remains readable.\n"
          "Strict capture/2 validation: invalid input is refused, never silently repaired.\n"
          "Optional offline workbench; process names are opt-in only.\n"
          "Limitations: no ETW, no automatic fixes, no causal/root-cause proof.\n", out);
}

int main(int argc, char **argv) {
    if (argc == 1) { usage(stdout); return 0; }
    int capture = 0, include_names = 0, mode = 0;
    unsigned seconds = 0;
    const char *out_path = NULL, *report_path = NULL;
    const char *input_path = NULL, *baseline_path = NULL;

    for (int i = 1; i < argc; ++i) {
        if (strcmp(argv[i], "--help") == 0 && argc == 2) {
            usage(stdout);
            return 0;
        }
        if (strcmp(argv[i], "--version") == 0 && argc == 2) {
            printf("AuraXRay %s Windows-x64 build=%s schema=%s\n",
                XR_VERSION, XR_BUILD_SHA, XR_SCHEMA);
            return 0;
        }
        if (strcmp(argv[i], "--self-test") == 0 && argc == 2) {
            return self_test() ? 0 : 2;
        }
        if (strcmp(argv[i], "--capture") == 0) { capture = 1; continue; }
        if (strcmp(argv[i], "--replay") == 0 && i+1 < argc && !mode) {
            mode = 1; input_path = argv[++i]; continue;
        }
        if (strcmp(argv[i], "--compare") == 0 && i+2 < argc && !mode) {
            mode = 2; baseline_path = argv[++i]; input_path = argv[++i]; continue;
        }
        if (strcmp(argv[i], "--include-names") == 0) { include_names = 1; continue; }
        if (strcmp(argv[i], "--seconds") == 0 && i+1 < argc) {
            const char *v = argv[++i];
            char *end = NULL;
            unsigned long s = strtoul(v, &end, 10);
            if (v == end || *end || s < 1 || s > 30) {
                fputs("ERROR --seconds must be between 1 and 30\n", stderr);
                return 2;
            }
            seconds = (unsigned)s;
            continue;
        }
        if (strcmp(argv[i], "--out") == 0 && i+1 < argc) {
            out_path = argv[++i];
            continue;
        }
        if (strcmp(argv[i], "--report") == 0 && i+1 < argc) {
            report_path = argv[++i];
            continue;
        }
        fprintf(stderr, "ERROR unknown or incomplete option: %s\n", argv[i]);
        return 2;
    }

    if (mode) {
        if (capture || seconds || out_path || !input_path || !input_path[0] ||
            !report_path || !report_path[0] ||
            strcmp(report_path,input_path)==0 ||
            (baseline_path && (strcmp(report_path,baseline_path)==0 ||
                               strcmp(baseline_path,input_path)==0))) {
            fputs("ERROR replay/compare requires new --report path and distinct input files\n",stderr);
            return 2;
        }
        FILE *report=NULL;
        if (fopen_s(&report,report_path,"wx")!=0 || !report) {
            fputs("ERROR report file already exists or cannot be created\n",stderr);
            return 3;
        }
        char diagnostics[240]={0};
        int ok=xr03_render(report,input_path,baseline_path,include_names,
                           diagnostics,sizeof diagnostics);
        if (fclose(report)!=0) ok=0;
        if (!ok) {
            remove(report_path);
            fprintf(stderr,"XRAY_REPLAY_REJECTED %s\n",diagnostics[0]?diagnostics:"report write failed");
            return 4;
        }
        printf("AURA_XRAY_EVIDENCE_OK mode=%s input=%s report=%s source=validated_capture2 aura=compiled\n",
               mode==2?"compare":"replay",input_path,report_path);
        return 0;
    }

    if (!capture || !out_path || !out_path[0] || !seconds) {
        fputs("ERROR --capture requires --seconds N and --out FILE\n", stderr);
        return 2;
    }

    XRAnalyzer *analyzer = xr_new();
    if (!analyzer) { fputs("ERROR cannot allocate bounded telemetry history\n",stderr); return 3; }
    FILE *report = NULL;
    if (report_path) {
        if (!report_path[0] || strcmp(report_path,out_path)==0 ||
            fopen_s(&report,report_path,"wx")!=0 || !report) {
            xr_free(analyzer);
            fputs("ERROR report output must be new and distinct from capture path\n",stderr);
            return 3;
        }
    }
    FILE *out = NULL;
    errno_t result = fopen_s(&out, out_path, "wx");
    if (result || !out) {
        if (report) { fclose(report); remove(report_path); }
        xr_free(analyzer);
        fprintf(stderr, "ERROR cannot create output (file must not exist): %s\n", out_path);
        return 3;
    }

    uint64_t started_ms = GetTickCount64();
    char started[40];
    utc_now(started, sizeof started);
    fprintf(out, "{\"type\":\"capture_start\",\"schema\":\"%s\",\"version\":\"%s\","
                 "\"build\":\"%s\",\"utc\":\"%s\",\"monotonic_ms\":%llu,\"seconds\":%u,"
                 "\"platform\":\"windows-x64\",\"source\":\"toolhelp32-psapi\","
                 "\"privacy\":\"%s\"}\n",
                 XR_SCHEMA, XR_VERSION, XR_BUILD_SHA, started, (unsigned long long)started_ms, seconds,
                 include_names ? "process_names_opt_in" : "process_names_redacted");

    uint64_t count = 0, readable = 0, denied = 0, open_failed = 0, errors = 0;
    for (unsigned s = 1; s <= seconds; ++s) {
        if (s > 1) wait_until_monotonic_ms(started_ms + 1000u * (uint64_t)(s - 1));
        SampleStats stats = {0};
        if (!collect_sample(out, s, include_names, analyzer, &stats)) {
            fprintf(out, "{\"type\":\"error\",\"sample\":%u,\"status\":\"snapshot_failed\"}\n", s);
            fprintf(out, "{\"type\":\"capture_end\",\"status\":\"partial\",\"samples\":%u}\n", s-1);
            fclose(out);
            if (report) { fclose(report); remove(report_path); }
            xr_free(analyzer);
            fprintf(stderr, "ERROR Win32 process snapshot failed (sample %u)\n", s);
            return 4;
        }
        count += stats.seen; readable += stats.readable_memory;
        denied += stats.denied; open_failed += stats.open_failed; errors += stats.errors;
    }

    /* Complete the full N-second capture window; N samples begin at 0..N-1s. */
    wait_until_monotonic_ms(started_ms + 1000u * (uint64_t)seconds);
    uint64_t ended_ms = GetTickCount64();
    uint64_t elapsed_ms = ended_ms - started_ms;
    char finished[40];
    utc_now(finished, sizeof finished);

    fprintf(out, "{\"type\":\"capture_end\",\"status\":\"complete\","
                 "\"samples\":%u,\"process_records\":%llu,\"readable_memory\":%llu,"
                 "\"open_denied\":%llu,\"open_failed\":%llu,\"errors\":%llu,"
                 "\"monotonic_ms\":%llu,\"elapsed_ms\":%llu,\"utc\":\"%s\","
                 "\"derived_records\":%llu,\"baseline_records\":%llu,"
                 "\"unknown_identity\":%llu,\"pid_reused\":%llu,\"counter_resets\":%llu,"
                 "\"history_capacity_misses\":%llu}\n",
                 seconds, (unsigned long long)count, (unsigned long long)readable,
                 (unsigned long long)denied, (unsigned long long)open_failed, (unsigned long long)errors,
                 (unsigned long long)ended_ms, (unsigned long long)elapsed_ms, finished,
                 (unsigned long long)analyzer->derived_records,
                 (unsigned long long)analyzer->baseline_records,
                 (unsigned long long)analyzer->missing_identity,
                 (unsigned long long)analyzer->reused_pids,
                 (unsigned long long)analyzer->counter_resets,
                 (unsigned long long)analyzer->track_overflows);
    if (fclose(out) != 0) {
        if (report) { fclose(report); remove(report_path); }
        xr_free(analyzer);
        fputs("ERROR unable to flush capture\n", stderr);
        return 5;
    }
    if (report) {
        char diagnostics[240]={0};
        int ok = xr03_render(report,out_path,NULL,include_names,
                             diagnostics,sizeof diagnostics);
        if (!ok) fprintf(stderr,"XRAY_REPORT_INVALID %s\n",diagnostics);
        if (fclose(report)!=0) ok=0;
        if (!ok) {
            remove(report_path);
            xr_free(analyzer);
            fputs("ERROR unable to finalize HTML report (capture remains valid)\n",stderr);
            return 6;
        }
    }
    printf("AURA_XRAY_CAPTURE_OK samples=%u processes=%llu readable=%llu denied=%llu open_failed=%llu errors=%llu derived=%llu elapsed_ms=%llu output=%s",
           seconds, (unsigned long long)count, (unsigned long long)readable,
           (unsigned long long)denied, (unsigned long long)open_failed,
           (unsigned long long)errors, (unsigned long long)analyzer->derived_records,
           (unsigned long long)elapsed_ms, out_path);
    if (report_path) printf(" report=%s",report_path);
    putchar('\n');
    xr_free(analyzer);
    return 0;
}
