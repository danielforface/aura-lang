# Q01/Q04 — AST statement to verifier boundary audit (Issue #37)

**Classification:** source-code inspection on parent Draft PR #36 commit `148a8b90a88c3a07bee284647899940d5f90447a`. This matrix is NOT executable proof, native equivalence, or public-language certification. All 24 variants are directly enumerated from `aura-ast/src/lib.rs::Stmt`. Cross-check `aura-core/src/sema.rs::Checker::{check_program,check_block}`, `aura-verify/src/verify.rs::{visit_top_stmt,check_stmt}`, `aura-core/src/lower.rs::Lowerer::lower_stmt_in_place`. Scope below uses top = file-level; nested = cell/body.

| Variant | Top Z3 disposition | Nested semantic disposition | Current nested Z3 / source-level audit | Native lowering observation |
|---|---|---|---|---|
| Import | inert | handles import | inert | nested import is not generally lowered (distinct Q03) |
| MacroDef | inert (pre-expansion policy) | rejects unexpanded macro | **wildcard Ok**; must make explicit | compile macros handled as expansion; nested lower currently permits inert |
| TypeAlias | inert | resolves alias | inert | nested type aliases not generally lowered |
| TraitDef | inert declaration | rejects | **wildcard Ok**; explicit rejection needed | unsupported nested lowering |
| RecordDef | inert declaration | rejects | **wildcard Ok**; explicit rejection needed | unsupported nested lowering |
| EnumDef | inert declaration | rejects | **wildcard Ok**; explicit rejection needed | unsupported nested lowering |
| StrandDef | unsupported (global state) | checks binding | models integer/bool and range subset | lowers strand |
| CellDef | symbolic proof | rejects nested cell | **wildcard Ok**; explicit rejection needed | nested lower rejects |
| ExternCell | inert declaration/trust separately tracked | rejects nested extern | inert; trust source audit separate | extern signature top only |
| UnsafeBlock | unsupported | checks nested body | explicit typed **Unsupported** (Issue #35) | unsafe lowering separate |
| Layout | recursively verifies body | checks nested body | recursively checks block | lowers layout |
| Render | recursively verifies body | checks nested body | recursively checks block | lowers render |
| Prop | unsupported | **accepts and infers expression** | **wildcard Ok** — runtime expression lowered, verifier discards; test-first candidate | **lowers expression for execution** |
| Assign | unsupported (global state) | checks assignment | evaluates/binds integer/bool | lowers assignment |
| If | checks condition & branches | checks if | checks both symbolic branches | lowers conditional |
| Match | unsupported | checks match | explicit typed **Unsupported** (Issue #31) | lowers match |
| While | unsupported | checks while | condition/invariant checks | lowers loop |
| Requires | unsupported at file scope | checks bool | trusted precondition | **native lower no-op**, separate native contract gate |
| Ensures | unsupported at file scope | checks bool | proves postcondition | **native lower no-op**, separate native contract gate |
| Assert | proves assertion | checks bool | proves assertion | **native lower no-op**, separate native contract gate |
| Assume | unsupported at file scope | checks bool | trusted assumption source | **native lower no-op**, separate native contract gate |
| MacroCall | unsupported at file scope | rejects unexpanded macro | **wildcard Ok**; explicit rejection needed | nested lower currently no-op, covered by separate macro-expansion boundary |
| FlowBlock | recursively verifies body | rejects nested flow | recursively checks block | top flow lowering |
| ExprStmt | unsupported at file scope | infers expression | evaluates expression | lowers expression |

### Acceptance discipline

- `Prop` is a **real source-accepted expression statement**: semantic checker calls `infer_expr`; Native lowerer calls `lower_expr`, yet Z3 `check_stmt` silently `Ok(())`. No observed CLI false proof at matrix creation; new Z3 test-only regression must demonstrate the verifier's real behavior, and unsupported behavior is an interim fail-closed contract, not implementation of property semantics.
- Macro/trait/record/enum/cell statements are rejected by nested semantic checking or expanded elsewhere. Explicit verifier arms must not invent executions; classification as `Unsupported` is a conservative defense for direct AST verifier API use.
- `Stmt::Import`, `TypeAlias`, `ExternCell` remain treated as non-executable declarations **only in this scoped verifier**, subject to Q04 trust/dependency qualification.
- Replace nested wildcard with exhaustive enumerated match and inspect the compiler's non-exhaustive errors for future `Stmt` additions. Add source-valid CLI Z3/no-Z3 tests and preserve old match/unsafe/top-level controls. Record RED→GREEN links and compile/LSP gate results.
- A separate Q03/Q04 review must still establish Native compiler-lowering and cache equivalence (especially ignored contracts), not infer it from an exhaustive verifier match.
