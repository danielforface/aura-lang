#ifndef AURA_XRAY_XR08_STUDIO_H
#define AURA_XRAY_XR08_STUDIO_H
#include <stdint.h>
#include <stddef.h>
#include "xr05_cases.h"

#define XR08_SAMPLES 30u
#define XR08_VIEW_W 1360u
#define XR08_VIEW_H 840u

/* Immutable, source-verified capture/2 projection owned by C only.
 * Aura's UI functions receive primitive scalar values and own UI decisions;
 * never pass this structure/pointers through unqualified compiler FFI. */
typedef struct {
    uint32_t seconds, actual_samples, raw_precision;
    uint64_t records, derived, raw_verified;
    char build[48];
    uint32_t sample_peak[XR08_SAMPLES+1];
    uint32_t sample_count[XR08_SAMPLES+1];
    uint32_t sample_valid[XR08_SAMPLES+1];
    XR05Casebook policy[3];
    XR05Observation *events;
    size_t event_count;
} XR08StudioModel;

/* Reject incomplete capture, tampered counters, over-capacity case lists. */
int xr03_load_studio(const char *path, XR08StudioModel *out,
                     char *error, size_t error_capacity);
/* Real GDI native window or offscreen evidence bitmap, same renderer. */
int xr08_studio(const char *path, const char *preview_bmp,
                char *error, size_t error_capacity);
int xr08_studio_self_test(void);
#endif
