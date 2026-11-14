# X-004 — Main-safe Core CI environment compatibility gate

**Type:** shared CI infrastructure / release unblocking; **status:** Draft review, not merged. **Owner:** integration steward X. **Target:** `main@e8f1c35a4a56adbde983dce38bba59a8c4eee105`.

## Reproducible regression (not a product-source regression)
On [AURA X-RAY #113](https://github.com/danielforface/aura-lang/pull/113), actual Windows x64 EXE workflow [#42414379432](https://github.com/danielforface/aura-lang/actions/runs/42414379432) succeeded, while same-head [Core CI #42414382781](https://github.com/danielforface/aura-lang/actions/runs/42414382781) failed in vendored Z3 `z3-sys 0.8.1` during C++ build. The bundled `z3/src/math/lp/column_info.h:50` refers to undefined `c.m_low_bound` and suggests `c.m_lower_bound` under the newer runner GCC. This is an external dependency source/compiler compatibility issue, **not** a proven compiler or product behavioral regression.

The failure was previously independently reproduced on [initial #111 Core #42398141091](https://github.com/danielforface/aura-lang/actions/runs/42398141091). The otherwise identical Core CI workflow on #111 uses `ubuntu-24.04` instead of floating `ubuntu-latest`, and passed [#42398637375](https://github.com/danielforface/aura-lang/actions/runs/42398637375) and [#42403542343](https://github.com/danielforface/aura-lang/actions/runs/42403542343).

## Bounded remedy
Change only Core workflow runner to `ubuntu-24.04`, preserving existing full workspace tests, real Z3 feature, compatibility tests, native dependencies, caching, permissions and timeouts. Add explicit fail-closed OS-version assertion plus logging of the exact C++/Clang/CMake toolchain: CI image drift becomes a detected failure with identifiable provenance rather than an opaque Z3 build error.

The workflow now uses a supported **OS family/version baseline**; this is *not* a true immutable image/toolchain or Cargo dependency pin and it does NOT fix vendored Z3 source or certify Ubuntu 26 / GCC 15. **Upstream remediation** should be a separate assessed dependency/lockfile migration with real verifier proof-quality tests, not deletion of Z3 or feature-gate bypass.

## Acceptance
1. Main+only-this-change actual Core PR CI `workspace` completes SUCCESS with `cargo test --workspace --locked --no-default-features`, `cargo check -p aura --locked --no-default-features --features z3`, compatibility suite and trusted core checks unchanged.
2. OS assertion prints `AURA_CORE_CI_OS`, `AURA_CORE_CI_CXX`, `AURA_CORE_CI_CLANG`, `AURA_CORE_CI_CMAKE` and rejects OS mismatch.
3. Peer reviewer checks exactly two integration-owned paths changed; no A/B/C product code, Cargo manifest/lockfile or root governance ledgers modified.
4. No auto-merge; owner decides adoption. Existing #113 only benefits once adopted or separately reviewer-approved integration branch includes it.
