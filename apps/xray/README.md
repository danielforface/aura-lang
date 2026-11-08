# AURA X-RAY XR-03 — multidimensional offline evidence workbench

**Version 0.6.0-xr06 · Windows x64 · unsigned experimental · local only**

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

## XR-04 — exact evidence and process topology (2026-11-07)

User hardware verified the previous product with 2,123 process records, 1,901 complete rate vectors, ten full samples over 10.015 seconds, and no reported collector errors. XR-02 Replay had 2,195 records, 1,966 vectors and 10.016 seconds. Independent Chromium tests of Live/Replay/Compare HTML found working interactions and no JS exceptions or external requests. These results do not demonstrate better overall performance between different workload snapshots.

**New source precision:** Windows collector records full 64-bit CPU ticks (cpu_total_100ns), cumulative read/write bytes and snapshots of parent_pid and thread_count. The old rounded CPU-ms and KiB columns remain for compatibility. Schema is still aura.xray.capture/2 with additive fields; the product version is 0.6.0-xr06. Parent PID is a **hint**, not an independently verified ancestral identity.

**Import/replay evidence:** In XR-04 captures the offline importer rechecks every complete process score against the original CPU/IO raw counters, exact monotonic inter-sample clock intervals, identity and working-set deltas. Old XR-02/XR-03 captures lack those counters and continue to open with an explicitly marked **legacy score-only validation**. This consistency check is NOT a signature and cannot prove that adversarially rewritten captures are authentic. Data loss, counter rollback and unsupported input are not quietly turned into zero.

**Workbench:** Adds a process-topology tab displaying parent PID hints, thread counts and same-time evidence, plus counts of source-rate vectors successfully validated from exact counters. Replay now preserves the baseline creation identity as recorded even when score is unavailable.

**Windows commands:**

    .\AuraXRay.exe --self-test
    .\AuraXRay.exe --replay capture-xr02.jsonl --report legacy-replay.html
    .\AuraXRay.exe --replay capture-xr03.jsonl --report legacy-xr03.html
    .\AuraXRay.exe --capture --seconds 10 --out xr04-actual.jsonl --report xr04-actual.html
    .\AuraXRay.exe --compare capture-xr03.jsonl xr04-actual.jsonl --report xr04-compared.html

Output paths must not exist; no overwrites. Names are redacted by default.

**Limits:** No ETW, kernel service, verified process ancestry, device-specific disk throughput, automatic remediation, root-cause proof, native Lumina shell/GUI or formal backend proof. The Windows boundary and data ingestion remain in C; nine domain functions are genuinely Aura-compiled.

## XR-05 — Aura Investigation Casebook (2026-11-07)

AURA X-RAY now extracts **auditable process episodes**, rather than only rendering per-sample scores. The new sixth report view opens by default and exposes each case with its complete same-process identity (PID + Windows creation FILETIME), start/end sample, measured peak, proof source and adjacent original observations.

New genuine **Aura-compiled** analytical cells in `src/analysis.aura` implement all five episode thresholds:
- `xray_burst_gate`: one sample working-set growth >=32 MiB;
- `xray_recovery_gate`: within the next three *consecutive comparable* observations, the measured working set contracts by at least 50% of the earlier burst. A detected retraction is evidence against *monotonic growth over that short window*; no diagnosis of leak absence;
- `xray_cpu_hot_gate`: >=700 millicores in one complete process observation (70% of one logical CPU);
- `xray_cpu_streak_gate`: at least three consecutive comparable CPU-hot samples;
- `xray_transfer_surge_gate`: read OR write >=16,384 KiB/s in a complete vector.

The native C episode engine `xr05_cases.c` does ONLY bounded order, identity matching, interval continuity, and evidence transport. It links actual generated Aura cells. It is *not* a pure-Aura compiler/runtime/Win32/HTML stack: capture and report shell remain native C and browser JS while Aura governs the analytical decisions. No false claim of automatic causation or malware detection.

**Field acceptance from user's private XR-04 capture**, 10 seconds / 2,153 process observations / 1,928 independently reconstructed complete vectors, zero raw-rate mismatches:
- PID 7140, sample 2: +91,932 KiB working set (score 182); at sample 3 the same identity has -106,348 KiB, observed contraction.
- PID 7140, samples 5–10: consecutive CPU occupancy 984–1,190 millicores, six observations.
- PID 13408, sample 7: write transfer 17,243 KiB/s.

These are *patterns of source measurements*, not memory leak, bottleneck, physical disk, verified ancestry or root-cause claims. Legacy XR-02/03 files still replay, transparently marked as score-only. XR-04 and XR-05 recordings use exact raw-counter verification. Casebook source records and case truncation remain visible; names stay redacted by default. The classifier and full native executable are exercised by `--self-test` and Windows CI including synthetic negative controls for near-threshold readings, gaps and PID reuse.

Windows product usage (all output paths must be new):

    .\AuraXRay.exe --self-test
    .\AuraXRay.exe --replay capture-xr04.jsonl --report xr05-casebook.html
    .\AuraXRay.exe --capture --seconds 10 --out capture-xr05.jsonl --report xr05-live.html
    .\AuraXRay.exe --compare capture-xr04.jsonl capture-xr05.jsonl --report xr05-compare.html

Open the HTML file and choose **06 / Aura Investigation Cases**. This is a compiled-Aura-native classification path with an offline report; no network, ETW, global installer, security agent or Lumina native GUI.


## XR-06 — Compiled Aura Temporal State Engine + JSON Casebook

The temporal investigation engine no longer uses only isolated Aura threshold calls.
Six additional **real Aura cells** in \`src/analysis.aura\` are compiled through the
actual repository C backend and linked into the Windows PE:

| Aura cell | Actual product responsibility |
|---|---|
| \`xray06_cpu_streak_next\` | consecutive hot-CPU recurrence; resets on missing/gapped observations |
| \`xray06_io_streak_next\` | process transfer activity recurrence; groups consecutive samples |
| \`xray06_memory_remaining\` | arithmetic state of outstanding half-burst memory retraction |
| \`xray06_memory_phase\` | transitions between pending, observed retraction, expired bounded window and interrupted evidence |
| \`xray06_signal_dimensions\` | measures co-occurring CPU, memory and I/O dimensions in the same process and sample |
| \`xray06_evidence_grade\` | graded source reliability; distinguish legacy score-only from exact counter rechecks |

The C adapter still safely supplies identity ordering (PID + exact creation
FILETIME), sample continuity, bounded storage and input preconditions. That
is a **native Aura business-logic engine**, not a claim that the OS adapter,
JSON parser or visual interface are fully implemented in Aura. All recurrence,
memory-state and confidence rules above are native compiled Aura calls. Native
negative/positive acceptance is wired into \`--self-test\` and Windows CI.

Four observable case types: memory pulse (with explicit retracted/pending/
window_expired/interrupted evidence phase), CPU streak, contiguous I/O transfer
episode, and multiple synchronous measured pressure dimensions. Cases are
heuristic **observational evidence**, not causality, memory-leak diagnosis,
security analysis or measured disk/device throughput. Missing counter data is
never assumed to be zero or to imply recovery.

### New headless interface

In addition to \`--replay\` and \`--compare\`, execute:

    .\AuraXRay.exe --explain capture-xr04.jsonl --out cases-xr06.json

The offline JSON casebook (\`aura.xray.casebook/1\`) contains the original
capture's exact build SHA, record counts, raw verification coverage, and
bounded cases with \`kind\`, \`pid\`, \`creation_filetime\`, sample interval,
peak sample/value, memory phase, independent counter fidelity and Aura evidence
grade. The export deliberately excludes process names (even if capture was
recorded with names), command lines and filesystem paths. It rejects
invalid/tampered captures, short capacities, existing output files and any
destination identical to the source; it never silently overwrites input.

A replay command produces an offline HTML view with the same model:

    .\AuraXRay.exe --replay capture-xr04.jsonl --report xr06-investigation.html

The JSON and HTML are **two representations of the same validated source and
compiled-Aura casebook**; neither performs new Win32 collection. An XR-04
capture remains replayable with source counter verification and old /2 legacy
sources remain marked as score-only; new live XR-06 captures also support the
exact-counter profile of capture/2. There is no network activity or installer.

Prior private Windows field recording acceptance: XR-05 replay produced three
casebook records with 1,928 source-exact vectors from 2,153 records.
An independent XR-06 model replay of those same observations preserved exactly
three matching cases (PID 7140 memory pulse S2→S3, CPU streak S5→S10,
PID 13408 transfer S7). This is a local model oracle, **not a claim that XR-06
itself already ran on the user's laptop**.

Limits: 30 samples per capture, bounded parser and caseboard, no ETW, no
kernel-level tracing, no root-cause claims, no unprompted uploads.
