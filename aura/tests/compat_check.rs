//! Q01 compatibility: explicitly distinguish syntax/type-checking from proof.
//! All tests invoke the actual Aura CLI without the optional Z3 feature.

#![cfg(not(feature = "z3"))]
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture { dir: PathBuf, path: PathBuf }
impl Fixture {
    fn new(source: &str) -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
        let dir = std::env::temp_dir().join(format!("aura-check-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("make fixture directory");
        let path = dir.join("main.aura");
        std::fs::write(&path, source).expect("write fixture");
        Self { dir, path }
    }
    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_aura"))
            .args(args)
            .arg(&self.path)
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root"))
            .output().expect("execute aura")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.dir); }
}
#[test]
fn check_accepts_type_correct_false_assertion_without_claiming_proof() {
    let f = Fixture::new("cell main() ->:\n    val x: u32 = 1\n    assert x == 2\n");
    let output = f.cli(&["check"]);
    assert!(output.status.success(), "check is a non-proving semantic check: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    assert!(stdout.contains("formal verification not run"), "no false proof claims: {stdout}");
    assert!(!stdout.contains("verified"), "check must not report verified: {stdout}");
}
#[test]
fn check_rejects_invalid_syntax() {
    let f = Fixture::new("cell main( ->:\n    val x: u32 = 1\n");
    let out = f.cli(&["check"]);
    assert!(!out.status.success(), "malformed syntax must fail even without Z3");
}
#[test]
fn avm_build_still_fails_closed_without_z3() {
    let f = Fixture::new("cell main() ->:\n    val x: u32 = 1\n    assert x == 2\n");
    let output = f.cli(&["build", "--mode", "avm"]);
    assert!(!output.status.success(), "build --mode avm must not claim proof when unavailable: stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert!(
        String::from_utf8_lossy(&output.stderr).to_ascii_lowercase().contains("z3"),
        "missing solver should be explicit"
    );
}
