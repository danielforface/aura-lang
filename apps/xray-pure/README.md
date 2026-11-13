# XR-09 — AURA X-RAY / Native Language-Owned Investigation UI

**Status:** experimental user-authored Aura application, not a replacement for
the already working [XR-08](https://github.com/danielforface/aura-lang/pull/113).
This is intentionally *not* full Windows process capture/JSONL parity.

## What is actually 100% Aura

All product-specific computation, sample evidence, policy thresholds,
three case decision cells, sample chart geometry, selected-case detail,
policy selection and callback business rules, analyst note state and
widget construction are written entirely in **`main.aura`**. The
independent `policy.aura` is an Aura-only mathematical oracle used in CI.

**No product-source C, Rust, JavaScript, Python, PowerShell or HTML.**
The standard Aura language runtime is nevertheless implemented in
Rust and provides operating-system window, graphics, mouse/keyboard,
and Z3 integration. That is the distinction between application
source purity and a language/runtime's implementation. Runtime+source
bundling is **not** ahead-of-time translation to a standalone
app-specific EXE. A user cannot reproduce XR-08's full capabilities
with this build.

## Source and provenance

Development is isolated in `product/xray-pure-aura-v1` / Draft #116,
with **no changes to A compiler, B Lumina implementation, C Quantum,
X integration, root Cargo, or shared governance**.

Base is the pinned, unmerged A+B `integration/x007-ab-full-workspace`.
The later [X009 qualification](https://github.com/danielforface/aura-lang/tree/integration/x009-ab-full-qualification)
is read-only reference, not silently merged. Only reviewed
producer-consumer interfaces may be consumed. The native UI source
declares eight `trusted extern cell` signatures for existing,
runtime-owned Lumina constructors; no FFI implementation is supplied
by the application. Core's auto-injected `sdk/std/lumina.aura` is
currently incompatible with the parser (X's D2-01 in Issue #109), so
the Aura UI's built-in constructor surface is used without importing
that stale shim. The application is not a fix for the SDK shared bug.

## Native UI in Aura

The Aura code implements a 29-cell decision and presentation model:
three switching policy buttons (Sensitive, Standard, Strict),
case selection (memory pulse, CPU streak and I/O transfer),
source-visible evidence summaries, a ten-sample timeline with
Aura-computed bar geometry, and a real Lumina `TextInput` for an
in-memory analyst note (no disk or telemetry write). User input is
handled by `on_change` and `ui.event_text()`; policy, selection
and text state are handled by `ui.get_text/set_text` in Aura callbacks.
The synthetic fixture returns 3/3/2 cases exactly; this is **not**
evidence of anomalies in any live system.

## Windows bundle

If and only if the [XR09 CI](https://github.com/danielforface/aura-lang/actions/workflows/xray-pure-aura-windows.yml)
has successfully built the exact commit, download the corresponding
`AURA-XR09-PureAura-Windows-x64-<SHA>` artifact. Keep the files
together in a directory on your Windows x64 machine.

In PowerShell:

    .\aura.exe check .\main.aura
    .\aura.exe check .\policy.aura
    .\aura.exe run .\policy.aura --mode avm
    .\aura.exe run .\main.aura --mode avm

The policy run has full Z3 enabled in the qualified build and should
print `XR09_PURE_AURA_POLICY_PASS sensitive=3 standard=3 strict=2`.
Do not set `AURA_AVM_NO_Z3` to claim certification. Lumina opens a
real native Raylib window with interactive widgets, without a
browser/server/HTML. The real Windows GUI itself must still be
accepted on your hardware; GitHub-hosted Windows runners lack a WGL
OpenGL display driver, and therefore cannot exercise the window.

## Qualification separation

* Windows: actual `aura.exe` build with Z3 and Lumina; strict
  `aura check` of product and policy; real Z3-gated policy execution;
  actual PE machine x64; SHA256 manifest; source-only artifact.
* Linux: Raylib/Lumina native UI through **real X11 software-rendered
  OpenGL** (`Xvfb` + Mesa llvmpipe), physical `xdotool` pointer
  events on policy buttons, frame-debug evidence and real screenshot.
  A Linux GUI pass does **not** certify Windows user32/driver behavior.

The runtime may have external system dependencies; a PE
header alone does not certify portable DLL completeness. No claim of
a cryptographically signed Windows release, production distribution,
formal correctness of the GUI, or independently compiled app-specific
PE is made.

## Feature gaps — not waived

The current app cannot read untrusted `capture/2` JSONL with bounded
strict validation, enumerate Win32 processes, reproduce real XR-08
capture, or drive a native GPU-less Windows session using only
qualified Aura source APIs. These are explicit producer API contracts
under [Issue #109](https://github.com/danielforface/aura-lang/issues/109)
(D2-01 SDK, D2-02 native file/IO and Win32 collector), not reasons
to insert hidden C glue into the product. **XR-08 remains the real
working capture tool.**

Branch and PR remain Draft, never auto-merge or force-rebase.
