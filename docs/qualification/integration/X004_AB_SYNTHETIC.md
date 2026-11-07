# X-004 — Synthetic combined A Core and B Lumina tree; no merge

**Type:** cross-lane D4/D1 compatibility experiment, not a main-ready feature PR or release. Created from **Agent A [#57](https://github.com/danielforface/aura-lang/pull/57) exact source HEAD `00f20f34ad59dd8e9d9d96dead5dedbd3da4d57f`** with A's complete upstream ancestral history intact. B source snapshot is from **Agent B [#106](https://github.com/danielforface/aura-lang/pull/106) exact HEAD `5f2e8160be881411f9d3ee85a827ffd16aebb8ea`**.

## Non-destructive, content-preserving integration operation
- Replace just `aura-plugin-lumina/**` with the **exact unchanged B Git subtree** `496a72f9af614ce0b580a4235c473cb2d86c9c33` (full plugin, not manually selected pieces; no edit to either agent source branch).
- Reconcile root `Cargo.lock` by reusing B's **exact blob** `bf778c997ed575e323cde39863ae8c4d3b5c5b30` (integration-owned shared root). A and B main Cargo.toml are byte-identical; A's lockfile packages are a subset of B by name/version. The only changed existing lockfile package block is `aura-plugin-lumina` with its new fontdue, rustybuzz and Unicode dependencies; 13 additive transitives. The `--locked` metadata and actual build must independently confirm this hypothesis.
- Add only a scoped X-owned CI workflow and this note; no other A/B/C source, central qualification ledger or frozen roadmap edits.

## Test acceptance
- Ubuntu 24.04 pinned to avoid the existing unrelated `z3-sys` GCC/Ubuntu26 mismatch: full `cargo test --workspace --locked --no-default-features`, real Aura Z3 build and current A Record native C test.
- Real B native raylib/IBus unit library tests plus **genuine X11/XTest mouse and tab/keyboard forms E2E** on the *A Core* source tree.
- Job success is **only combined A+B staging compatibility for these exact snapshots**. It does not prove complete Lumina/IBus, scientific Q28 or full release safety. Cross-lane D2 signed numeric return/mutable SSA contracts remain unapproved; no semantic API changes are made here.
- **Before merging anything:** independent reviewer must explicitly select kept A/B ancestor dependencies, resolve central PROGRESS/HANDOFF/EVIDENCE serially, run complete affected CI matrix on the proposed target and obtain maintainer approval. Both A/B remain Draft and continue independently.

## Controlled failure disposition
A failing synthetic integration test is not automatically a production defect of either agent. Reproduce failure, pin exact log, assign owner of contract or lockfile, and leave both productive owned branches untouched until the cause and minimal fix are agreed.

## X-004 integration harness correction — no B source regression proven

The first synthetic job [#42417302205](https://github.com/danielforface/aura-lang/actions/runs/42417302205) incorrectly enabled `raylib,lumina-diagnostics` for the **full library test suite**. It invoked the specifically isolated `ibus_native::tests::p37_real_post_r_ibus_ack_fence_keeps_editsession_and_undo_authoritative`, which explicitly requires `LUMINA_IBUS_ENGINE=hangul`, `LUMINA_IBUS_P37_RUST_ACK_FENCE=1` and its real Hangul helper. The X harness had not set up that isolated diagnostic context. Log observed **164 passing standard units, one `NotPresent` diagnostic fixture failure**. B's production-facing CI runs broad units using `raylib --lib` and the P37 diagnostic separately under authenticated real Hangul; this synthetic workflow must preserve the same test topology.

Changed only the **X-owned integration CI invocation** to `--features raylib --lib` for broad units; real X11 mouse/forms/Tab tests still run with `raylib,lumina-diagnostics` as their dedicated test binaries. No A/B production change, source tree or Cargo.lock modification. The initial synthetic test cannot count as a cross-lane product bug; only the new exact-head CI establishes compatibility.
