//! EV-201 RED-first: two distinct qualified Aura callables must never
//! resolve to the same native C symbol without an explicit safe ABI contract.
use aura_backend_c::{emit_module, emit_module_with_source_contract};
use aura_core::{lower_program, Checker};
use aura_ir::{validate_module, ModuleIR};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

fn checked_source(source: &str) -> (aura_ast::Program, ModuleIR) {
    let ast = aura_parse::parse_source(source).expect("real Aura source parses");
    Checker::new().check_program(&ast).expect("real Aura source typechecks");
    let module = lower_program(&ast).expect("real Aura source lowers");
    validate_module(&module).expect("real Aura SSA validates");
    (ast, module)
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("aura-ev201-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

const SHADOW_SOURCE: &str = r#"
trusted extern cell bridge.signal(n: u32) -> Unit
cell bridge_signal(n: u32) ->:
    yield n
cell invoke(n: u32) ->:
    bridge.signal(n)
    yield n
"#;

#[test]
fn distinct_external_and_local_aura_names_must_not_silently_bind_same_c_function() {
    let (ast, ir) = checked_source(SHADOW_SOURCE);
    assert!(ir.externs.contains_key("bridge.signal"), "declaration survived source lowering");
    assert!(ir.functions.contains_key("bridge_signal"), "independent user Cell survived source lowering");
    assert!(ir.functions.contains_key("invoke"), "real Aura extern Call emitted in IR");
    // Source-anchored public C emission must enforce the same ABI rule.
    let source_checked = emit_module_with_source_contract(&ast, &ir, None);
    let emitted = match source_checked {
        Err(err) => {
            assert!(err.message.contains("extern") && err.message.contains("collision"),
                "unexpected unrelated source-anchored error: {err}");
            return;
        }
        Ok(code) => code,
    };
    // Evidence for RED: under the old lossy dot→underscore fallback,
    // an external bridge.signal(n) is silently routed to the unrelated
    // local user Cell bridge_signal(n), even with no extern library linked.
    assert!(emitted.module_c.contains("bridge_signal(n);"),
        "expected to demonstrate actual C alias; generated: {}", emitted.module_c);
    let scratch = Scratch::new();
    fs::write(scratch.0.join("aura_runtime.h"), emitted.runtime_h).unwrap();
    fs::write(scratch.0.join("module.c"), &emitted.module_c).unwrap();
    fs::write(scratch.0.join("driver.c"), concat!(
        "#include <stdint.h>\n",
        "uint32_t invoke(uint32_t);\n",
        "int main(void){return invoke(37u)==37u?0:1;}\n"
    )).unwrap();
    for opt in ["-O0", "-O2"] {
        let exe = scratch.0.join(format!("alias_{}", &opt[1..]));
        let built = Command::new("clang")
            .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
            .arg(&scratch.0).arg(scratch.0.join("module.c"))
            .arg(scratch.0.join("driver.c"))
            .arg("-o").arg(&exe).output().expect("clang in Native CI");
        assert!(built.status.success(), "alias diagnostic failed to build {opt}: {}\n{}",
            String::from_utf8_lossy(&built.stderr), emitted.module_c);
        assert!(Command::new(&exe).status().unwrap().success(),
            "old C build did not demonstrate silent extern-symbol capture {opt}");
    }
    panic!("source-authorized extern bridge.signal silently captured by local bridge_signal; \
        C compiled/linked/executed without any real external symbol on O0 and O2");
}

#[test]
fn two_distinct_aura_extern_declarations_cannot_alias_same_native_c_symbol() {
    let source = r#"
trusted extern cell bridge.signal(n: u32) -> Unit
trusted extern cell bridge_signal(n: u32) -> Unit
cell call_both(n: u32) ->:
    bridge.signal(n)
    bridge_signal(n)
    yield n
"#;
    let (ast, module) = checked_source(source);
    assert_eq!(module.externs.len(), 2);
    let bare = emit_module(&module, None);
    assert!(bare.as_ref().err().is_some_and(|e|
        e.message.contains("extern") && e.message.contains("collision")),
        "two different Aura externs must not map to one C symbol: {:?}",
        bare.map(|code| code.module_c));
    let anchored = emit_module_with_source_contract(&ast, &module, None);
    assert!(anchored.as_ref().err().is_some_and(|e|
        e.message.contains("extern") && e.message.contains("collision")),
        "source contract must also reject colliding external aliases: {:?}",
        anchored.map(|code| code.module_c));
}

#[test]
fn unaffected_safe_user_cell_keeps_legacy_native_c_abi() {
    let (ast, ir) = checked_source("cell keep_native(n: u32) ->:\n    yield n\n");
    let emitted = emit_module_with_source_contract(&ast, &ir, None)
        .expect("ordinary source ABI unchanged");
    assert!(emitted.module_c.contains("uint32_t keep_native("));
    let scratch = Scratch::new();
    fs::write(scratch.0.join("aura_runtime.h"), emitted.runtime_h).unwrap();
    fs::write(scratch.0.join("module.c"), emitted.module_c).unwrap();
    fs::write(scratch.0.join("driver.c"), concat!(
        "#include <stdint.h>\nuint32_t keep_native(uint32_t);\n",
        "int main(void){return keep_native(41u)==41u?0:1;}\n"
    )).unwrap();
    for opt in ["-O0", "-O2"] {
        let exe = scratch.0.join(format!("safe_{}", &opt[1..]));
        let built = Command::new("clang")
            .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
            .arg(&scratch.0).arg(scratch.0.join("module.c"))
            .arg(scratch.0.join("driver.c"))
            .arg("-o").arg(&exe).output().unwrap();
        assert!(built.status.success(), "ABI control {opt}: {}",
            String::from_utf8_lossy(&built.stderr));
        assert!(Command::new(exe).status().unwrap().success());
    }
}
