//! EV-198 test-first: non-refined exact scalar Return ABI width and
//! signedness. All negative IR mutations begin from checked Aura source.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{optimize_module, validate_module, InstKind, ModuleIR, Terminator};
use std::{fs, process::Command, time::{SystemTime, UNIX_EPOCH}};

const SOURCE: &str = r#"
cell echo_u8(small: u8, wide: u16) ->:
    yield small
cell echo_u16(wide: u16, small: u8) ->:
    yield wide
cell echo_i32(signed: i32, unsigned: u32) ->:
    yield signed
cell select_i64(signed: i64, unsigned: u64) ->:
    yield signed
cell select_u64(signed: i64, unsigned: u64) ->:
    yield unsigned
cell forward_i64(signed: i64, unsigned: u64) ->:
    yield select_i64(signed, unsigned)
"#;

fn original_ir() -> ModuleIR {
    let ast=aura_parse::parse_source(SOURCE).expect("EV-198 real Aura source parses");
    Checker::new().check_program(&ast).expect("EV-198 real Aura source checks");
    let ir=lower_program(&ast).expect("EV-198 real Aura source lowers");
    validate_module(&ir).expect("EV-198 real Aura source SSA validates");
    ir
}

fn require_c_return_type_error(ir:&ModuleIR, label:&str) {
    validate_module(ir).expect("IR primitive-category consistency cannot prove exact ABI signedness/width");
    let err=emit_module(ir,None).err()
        .expect("forged integer Return ABI conversion must reject before C emission");
    assert!(err.message.contains("return") && err.message.contains("type mismatch"),
        "{label}: expected explicit Return type mismatch: {err}");
}

#[test]
fn source_scalar_signed_and_narrow_returns_run_under_strict_clang_original_and_optimized() {
    let initial=original_ir();
    let mut optimized=initial.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized real source still SSA-valid");
    for (label,ir) in [("original",&initial),("optimized",&optimized)] {
        let c=emit_module(ir,None).expect("real source native C emission");
        let ns=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("aura-ev198-{}-{ns}",std::process::id()));
        fs::create_dir_all(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
        let _clean=Cleanup(root.clone());
        fs::write(root.join("aura_runtime.h"),c.runtime_h).unwrap();
        fs::write(root.join("module.c"),&c.module_c).unwrap();
        fs::write(root.join("driver.c"),concat!(
            "#include <stdint.h>\n",
            "uint8_t echo_u8(uint8_t,uint16_t);\n",
            "uint16_t echo_u16(uint16_t,uint8_t);\n",
            "int32_t echo_i32(int32_t,uint32_t);\n",
            "int64_t select_i64(int64_t,uint64_t);\n",
            "uint64_t select_u64(int64_t,uint64_t);\n",
            "int64_t forward_i64(int64_t,uint64_t);\n",
            "int main(void){return (",
            "echo_u8(251u,60123u)==251u",
            " && echo_u16(60123u,251u)==60123u",
            " && echo_i32(-2000000000,4000000000u)==-2000000000",
            " && select_i64(-4294967301LL,4294967303ULL)==-4294967301LL",
            " && select_u64(-4294967301LL,4294967303ULL)==4294967303ULL",
            " && forward_i64(-4294967301LL,4294967303ULL)==-4294967301LL",
            ")?0:1;}\n"
        )).unwrap();
        for opt in ["-O0","-O2"] {
            let exe=root.join(format!("{}_{}",label,&opt[1..]));
            let cc=Command::new("clang")
                .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
                .arg(&root).arg(root.join("module.c")).arg(root.join("driver.c"))
                .arg("-o").arg(&exe).output().expect("Clang on Native CI");
            assert!(cc.status.success(),"{label}/{opt}: {}\n{}",
                String::from_utf8_lossy(&cc.stderr),c.module_c);
            let run=Command::new(exe).output().expect("run generated native executable");
            assert!(run.status.success(),"{label}/{opt}: {}",
                String::from_utf8_lossy(&run.stderr));
        }
    }
}

fn mutate_return_to_param(ir:&mut ModuleIR,func:&str,param_index:usize) {
    let f=ir.functions.get_mut(func).expect("source cell found");
    let value=f.params[param_index].value;
    let bb=f.blocks.iter_mut().find(|b|matches!(b.term,Terminator::Return(Some(_))))
        .expect("source-generated Return exists");
    bb.term=Terminator::Return(Some(value));
}

#[test]
fn forged_u16_into_u8_return_must_fail_closed() {
    let mut ir=original_ir();
    mutate_return_to_param(&mut ir,"echo_u8",1);
    require_c_return_type_error(&ir,"u16 -> u8 (lossy)");
}

#[test]
fn forged_u8_into_u16_return_must_fail_closed_without_proven_literal() {
    let mut ir=original_ir();
    mutate_return_to_param(&mut ir,"echo_u16",1);
    require_c_return_type_error(&ir,"u8 -> u16 (unproven)");
}

#[test]
fn forged_u32_into_i32_return_must_fail_closed() {
    let mut ir=original_ir();
    mutate_return_to_param(&mut ir,"echo_i32",1);
    require_c_return_type_error(&ir,"u32 -> i32 sign domain");
}

#[test]
fn forged_u64_into_i64_return_must_fail_closed() {
    let mut ir=original_ir();
    mutate_return_to_param(&mut ir,"select_i64",1);
    require_c_return_type_error(&ir,"u64 -> i64 high bit reinterpretation");
}

#[test]
fn forged_i64_into_u64_return_must_fail_closed() {
    let mut ir=original_ir();
    mutate_return_to_param(&mut ir,"select_u64",0);
    require_c_return_type_error(&ir,"i64 -> u64 negative reinterpretation");
}

#[test]
fn forged_unsigned_user_cell_result_into_signed_return_must_fail_closed() {
    let mut ir=original_ir();
    let f=ir.functions.get_mut("forward_i64").unwrap();
    let call=f.blocks.iter_mut().flat_map(|b|b.insts.iter_mut())
        .find(|i|matches!(&i.kind,InstKind::Call{callee,..} if callee=="select_i64"))
        .expect("source forward uses typed Call");
    let InstKind::Call{callee,..}=&mut call.kind else {unreachable!()};
    *callee="select_u64".into(); // identical parameter ABI; only return sign differs
    require_c_return_type_error(&ir,"u64 Call result into i64 Return");
}
