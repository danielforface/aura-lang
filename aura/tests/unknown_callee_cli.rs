//! Issue #47: exercise the actual Aura CLI without Z3, not a mock checker.
#![cfg(not(feature = "z3"))]
use std::{path::{Path, PathBuf}, process::{Command, Output}, time::{SystemTime, UNIX_EPOCH}};

struct Fixture { dir: PathBuf, source: PathBuf }
impl Fixture {
    fn new(source: &str) -> Self {
        let ns = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
        let dir = std::env::temp_dir().join(format!("aura-q06-callee-{}-{ns}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let path = dir.join("main.aura");
        std::fs::write(&path, source).expect("fixture source");
        Self { dir, source: path }
    }
    fn command(&self, subcommand: &str, extra: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_aura"))
            .arg(subcommand)
            .arg(&self.source)
            .args(extra)
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root"))
            .output().expect("run actual Aura CLI")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.dir); }
}
fn reject_unknown(source: &str, name: &str) {
    let f = Fixture::new(source);
    for (command, extra) in [
        ("check", &[][..]),
        ("build", &["--mode", "hybrid", "--backend", "c", "--profile", "dev"][..]),
    ] {
        let out = f.command(command, extra);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(),
            "aura {command} accepted unregistered {name}: stdout={stdout}, stderr={stderr}");
        let msg = stderr.to_ascii_lowercase();
        assert!(msg.contains("unknown") && msg.contains(name),
            "aura {command} must reject missing callee by name, not for unrelated reasons: {stderr}");
    }
}
fn accept_check(source: &str) {
    let out = Fixture::new(source).command("check", &[]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(),
        "declared call must check: stdout={stdout}, stderr={}",
        String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains("formal verification NOT RUN"),
        "non-proving check must not claim a proof: {stdout}");
}
#[test]
fn unknown_direct_must_fail_check_and_c_build() {
    reject_unknown("cell main() ->:\n    never_declared()\n", "never_declared");
}
#[test]
fn unknown_nested_must_fail_check_and_c_build() {
    reject_unknown("cell main() ->:\n    if true:\n        missing_nested()\n", "missing_nested");
}
#[test]
fn unknown_qualified_must_fail_check_and_c_build() {
    reject_unknown("import aura::io\n\ncell main() ->:\n    io::ghost_unregistered()\n", "ghost_unregistered");
}
#[test]
fn declared_local_cell_must_pass_cli_check() {
    accept_check("cell helper() ->:\n    yield 42\n\ncell main() ->:\n    helper()\n");
}
#[test]
fn registered_builtin_must_pass_cli_check() {
    accept_check("import aura::io\n\ncell main() ->:\n    io::println(\"hello\")\n");
}
#[test]
fn declared_trusted_extern_must_pass_cli_check() {
    accept_check("trusted extern cell native_ping(x: u32) -> u32\n\ncell main() ->:\n    native_ping(1)\n");
}
#[test]
fn local_function_call_must_still_build_with_c_backend() {
    // Artifact build smoke, NOT execution of generated native code.
    let f = Fixture::new("cell helper() ->:\n    yield 42\n\ncell main() ->:\n    helper()\n");
    let out = f.command("build", &["--mode", "hybrid", "--backend", "c", "--profile", "dev"]);
    assert!(out.status.success(),
        "registered local call must still build C artifact; stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

#[test]
fn unknown_flow_literal_must_fail_public_check_and_c_build() {
    reject_unknown(
        "cell main() ->:\n    yield 1 -> unknown_flow()\n",
        "unknown_flow",
    );
}
#[test]
fn unknown_flow_named_local_must_fail_public_check_and_c_build() {
    reject_unknown(
        "cell main() ->:\n    val n: u32 = 1\n    n -> missing_flow()\n",
        "missing_flow",
    );
}
#[test]
fn unknown_flow_qualified_must_fail_public_check_and_c_build() {
    reject_unknown(
        "import aura::io\n\ncell main() ->:\n    yield \"hello\" -> io::ghost_flow()\n",
        "ghost_flow",
    );
}
#[test]
fn known_injected_local_flow_must_pass_public_check() {
    accept_check(
        "cell sink(n: u32) ->:\n    yield n\n\ncell main() ->:\n    yield 1 -> sink()\n",
    );
}
#[test]
fn known_injected_builtin_flow_must_pass_public_check() {
    accept_check(
        "import aura::io\n\ncell main() ->:\n    yield \"hello\" -> io::println()\n",
    );
}
#[test]
fn known_injected_local_flow_c_artifact_build_smoke() {
    // Artifact production only, not native binary execution.
    let f = Fixture::new(
        "cell sink(n: u32) ->:\n    yield n\n\ncell main() ->:\n    yield 1 -> sink()\n",
    );
    let output = f.command(
        "build",
        &["--mode", "hybrid", "--backend", "c", "--profile", "dev"],
    );
    assert!(output.status.success(),
        "declared injected Flow must still build C artifact; stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr));
}
