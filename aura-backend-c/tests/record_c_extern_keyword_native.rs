//! EV-204 test-first: Aura identifiers may be illegal C ABI spellings.
use aura_backend_c::{emit_module, emit_module_with_source_contract};
use aura_core::{lower_program, Checker};
use aura_ir::validate_module;
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let stamp=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path=std::env::temp_dir().join(format!("aura-ev204-{}-{stamp}",std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {let _=fs::remove_dir_all(&self.0);}
}
#[test]
fn valid_aura_extern_c_keyword_must_be_refused_before_emitting_invalid_c() {
    // Aura-legal signed is a C keyword, so it cannot be a foreign C function.
    let source=r#"
trusted extern cell signed(n: u32) -> u32
cell invoke_keyword(n: u32) ->:
    yield signed(n)
"#;
    let ast=aura_parse::parse_source(source).expect("Aura accepts signed as source identifier");
    Checker::new().check_program(&ast).expect("source extern semantically valid");
    let ir=lower_program(&ast).expect("source extern lowers");
    validate_module(&ir).expect("source IR structurally validates");
    for (label, emitted) in [
        ("raw",emit_module(&ir,None)),
        ("anchored",emit_module_with_source_contract(&ast,&ir,None)),
    ] {
        match emitted {
            Err(err) => assert!(err.message.contains("extern C identifier")
                && err.message.contains("signed"),"{label}: wrong diagnostic: {err}"),
            Ok(c) => {
                let root=Scratch::new();
                fs::write(root.0.join("aura_runtime.h"),c.runtime_h).unwrap();
                fs::write(root.0.join("module.c"),&c.module_c).unwrap();
                let cc=Command::new("clang")
                    .args(["-std=gnu11","-Wall","-Werror","-O0","-I"])
                    .arg(&root.0).arg("-c").arg(root.0.join("module.c"))
                    .arg("-o").arg(root.0.join("module.o"))
                    .output().expect("Clang in Native CI");
                assert!(!cc.status.success(),"old C unexpectedly compiles illegal keyword ABI: {}",c.module_c);
                let diagnostic=String::from_utf8_lossy(&cc.stderr);
                assert!(diagnostic.contains("signed") || diagnostic.contains("expected"),
                    "{label}: unrelated Clang failure: {diagnostic}");
                panic!("{label}: backend emitted invalid C keyword extern instead of rejecting ABI:\n{diagnostic}");
            }
        }
    }
}
#[test]
fn qualified_dotted_extern_with_keyword_segment_keeps_valid_native_linkage() {
    // native.signed maps to native_signed, which is C-safe.
    let source=r#"
trusted extern cell native.signed(n: u32) -> u32
cell invoke_valid(n: u32) ->:
    yield native.signed(n)
"#;
    let ast=aura_parse::parse_source(source).expect("dotted Aura source parses");
    Checker::new().check_program(&ast).expect("dotted extern checks");
    let ir=lower_program(&ast).expect("dotted extern lowers");
    validate_module(&ir).expect("source SSA validates");
    let c=emit_module_with_source_contract(&ast,&ir,None)
        .expect("safe native link spelling must be supported");
    assert!(c.module_c.contains("extern uint32_t native_signed(uint32_t);"),
        "wrong extern native spelling: {}",c.module_c);
    let root=Scratch::new();
    fs::write(root.0.join("aura_runtime.h"),c.runtime_h).unwrap();
    fs::write(root.0.join("module.c"),c.module_c).unwrap();
    fs::write(root.0.join("external.c"),
        "#include <stdint.h>\nuint32_t native_signed(uint32_t n){return n+11u;}\n").unwrap();
    fs::write(root.0.join("driver.c"),
        "#include <stdint.h>\nuint32_t invoke_valid(uint32_t);\nint main(void){return invoke_valid(31u)==42u?0:1;}\n").unwrap();
    for opt in ["-O0","-O2"] {
        let exe=root.0.join(format!("ev204_{}",&opt[1..]));
        let cc=Command::new("clang")
            .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
            .arg(&root.0).arg(root.0.join("module.c"))
            .arg(root.0.join("external.c")).arg(root.0.join("driver.c"))
            .arg("-o").arg(&exe).output().expect("Clang in Native CI");
        assert!(cc.status.success(),"{opt}: native C linkage: {}",String::from_utf8_lossy(&cc.stderr));
        assert!(Command::new(exe).status().unwrap().success(),"{opt}: wrong C result");
    }
}
