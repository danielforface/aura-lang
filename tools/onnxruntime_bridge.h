// Minimal ONNX Runtime ABI shim for Aura (bootstrap)
// This header is designed to be parsed by aura-bridge's regex-based extractor.
// Keep prototypes simple (no structs in the signature).

#ifdef __cplusplus
extern "C" {
#endif

// Returns 1 if ORT is linkable/available.
unsigned int onnxruntime_available(void);

#ifdef __cplusplus
}
#endif
