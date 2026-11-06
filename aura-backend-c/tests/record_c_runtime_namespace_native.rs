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


#[test]
fn checked_stdio_stdlib_and_thread_header_names_are_remapped_and_linked() {
    // Some C names are function declarations; others are preprocessor macros
    // or typedefs. Only count genuine Aura-valid spellings as backend tests.
    let candidates = [
        "getchar", "putchar", "getc", "stdin", "stdout", "stderr",
        "FILE", "fpos_t", "getenv", "qsort", "strtoul",
        "thrd_create", "INT32_MAX",
    ];
    let mut accepted = Vec::new();
    for name in candidates {
        let source = format!("cell {name}(n: u32) ->:\n    yield n\n");
        let Ok(ast) = aura_parse::parse_source(&source) else { continue; };
        if Checker::new().check_program(&ast).is_err() { continue; }
        let original = lower_program(&ast).expect("checked source lowers");
        validate_module(&original).expect("source SSA validates");
        let mut optimized = original.clone();
        optimize_module(&mut optimized);
        validate_module(&optimized).expect("optimized source SSA validates");
        let encoded = format!("aura_cell_{}", name.as_bytes().iter()
            .map(|b| format!("{b:02x}")).collect::<String>());
        for (version, ir) in [("original", &original), ("optimized", &optimized)] {
            let got = run_oracle_entry(ir, name, &[OracleValue::U32(37)],
                OracleConfig::default()).expect("checked source IR Oracle runs");
            assert!(got.ok, "{name}/{version}: {got:?}");
            assert_eq!(got.return_value, Some(OracleValue::U32(37)));
            let c = emit_module(ir, None).expect("valid source emits C");
            assert!(c.module_c.contains(&format!("uint32_t {encoded}(")),
                "{name}/{version}: reserved C symbol not remapped: {}", c.module_c);
            let scratch = Scratch::new();
            fs::write(scratch.0.join("aura_runtime.h"), c.runtime_h).unwrap();
            fs::write(scratch.0.join("module.c"), c.module_c).unwrap();
            fs::write(scratch.0.join("driver.c"), format!(
                "#include <stdint.h>\n#include <stdio.h>\n#include <stdlib.h>\nuint32_t {encoded}(uint32_t);\nint main(void){{return {encoded}(37u)==37u?0:1;}}\n"
            )).unwrap();
            for opt in ["-O0", "-O2"] {
                let exe = scratch.0.join(format!("headers_{version}_{}", &opt[1..]));
                let build = Command::new("clang")
                    .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
                    .arg(&scratch.0)
                    .arg(scratch.0.join("module.c")).arg(scratch.0.join("driver.c"))
                    .arg("-o").arg(&exe).output().expect("clang in Native CI");
                assert!(build.status.success(), "{name}/{version}/{opt}: {}",
                    String::from_utf8_lossy(&build.stderr));
                assert!(Command::new(exe).status().unwrap().success(),
                    "{name}/{version}/{opt}: wrong independent C ABI result");
            }
        }
        accepted.push(name);
    }
    for required in ["getchar", "putchar", "getenv"] {
        assert!(accepted.contains(&required),
            "expected an actual parsed/checked Aura library-name probe for {required}: {accepted:?}");
    }
}

#[test]
fn generated_fallback_namespace_must_not_steal_a_safe_public_cell_symbol() {
    // The hex-encoded fallback name for "getchar" is a legitimate Aura
    // identifier itself. It must remain ABI-stable when both coexist.
    let source = r#"
cell aura_cell_67657463686172(n: u32) ->:
    yield n
cell getchar(n: u32) ->:
    yield n
cell choose_runtime(n: u32) ->:
    yield getchar(aura_cell_67657463686172(n))
"#;
    let ast = aura_parse::parse_source(source).expect("source collision probe parses");
    Checker::new().check_program(&ast).expect("source collision probe checks");
    let initial = lower_program(&ast).expect("qualified source lowers");
    validate_module(&initial).expect("source SSA validates");
    let mut optimized = initial.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized SSA validates");
    for (version, ir) in [("original", &initial), ("optimized", &optimized)] {
        let got = run_oracle_entry(ir, "choose_runtime", &[OracleValue::U32(55)],
            OracleConfig::default()).expect("source IR Oracle runs");
        assert!(got.ok);
        assert_eq!(got.return_value, Some(OracleValue::U32(55)));
        let c = emit_module(ir, None).expect("mapped ABI emits C");
        assert!(c.module_c.contains("uint32_t aura_cell_67657463686172("),
            "keep legacy external name of safe user Cell");
        assert!(!c.module_c.contains("uint32_t getchar("),
            "C runtime owns the original getchar identifier");
        let scratch = Scratch::new();
        fs::write(scratch.0.join("aura_runtime.h"), &c.runtime_h).unwrap();
        fs::write(scratch.0.join("module.c"), &c.module_c).unwrap();
        fs::write(scratch.0.join("driver.c"), concat!(
            "#include <stdint.h>\n#include <stdio.h>\n",
            "uint32_t aura_cell_67657463686172(uint32_t);\n",
            "uint32_t choose_runtime(uint32_t);\n",
            "int main(void){return (",
            "aura_cell_67657463686172(55u)==55u",
            " && choose_runtime(55u)==55u",
            ")?0:1;}\n"
        )).unwrap();
        for opt in ["-O0", "-O2"] {
            let exe = scratch.0.join(format!("collision_{version}_{}", &opt[1..]));
            let build = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
                .arg(&scratch.0)
                .arg(scratch.0.join("module.c")).arg(scratch.0.join("driver.c"))
                .arg("-o").arg(&exe).output().expect("clang in Native CI");
            assert!(build.status.success(), "{version}/{opt}: {}\n{}",
                String::from_utf8_lossy(&build.stderr), c.module_c);
            assert!(Command::new(exe).status().unwrap().success(),
                "{version}/{opt}: generated fallback captured public symbol");
        }
    }
}

#[test]
fn source_formals_named_stdio_macros_do_not_get_preprocessor_substituted() {
    let source = r#"
cell read_stream_params(stdout: u32, stderr: u32, stdin: u32) ->:
    yield stdout
"#;
    let ast = aura_parse::parse_source(source).expect("C macro names as Aura params parse");
    Checker::new().check_program(&ast).expect("valid C macro-name Aura params check");
    let initial = lower_program(&ast).expect("source lowers");
    validate_module(&initial).expect("source SSA validates");
    let mut optimized = initial.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized SSA validates");
    for (version, ir) in [("original", &initial), ("optimized", &optimized)] {
        let got = run_oracle_entry(ir, "read_stream_params",
            &[OracleValue::U32(41), OracleValue::U32(11), OracleValue::U32(7)],
            OracleConfig::default()).expect("source IR Oracle executes");
        assert!(got.ok);
        assert_eq!(got.return_value, Some(OracleValue::U32(41)));
        let c = emit_module(ir, None).expect("C macro-name formal mappings emit");
        assert!(c.module_c.contains("read_stream_params("));
        let scratch = Scratch::new();
        fs::write(scratch.0.join("aura_runtime.h"), c.runtime_h).unwrap();
        fs::write(scratch.0.join("module.c"), c.module_c).unwrap();
        fs::write(scratch.0.join("driver.c"), concat!(
            "#include <stdint.h>\n#include <stdio.h>\n",
            "uint32_t read_stream_params(uint32_t,uint32_t,uint32_t);\n",
            "int main(void){return read_stream_params(41u,11u,7u)==41u?0:1;}\n"
        )).unwrap();
        for opt in ["-O0", "-O2"] {
            let exe = scratch.0.join(format!("macro_params_{version}_{}", &opt[1..]));
            let build = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
                .arg(&scratch.0)
                .arg(scratch.0.join("module.c")).arg(scratch.0.join("driver.c"))
                .arg("-o").arg(&exe).output().expect("clang in Native CI");
            assert!(build.status.success(), "{version}/{opt}: {}",
                String::from_utf8_lossy(&build.stderr));
            assert!(Command::new(exe).status().unwrap().success(),
                "{version}/{opt}: stream macro formals corrupted");
        }
    }
}
