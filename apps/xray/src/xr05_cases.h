#ifndef AURA_XRAY_CASES_H
#define AURA_XRAY_CASES_H
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define XR05_MAX_CASES 512
#define XR05_MAX_INPUT 16000
/* OS-free. C transports observed identities; actual signal gates are Aura. */
typedef struct {
    uint32_t sample,pid,score,cpu,read,write;
    uint64_t creation;
    int64_t delta_kib;
    uint8_t comparable,rate_reverified;
} XR05Observation;
typedef struct {
    uint32_t kind,pid,start,end,peak_sample,peak_value,recovered_kib;
    uint64_t creation;
    uint8_t raw_verified;
} XR05Case;
typedef struct {
    XR05Case items[XR05_MAX_CASES];
    uint32_t count,truncated;
} XR05Casebook;
int xr05_build_cases(const XR05Observation *src,size_t n,XR05Casebook *out);
int xr05_self_test(void);
#endif
