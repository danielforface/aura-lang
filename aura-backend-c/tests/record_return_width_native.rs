//! EV-197 test-first: C Return ABI exact U32/U64 width must not
//! silently convert a same-category SSA value from valid Aura source.
//! Uses real source to obtain all function signatures and CFGs, then
//! changes only a single Return SSA use / Call result provenance.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{optimize_module, run_oracle_entry, validate_module, InstKind,
    ModuleIR, OracleConfig, OracleValue, Terminator};
use std::{fs,process::Command,time::{SystemTime,UNIX_EPOCH}};

const SOURCE: &str=r#"
cell return_wide(wide: u64, small: u32) ->:
    yield wide
cell return_narrow(wide: u64, small: u32) ->:
    yield small
cell forward_wide(wide: u64, small: u32) ->:
    yield return_wide(wide, small)
cell forward_narrow(wide: u64, small: u32) ->:
    yield return_narrow(wide, small)
"#;

fn real_ir()->ModuleIR {
    let ast=aura_parse::parse_source(SOURCE).expect("EV-197 source parses");
    Checker::new().check_program(&ast).expect("EV-197 source passes Checker");
    let ir=lower_program(&ast).expect("EV-197 source lowers");
    validate_module(&ir).expect("source-generated SSA structurally valid");
    ir
}
fn must_reject_c_return_width(ir:&ModuleIR,case:&str){
    let err=emit_module(ir,None).err()
        .expect("forged width-mismatched Return must fail closed at C ABI boundary");
    assert!(err.message.contains("return") && err.message.contains("type mismatch"),
        "{case}: wrong C Return diagnostic: {err}");
}

#[test]
fn real_u64_u32_source_oracle_optimizer_and_strict_native_preserve_abi() {
    let source=real_ir();
    let mut optimized=source.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("post-optimization SSA valid");
    for (label,ir) in [("original",&source),("optimized",&optimized)] {
        // Oracle models the small scalar domain but Native tests high U64.
        for (name,expected) in [("return_wide",7),("return_narrow",13),
            ("forward_wide",7),("forward_narrow",13)] {
            let got=run_oracle_entry(ir,name,
                &[OracleValue::U32(7),OracleValue::U32(13)],
                OracleConfig::default()).expect("source Oracle run");
            assert!(got.ok,"{label} {name} Oracle execution");
            assert_eq!(got.return_value,Some(OracleValue::U32(expected)),
                "{label} {name} Oracle mismatch");
        }
        let c=emit_module(ir,None).expect("valid source emits C");
        let ns=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("aura-ev197-{}-{ns}",std::process::id()));
        fs::create_dir_all(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
        let _clean=Cleanup(root.clone());
        fs::write(root.join("aura_runtime.h"),c.runtime_h).unwrap();
        fs::write(root.join("module.c"),&c.module_c).unwrap();
        fs::write(root.join("driver.c"),concat!(
            "#include <stdint.h>\n",
            "uint64_t return_wide(uint64_t,uint32_t);\n",
            "uint32_t return_narrow(uint64_t,uint32_t);\n",
            "uint64_t forward_wide(uint64_t,uint32_t);\n",
            "uint32_t forward_narrow(uint64_t,uint32_t);\n",
            "int main(void) { return (",
            "return_wide(4294967299ULL,13u)==4294967299ULL",
            " && return_narrow(4294967299ULL,13u)==13u",
            " && forward_wide(4294967299ULL,13u)==4294967299ULL",
            " && forward_narrow(4294967299ULL,13u)==13u",
            ")?0:1; }\n"
        )).unwrap();
        for opt in ["-O0","-O2"] {
            let exe=root.join(format!("{}_{}",label,&opt[1..]));
            let built=Command::new("clang")
                .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
                .arg(&root).arg(root.join("module.c")).arg(root.join("driver.c"))
                .arg("-o").arg(&exe).output().expect("Clang available in Native CI");
            assert!(built.status.success(),"{label}/{opt}: {}\n{}",
                String::from_utf8_lossy(&built.stderr),c.module_c);
            let ran=Command::new(exe).output().expect("run independently linked executable");
            assert!(ran.status.success(),"{label}/{opt}: {}",
                String::from_utf8_lossy(&ran.stderr));
        }
    }
}

#[test]
fn forged_u32_ssa_as_u64_return_must_reject() {
    let mut ir=real_ir();
    let f=ir.functions.get_mut("return_wide").unwrap();
    let small=f.params[1].value;
    let bb=f.blocks.iter_mut().find(|b|matches!(b.term,Terminator::Return(Some(_))))
        .expect("source wide Return");
    bb.term=Terminator::Return(Some(small));
    validate_module(&ir).expect("category-only validator is not an exact width proof");
    must_reject_c_return_width(&ir,"known U32 SSA replacing U64 return");
}

#[test]
fn forged_u64_ssa_as_u32_return_must_reject_truncation() {
    let mut ir=real_ir();
    let f=ir.functions.get_mut("return_narrow").unwrap();
    let wide=f.params[0].value;
    let bb=f.blocks.iter_mut().find(|b|matches!(b.term,Terminator::Return(Some(_))))
        .expect("source narrow Return");
    bb.term=Terminator::Return(Some(wide));
    validate_module(&ir).expect("category-only validator is not an exact width proof");
    must_reject_c_return_width(&ir,"known U64 SSA replacing U32 return");
}

#[test]
fn forged_wide_call_result_as_narrow_return_must_reject() {
    let mut ir=real_ir();
    let f=ir.functions.get_mut("forward_narrow").unwrap();
    let call=f.blocks.iter_mut().flat_map(|b|b.insts.iter_mut())
        .find(|inst|matches!(&inst.kind,InstKind::Call{callee,..} if callee=="return_narrow"))
        .expect("source forwards narrow Call result");
    let InstKind::Call{callee,..}=&mut call.kind else {unreachable!()};
    *callee="return_wide".into(); // identical source param signature, wider result only
    validate_module(&ir).expect("known primitive categories remain Integer");
    must_reject_c_return_width(&ir,"wide user-Cell result truncating narrow return");
}
