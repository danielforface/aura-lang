#ifndef AURA_XRAY_XR03_EXPLORER_H
#define AURA_XRAY_XR03_EXPLORER_H
#include <stdio.h>
#include <stddef.h>
/* Offline, bounded, strictly validated capture/2 importer and local HTML
 * investigation view. The three impact components call actual Aura cells. */
int xr03_render(FILE *out, const char *candidate_path,
                const char *baseline_path, int include_names,
                char *error, size_t error_capacity);
#endif
