# AURA X-RAY XR-01.1 — Windows x64 process observation

**Status:** experimental native product slice; unmerged. This is a genuine Windows command-line program using a business-analysis module authored in **Aura Lang**, compiled to C by the repository's Aura compiler and linked with a minimal Win32 read-only adapter.

## What it actually does

- Enumerates live Windows processes with Toolhelp32 snapshots.
- Reads working-set memory, cumulative read I/O and process creation time when access permits. Uses limited query rights only; does not request PROCESS_VM_READ.
- Calls compiled Aura functions to normalize KiB to MiB and calculate 256-MiB memory and 64-MiB cumulative-I/O buckets.
- Writes local JSONL capture with UTC/monotonic timestamps, actual elapsed duration, quality statuses, explicit permission errors and a build SHA. Sample summary distinguishes all process-open failures from actual access-denied failures.
- Runs without admin, without network access, without installing drivers or services. Process names are **excluded by default**.
- **Not included yet:** CPU-utilization rates, time-travel replay, cross-service causality, eBPF/ETW, GUI, remote collection, intervention, quantum tools or universal program proof.

## User commands (PowerShell)

Unzip the GitHub Actions artifact and open PowerShell in the extracted directory:

    .\AuraXRay.exe --version
    .\AuraXRay.exe --help
    .\AuraXRay.exe --self-test
    .\AuraXRay.exe --capture --seconds 10 --out xray-my-pc.jsonl

Optionally allow process executable names in a sensitive local capture:

    .\AuraXRay.exe --capture --seconds 10 --out xray-with-names.jsonl --include-names

**Data privacy:** Captured process metadata may be sensitive even without process names. All files stay on your own computer; do not upload unsanitized traces. A failed or incomplete capture is labeled as such. Output paths must not already exist (no accidental overwrite).

**Unsigned build:** early experimental CI binaries are unsigned and Windows may warn before execution. Examine the CI provenance / SHA256, verify the source and scan the file before execution. Windows 10/11 x64 is the initial target; actual Windows 10 coverage still requires separate user testing.

## Data contract

- Capture schema: aura.xray.capture/1
- Record types: capture_start, sample_start, process, sample_end, capture_end, error
- Process identity: PID plus process_create_filetime when available. PID alone can be reused.
- Time: UTC millisecond time and monotonic milliseconds for each sample; deadline-based one-second sampling. The capture_end record includes measured elapsed milliseconds and UTC time, spanning at least the requested N seconds.
- Access limitations: unreadable fields encoded as JSON null, quality=partial and optional win32_error; **null never means zero**.
- The I/O counters are cumulative and should **not** be interpreted as a per-second I/O rate.
- Memory is current working-set size, not total committed private bytes or total system RAM.
- Threshold buckets are simple pure Aura algorithms, not learned anomaly detection.

## CI delivery contract

Workflow: .github/workflows/xray-windows.yml, on push to product/xray-windows-v0.

The workflow builds the Aura compiler from the exact source revision, compiles apps/xray/src/analysis.aura to generated C, links Win32 + stdlib + Aura output into dist/AuraXRay.exe, runs Aura semantic self-test and **real process snapshot** on windows-2022, verifies PE x64, records SHA256 and uploads a release artifact.

**Release hardening XR-01.1:** CI also checks rejection of invalid durations and forbids overwriting an existing capture; Rust dependencies are cached even if a downstream runtime check fails.\n\n**Do not treat code or a green PR check as a delivered EXE** without verified artifact upload. A successful artifact includes AuraXRay.exe, BUILD-MANIFEST.json and README.txt. The developer must report exact SHA, CI run ID, artifact file and limitations with every submitted build.

## Development ownership

App source + blueprint: apps/xray/. Isolated CI: .github/workflows/xray-windows.yml. Neither compiler Core, Lumina, Quantum nor central qualification files are modified. Compiler pinned to the current product branch base; new Aura features are adopted only after reviewed integration, conformance and explicit release selection.

See [BLUEPRINT.md](BLUEPRINT.md).
