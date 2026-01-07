use std::path::PathBuf;

use miette::{IntoDiagnostic, Result};

mod common;

use common::differential_util::{compile_and_run_c, compile_and_run_llvm, find_c_compiler};

#[test]
fn translation_validation_oracle_matches_c_for_const_println() -> Result<()> {
    if find_c_compiler().is_none() {
        eprintln!("skipping translation validation: no C compiler in PATH");
        return Ok(());
    }

    let src = r#"
import aura::io

cell main() ->:
    io::println("hello")
"#;

    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(".aura")
        .join("translation_validation")
        .join("const_println");

    // IR oracle
    let src_aug = aura_sdk::augment_source_with_default_std(src).into_diagnostic()?;
    let program = aura_parse::parse_source(&src_aug)?;

    let mut checker = aura_core::Checker::new();
    checker.set_defer_range_proofs(true);
    if let Err(e) = checker.check_program(&program) {
        return Err(miette::miette!("sema failed: {e:?}"));
    }

    let module_ir = aura_core::lower_program(&program).map_err(|e| miette::miette!("lowering failed: {e:?}"))?;
    let oracle = aura_ir::run_oracle_entry(
        &module_ir,
        "main",
        &[],
        aura_ir::OracleConfig::default(),
    )
    .into_diagnostic()?;

    // Backend execution
    let c = compile_and_run_c("translation_validation_const_println", src, &out_dir)?
        .expect("C compiler present");

    assert!(oracle.ok, "oracle execution should succeed: {oracle:?}");
    assert!(c.ok, "C execution should succeed: {c:?}");
    assert_eq!(oracle.stdout, c.stdout, "stdout mismatch between oracle and C backend");

    if let Some(llvm) = compile_and_run_llvm("translation_validation_const_println", src, &out_dir)? {
        assert!(llvm.ok, "LLVM execution should succeed: {llvm:?}");
        assert_eq!(oracle.stdout, llvm.stdout, "stdout mismatch between oracle and LLVM backend");
    }

    Ok(())
}
