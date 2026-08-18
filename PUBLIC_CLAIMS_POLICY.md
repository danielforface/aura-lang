# Aura Public Claims Policy

Aura is a proof-driven programming language. Its public documentation should apply the same discipline to claims about Aura itself.

## Evidence classes

### CODE-BACKED

The implementation exists on the audited repository tree.

Example:

> Aura has a Z3-backed verification path behind a feature gate.

### REFERENCE-BACKED

The behavior is described in the current compact language/protocol reference and is expected to match the implementation.

Example:

> `u32[lo..hi]` range refinements are part of the current reference.

### WORKFLOW-BACKED

A CI/release workflow is defined, but a public pass claim requires a specific run/artifact.

Example:

> The repository defines an Android sample APK workflow.

This is different from:

> The latest Android workflow passed.

The second statement needs a run link/artifact.

### HISTORICAL-REPORT

A milestone/completion report records previous project state or intended completion.

Historical reports are useful evidence but do not override current code when they disagree.

### TARGET

A desired SLO, roadmap outcome, compatibility goal, or design objective.

Example:

> `<200ms P95` verification latency is a project target.

Do not convert a target into a measured result by dropping the word “target.”

### EXPERIMENTAL

Code or a vertical slice exists, but its stability/compatibility contract is not yet frozen.

Example:

> The LLVM IR backend is implemented and evolving.

## Required wording distinctions

### “Proof-driven” vs “formally verified compiler”

Approved:

> Aura is a proof-driven language with Z3-backed program verification.

Not established:

> Aura is a formally verified compiler.

### “Memory-safety work” vs universal guarantee

Approved:

> Aura implements ownership/move, capability, region and race-analysis infrastructure, with a current MVP safety model documented in the SDK reference.

Not established:

> Aura guarantees memory safety for all programs and all execution paths.

### Performance

Approved without benchmark artifact:

> Aura targets sub-200ms P95 verification for interactive workflows.

Approved with benchmark artifact:

> On `<commit/workload/environment>`, Aura measured `<result>`.

Not acceptable without artifact:

> Aura proves code in under 200ms.

### Android

Approved:

> Aura includes Android SDK/NDK tooling, runtime cross-compilation paths and a sample APK workflow.

Too broad today:

> Aura has full production Android support.

### Hybrid execution

Approved:

> Aura exposes AVM, LLVM and hybrid mode selection.

Required caveat:

> automatic AVM-to-LLVM promotion is not yet implemented in the current hybrid run path.

Not acceptable:

> Aura has a production JIT.

### Differential testing

A percentage such as “100% agreement” must be computed from a workflow that fails on disagreement and must link to a specific run.

Never hardcode a success percentage into a generated report.

## Version language

Until the project cuts a single repository-wide release contract:

Use:

> Aura 2026 Edition — active, pre-stable language platform.

Do not use component `1.0.0` versions as evidence that the entire language is stable v1.0.

## Documentation precedence

When content conflicts:

1. implementation,
2. current compact reference/protocol docs,
3. measured workflow/artifact,
4. focused current design doc,
5. historical completion/status report,
6. roadmap/target.

## README rule

The README should be the most accurate document in the repository, not the most optimistic one.

A strong technical project gains credibility by saying “experimental,” “target,” or “not yet implemented” precisely where appropriate.
