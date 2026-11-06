//! EV-203: true source-qualified extern call SSA Phi / heterogeneous C ABI.
//! Compile independent C translation units, not just compare emitted strings.
use aura_backend_c::{emit_module, emit_module_with_source_contract};
use aura_core::{lower_program, Checker};
use aura_ir::{optimize_module, validate_module, InstKind, ModuleIR};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

const SOURCE: &str = r#"
trusted extern cell native.on(small: u8, neg: i16, high: u64) -> i64
trusted extern cell native.off(small: u8, neg: i16, high: u64) -> i64
cell choose_extern(flag: bool, small: u8, neg: i16, high: u64) ->:
    val mut selected: i64 = native.on(small, neg, high)
    if flag:
        selected = native.on(small, neg, high)
    else:
        selected = native.off(small, neg, high)
    yield selected
"#;
const NATIVE: &str = concat!(
    "#include <stdint.h>\n",
    "int64_t native_on(uint8_t small,int16_t neg,uint64_t high){",
    "return (small==251u && neg==-1234 && high==4294967299ULL)",
    "?-4294967297LL:1LL;}\n",
    "int64_t native_off(uint8_t small,int16_t neg,uint64_t high){",
    "return (small==251u && neg==-1234 && high==4294967299ULL)",
    "?-4294967298LL:2LL;}\n"
);
const DRIVER: &str = concat!(
    "#include <stdbool.h>\n#include <stdint.h>\n",
    "int64_t choose_extern(bool,uint8_t,int16_t,uint64_t);\n",
    "int main(void){return (",
    "choose_extern(true,251u,-1234,4294967299ULL)==-4294967297LL",
    " && choose_extern(false,251u,-1234,4294967299ULL)==-4294967298LL",
    ")?0:1;}\n"
);

fn real_source() -> (aura_ast::Program, ModuleIR) {
    let ast = aura_parse::parse_source(SOURCE).expect("EV-203 Aura source parses");
    Checker::new().check_program(&ast).expect("EV-203 extern and Phi source typechecks");
    let ir = lower_program(&ast).expect("real Aura source lowers");
    validate_module(&ir).expect("source SSA validates");
    (ast, ir)
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("aura-ev203-{}-{stamp}",std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}
}

#[test]
fn heterogeneous_extern_calls_phi_return_retain_signed_width_and_param_order() {
    let (ast, original) = real_source();
    let mut optimized = original.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized source SSA validates");
    for (label,ir) in [("original",&original),("optimized",&optimized)] {
        let phi_count=ir.functions["choose_extern"].blocks.iter()
            .flat_map(|b| &b.insts)
            .filter(|i| matches!(i.kind,InstKind::Phi { .. })).count();
        assert!(phi_count>0,"{label}: expected an actual source-derived SSA Phi");
        let out = emit_module_with_source_contract(&ast,ir,None)
            .expect("checked source extern calls emit C with exact ABI");
        assert!(out.module_c.contains("extern int64_t native_on(uint8_t, int16_t, uint64_t);"),
            "{label}: type-preserving extern declaration lost: {}",out.module_c);
        assert!(out.module_c.contains("extern int64_t native_off(uint8_t, int16_t, uint64_t);"),
            "{label}: second extern declaration lost: {}",out.module_c);
        let root=Scratch::new();
        fs::write(root.0.join("aura_runtime.h"),&out.runtime_h).unwrap();
        fs::write(root.0.join("module.c"),&out.module_c).unwrap();
        fs::write(root.0.join("native.c"),NATIVE).unwrap();
        fs::write(root.0.join("driver.c"),DRIVER).unwrap();
        for opt in ["-O0","-O2"] {
            let exe=root.0.join(format!("{label}_{}",&opt[1..]));
            let cc=Command::new("clang")
                .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
                .arg(&root.0).arg(root.0.join("module.c"))
                .arg(root.0.join("native.c")).arg(root.0.join("driver.c"))
                .arg("-o").arg(&exe).output().expect("Clang in Native CI");
            assert!(cc.status.success(),"{label}/{opt}: {}\n{}",
                String::from_utf8_lossy(&cc.stderr),out.module_c);
            let run=Command::new(exe).output().expect("native executable runs");
            assert!(run.status.success(),"{label}/{opt}: incorrect mixed extern/Phi ABI: {}",
                String::from_utf8_lossy(&run.stderr));
        }
    }
}

#[test]
fn heterogeneous_extern_call_ir_operand_corruption_refuses_ambiguous_native_abi() {
    let (_ast,original) = real_source();
    let mut damaged=original.clone();
    let f=damaged.functions.get_mut("choose_extern").unwrap();
    let neg=f.params[2].value;
    let call=f.blocks.iter_mut().flat_map(|b|&mut b.insts)
        .find(|i|matches!(&i.kind,InstKind::Call{callee,..} if callee=="native.on"))
        .expect("source Call exists");
    if let InstKind::Call{args,..}=&mut call.kind {args[0]=neg;}
    let emission=emit_module(&damaged,None);
    let err=emission.err().expect("mutated i16 in extern u8 must fail closed");
    assert!(err.message.contains("extern C ABI call")
        || err.message.contains("IR validation")
        || err.message.contains("type mismatch"),
        "unsafe external argument mutation incorrectly diagnosed: {err}");
}
