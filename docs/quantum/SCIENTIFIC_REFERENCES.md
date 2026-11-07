# Research standards and independently verifiable reference stack

**Research date:** 2026-11-05. Pin explicit versions during implementation; these URLs are external scientific/standards sources, not endorsements or code imports. Licensing and compatibility require separate review.

| Baseline | Official or primary source | What we must learn / compare |
|---|---|---|
| OpenQASM 3.1 | https://github.com/openqasm/openqasm | Current language specification, classical feedback, calibrations, supported subsets and grammar |
| OpenQASM live spec | https://openqasm.com/ | Gates, timing, OpenPulse grammar and actual spec version |
| QIR Alliance | https://github.com/qir-alliance/qir-spec | LLVM-based Quantum IR profiles, QIS, metadata, output recording |
| QIR Base Profile | https://github.com/qir-alliance/qir-spec/blob/main/specification/profiles/Base_Profile.md | End measurements; no automatic dynamic-circuit claim |
| Guppy ownership | https://docs.quantinuum.com/guppy/language_guide/ownership.html | Linear qubit handling; do not misstate repeated measurement semantics |
| Guppy V1 release | https://docs.quantinuum.com/product-updates/posts/platform_8_2026_dev_release/ | Existing competitor features, control/adjoint, TKET optimization |
| MQT QCEC | https://github.com/munich-quantum-toolkit/qcec | Decision-diagram, ZX, phase-aware and compilation-flow equivalence |
| MQT QCEC docs | https://mqt.readthedocs.io/projects/qcec/en/stable/ | Proof vs simulation, supported partial/parameterized/approximate profiles |
| Qiskit target model | https://docs.quantum.ibm.com/api/qiskit/transpiler | Connectivity, gate basis and device target constraints |
| Q# / QDK | https://learn.microsoft.com/en-us/azure/quantum/ | Classical/quantum language, QIR interop and developer workflow |
| CUDA-Q | https://nvidia.github.io/cuda-quantum/latest/ | Quantum/classical hybrid, simulation targets, GPU acceleration baselines |
| tket | https://docs.quantinuum.com/tket/ | Optimization, routing, hardware awareness and real-world flow |

**Contract:** citations are evidence of what the external technologies currently offer; Aura's distinctive value requires independent reproducible benchmark demonstration. A quantum standard document does not imply Aura implements it. Profiles must pin versions and reject unknown extensions explicitly.
