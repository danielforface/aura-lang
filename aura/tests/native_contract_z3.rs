//! AURA-Q01-003/004: actual native C build must honor Z3 proof result.
//! Separate from no-Z3 tests, because a successful proof is not a typecheck.

#![cfg(feature = "z3")]

use std::{path::{Path, PathBuf}, process::{Command, Output}, time::{SystemTime, UNIX_EPOCH}};

struct Fixture { dir: PathBuf, source: PathBuf }
impl Fixture {
    fn new(assertion: &str) -> Self {
        let id = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
        let dir = std::env::temp_dir().join(format!("aura-native-z3-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture directory");
        let source = dir.join("main.aura");
        std::fs::write(&source, format!(
            "cell main() ->:\n    val x: u32 = 1\n    assert {assertion}\n"
        )).expect("fixture code");
        Self { dir, source }
    }
    fn build(&self, profile: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_aura"))
            .arg("build").arg(&self.source)
            .args(["--mode", "hybrid", "--backend", "c", "--profile", profile])
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace"))
            .output().expect("launch Aura")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.dir); }
}
#[test]
fn native_dev_with_z3_rejects_unprovable_assertion() {
    let fixture = Fixture::new("x == 2");
    let out = fixture.build("dev");
    assert!(!out.status.success(), "Z3 must reject false assertion; stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}
#[test]
fn native_dev_with_z3_accepts_proved_assertion() {
    let fixture = Fixture::new("x == 1");
    let out = fixture.build("dev");
    assert!(out.status.success(), "Z3 should prove true assertion; stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}
#[test]
fn native_release_with_z3_rejects_unprovable_assertion() {
    let fixture = Fixture::new("x == 2");
    let out = fixture.build("release");
    assert!(!out.status.success(), "Z3 must reject false assertion in release; stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}
