//! EV-202 test-first: qualified, noncolliding C extern Cells must obey
//! their declared source-native ABI when actually linked to a C library.
use aura_backend_c::{emit_module, emit_module_with_source_contract};
use aura_core::{lower_program, Checker};
use aura_ir::{validate_module, ModuleIR};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

fn qualified(source: &str) -> (aura_ast::Program, ModuleIR) {
    let program = aura_parse::parse_source(source).expect("real Aura syntax parses");
    Checker::new().check_program(&program).expect("source-defined extern and calls check");
    let ir = lower_program(&program).expect("source-qualified Aura lowers");
    validate_module(&ir).expect("source SSA structurally validates");
    (program, ir)
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("aura-ev202-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

fn linked_c(source: &str, native_library: &str, driver: &str, label: &str) {
    let (ast, ir) = qualified(source);
    let c = emit_module_with_source_contract(&ast, &ir, None)
        .expect("trusted noncolliding source extern must emit type-correct C");
    let scratch = Scratch::new();
    fs::write(scratch.0.join("aura_runtime.h"), &c.runtime_h).unwrap();
    fs::write(scratch.0.join("module.c"), &c.module_c).unwrap();
    fs::write(scratch.0.join("external_library.c"), native_library).unwrap();
    fs::write(scratch.0.join("driver.c"), driver).unwrap();
    for opt in ["-O0", "-O2"] {
        let binary = scratch.0.join(format!("{label}_{}", &opt[1..]));
        let built = Command::new("clang")
            .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
            .arg(&scratch.0)
            .arg(scratch.0.join("module.c"))
            .arg(scratch.0.join("external_library.c"))
            .arg(scratch.0.join("driver.c"))
            .arg("-o").arg(&binary).output().expect("Clang on Native CI");
        assert!(built.status.success(), "{label}/{opt}: native ABI linking failed: {}\n{}",
            String::from_utf8_lossy(&built.stderr), c.module_c);
        let ran = Command::new(binary).output().expect("linked executable runs");
        assert!(ran.status.success(), "{label}/{opt}: native behavior mismatch: {}",
            String::from_utf8_lossy(&ran.stderr));
    }
}

#[test]
fn source_trusted_extern_u32_return_and_call_are_linked_to_real_c_library() {
    let source = r#"
trusted extern cell native.bump(n: u32) -> u32
cell forward_external(n: u32) ->:
    yield native.bump(n)
"#;
    let (ast, ir) = qualified(source);
    assert_eq!(ir.externs.len(), 1);
    assert!(ir.externs.contains_key("native.bump"));
    assert!(emit_module(&ir, None).is_ok(),
        "ordinary qualified source must be representable without guessed Tensor return");
    assert!(emit_module_with_source_contract(&ast, &ir, None).is_ok(),
        "source ABI contract should admit supported scalar extern");
    linked_c(source,
        "#include <stdint.h>\nuint32_t native_bump(uint32_t n){return n+9u;}\n",
        concat!(
            "#include <stdint.h>\n",
            "uint32_t forward_external(uint32_t);\n",
            "int main(void){return forward_external(41u)==50u?0:1;}\n"
        ), "u32_return");
}

#[test]
fn source_trusted_extern_void_result_must_actually_call_external_library() {
    let source = r#"
trusted extern cell native.note(n: u32) -> Unit
cell forward_note(n: u32) ->:
    native.note(n)
    yield n
"#;
    linked_c(source,
        concat!(
            "#include <stdint.h>\n",
            "uint32_t native_note_seen=0u;\n",
            "void native_note(uint32_t n){native_note_seen=n;}\n"
        ),
        concat!(
            "#include <stdint.h>\n",
            "uint32_t forward_note(uint32_t);\n",
            "extern uint32_t native_note_seen;\n",
            "int main(void){return forward_note(73u)==73u && native_note_seen==73u?0:1;}\n"
        ), "void_effect");
}

#[test]
fn ordinary_local_cell_still_keeps_public_c_abi_when_no_extern_is_linked() {
    linked_c("cell independent_native(n: u32) ->:\n    yield n\n",
        "/* no extern library needed */\n",
        concat!(
            "#include <stdint.h>\nuint32_t independent_native(uint32_t);\n",
            "int main(void){return independent_native(19u)==19u?0:1;}\n"
        ), "local_control");
}
