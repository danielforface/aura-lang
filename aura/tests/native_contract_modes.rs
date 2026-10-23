//! AURA-Q01-003/004: native build must not silently erase unverified assertions.
//! Test runs the *real* CLI without z3; it is intentionally RED on the baseline.

#![cfg(not(feature = "z3"))]
use std::{path::{Path, PathBuf}, process::{Command, Output}, time::{SystemTime, UNIX_EPOCH}};

struct Fixture { dir: PathBuf, src: PathBuf }
impl Fixture {
    fn new(body: &str) -> Self {
        let id = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
        let dir = std::env::temp_dir().join(format!("aura-native-contract-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let src = dir.join("main.aura");
        std::fs::write(&src, format!("cell main() ->:\n{body}")).expect("fixture source");
        Self { dir, src }
    }
    fn build(&self, profile: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_aura"))
            .args(["build", self.src.to_str().expect("utf8 source"), "--mode", "hybrid", "--backend", "c", "--profile", profile])
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace"))
            .output().expect("aura binary")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.dir); }
}
fn reject_unverified_native(profile: &str) {
    let f = Fixture::new("    val x: u32 = 1\n    assert x == 2\n");
    let out = f.build(profile);
    assert!(
        !out.status.success(),
        "native --profile {profile} must not silently erase a false assert; status={:?}; stdout={}; stderr={}",
        out.status, String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr).to_ascii_lowercase();
    assert!(
        stderr.contains("verif") || stderr.contains("assert") || stderr.contains("contract") || stderr.contains("proof") || stderr.contains("z3"),
        "rejection must explain assurance failure, not a compiler/fixture error: {stderr}"
    );
}
#[test] fn dev_native_build_must_not_erase_false_assert() { reject_unverified_native("dev"); }
#[test] fn release_native_build_must_not_erase_false_assert() { reject_unverified_native("release"); }
#[test]
fn contract_free_dev_native_build_remains_available_without_z3() {
    let f = Fixture::new("    val x: u32 = 1\n    val y: u32 = x + 2\n");
    let out = f.build("dev");
    assert!(out.status.success(), "contract-free dev build should succeed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}
