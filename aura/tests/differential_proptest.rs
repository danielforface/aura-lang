use std::path::PathBuf;

use miette::Result;
use proptest::{
    prelude::{any, prop},
    test_runner::{Config, TestCaseError, TestRunner},
};

mod common;
use common::differential_util::{
    compile_and_run_c, compile_and_run_llvm, ensure_required_tooling_available, failures_root,
    run_avm, RunOutcome,
};

fn outcomes_match(a: &RunOutcome, b: &RunOutcome) -> bool {
    a.ok == b.ok && a.stdout == b.stdout && a.stderr == b.stderr
}

fn minimize_lines<F>(src: &str, mut still_fails: F) -> String
where
    F: FnMut(&str) -> bool,
{
    let mut lines: Vec<String> = src.lines().map(|l| l.to_string()).collect();

    'outer: loop {
        for i in 0..lines.len() {
            let mut candidate = lines.clone();
            candidate.remove(i);
            let cand_src = candidate.join("\n");
            if cand_src.trim().is_empty() {
                continue;
            }
            if still_fails(&cand_src) {
                lines = candidate;
                continue 'outer;
            }
        }
        break;
    }

    lines.join("\n") + "\n"
}

fn program_from_ops(ops: &[u32], seed: u32) -> String {
    // Keep programs tiny + deterministic.
    // Canonical syntax: colon + indentation.
    let a = ops.get(0).copied().unwrap_or(0) % 100;
    let b = ops.get(1).copied().unwrap_or(1) % 100 + 1; // avoid div-by-zero
    let c = ops.get(2).copied().unwrap_or(2) % 100;

    // Provide a few arithmetic shapes to shake out codegen differences.
    // NOTE: we intentionally keep types u32 to avoid signed corner-cases.
    format!(
        "import aura::io\n\ncell main() ->:\n    val a: u32 = {}\n    val b: u32 = {}\n    val c: u32 = {}\n    val x: u32 = (a + b) * 2\n    val y: u32 = x / b\n    val z: u32 = y + c\n    io::println(\"seed = {}\")\n    io::println(\"a={{a}} b={{b}} c={{c}} x={{x}} y={{y}} z={{z}}\")\n",
        a, b, c, seed
    )
}

#[test]
fn differential_random_programs_match_across_backends() -> Result<()> {
    ensure_required_tooling_available()?;

    let mut runner = TestRunner::new(Config {
        cases: 64,
        ..Config::default()
    });

    let strat = (
        any::<u32>(),
        prop::collection::vec(any::<u32>(), 0..8),
    );

    runner
        .run(&strat, |(seed, ops)| {
            let src = program_from_ops(&ops, seed);

            let avm = run_avm(&src).map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
            let out_dir = PathBuf::from("build")
                .join("differential")
                .join("proptest")
                .join(format!("{seed}"));

            let c = compile_and_run_c("proptest", &src, &out_dir)
                .map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
            let llvm = compile_and_run_llvm("proptest", &src, &out_dir)
                .map_err(|e| TestCaseError::fail(format!("{e:?}")))?;

            let mut mismatch = false;
            if let Some(c_out) = &c {
                mismatch |= !outcomes_match(&avm, c_out);
            }
            if let Some(l_out) = &llvm {
                mismatch |= !outcomes_match(&avm, l_out);
            }

            if !mismatch {
                return Ok(());
            }

            let case_name = format!("proptest_{seed}");
            let minimized = minimize_lines(&src, |candidate| {
                let avm2 = run_avm(candidate).ok().unwrap_or(RunOutcome {
                    ok: false,
                    stdout: String::new(),
                    stderr: "avm failed".to_string(),
                });

                let out_dir = PathBuf::from("build").join("differential").join("_min");
                let c2 = compile_and_run_c("_min", candidate, &out_dir).ok().flatten();
                let l2 = compile_and_run_llvm("_min", candidate, &out_dir).ok().flatten();

                if let Some(c2) = c2 {
                    if !outcomes_match(&avm2, &c2) {
                        return true;
                    }
                }
                if let Some(l2) = l2 {
                    if !outcomes_match(&avm2, &l2) {
                        return true;
                    }
                }
                false
            });

            let root = failures_root().join(&case_name);
            std::fs::create_dir_all(&root)
                .map_err(|e| TestCaseError::fail(format!("{e}")))?;
            std::fs::write(root.join("minimized.aura"), minimized)
                .map_err(|e| TestCaseError::fail(format!("{e}")))?;
            std::fs::write(root.join("original.aura"), &src)
                .map_err(|e| TestCaseError::fail(format!("{e}")))?;

            let mut report = String::new();
            report.push_str(&format!("case: {case_name}\nreference: AVM\n\n"));
            report.push_str(&format!(
                "=== AVM ===\nok: {}\nstdout:\n{}\nstderr:\n{}\n\n",
                avm.ok, avm.stdout, avm.stderr
            ));
            if let Some(c_out) = c {
                report.push_str(&format!(
                    "=== C ===\nok: {}\nstdout:\n{}\nstderr:\n{}\n\n",
                    c_out.ok, c_out.stdout, c_out.stderr
                ));
            }
            if let Some(l_out) = llvm {
                report.push_str(&format!(
                    "=== LLVM ===\nok: {}\nstdout:\n{}\nstderr:\n{}\n\n",
                    l_out.ok, l_out.stdout, l_out.stderr
                ));
            }
            std::fs::write(root.join("report.txt"), report)
                .map_err(|e| TestCaseError::fail(format!("{e}")))?;

            Err(TestCaseError::fail(format!(
                "differential mismatch in {case_name}; saved repro under .aura/differential_failures/{case_name}/"
            )))
        })
        .map_err(|e| miette::miette!("proptest differential failed: {e}"))?;

    Ok(())
}

// Ensure the test file compiles even without proptest failures.
#[test]
fn differential_proptest_smoke() -> Result<()> {
    ensure_required_tooling_available()?;
    Ok(())
}
