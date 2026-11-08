// New source-level String equality through nominal Record fields.
// Incorporated in the existing mandatory record_function_abi_native gate.

use aura_backend_c::emit_module;
use aura_ir::{run_oracle_entry, validate_module, OracleConfig, OracleValue, InstKind, Type};
use std::{fs, process::Command};

/// Isolated per-test harness: Cargo also compiles this file as an independent
/// integration test, not just under record_function_abi_native's #[path] mod.
struct Scratch(std::path::PathBuf);
impl Scratch {
    fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after Unix epoch").as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "aura-record_string_eq_native-{}-{nanos}", std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create native test scratch directory");
        Self(dir)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
use aura_core::{lower_program, Checker};


const SOURCE: &str = r#"
type Label = record { title: String, rank: u32 }
cell equivalent(first: String, second: String) ->:
    val left: Label = Label { title: first, rank: 1 }
    val right: Label = Label { title: second, rank: 2 }
    yield left.title == right.title
cell different(first: String, second: String) ->:
    val left: Label = Label { title: first, rank: 1 }
    val right: Label = Label { title: second, rank: 2 }
    yield left.title != right.title
cell unicode_literal_equality() ->:
    val left: Label = Label { title: "שלום🙂", rank: 1 }
    val right: Label = Label { title: "שלום🙂", rank: 2 }
    yield left.title == right.title
cell aura_string_content_equal() ->:
    yield 7
"#;

#[test]
fn unicode_record_string_eq_ne_compiles_and_executes_at_o0_o2() {
    let parsed = aura_parse::parse_source(SOURCE).expect("source parses");
    Checker::new().check_program(&parsed).expect("source String Eq/Ne supported");
    let original = lower_program(&parsed).expect("real source lowers");
    validate_module(&original).expect("original valid IR");
    let mut optimized = original.clone();
    aura_ir::optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized valid IR");
    let cases = [
        ("equal", "equal", true), ("equal", "Equal", false),
        ("", "", true), ("", "x", false),
        ("שלום🙂", "שלום🙂", true), ("שלום🙂", "שלום", false),
        ("line\nbreak", "line\nbreak", true), ("a%b", "a%b", true),
    ];
    let driver = concat!(
        "#include <stdbool.h>\n#include <stdint.h>\n#include <stddef.h>\n",
        "bool equivalent(const char*,const char*);\n",
        "bool different(const char*,const char*);\n",
        "bool unicode_literal_equality(void);\n",
        "uint32_t aura_cell_617572615f737472696e675f636f6e74656e745f657175616c(void);\n",
        "int main(int argc, char** argv){\n",
        "  if(argc>1){(void)argv;(void)equivalent(NULL,\"ok\");return 99;}\n",
        "  char a[]=\"equal\",b[]=\"equal\",c[]=\"Equal\";\n",
        "  char h1[]=\"שלום🙂\",h2[]=\"שלום🙂\";\n",
        "  const char *pa=a,*pb=b,*pc=c,*ph1=h1,*ph2=h2;\n",
        "  if(pa==pb||pa==pc||ph1==ph2)return 97;\n",
        "  if(!equivalent(a,b)||different(a,b))return 1;\n",
        "  if(equivalent(a,c)||!different(a,c))return 2;\n",
        "  if(!equivalent(h1,h2)||different(h1,h2))return 3;\n",
        "  if(!equivalent(\"\",\"\")||!different(\"\",\"x\"))return 4;\n",
        "  if(!unicode_literal_equality())return 5;\n",
        "  if(aura_cell_617572615f737472696e675f636f6e74656e745f657175616c()!=7u)return 6;\n",
        "  return 0;\n}\n"
    );
    for (profile, ir) in [("source", &original), ("optimized", &optimized)] {
        for (lhs, rhs, eq) in cases {
            let arguments = [OracleValue::String(lhs.into()), OracleValue::String(rhs.into())];
            for (function, expected) in [("equivalent", eq), ("different", !eq)] {
                let result = run_oracle_entry(ir, function, &arguments, OracleConfig::default())
                    .expect("Record String IR Oracle");
                assert!(result.ok && result.stderr.is_empty(), "{profile}/{function}");
                assert_eq!(result.return_value, Some(OracleValue::Bool(expected)),
                    "{profile}/{function}: String values {lhs:?} vs {rhs:?}");
            }
        }
        let unicode = run_oracle_entry(ir, "unicode_literal_equality", &[], OracleConfig::default())
            .expect("Unicode literal equality Oracle");
        assert_eq!(unicode.return_value, Some(OracleValue::Bool(true)));
        let c = emit_module(ir, None).expect("String value comparison emits C");
        assert!(c.module_c.contains("aura_string_content_equal("),
            "{profile}: String values compared by pointer");
        assert!(c.module_c.contains("aura_cell_617572615f737472696e675f636f6e74656e745f657175616c"),
            "{profile}: user Cell collided with runtime helper");
        let tmp = Scratch::new();
        fs::write(tmp.0.join("aura_runtime.h"), &c.runtime_h).unwrap();
        fs::write(tmp.0.join("module.c"), &c.module_c).unwrap();
        fs::write(tmp.0.join("driver.c"), driver).unwrap();
        for opt in ["-O0", "-O2"] {
            let exe = tmp.0.join(format!("string_comparison_{profile}_{}", &opt[1..]));
            let cc = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror", "-fsanitize=undefined",
                       "-fno-sanitize-recover=all", opt, "-I"])
                .arg(&tmp.0).arg(tmp.0.join("module.c"))
                .arg(tmp.0.join("driver.c")).arg("-o").arg(&exe)
                .output().expect("Clang available in Native CI");
            assert!(cc.status.success(),
                "{profile}/{opt}: C compile: {}\n{}",String::from_utf8_lossy(&cc.stderr), c.module_c);
            let run = Command::new(&exe).output().expect("compiled String Record");
            assert!(run.status.success(),
                "{profile}/{opt}: String comparison runtime: {:?}",run);
            let invalid = Command::new(&exe).arg("null").output()
                .expect("malformed C input scenario runs");
            assert!(!invalid.status.success(),"{profile}/{opt}: null C String accepted");
            assert!(String::from_utf8_lossy(&invalid.stderr)
                .contains("Aura String comparison rejected: null native String"),
                "{profile}/{opt}: no explicit native null diagnostic: {:?}",invalid);
        }
    }
}

#[test]
fn string_order_mixed_and_deferred_snapshot_fail_closed() {
    for (kind, src) in [
        ("order", r#"
type Label = record { title: String, rank: u32 }
cell invalid(a: Label, b: Label) ->:
    yield a.title < b.title
"#),
        ("mixed", r#"
type Label = record { title: String, rank: u32 }
cell invalid(a: Label) ->:
    yield a.title == 7
"#),
    ] {
        let parsed = aura_parse::parse_source(src).expect("negative comparison parses");
        let err = Checker::new().check_program(&parsed)
            .err().expect("String ordering/mixed types remain unsupported");
        assert!(err.message.contains("comparison op expects u32"),
            "{kind}: incorrect Checker guard: {err:?}");
    }
    let src = r#"
cell compare_deferred(n: u32, other: String) ->:
    val saved: String = "value={n}"
    yield saved == other
"#;
    let parsed = aura_parse::parse_source(src).expect("deferred String source parses");
    Checker::new().check_program(&parsed).expect("source String Eq is type-correct");
    let original = lower_program(&parsed).expect("deferred String lowers");
    validate_module(&original).expect("original deferred IR structurally valid");
    let mut optimized = original.clone();
    aura_ir::optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized deferred IR valid");
    for (label, ir) in [("source", &original), ("optimized", &optimized)] {
        let err = emit_module(ir, None).err()
            .expect("native deferred String equality must fail closed");
        assert!(err.message.contains("unqualified interpolated String equality"),
            "{label}: unexpected native interpolation guard: {err}");
    }
}
