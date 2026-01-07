#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// ---- Aura StdLib ABI (prototype, C23-compatible) ----

// `io.println(String)` (prototype): expects a null-terminated UTF-8 string.
void aura_io_println(const char* s);

// Runtime-enforced range check used by the native pipeline.
// Traps (aborts) on failure.
void aura_range_check_u32(uint32_t v, uint32_t lo, uint32_t hi);

// Minimal Tensor model for Stage 11/12 prototyping.
// We represent `Tensor` as an opaque u32 handle in the compiler.
uint32_t aura_tensor_new(uint32_t len);
uint32_t aura_tensor_len(uint32_t t);
uint32_t aura_tensor_get(uint32_t t, uint32_t index);
void aura_tensor_set(uint32_t t, uint32_t index, uint32_t value);

// === AI builtins (prototype) ===
// Model handles are opaque u32 values.
uint32_t aura_ai_load_model(const char* path);
uint32_t aura_ai_infer(uint32_t model, uint32_t input);

// Compatibility with existing demo program builtins.
uint32_t io_load_tensor(const char* path);
void io_display(uint32_t t);
uint32_t compute_gradient(uint32_t data, uint32_t weight);

#ifdef __cplusplus
}
#endif
