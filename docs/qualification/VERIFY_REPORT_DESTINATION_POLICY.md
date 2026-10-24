# Verification report destination integrity — bounded Q01 policy

**Task:** AURA-Q01-002 / AURA-Q01-006; [Issue #20](https://github.com/danielforface/aura-lang/issues/20); [Draft PR #21](https://github.com/danielforface/aura-lang/pull/21), stacked/unmerged.
**Observed acceptance:** [Actions 40120564647](https://github.com/danielforface/aura-lang/actions/runs/40120564647), both real CLI no-Z3 and Z3 jobs GREEN on Ubuntu. This file does not alter the frozen master roadmap.

The CLI must **not leave a previous `ok:true` report** when starting a fresh `verify --report` invocation that fails before proof. It detects when report output is the Aura input itself (same path or canonical alias), rejects that target to preserve source, invalidates prior report output before processing, and emits truthful `status=invalid_input` machine-readable report on early file/manifest errors when output is writable. The error handler propagates report-write errors rather than silently swallowing them.

`--report` is an explicit output artifact, **not a trustworthy proof solely because a file exists**. Consumers must check command success, schema, status, and proof evidence, and must not reuse old files as current evidence.

## Evidence
- Test-first baseline [40119465608](https://github.com/danielforface/aura-lang/actions/runs/40119465608) **1/4 pass**; report integrity failed in 3 cases.
- First patch [40119755541](https://github.com/danielforface/aura-lang/actions/runs/40119755541) **3/4 pass**; missing input bypassed invalidation in manifest loading.
- Expanded regression [40120058985](https://github.com/danielforface/aura-lang/actions/runs/40120058985) **4/5 pass** both modes; same missing-file issue.
- Final [40120564647](https://github.com/danielforface/aura-lang/actions/runs/40120564647) **5/5 durability + 2/2 existing report tests** passed in **each** feature mode.

## Unresolved risks, not certified
This is *not* a crash-atomic, fsync-durable or transactionally committed report output. It does not defend all concurrent rename/symlink races or hardlink aliases to the same input inode, and was not tested on Windows. Multi-file/workspace reports can still be partially generated on failures; CI and downstream consumers must require an explicit complete-success state. Later Q01/Q04 safety tasks should use atomic replacement and source/inode-aware path checks with cross-platform tests. No stable release assurance is implied.
