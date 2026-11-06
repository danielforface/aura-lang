//! EV-199 test-first: Aura-valid C keywords in formal parameter names.
//! Preserve source semantics, internal Calls, branch Phi and the existing
//! externally callable C ABI for ordinary user Cell names.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{optimize_module, run_oracle_entry, validate_module, ModuleIR, OracleConfig, OracleValue};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

const SOURCE: &str = r#"
cell keyword_echo(signed: u32, unsigned: u32) ->:
    yield signed
cell keyword_forward(unsigned: u32, signed: u32) ->:
    yield keyword_echo(signed, unsigned)
cell branch_keywords(signed: u32, unsigned: u32, flag: bool) ->:
    val mut chosen: u32 = signed
    if flag:
        chosen = keyword_forward(unsigned, signed)
    else:
        chosen = keyword_echo(unsigned, signed)
    yield chosen
"#;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("aura-ev199-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

fn qualified_source() -> ModuleIR {
    let ast = aura_parse::parse_source(SOURCE).expect("valid Aura keyword-named params parse");
    Checker::new().check_program(&ast).expect("valid Aura keyword-named params check");
    let ir = lower_program(&ast).expect("checked source lowers");
    validate_module(&ir).expect("source SSA validates");
    ir
}

#[test]
fn aura_c_keyword_params_retain_semantics_through_calls_phi_and_native_clang() {
    let initial = qualified_source();
    let mut optimized = initial.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized source SSA validates");
    let cases = [
        ("keyword_echo", vec![OracleValue::U32(41), OracleValue::U32(73)], 41),
        ("keyword_forward", vec![OracleValue::U32(73), OracleValue::U32(41)], 41),
        ("branch_keywords", vec![OracleValue::U32(41), OracleValue::U32(73), OracleValue::Bool(true)], 41),
        ("branch_keywords", vec![OracleValue::U32(41), OracleValue::U32(73), OracleValue::Bool(false)], 73),
    ];
    for (version, ir) in [("original", &initial), ("optimized", &optimized)] {
        for (cell, args, expected) in &cases {
            let result = run_oracle_entry(ir, cell, args, OracleConfig::default())
                .expect("IR Oracle runs actual source Cell");
            assert!(result.ok, "{version}/{cell}: {result:?}");
            assert_eq!(result.return_value, Some(OracleValue::U32(*expected)),
                "{version}/{cell}: semantic preservation");
        }
        let c = emit_module(ir, None).expect("valid Aura emits C");
        // Names of ordinary exported Cells must remain link-compatible.
        assert!(c.module_c.contains("keyword_echo("));
        assert!(c.module_c.contains("keyword_forward("));
        assert!(c.module_c.contains("branch_keywords("));
        let scratch = Scratch::new();
        fs::write(scratch.0.join("aura_runtime.h"), c.runtime_h).unwrap();
        fs::write(scratch.0.join("module.c"), &c.module_c).unwrap();
        fs::write(scratch.0.join("driver.c"), concat!(
            "#include <stdint.h>\n#include <stdbool.h>\n",
            "uint32_t keyword_echo(uint32_t,uint32_t);\n",
            "uint32_t keyword_forward(uint32_t,uint32_t);\n",
            "uint32_t branch_keywords(uint32_t,uint32_t,bool);\n",
            "int main(void){return (",
            "keyword_echo(41u,73u)==41u",
            " && keyword_forward(73u,41u)==41u",
            " && branch_keywords(41u,73u,true)==41u",
            " && branch_keywords(41u,73u,false)==73u",
            ")?0:1;}\n"
        )).unwrap();
        for opt in ["-O0", "-O2"] {
            let executable = scratch.0.join(format!("ev199_{version}_{}", &opt[1..]));
            let built = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
                .arg(&scratch.0)
                .arg(scratch.0.join("module.c")).arg(scratch.0.join("driver.c"))
                .arg("-o").arg(&executable).output().expect("Clang exists on CI");
            assert!(built.status.success(),
                "{version}/{opt}: Clang rejected generated C: {}\n{}",
                String::from_utf8_lossy(&built.stderr), c.module_c);
            let ran = Command::new(&executable).output().expect("execute Native C driver");
            assert!(ran.status.success(), "{version}/{opt}: native output mismatch: {}",
                String::from_utf8_lossy(&ran.stderr));
        }
    }
}
