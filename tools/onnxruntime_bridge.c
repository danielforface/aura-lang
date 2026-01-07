// Minimal ONNX Runtime ABI shim for Aura.
//
// We intentionally avoid including ONNX Runtime headers to keep the shim
// independent of include paths and bridge parser limitations.

#ifdef __cplusplus
extern "C" {
#endif

// ORT exports this symbol from onnxruntime.dll.
// In official headers it is: const OrtApiBase* OrtGetApiBase(void);
// For this shim we only need the address to exist.
const void* OrtGetApiBase(void);

unsigned int onnxruntime_available(void) {
    return OrtGetApiBase() ? 1u : 0u;
}

#ifdef __cplusplus
}
#endif
