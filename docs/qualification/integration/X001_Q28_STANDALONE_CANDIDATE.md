# X-001 — Q28 standalone candidate extracted onto clean main
Date: 2026-11-07. **STATUS: experimental integration candidate only. NOT MERGED, NOT QUALIFIED, NOT RELEASED.**

## Provenance and scope
- Clean target parent: `main@e8f1c35a4a56adbde983dce38bba59a8c4eee105`.
- Exact source: Agent C Draft #108 `082954880b07abfd5c3d2f7b0e5c924c14cdcab7` (C-009).
- Reused **unchanged Git tree** for `aura-quantum-sim/**`; reused **unchanged blob** for `.github/workflows/quantum-reference-sim.yml`; reused unchanged agent-C notes tree `docs/quantum/agent-c/**`. Source ownership remains Agent C.
- New integration-steward-owned `.github/workflows/x001-q28-landing.yml` tests the candidate at the **actual main-parent tree**. This note is an integration artifact, not a new Q28 implementation.
- One additional X/steward change to legacy `.github/workflows/ci.yml`: pin the Core workspace runner from floating `ubuntu-latest` (resolved to Ubuntu 26.04/GCC 15.2 in candidate PR) to previously qualified `ubuntu-24.04`. The new CI run on the earlier candidate SHA failed in **vendored upstream z3-sys 0.8.1 C++** with `column_info.h:50` referring to nonexistent `m_low_bound`; C source files are unchanged. Historical Core CI run #32820641326 SUCCESS on main Ubuntu 24.04. This narrows the tested CI environment; it does **not** claim Ubuntu 26.04/GCC15 support and cannot be treated as permanent upstream dependency remediation.
- **Deliberately not imported:** Core / compiler source, Lumina, PR #95 governance/roadmap and central evidence ledgers, root Cargo manifest or lockfiles, old stacked #9–#95 history. The single legacy Core CI runner pin above is an explicit **integration-owned workflow adjustment**, not a Core-source edit.
- Do not mistake C's #108 27 own commits / 39 paths against its parent for the 260 commits / 170 paths that `main...#108` includes. This candidate is an isolated content-preserving extraction, not the C PR's history.

## Boundaries and acceptance
1. Both exact candidate CI jobs must pass: all-targets Rust + strict Clippy, actual pure/noisy/stabilizer JSON callers and pinned external Qiskit parity.
2. Human review must separately confirm C-owned file-tree equality with #108, scientific trust limits, no intended dependency on still-unmerged V3 and safe main-surface integration.
3. Q28 full science / Q23 proof / QIR language integration / actual QPU execution / hardware credentials and multi-platform releases are NOT established.
4. No auto-merge; maintainer explicitly authorizes any landing only after integration/reviewer ACK.
5. If shared governance requires adopted V3 policy first, mark BLOCKED_ON_CONTRACT; do not bypass controls by describing a draft as a shipped feature.

Authoritative coordination: Issue #110 (X), Issue #109 (integration), #59 (ownership); source PR #108.

## X-002 — externally executable consumer acceptance (same frozen C-009)
- Source package SHA stays pinned to C-009; Agent C acknowledged identical source trees in [Issue #110](https://github.com/danielforface/aura-lang/issues/110#issuecomment-6711318452). Later C-010 symbolic source is deliberately excluded.
- **No edits** to C-owned Rust, agent-C workflow or note trees. The integration steward owns new `tools/integration/q28_cli_contract.py` and added `X002` job step in the integration-only workflow.
- Black-box **compiled CLI** consumer acceptance covers all three distinct JSON v1 profiles, exact seed replay across independent processes, per-shot Bell Z correlation in pure/stabilizer engines, native error code/stderr/empty-stdout, bounded input and schema rejection, a nonphysical noise probability, non-Clifford T rejection and analytical noisy density `[[0.68,0.20],[0.20,0.32]]`, trace 1 and purity 0.6448.
- These tests are integration guards, **not** an Aura compiler integration, scientific formal proof, cross-platform release test or owner approval to merge. The new exact-head candidate CI must complete before this gate is called GREEN.
