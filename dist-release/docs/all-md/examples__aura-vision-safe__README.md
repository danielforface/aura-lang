Aura Vision Safe (demo)

This demo exercises the "shape-safe inference" vertical slice:
- `ai.load_model("...")` reads the ONNX model IO contract at compile time.
- `model.infer(input)` requires Z3 to prove the input tensor shape matches the model contract.

Run from repo root:
- `cargo run -p aura --features z3,llvm -- run examples/aura-vision-safe/vision_safe.aura --backend llvm --optimize full`

Notes:
- The model file in this folder is a minimal contract-only ONNX-like protobuf blob used by the verifier.
- Runtime inference is currently a prototype stub (it returns a copied tensor). This demo focuses on compile-time shape safety.
