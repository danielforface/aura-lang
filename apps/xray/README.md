# AURA X-RAY XR-02 — native Windows telemetry and offline evidence explorer

**0.2.0-xr02 · Windows x64 · experimental · local only**

AURA X-RAY is a real Windows executable. Windows Toolhelp32, PSAPI and GetProcessTimes are accessed through a narrow read-only C adapter; the rate calculations and activity scoring are genuinely compiled from Aura Lang through the repository's C backend. Process-history state and file/browser serialization are written in portable C. This is not Lumina, not a kernel driver, not a causal proof or a formally proven compiler.

## A major functional leap from XR-01

- Tracks process history using PID **and process creation time**, avoiding PID-reuse confusion.
- Computes CPU millicores (1000 = one full logical CPU), read/write KiB/s from cumulative byte deltas, and signed working-set delta in KiB. Rates and scoring call Aura-compiled functions; integer math is deliberately bounded and truncated.
- Explicitly withholds temporal results when observation is only a first-sample baseline, identity is missing, process was replaced, a counter resets, sample timing is invalid or bounded arithmetic is exceeded. Null and zero have different meanings.
- Aura-based activity index scores only complete, identity-validated observations. Formula: floor(CPU_milli/100) + floor(read_KiB_s/2048) + floor(write_KiB_s/2048) + 2*min(16384, floor(positive working-set growth KiB/1024)). **This is a descriptive heuristic, not anomaly proof or causal attribution.**
- Generates a self-contained **offline HTML field report** with observed process measurements, timeline chart, interactive filtering, ranked processes and evidence-quality ledger. No scripts or style assets are downloaded, no telemetry leaves the machine.
- Includes deterministic test vectors injecting counter reset, PID reuse and missing identity in addition to real Windows test captures.

## Run on Windows (PowerShell)

Unzip the tested GitHub Actions release artifact, open PowerShell in the extracted directory and run:

    .\AuraXRay.exe --version
    .\AuraXRay.exe --self-test
    .\AuraXRay.exe --capture --seconds 10 --out capture-xr02.jsonl --report xray-report.html

Then double-click xray-report.html in a browser. No network, web server, drivers, elevation or installer are required.

Only if you explicitly want process executable names to appear in **both** local outputs:

    .\AuraXRay.exe --capture --seconds 10 --out capture-with-names.jsonl --report with-names.html --include-names

Capture and HTML output paths must not already exist. The tool refuses overwrites, even of an older capture. Use a fresh output name for every run.

**Privacy:** Even with names redacted, PID, timestamp and resource observations can be sensitive. Keep the JSONL and report local; never post unsanitized traces unless necessary and deliberately authorized. No cloud sync, network calls or interventions are implemented.

## Data semantics / capability boundaries

- Schema is **aura.xray.capture/2**. This is a deliberate version bump; XR-01 v1 capture files are not silently rewritten or treated as v2. Formats coexist.
- Capture records: capture_start, sample_start, process, sample_end, capture_end; errors remain explicit.
- New raw fields: cumulative_io_write_kib, cpu_total_ms. New calculated fields: interval_ms, cpu_millicores, io_read_kib_s, io_write_kib_s, memory_delta_kib, activity_score and delta_quality.
- No temporal rate is inferred from the first observation. A process is comparable only when PID **and** Windows process creation time match in adjacent samples.
- CPU millicores measure process CPU time relative to **one** logical CPU, not percent of all processors (multi-threaded processes can exceed 1000).
- I/O is OS process transferred bytes, not necessarily physical disk throughput; memory is working set, not private committed memory or total physical RAM.
- Missing identity/counter/permissions is not silently zero. A negative memory delta is valid; an I/O counter rollback is NOT a negative rate.
- Bounded history supports up to 4096 process records per sample, HTML ranking up to 128 candidates. Exceeding limits is recorded as a capacity miss rather than allowing unbounded growth.
- A disappearance from process enumeration is NOT necessarily a confirmed process exit. The report does not claim it is.
- Sampling uses GetTickCount64 monotonic scheduling with at least the requested N-second window; process rates use timestamps of successive sample starts.
- Activity score does not diagnose malware, a leak, bottleneck, misuse or any other root cause. It only ranks measured activity under its published formula.

## Build, testing and handoff

The dedicated product workflow compiles Aura compiler from an exact SHA, compiles analysis.aura to a real generated C module, verifies six ABI symbols, links Aura-generated C, stdlib, Win32 adapter and XR-02 process engine into a real Windows x64 PE. It runs 18 Aura semantic vectors and a deterministic temporal fault-injection oracle, then collects at least three live Windows snapshots, validates JSONL against independent arithmetic, checks HTML offline/report functionality and redaction, rejects invalid arguments/overwriting, hashes the PE and uploads EXE+README+SHA manifest+anonymous CI report only if all gates pass.

Windows 2022 CI can verify Windows behavior on the runner. The user still runs the EXE manually on their machine. It is experimental and unsigned; Windows may display security warnings.

**Not included:** native Lumina GUI, system-wide ETW tracing, eBPF, network telemetry, automatic repairs, cross-session import or compare, trustworthy root-cause proof or quantum simulation. These require separate work and validation.
