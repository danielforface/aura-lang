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


#[test]
fn u64_i64_bool_and_no_arg_externs_keep_exact_independently_linked_abi() {
    // Each scenario is a separately parsed/checked/lowered Aura program.
    // The external C body lives in another translation unit, so the C ABI
    // (including 64-bit high bits and sign) must truly match on both opt levels.
    linked_c(
        "trusted extern cell native.wide(n: u64) -> u64\ncell forward_wide(n: u64) ->:\n    yield native.wide(n)\n",
        "#include <stdint.h>\nuint64_t native_wide(uint64_t n){return n-7ULL;}\n",
        concat!(
            "#include <stdint.h>\nuint64_t forward_wide(uint64_t);\n",
            "int main(void){return forward_wide(4294967305ULL)==4294967298ULL?0:1;}\n"
        ), "u64_high_bits");
    linked_c(
        "trusted extern cell native.negative(n: i64) -> i64\ncell forward_negative(n: i64) ->:\n    yield native.negative(n)\n",
        "#include <stdint.h>\nint64_t native_negative(int64_t n){return n-11LL;}\n",
        concat!(
            "#include <stdint.h>\nint64_t forward_negative(int64_t);\n",
            "int main(void){return forward_negative(-4294967301LL)==-4294967312LL?0:1;}\n"
        ), "i64_signed");
    linked_c(
        "trusted extern cell native.switch(n: bool) -> bool\ncell forward_switch(n: bool) ->:\n    yield native.switch(n)\n",
        "#include <stdbool.h>\nbool native_switch(bool n){return !n;}\n",
        concat!(
            "#include <stdbool.h>\nbool forward_switch(bool);\n",
            "int main(void){return forward_switch(true)==false && forward_switch(false)==true?0:1;}\n"
        ), "bool_flip");
    linked_c(
        "trusted extern cell native.ping() -> Unit\ncell forward_ping(n: u32) ->:\n    native.ping()\n    yield n\n",
        concat!(
            "#include <stdint.h>\nuint32_t native_ping_count=0;\n",
            "void native_ping(void){++native_ping_count;}\n"
        ),
        concat!(
            "#include <stdint.h>\nuint32_t forward_ping(uint32_t);\n",
            "extern uint32_t native_ping_count;\n",
            "int main(void){return forward_ping(13u)==13u && native_ping_count==1u?0:1;}\n"
        ), "unit_no_args");
}

#[test]
fn mutable_ir_extern_width_and_unsupported_abi_are_rejected_before_c_output() {
    // Even an originally source-qualified call is not proof that its later
    // mutable IR extern signature still matches the operands at this point.
    let source = "trusted extern cell native.bump(n: u32) -> u32\ncell forward_external(n: u32) ->:\n    yield native.bump(n)\n";
    let (ast, original) = qualified(source);
    let good = emit_module_with_source_contract(&ast, &original, None)
        .expect("original source-qualified scalar ABI remains valid");
    assert!(good.module_c.contains("extern uint32_t native_bump(uint32_t);"),
        "extern header must be type-qualified: {}", good.module_c);
    let mut changed_width = original.clone();
    changed_width.externs.get_mut("native.bump").unwrap().params[0] = aura_ir::Type::U64;
    let error = emit_module(&changed_width, None)
        .err().expect("mutated U64 ABI must not take U32 SSA argument");
    assert!(error.message.contains("extern C ABI call") || error.message.contains("IR validation"),
        "width mismatch must fail closed at IR or native ABI: {error}");
    assert!(emit_module_with_source_contract(&ast, &changed_width, None).is_err(),
        "original trusted source must reject rewritten extern contract");

    let mut unsupported = original.clone();
    unsupported.externs.get_mut("native.bump").unwrap().ret = aura_ir::Type::Tensor;
    let error = emit_module(&unsupported, None)
        .err().expect("opaque Tensor cannot be silently used as a native C scalar");
    assert!(error.message.contains("unqualified extern C scalar ABI")
        || error.message.contains("IR validation"),
        "unsupported extern result must fail closed: {error}");

    let mut non_c_abi = original.clone();
    non_c_abi.externs.get_mut("native.bump").unwrap().call_conv = aura_ir::CallConv::Stdcall;
    let error = emit_module(&non_c_abi, None)
        .err().expect("unsupported calling convention cannot be emitted as C ABI");
    assert!(error.message.contains("calling convention") || error.message.contains("IR validation"),
        "unqualified calling convention must fail closed: {error}");
}
