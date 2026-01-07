use std::path::PathBuf;

use miette::{IntoDiagnostic, Result};

mod common;
use common::differential_util::{
    compile_and_run_c, compile_and_run_llvm, corpus_dir, ensure_required_tooling_available,
    failures_root, find_files, fixture_write_dir, run_avm, RunOutcome,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    Avm,
    C,
    Llvm,
}



fn outcomes_match(a: &RunOutcome, b: &RunOutcome) -> bool {
    a.ok == b.ok && a.stdout == b.stdout && a.stderr == b.stderr
}

fn minimize_lines<F>(src: &str, mut still_fails: F) -> String
where
    F: FnMut(&str) -> bool,
{
    let mut lines: Vec<String> = src.lines().map(|l| l.to_string()).collect();

    // Greedy line deletion until no single-line delete keeps the failure.
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

fn write_failure_artifacts(case_name: &str, src: &str, details: &str) -> Result<()> {
    let root = failures_root().join(case_name);
    std::fs::create_dir_all(&root).into_diagnostic()?;

    std::fs::write(root.join("minimized.aura"), src).into_diagnostic()?;
    std::fs::write(root.join("report.txt"), details).into_diagnostic()?;

    if let Some(dir) = fixture_write_dir() {
        std::fs::create_dir_all(&dir).into_diagnostic()?;
        std::fs::write(dir.join(format!("{case_name}.aura")), src).into_diagnostic()?;
        std::fs::write(dir.join(format!("{case_name}.txt")), details).into_diagnostic()?;
    }

    Ok(())
}

#[test]
fn differential_corpus_matches_across_backends() -> Result<()> {
    ensure_required_tooling_available()?;

    let files = find_files(&corpus_dir())?;
    if files.is_empty() {
        return Err(miette::miette!("no corpus files found"));
    }

    for path in files {
        let case_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("case")
            .to_string();
        let src = std::fs::read_to_string(&path).into_diagnostic()?;

        let avm = run_avm(&src)?;

        let out_dir = PathBuf::from("build").join("differential").join(&case_name);

        let c = compile_and_run_c(&case_name, &src, &out_dir)?;
        let llvm = compile_and_run_llvm(&case_name, &src, &out_dir)?;

        let mut failures: Vec<(Backend, RunOutcome)> = Vec::new();
        if let Some(c_out) = c {
            if !outcomes_match(&avm, &c_out) {
                failures.push((Backend::C, c_out));
            }
        }
        if let Some(l_out) = llvm {
            if !outcomes_match(&avm, &l_out) {
                failures.push((Backend::Llvm, l_out));
            }
        }

        if failures.is_empty() {
            continue;
        }

        let mut details = String::new();
        details.push_str(&format!("case: {}\nfile: {}\n\n", case_name, path.display()));
        details.push_str("reference backend: AVM\n\n");
        details.push_str("=== AVM ===\n");
        details.push_str(&format!("ok: {}\nstdout:\n{}\nstderr:\n{}\n\n", avm.ok, avm.stdout, avm.stderr));

        for (b, out) in &failures {
            details.push_str(&format!("=== {:?} ===\n", b));
            details.push_str(&format!("ok: {}\nstdout:\n{}\nstderr:\n{}\n\n", out.ok, out.stdout, out.stderr));
        }

        // Attempt minimization (line-based) to get a small repro.
        let minimized = minimize_lines(&src, |candidate| {
            // Only try to preserve “any mismatch”, using the same availability.
            let avm2 = run_avm(candidate).ok();
            let avm2 = avm2.unwrap_or(RunOutcome {
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

        write_failure_artifacts(&case_name, &minimized, &details)?;

        return Err(miette::miette!(
            "differential mismatch in '{case_name}'. Saved repro to .aura/differential_failures/{case_name}/"
        ));
    }

    Ok(())
}
