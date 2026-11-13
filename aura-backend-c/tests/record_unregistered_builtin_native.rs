//! Q03-007 source-to-native capability boundary.
//! A source-valid AVM-only standard function must not become an unresolved
//! C symbol and an unqualified Tensor return through a generic C call mapper.
//! Existing declared user cells, trusted externs and implemented native
//! builtins must remain executable under strict Clang O0/O2.

use aura_backend_c::{emit_module, emit_module_with_source_contract};
use aura_core::{lower_program, Checker};
use aura_ir::{run_oracle_entry, validate_module, InstKind, ModuleIR, OracleConfig};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

fn source_ir(src: &str) -> (aura_ast::Program, ModuleIR) {
    let ast = aura_parse::parse_source(src).expect("real Aura source must parse");
    Checker::new().check_program(&ast).expect("source is valid for Aura Checker");
    let ir = lower_program(&ast).expect("real source must lower without shortcut");
    validate_module(&ir).expect("source IR must have valid CFG/SSA");
    (ast, ir)
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static COUNT: AtomicU64 = AtomicU64::new(0);
        let ns = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "aura-q03-native-call-trust-{}-{ns}-{}",
            std::process::id(), COUNT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

#[test]
fn source_valid_avm_only_builtins_are_refused_before_emitting_unresolved_native_c() {
    for builtin in ["io.read_text", "io.read_line"] {
        let source = format!(
            "cell unsupported() ->:\n    {builtin}(\"ignored.txt\")\n"
        );
        let (ast, ir) = source_ir(&source);
        let original_err = emit_module_with_source_contract(&ast, &ir, None)
            .err().expect("source-authenticated C backend must refuse AVM-only builtin");
        assert!(original_err.message.contains(builtin), "{original_err}");
        assert!(original_err.message.contains("unqualified native C Call"), "{original_err}");

        let err = emit_module(&ir, None).err()
            .expect("IR-only native emitter must reject the same unsupported builtin");
        assert!(err.message.contains(builtin), "{err}");
        let mut optimized = ir.clone();
        aura_ir::optimize_module(&mut optimized);
        validate_module(&optimized).expect("source IR remains structurally valid after optimize");
        let optimized_err = emit_module(&optimized, None).err()
            .expect("optimizer must not launder AVM-only builtin into a C artifact");
        assert!(optimized_err.message.contains(builtin), "{optimized_err}");
    }
}

#[test]
fn forged_undeclared_call_is_rejected_without_claiming_an_existing_runtime_symbol() {
    let (_ast, mut ir) = source_ir(
        "cell existing() ->:\n    io.println(\"supported\")\n"
    );
    let call = ir.functions.get_mut("existing").unwrap()
        .blocks.iter_mut().flat_map(|b| &mut b.insts)
        .find(|i| matches!(&i.kind, InstKind::Call { .. }))
        .expect("real source must contain a Call");
    let InstKind::Call { callee, .. } = &mut call.kind else { unreachable!() };
    *callee = "io_read_text".to_owned(); // underscore alias must not spoof io.read_text
    validate_module(&ir).expect("forged callee is still structurally valid IR");
    let err = emit_module(&ir, None).err().expect("native must refuse undeclared callee");
    assert!(err.message.contains("io_read_text") &&
        err.message.contains("unqualified native C Call"), "{err}");
}

#[test]
fn declared_cell_extern_and_native_println_remain_real_linkable_c() {
    let source = r#"
trusted extern cell native_echo(n: u32) -> u32
cell identity(n: u32) ->:
    yield n
cell checked_user_cell() ->:
    yield identity(native_echo(9))
cell supported() ->:
    io.println("native call")
"#;
    let (ast, ir) = source_ir(source);
    let mut optimized = ir.clone();
    aura_ir::optimize_module(&mut optimized);
    validate_module(&optimized).expect("known-call optimized IR validates");

    for (label, module) in [("original", &ir), ("optimized", &optimized)] {
        let c = emit_module_with_source_contract(&ast, module, None)
            .unwrap_or_else(|err| panic!("{label}: known native calls must remain supported: {err}"));
        let dir = Scratch::new();
        fs::write(dir.0.join("aura_runtime.h"), &c.runtime_h).unwrap();
        fs::write(dir.0.join("module.c"), &c.module_c).unwrap();
        let driver = r#"
#include "aura_runtime.h"
#include <string.h>
static unsigned calls = 0;
void aura_io_println(const char *text) {
    if (!text || strcmp(text, "native call") != 0) abort();
    ++calls;
}
uint32_t native_echo(uint32_t n) { return n; }
uint32_t checked_user_cell(void);
void supported(void);
int main(void) {
    if (checked_user_cell() != 9u) return 1;
    supported();
    return calls == 1u ? 0 : 2;
}
"#;
        fs::write(dir.0.join("driver.c"), driver).unwrap();
        for opt in ["-O0", "-O2"] {
            let exe = dir.0.join(format!("native_call_{}_{}", label, &opt[1..]));
            let build = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror",
                    "-fsanitize=undefined", "-fno-sanitize-recover=all", opt,
                    "-I"])
                .arg(&dir.0)
                .arg(dir.0.join("module.c")).arg(dir.0.join("driver.c"))
                .arg("-o").arg(&exe).output().expect("Clang in Native Q03 gate");
            assert!(build.status.success(),
                "{label}/{opt}: linked C build failed: {}\n{}",
                String::from_utf8_lossy(&build.stderr), c.module_c);
            let run = Command::new(&exe).output().expect("execute native C artifact");
            assert!(run.status.success(),
                "{label}/{opt}: linked native program failed: {}",
                String::from_utf8_lossy(&run.stderr));
        }
    }

    let oracle = run_oracle_entry(&ir, "supported", &[], OracleConfig::default())
        .expect("supported builtin must execute in independent IR Oracle");
    assert!(oracle.ok, "independent Oracle must keep declared println behavior");
    assert!(oracle.stdout.contains("native call"), "{oracle:?}");
}
