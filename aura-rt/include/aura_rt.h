#pragma once

// Aura runtime header (C23 ABI).
// This is intentionally minimal for the Stage 5 pipeline.

#ifdef __cplusplus
extern "C" {
#endif

// Entry point emitted by the compiler into module.ll
int aura_entry(void);

#ifdef __cplusplus
}
#endif
