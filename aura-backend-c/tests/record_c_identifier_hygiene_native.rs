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

#[test]
fn checked_aura_c_reserved_function_names_are_mapped_and_compile_as_real_c() {
    // Probe the parser and Checker rather than presuming all C keywords
    // are legal Aura Cell names. Every accepted name must emit working C.
    let candidates = [
        "signed", "unsigned", "long", "short", "char", "const",
        "volatile", "restrict", "inline", "static", "typedef",
        "struct", "union", "enum", "sizeof", "switch", "printf", "aura_main",
    ];
    let mut qualified = Vec::new();
    for source_name in candidates {
        let source = format!("cell {source_name}(n: u32) ->:\n    yield n\n");
        let Ok(ast) = aura_parse::parse_source(&source) else { continue; };
        if Checker::new().check_program(&ast).is_err() { continue; }
        let original = lower_program(&ast).expect("checked export source lowers");
        validate_module(&original).expect("real source validates");
        let mut optimized = original.clone();
        optimize_module(&mut optimized);
        validate_module(&optimized).expect("optimized source validates");
        let expected_symbol = format!("aura_cell_{}", source_name.as_bytes().iter()
            .map(|b| format!("{b:02x}")).collect::<String>());
        for (version, ir) in [("original", &original), ("optimized", &optimized)] {
            let c = emit_module(ir, None).expect("accepted Aura C-keyword Cell emits");
            assert!(c.module_c.contains(&format!("{expected_symbol}(")),
                "{source_name}/{version}: safe remapping not present: {}", c.module_c);
            let scratch = Scratch::new();
            fs::write(scratch.0.join("aura_runtime.h"), c.runtime_h).unwrap();
            fs::write(scratch.0.join("module.c"), &c.module_c).unwrap();
            let driver = format!(
                "#include <stdint.h>\nuint32_t {expected_symbol}(uint32_t);\nint main(void){{return {expected_symbol}(19u)==19u?0:1;}}\n"
            );
            fs::write(scratch.0.join("driver.c"), driver).unwrap();
            for opt in ["-O0", "-O2"] {
                let exe = scratch.0.join(format!("keyword_{version}_{}", &opt[1..]));
                let built = Command::new("clang")
                    .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
                    .arg(&scratch.0)
                    .arg(scratch.0.join("module.c")).arg(scratch.0.join("driver.c"))
                    .arg("-o").arg(&exe).output().expect("clang on Native CI");
                assert!(built.status.success(),
                    "{source_name}/{version}/{opt}: {}\n{}",
                    String::from_utf8_lossy(&built.stderr), c.module_c);
                assert!(Command::new(exe).status().unwrap().success(),
                    "{source_name}/{version}/{opt}: execution mismatch");
            }
        }
        qualified.push(source_name);
    }
    assert!(qualified.contains(&"signed") && qualified.contains(&"unsigned"),
        "known valid-Aura keyword exports must be tested: {qualified:?}");
}

#[test]
fn public_v_number_cell_symbols_must_not_be_shadowed_by_backend_ssa_temporaries() {
    // Ordinary valid exported names are externally visible C ABI and cannot
    // be silently remapped. User Cell Calls must be correct even if their
    // names resemble generated SSA temporaries.
    let source = r#"
cell v0(n: u32) ->:
    yield n
cell v1(n: u32) ->:
    yield n
cell v2(n: u32) ->:
    yield n
cell v3(n: u32) ->:
    yield n
cell v4(n: u32) ->:
    yield n
cell v5(n: u32) ->:
    yield n
cell v6(n: u32) ->:
    yield n
cell chain_v(n: u32) ->:
    val mut value: u32 = n
    value = v0(value)
    value = v1(value)
    value = v2(value)
    value = v3(value)
    value = v4(value)
    value = v5(value)
    value = v6(value)
    yield value
"#;
    let ast = aura_parse::parse_source(source).expect("source user vN Cells parse");
    Checker::new().check_program(&ast).expect("source vN Calls check");
    let original = lower_program(&ast).expect("source vN Calls lower");
    validate_module(&original).expect("source vN IR validates");
    let mut optimized = original.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized vN IR validates");
    for (version, ir) in [("original", &original), ("optimized", &optimized)] {
        let got = run_oracle_entry(ir, "chain_v", &[OracleValue::U32(119)],
            OracleConfig::default()).expect("source chain IR Oracle runs");
        assert!(got.ok);
        assert_eq!(got.return_value, Some(OracleValue::U32(119)));
        let c = emit_module(ir, None).expect("source vN Cells emit");
        for i in 0..=6 {
            assert!(c.module_c.contains(&format!("uint32_t v{i}(")),
                "existing C ABI for safe user Cell v{i} must stay intact");
        }
        let scratch = Scratch::new();
        fs::write(scratch.0.join("aura_runtime.h"), c.runtime_h).unwrap();
        fs::write(scratch.0.join("module.c"), &c.module_c).unwrap();
        fs::write(scratch.0.join("driver.c"), concat!(
            "#include <stdint.h>\n",
            "uint32_t chain_v(uint32_t);\n",
            "int main(void){return chain_v(119u)==119u?0:1;}\n"
        )).unwrap();
        for opt in ["-O0", "-O2"] {
            let exe = scratch.0.join(format!("vn_{version}_{}", &opt[1..]));
            let built = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
                .arg(&scratch.0)
                .arg(scratch.0.join("module.c")).arg(scratch.0.join("driver.c"))
                .arg("-o").arg(&exe).output().expect("clang exists on CI");
            assert!(built.status.success(), "{version}/{opt}: {}\n{}",
                String::from_utf8_lossy(&built.stderr), c.module_c);
            assert!(Command::new(exe).status().unwrap().success(),
                "{version}/{opt}: native vN chain corrupted");
        }
    }
}
