#include "aura_stdlib.h"

#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <stddef.h>

#define AURA_MAX_TENSORS 1024u

#if defined(AURA_ALLOC_REGION)
// Simple bump arena allocator for the stdlib/runtime (prototype).
// - No frees; memory is reclaimed when the process exits.
// - Designed for deterministic perf in short-lived CLI runs.
#ifndef AURA_ARENA_BYTES
#define AURA_ARENA_BYTES (16u * 1024u * 1024u)
#endif

static unsigned char g_aura_arena[AURA_ARENA_BYTES];
static size_t g_aura_arena_off = 0u;

static size_t aura_align_up(size_t v, size_t align) {
    if (align == 0u) {
        return v;
    }
    size_t r = v % align;
    if (r == 0u) {
        return v;
    }
    return v + (align - r);
}

static void* aura_arena_alloc(size_t bytes, size_t align) {
    size_t off = aura_align_up(g_aura_arena_off, align);
    if (off > (size_t)AURA_ARENA_BYTES || bytes > ((size_t)AURA_ARENA_BYTES - off)) {
        fprintf(stderr, "Aura region allocator OOM: requested %zu bytes (arena=%u)\n", bytes, (unsigned)AURA_ARENA_BYTES);
        fflush(stderr);
        abort();
    }
    void* p = (void*)(g_aura_arena + off);
    g_aura_arena_off = off + bytes;
    return p;
}

static void* aura_alloc_zeroed(size_t count, size_t elem_size) {
    size_t bytes = count * elem_size;
    void* p = aura_arena_alloc(bytes, elem_size);
    memset(p, 0, bytes);
    return p;
}
#endif

typedef struct AuraTensor {
    uint32_t len;
    uint32_t* data;
} AuraTensor;

static AuraTensor g_tensors[AURA_MAX_TENSORS];
static uint32_t g_next_tensor = 1u; // 0 is reserved as "invalid".

#define AURA_MAX_MODELS 256u
static uint32_t g_next_model = 1u; // 0 is reserved as "invalid".

void aura_io_println(const char* s) {
    if (!s) {
        puts("<null>");
        return;
    }
    puts(s);
}

void aura_range_check_u32(uint32_t v, uint32_t lo, uint32_t hi) {
    if (v < lo || v > hi) {
        fprintf(stderr, "Aura range check failed: %u not in [%u..%u]\n", (unsigned)v, (unsigned)lo, (unsigned)hi);
        fflush(stderr);
        abort();
    }
}

uint32_t aura_tensor_new(uint32_t len) {
    if (g_next_tensor >= AURA_MAX_TENSORS) {
        return 0u;
    }
    uint32_t h = g_next_tensor++;
    g_tensors[h].len = len;
#if defined(AURA_ALLOC_REGION)
    g_tensors[h].data = (uint32_t*)aura_alloc_zeroed((size_t)len, sizeof(uint32_t));
#else
    g_tensors[h].data = (uint32_t*)calloc((size_t)len, sizeof(uint32_t));
#endif
    return h;
}

uint32_t aura_tensor_len(uint32_t t) {
    if (t == 0u || t >= g_next_tensor) {
        return 0u;
    }
    return g_tensors[t].len;
}

uint32_t aura_tensor_get(uint32_t t, uint32_t index) {
    if (t == 0u || t >= g_next_tensor) {
        return 0u;
    }
    if (index >= g_tensors[t].len) {
        return 0u;
    }
    return g_tensors[t].data[index];
}

void aura_tensor_set(uint32_t t, uint32_t index, uint32_t value) {
    if (t == 0u || t >= g_next_tensor) {
        return;
    }
    if (index >= g_tensors[t].len) {
        return;
    }
    g_tensors[t].data[index] = value;
}

uint32_t aura_ai_load_model(const char* path) {
    (void)path;
    if (g_next_model >= AURA_MAX_MODELS) {
        return 0u;
    }
    return g_next_model++;
}

uint32_t aura_ai_infer(uint32_t model, uint32_t input) {
    (void)model;
    uint32_t len = aura_tensor_len(input);
    uint32_t out = aura_tensor_new(len);
    if (out == 0u) {
        return 0u;
    }
    for (uint32_t i = 0u; i < len; i++) {
        uint32_t v = aura_tensor_get(input, i);
        aura_tensor_set(out, i, v);
    }
    return out;
}

uint32_t io_load_tensor(const char* path) {
    (void)path;
    return aura_tensor_new(16u);
}

void io_display(uint32_t t) {
    printf("Tensor{id=%u}\n", (unsigned)t);
}

uint32_t compute_gradient(uint32_t data, uint32_t weight) {
    return data + weight;
}
