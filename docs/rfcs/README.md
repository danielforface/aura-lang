# RFCs

This folder contains RFCs (Request For Comments) for changes that affect language semantics, verifier behavior, stability contracts, or major tooling UX.

## When to write an RFC
Write an RFC when a change:
- Alters language syntax/semantics or type system rules
- Changes verifier meaning, trusted boundaries, or proof UX contracts
- Changes compatibility guarantees or feature gating
- Introduces a new strategic tool (debugger/profiler/package workflow)

Small refactors and bug fixes do not need an RFC.

## Process
1. Copy the template: `0000-template.md` → `NNNN-short-title.md`
2. Open a PR with the RFC.
3. Iterate based on review.
4. Once accepted, mark the RFC as **Accepted** and merge.

## Status
Use one of: Draft, Proposed, Accepted, Rejected, Implemented.
