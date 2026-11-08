#ifndef AURA_XRAY_XR03_EXPLORER_H
#define AURA_XRAY_XR03_EXPLORER_H
#include <stdio.h>
#include <stddef.h>
/* Offline, bounded, strictly validated capture/2 importer and local HTML
 * investigation view. The three impact components call actual Aura cells. */
int xr03_render(FILE *out, const char *candidate_path,
                const char *baseline_path, int include_names, unsigned policy,
                char *error, size_t error_capacity);
/* Strict source-data -> headless JSON casebook (no names, fail-closed). */
int xr03_export_cases(FILE *out, const char *capture_path, unsigned policy,
                      char *error, size_t error_capacity);
/* Three Aura-compiled policy interpretations, one strict source parse. */
int xr03_export_sweep(FILE *out, const char *capture_path,
                      char *error, size_t error_capacity);
#endif
