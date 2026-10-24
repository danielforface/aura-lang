# Q01-007: Typed origin of verification trust inputs

**Tracking:** [Issue #26](https://github.com/danielforface/aura-lang/issues/26) / stacked Draft PR (see live GitHub). **State:** baseline no-Z3 real CLI RED (4/4 as expected), typed AST source patch under new CI. Not yet accepted. Master roadmap immutable.

`aura.trusted-core.v2` currently conflates `requires` (caller preconditions) with `assume` (explicit, trusted premises), and `trusted.unsafe_blocks` can miss unsafe blocks inside a cell because traversal omits CellDef. These are provenance defects, not necessarily direct proof unsoundness; however they make proof results hard to audit.

New real CLI negative fixtures must check `trusted.explicit_assumes`, `trusted.preconditions` and nested `trusted.unsafe_blocks`, with source offsets and no claim of `proved` without actual solver evidence. Both Z3 and no-Z3 states must preserve provenance when a parsed AST exists. The legacy `verify.assumptions` array must remain stable or schema versioning must be explicit if contracts change. Disambiguating trust origins is **not** proof that an assumption itself is true; a full proof-dependency DAG or inconsistent-assumption check remains separate work.

CI must show baseline RED due to missing trust fields/recursion, then a scoped GREEN after a source fix. Maintain append-only evidence and cross-chat handoff on each commit, keep all PRs Draft/unmerged, never auto-merge or edit frozen MASTER_ROADMAP_V1.

## Baseline and implementation status (2026-10-24)

[Baseline 40132655326](https://github.com/danielforface/aura-lang/actions/runs/40132655326) no-Z3 0/4 PASS, all failures demonstrate missing distinct trust sources or unsafe-in-cell recursion. The first code patch adds typed `AssumptionSourceKind` and `AssumptionSource{kind,span,cell}`, with corresponding arrays on `TrustedSurface`, and descends into cells, macros, render and nested control flow. The legacy `verify.assumptions` array remains backward compatible. `docs/trusted-core.md` updated from obsolete v1 description. **New code CI pending.** Next repair the legacy audit script and v1 baseline without allowing a no-Z3 report to be mistaken for proof. No automatic merge.
