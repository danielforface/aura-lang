//! EV-200 test-first: source-legal exported Aura Cells must not collide
//! with identifiers declared by C runtime headers included by the backend.
//! Linked C is the ABI truth, not syntactically valid-looking emitted text.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{optimize_module, run_oracle_entry, validate_module, OracleConfig, OracleValue};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

const SOURCE: &str = r#"
cell getchar(n: u32) ->:
    yield n
cell from_aura_getchar(n: u32) ->:
    yield getchar(n)
"#;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("aura-ev200-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

#[test]
fn source_user_cell_getchar_does_not_conflict_with_stdio_export_and_preserves_calls() {
    let ast = aura_parse::parse_source(SOURCE).expect("source-real getchar Cell parses");
    Checker::new().check_program(&ast).expect("source-real getchar Cell checks");
    let initial = lower_program(&ast).expect("checked source lowers");
    validate_module(&initial).expect("source Cell SSA validates");
    let mut optimized = initial.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized source Cell SSA validates");
    for (version, ir) in [("original", &initial), ("optimized", &optimized)] {
        for name in ["getchar", "from_aura_getchar"] {
            let got = run_oracle_entry(ir, name, &[OracleValue::U32(37)],
                OracleConfig::default()).expect("source Cell IR Oracle executes");
            assert!(got.ok, "{version}/{name}: {got:?}");
            assert_eq!(got.return_value, Some(OracleValue::U32(37)),
                "{version}/{name}: wrong source semantics");
        }
        let c = emit_module(ir, None).expect("source-valid Cells produce C");
        // Existing, C-safe public Cell names are stable at C link boundary.
        assert!(c.module_c.contains("from_aura_getchar("));
        let scratch = Scratch::new();
        fs::write(scratch.0.join("aura_runtime.h"), &c.runtime_h).unwrap();
        fs::write(scratch.0.join("module.c"), &c.module_c).unwrap();
        // External translation unit keeps including stdio and the runtime.
        // The libc getchar() declaration must NOT be replaced or overloaded.
        fs::write(scratch.0.join("driver.c"), concat!(
            "#include <stdint.h>\n#include <stdio.h>\n",
            "uint32_t from_aura_getchar(uint32_t);\n",
            "int main(void){",
            "return from_aura_getchar(37u)==37u?0:1;",
            "}\n"
        )).unwrap();
        for opt in ["-O0", "-O2"] {
            let binary = scratch.0.join(format!("stdio_{version}_{}", &opt[1..]));
            let built = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
                .arg(&scratch.0)
                .arg(scratch.0.join("module.c")).arg(scratch.0.join("driver.c"))
                .arg("-o").arg(&binary).output().expect("Clang on Native CI");
            assert!(built.status.success(),
                "{version}/{opt}: Clang C header/ABI collision: {}\n{}",
                String::from_utf8_lossy(&built.stderr), c.module_c);
            let ran = Command::new(&binary).output().expect("execute independently linked C");
            assert!(ran.status.success(), "{version}/{opt}: incorrect linked call: {}",
                String::from_utf8_lossy(&ran.stderr));
        }
    }
}
