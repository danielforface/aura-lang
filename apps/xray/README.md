# AURA X-RAY XR-03 — multidimensional offline evidence workbench

**Version 0.3.0-xr03 · Windows x64 · unsigned experimental · local only**

XR-03 evolves the Windows process analyzer into an **importable, comparable, self-contained investigation system**. It is a real Windows EXE built with the repository's Aura C backend. Nine application-domain functions (normalization, rate calculations, activity scoring and CPU/IO/memory impact) are authored in Aura Lang and generated directly from apps/xray/src/analysis.aura. Win32 API collection, 64-bit identity checks and bounded storage are handled in C, not falsely described as fully Aura. The OS adapter does not modify process state.

## Commands — Windows PowerShell

Unzip the Windows CI artifact into a new folder. The binary inside is AuraXRay.exe:

    .\AuraXRay.exe --version
    .\AuraXRay.exe --self-test

**Live capture with offline investigation:**

    .\AuraXRay.exe --capture --seconds 10 --out capture-03.jsonl --report investigate-03.html

**Reopen and investigate a saved XR-02 schema /2 capture without recollecting:**

    .\AuraXRay.exe --replay capture-xr02.jsonl --report replay-xr02.html

**Compare two locally captured sessions — aggregate evidence only:**

    .\AuraXRay.exe --compare older-capture.jsonl newer-capture.jsonl --report compare-03.html

Open the generated HTML in a browser with double-click or Start-Process. Reports contain no external assets, network calls or telemetry upload. Process names remain redacted unless the user explicitly supplies --include-names. Even without names, PID/timestamps/resource data is sensitive and should remain local.

## Multidimensional workbench

- Time axis: switch between CPU millicores, read and write KiB/s, positive working-set growth and the peak heuristic activity score. Click on a sample to filter the event table. Compare two captures on the same chart without pretending they share process identity.
- Event evidence: every row is ONE process at ONE time. CPU, I/O, memory changes, score and quality are drawn from that same sample, not different peak times as in XR-02.
- Identity-aware process matrix: score by PID+creation time and sample. Missing observations are distinct from observed zero.
- Focused evidence: click an event to inspect its actual measured components and its same-identity history, with Aura-computed CPU, IO and memory contributions.
- Quality and comparison: explicit counters for missing identities, resets, PID replacements, capacity misses and aggregate differences across independent sessions. These are observation tools, not causal proofs.

## Replay/compare input contract and safety

Accepts only complete JSONL schema aura.xray.capture/2. Strict local parser limits file to 80 MB, a line to 16,384 bytes, 30 samples, 4,096 process records per sample, and 16,000 displayed events. It parses and validates all input records even when the UI display limit is reached. Unsupported, incomplete, tampered or malformed captures are rejected, with no report left behind. No silent data repair or null-to-zero substitution.

Checks include required start/end, monotonic sample sequence, per-sample and total counts, elapsed duration, evidence-quality preconditions and recomputation of every scored vector using THREE newly compiled Aura impact cells. An invalid score results in a fail-closed error. User-controlled names are treated as data and escaped in JSON/DOM, with a Content Security Policy blocking network access. Output paths must not already exist. Comparison does not assert equivalence of workloads, hosts or identical PID identities across independent captures.

## Published scoring equation (heuristic ONLY)

    activity_score = floor(cpu_millicores / 100)
                   + floor(read_KiB_s / 2048)
                   + floor(write_KiB_s / 2048)
                   + 2 * min(16384, floor(positive_working_set_growth_KiB / 1024))

Each term comes from the Aura product logic, not an undeclared model. High score may indicate a short-lived allocation; it cannot diagnose a memory leak, malware or causal root cause. Millicores reference ONE fully utilized logical CPU at 1000. Process I/O transferred bytes can include nondisk activity. Working sets can share physical memory pages.

## CI delivery and security

The dedicated Windows CI recompiles the Aura compiler from exact source SHA, compiles nine Aura cells into real C functions and fails if any ABI symbol is missing, links a Windows x64 executable with XR-02 temporal engine and XR-03 explorer, validates 24 Aura semantic vectors, process identity and counter-reset cases, and performs actual live Windows capture. It replays the newly generated JSONL, compares independently named baseline/candidate copies and rejects both score tampering and invalid JSON. It checks no-overwrite rules, local HTML functionality/privacy, PE machine architecture, SHA-256 and build manifest before uploading the executable.

Until exact SHA CI and artifact upload pass, no XR-03 executable is qualified. External user-machine acceptance is a separate step.

**Not included:** native Lumina GUI, background service, ETW/eBPF, root-cause proofs, auto-remediation, formal backend proof or cross-host workload normalization. This is an interactive offline evidence workbench generated locally by an Aura-powered Windows executable.

Product ownership is limited to apps/xray/ and its scoped Windows workflow; other agents' Core, Lumina and Quantum production paths remain untouched. Maintainer tracking: Draft PR #113 and Issue #112.
