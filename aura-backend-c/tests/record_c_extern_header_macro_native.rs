//! EV-205 RED: C header macros can capture externally declared function names.
//! Verify actual Aura source, emitted C and the compiler's macro definitions.
use aura_backend_c::{emit_module, emit_module_with_source_contract};
use aura_core::{lower_program, Checker};
use aura_ir::{validate_module, ModuleIR};
use std::{fs,path::PathBuf,process::Command,time::{SystemTime,UNIX_EPOCH}};
struct Scratch(PathBuf);
impl Scratch {
 fn new()->Self {
  let ns=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
  let p=std::env::temp_dir().join(format!("aura-ev205-{}-{ns}",std::process::id()));
  fs::create_dir_all(&p).unwrap(); Self(p)
 }
}
impl Drop for Scratch {fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
fn source_ir(src:&str)->(aura_ast::Program,ModuleIR){
 let ast=aura_parse::parse_source(src).expect("actual Aura source parses");
 Checker::new().check_program(&ast).expect("source extern typechecks");
 let ir=lower_program(&ast).expect("source lowers");
 validate_module(&ir).expect("real source IR validates");
 (ast,ir)
}
#[test]
fn actual_stdint_and_inttypes_macros_are_not_foreign_linker_symbols(){
 for name in ["PRId64","SIZE_MAX"] {
  let src=format!("trusted extern cell {name}(n: u32) -> u32\ncell call_macro(n: u32) ->:\n    yield {name}(n)\n");
  let (ast,ir)=source_ir(&src);
  for (label,result) in [("raw",emit_module(&ir,None)),("source",emit_module_with_source_contract(&ast,&ir,None))] {
   match result {
    Err(e)=>assert!(e.message.contains("extern C symbol collision")
       && e.message.contains("macro") && e.message.contains(name),
       "{name}/{label}: wrong fail-closed diagnosis: {e}"),
    Ok(c)=>{
     let tmp=Scratch::new();
     fs::write(tmp.0.join("aura_runtime.h"),c.runtime_h).unwrap();
     fs::write(tmp.0.join("module.c"),&c.module_c).unwrap();
     fs::write(tmp.0.join("headers.c"),"#include \"aura_runtime.h\"\n").unwrap();
     let macros=Command::new("clang").args(["-std=gnu11","-dM","-E","-I"])
       .arg(&tmp.0).arg(tmp.0.join("headers.c")).output().expect("Clang macro list");
     assert!(macros.status.success(),"Clang macros: {}",String::from_utf8_lossy(&macros.stderr));
     assert!(String::from_utf8_lossy(&macros.stdout).lines()
       .any(|line|line.starts_with(&format!("#define {name} "))),
       "{name} was not a real macro in target runtime headers");
     let cc=Command::new("clang").args(["-std=gnu11","-Wall","-Werror","-O0","-I"])
       .arg(&tmp.0).arg("-c").arg(tmp.0.join("module.c"))
       .arg("-o").arg(tmp.0.join("module.o"))
       .output().expect("Clang native macro conflict");
     assert!(!cc.status.success(),"{name}: native C unexpectedly compiled: {}",c.module_c);
     let errors=String::from_utf8_lossy(&cc.stderr);
     assert!(errors.contains(name) || errors.contains("macro") || errors.contains("expected"),
        "{name}: unrelated native failure: {errors}");
     panic!("{name}/{label}: real C header macro captured extern instead of a source-level ABI error:\n{errors}");
    }
   }
  }
 }
}
#[test]
fn unused_explicit_macro_extern_also_fails_closed(){
 let (_ast,ir)=source_ir("trusted extern cell PRId64(n: u32) -> u32\ncell preserve_local(n: u32) ->:\n    yield n\n");
 let e=emit_module(&ir,None).err().expect("unused extern macro declaration must not evade ABI validation");
 assert!(e.message.contains("extern C symbol collision") && e.message.contains("macro") && e.message.contains("PRId64"),"{e}");
}
#[test]
fn dotted_external_name_with_nonmacro_c_symbol_links_and_runs_o0_o2(){
 let src="trusted extern cell native.PRId64(n: u64) -> u64\ncell forward_safe(n: u64) ->:\n    yield native.PRId64(n)\n";
 let (ast,ir)=source_ir(src);
 let c=emit_module_with_source_contract(&ast,&ir,None).expect("safe native_PRId64 ABI");
 assert!(c.module_c.contains("extern uint64_t native_PRId64(uint64_t);"));
 let tmp=Scratch::new();
 fs::write(tmp.0.join("aura_runtime.h"),c.runtime_h).unwrap();
 fs::write(tmp.0.join("module.c"),c.module_c).unwrap();
 fs::write(tmp.0.join("native.c"),"#include <stdint.h>\nuint64_t native_PRId64(uint64_t n){return n+17ULL;}\n").unwrap();
 fs::write(tmp.0.join("driver.c"),"#include <stdint.h>\nuint64_t forward_safe(uint64_t);\nint main(void){return forward_safe(4294967298ULL)==4294967315ULL?0:1;}\n").unwrap();
 for opt in ["-O0","-O2"]{
  let exe=tmp.0.join(format!("ev205_{}",&opt[1..]));
  let cc=Command::new("clang").args(["-std=gnu11","-Wall","-Werror",opt,"-I"]).arg(&tmp.0)
   .arg(tmp.0.join("module.c")).arg(tmp.0.join("native.c")).arg(tmp.0.join("driver.c"))
   .arg("-o").arg(&exe).output().expect("Clang external");
  assert!(cc.status.success(),"{opt}: {}",String::from_utf8_lossy(&cc.stderr));
  assert!(Command::new(exe).status().unwrap().success(),"{opt}: linked C result wrong");
 }
}
