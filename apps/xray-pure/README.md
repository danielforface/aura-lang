# AURA X-RAY / Pure Aura Language-Native Proof

**Scope: deliberate experimental language-purity track, NOT XR-08 replacement.**

## Source-only application

`main.aura` is the entire user-authored application. All business math,
source sample lookup, policy decisions, temporal recurrence, interactive
case selection, visual chart sizes, widget hierarchy, on-click callbacks,
and UI state are written **solely in Aura Lang**. There is no product C,
C++, Rust, HTML/JS/TS, Python, shell launch code, or hand-coded GDI
renderer. Existing **Aura AVM + Lumina** are the language runtime
(principally Rust and its underlying graphics library), as with any
native language's standard runtime. 'Pure Aura source' means application
source, not absence of operating-system/runtime implementation.

The project consumes existing `integration/x007-ab-full-workspace`
snapshot `a27361e2d30548c3b2268bd35e1be96a8f88e73c` as a pinned
**unmerged dependency**; the Aura AVM's user-defined cell calling
contract was not present on the older product branch. Lumina authored
by Agent B stays in the producer's files; this consumer branch changes
no shared path.

### Run Windows bundle

The CI artifact includes the real `aura.exe` with Lumina plus the
Aura-only `main.aura`. Both files must stay together. In PowerShell:

    $env:AURA_AVM_NO_Z3 = '1'
    .\aura.exe run .\main.aura --mode avm

This runs the actual Aura program as a script inside the existing
Windows Aura runtime; **it is not an independently compiled, single
process-specific EXE**. Setting `AURA_AVM_NO_Z3=1` explicitly skips
formal verification and MUST NOT be presented as a proof result.
UI is produced through the existing Lumina runtime, not HTML or the
XR-08 GDI compositor. If CI validation succeeds, the exact SHA is
listed in its manifest.

### What the current prototype does

Synthetic, non-private evidence exhibits a memory pulse with observed
retraction (samples S2-S3), six-sample CPU streak (S5-S10), and single
I/O spike (S7). Three Aura-defined policies yield 3/3/2 independently;
all threshold formulas, recurrence, visual scores, policy selection and
case details execute in user-authored cells. Buttons navigate policy
and evidence, chart heights are computed in the same Aura source.

**It does NOT yet ingest arbitrary capture/2 JSONL, sample Win32
processes, replay all 2,153 real user samples, or preserve XR-08's full
feature matrix.** The AVM has `io.read_text` but no qualified
source-level text iteration/slicing/JSON parser plus bounded,
versioned process-observation collection. These are genuine language
runtime boundaries, not reasons to silently call C adapter functions.
To maintain existing XR-08 functionality use PR #113 / stable XR-08
executable until a reviewed producer/consumer API is available.

### Language contracts still necessary

1. General-purpose, bounded, deterministic UTF-8/JSONL tokenization
and source collection APIs callable from Aura cells with strict error
semantics, no special-case X-RAY builtins.
2. Stable owned or region-snapshot observation collection with full
u64 creation FILETIME and signed i64 deltas; Agent X Issue #109
explicitly blocks unapproved nominal Record/Slice Win64 FFI.
3. Native Windows Aura process sampler with explicit read-only
capabilities and fail-closed missing/denied/partial observation status.
4. Reviewed Windows Lumina runtime build/test and packaging of app
source into a native distributable without non-Aura business logic.
Until producer ownership and tests satisfy these, do not claim full
feature parity, single-EXE Aura compilation or release readiness.

All changes scoped to `apps/xray-pure/**` and its own
`.github/workflows/xray-pure-aura-windows.yml`. No merges, no copying
A/B implementation, no modifications to root workspace or governance.
