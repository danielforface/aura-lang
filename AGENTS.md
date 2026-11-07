# AGENTS.md — Aura Lang contributor and AI-agent entrypoint
**2026-11-06 — Mandatory per-phase navigation and shared-interface contract**

Do NOT infer project truth from this file's branch alone. The latest git PR/Issue/CI may be on different branches; read the live record before acting.

## On every development phase and every "ממשיכים"
1. READ [Phase Lifecycle Contract](docs/qualification/PHASE_LIFECYCLE_V1.md) **from Stage 0 through Stage 4** and [Cross-Agent Protocol](docs/qualification/CROSS_AGENT_COORDINATION_V1.md). Re-read at entry, before any shared write/push, and at handoff.
2. READ [Production-First Delivery Policy](docs/qualification/DELIVERY_OPERATING_POLICY_V1.md), [Operating Protocol](docs/qualification/OPERATING_PROTOCOL.md) and your lane order: [A Core](docs/qualification/agents/AGENT_A_CORE_ORDER.md), [B Lumina](docs/qualification/agents/AGENT_B_LUMINA_ORDER.md) or [C Quantum](docs/qualification/agents/AGENT_C_QUANTUM_ORDER.md).
3. Check live HEAD/base/merge-base/diff/CI; current PR, Issue claims, [#59 coordination](https://github.com/danielforface/aura-lang/issues/59), [#109 integration debt](https://github.com/danielforface/aura-lang/issues/109); do not silently assume independent chats share file edits.
4. Classify overlaps D0–D5 (files, API/ABI/IR/schema, shared history, CI/docs). One writer per path and one approved contract owner. If common interface changes, obtain producer-consumer acknowledgment, write downstream contract tests, and keep only the dependent portion blocked.
5. Deliver real compiler/runtime/simulator functionality or actual bug closure. Evidence is accurate but not the product. Run buildable focused tests before normal CI; no intentionally missing-function remote RED; don't create nested micro-Draft PRs.
6. A PR mergeable to its own *base* is not necessarily safe to integrate to main, especially if it contains unmerged ancestors. No autonomous merges, forced rebases or other-agent path edits.
7. End every phase with exact HEAD/base, BEFORE→AFTER, tests, conflict/contract status, CI and next safe action. Follow [universal continuation prompt](docs/qualification/CONTINUE_AGENT_PROMPT.md) when switching chats.

## Boundaries
- **A:** Core/AST/Parser/IR/C/native/verifier; Draft #57, Issue #51. Shared API changes require consumer review.
- **B:** Lumina UI/IBus, Draft #106; Linux native TextInput Autofocus already implemented in unmerged branch; do not restart obsolete diagnostic research.
- **C:** standalone Aura Quantum simulator, Draft #108; bounded dynamic simulator and offline JSON CLI already implemented on its branch. Next work must depend on live code and Q28 contract.
- **Integration steward:** root Cargo/lockfile, central qualification/evidence files, global roadmap/release policy and cross-lane workflow harmonization. The steward role is not magically assumed by any of A/B/C.

Older AGENT guidance in other branches may be stale. If the current branch lacks these docs, READ their current source from [Draft PR #95](https://github.com/danielforface/aura-lang/pull/95). Adoption by an agent requires it actually reads/uses these instructions. No change to frozen Roadmap V1/V2/V3 acceptance tasks or scientific honesty.


## Agent X — continuous, supportive architecture leader (2026-11-07)

The maintainer has appointed a dedicated **fourth active agent X** to lead integrations and guide A/B/C at every user-triggered pulse. X is accountable for architecture, dependency DAG, shared contract quality, evidence-based review, downstream integration readiness and cross-agent decisions. X must **support GREEN agents without needless redirection** and correct only evidence-backed material deviations, after considering justified technical reasons. An agent's differing implementation approach alone is NOT misconduct.

**Read** [X's standing charter](docs/qualification/agents/AGENT_X_INTEGRATION_DIRECTOR.md), [first/recurring X prompt](docs/qualification/AGENT_X_CONTINUE_PROMPT.md) and [Issue #110](https://github.com/danielforface/aura-lang/issues/110). X must still follow [Phase Lifecycle](docs/qualification/PHASE_LIFECYCLE_V1.md), [Cross-Agent D0–D5](docs/qualification/CROSS_AGENT_COORDINATION_V1.md) and [Production-First](docs/qualification/DELIVERY_OPERATING_POLICY_V1.md). X is a coordinator, not a code-path owner of A/B/C, and has no autonomous merge/release authority. A/B/C continue independently under their lane orders; cross-agent decisions appear in GitHub rather than depending on chat synchronization.
