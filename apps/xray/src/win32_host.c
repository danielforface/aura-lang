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

#ifndef XR_BUILD_SHA
#define XR_BUILD_SHA "unknown"
#endif
#define XR_SCHEMA "aura.xray.capture/1"
#define XR_VERSION "0.1.0-xr01"

extern uint32_t xray_memory_quanta_256m(uint32_t working_set_mib);
extern uint32_t xray_io_read_quanta_64m(uint32_t io_read_kib);
extern uint32_t xray_working_set_mib(uint32_t working_set_kib);

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
    unsigned long errors;
} SampleStats;

static int collect_sample(FILE *out, unsigned sample, int include_names, SampleStats *stats) {
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
        int have_memory = 0;
        int have_io = 0;
        int have_creation = 0;

        HANDLE p = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, FALSE, pid);
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
                read_kib = io.ReadTransferCount / 1024u;
                have_io = read_kib <= UINT32_MAX;
            } else if (!err) {
                err = GetLastError();
            }

            FILETIME creation, exit_time, kernel, user;
            if (GetProcessTimes(p, &creation, &exit_time, &kernel, &user)) {
                create_ticks = filetime_ticks(creation);
                have_creation = 1;
            }
            CloseHandle(p);
        } else {
            err = GetLastError();
            ++stats->denied;
        }

        fprintf(out, "{\"type\":\"process\",\"sample\":%u,\"pid\":%lu",
            sample, (unsigned long)pid);

        if (include_names) {
            char utf8[MAX_PATH * 4];
            process_name_utf8(entry.szExeFile, utf8, sizeof utf8);
            fputs(",\"process_name\":", out);
            json_string(out, utf8);
        }

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

        if (!have_memory || !have_io || !have_creation) {
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
    fprintf(out, "{\"type\":\"sample_end\",\"sample\":%u,\"processes\":%lu,"
                 "\"readable_memory\":%lu,\"open_denied\":%lu,\"errors\":%lu}\n",
                 sample, stats->seen, stats->readable_memory, stats->denied, stats->errors);
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
    puts("AURA_CORE_CONTRACT_PASS 12 vectors; source=analysis.aura; backend=c");
    return 1;
}

static void usage(FILE *out) {
    fputs("AURA X-RAY XR-01 -- Native Windows Process Observation\n"
          "Usage:\n"
          "  AuraXRay.exe --help\n"
          "  AuraXRay.exe --version\n"
          "  AuraXRay.exe --self-test\n"
          "  AuraXRay.exe --capture --seconds N --out FILE [--include-names]\n"
          "Default is read-only, local-only, unprivileged, with no network.\n"
          "Process names are excluded unless --include-names is requested.\n"
          "Current limitations: no CPU rate, no ETW, no GUI, no root-cause proof.\n", out);
}

int main(int argc, char **argv) {
    if (argc == 1) { usage(stdout); return 0; }
    int capture = 0, include_names = 0;
    unsigned seconds = 0;
    const char *out_path = NULL;

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
        fprintf(stderr, "ERROR unknown or incomplete option: %s\n", argv[i]);
        return 2;
    }

    if (!capture || !out_path || !out_path[0] || !seconds) {
        fputs("ERROR --capture requires --seconds N and --out FILE\n", stderr);
        return 2;
    }

    FILE *out = NULL;
    errno_t result = fopen_s(&out, out_path, "wx");
    if (result || !out) {
        fprintf(stderr, "ERROR cannot create output (file must not exist): %s\n", out_path);
        return 3;
    }

    char started[40];
    utc_now(started, sizeof started);
    fprintf(out, "{\"type\":\"capture_start\",\"schema\":\"%s\",\"version\":\"%s\","
                 "\"build\":\"%s\",\"utc\":\"%s\",\"seconds\":%u,"
                 "\"platform\":\"windows-x64\",\"source\":\"toolhelp32-psapi\","
                 "\"privacy\":\"%s\"}\n",
                 XR_SCHEMA, XR_VERSION, XR_BUILD_SHA, started, seconds,
                 include_names ? "process_names_opt_in" : "process_names_redacted");

    uint64_t count = 0, readable = 0, denied = 0, errors = 0;
    for (unsigned s = 1; s <= seconds; ++s) {
        SampleStats stats = {0};
        if (!collect_sample(out, s, include_names, &stats)) {
            fprintf(out, "{\"type\":\"error\",\"sample\":%u,\"status\":\"snapshot_failed\"}\n", s);
            fprintf(out, "{\"type\":\"capture_end\",\"status\":\"partial\",\"samples\":%u}\n", s-1);
            fclose(out);
            fprintf(stderr, "ERROR Win32 process snapshot failed (sample %u)\n", s);
            return 4;
        }
        count += stats.seen; readable += stats.readable_memory;
        denied += stats.denied; errors += stats.errors;
        if (s < seconds) Sleep(1000);
    }

    fprintf(out, "{\"type\":\"capture_end\",\"status\":\"complete\","
                 "\"samples\":%u,\"process_records\":%llu,\"readable_memory\":%llu,"
                 "\"open_denied\":%llu,\"errors\":%llu}\n",
                 seconds, (unsigned long long)count, (unsigned long long)readable,
                 (unsigned long long)denied, (unsigned long long)errors);
    if (fclose(out) != 0) {
        fputs("ERROR unable to flush capture\n", stderr);
        return 5;
    }
    printf("AURA_XRAY_CAPTURE_OK samples=%u processes=%llu readable=%llu denied=%llu errors=%llu output=%s\n",
           seconds, (unsigned long long)count, (unsigned long long)readable,
           (unsigned long long)denied, (unsigned long long)errors, out_path);
    return 0;
}
