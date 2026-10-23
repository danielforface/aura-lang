//! AURA-Q01-001/002: the public verification command must fail closed without Z3.
//! These cases intentionally run the real binary with --no-default-features.
//! See docs/qualification/MASTER_ROADMAP_V1.md and GitHub issue #10.

#![cfg(not(feature = "z3"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempAuraProgram {
    dir: PathBuf,
    source: PathBuf,
}

impl TempAuraProgram {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "aura-no-z3-verification-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create test directory");
        let source = dir.join("main.aura");
        // Parsing and type-checking must succeed; formal proof must not.
        std::fs::write(
            &source,
            "cell main() ->:\n    val x: u32 = 1\n    assert x == 2\n",
        )
        .expect("write Aura test fixture");
        Self { dir, source }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_aura"))
            .arg("verify")
            .arg(&self.source)
            .args(args)
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root"))
            .output()
            .expect("execute Aura CLI")
    }
}

impl Drop for TempAuraProgram {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn verify_without_z3_exits_nonzero_instead_of_claiming_success() {
    let program = TempAuraProgram::new();
    let output = program.run(&[]);
    assert!(
        !output.status.success(),
        "aura verify without z3 must fail; got exit {:?}, stdout={}, stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    assert!(
        stderr.contains("z3") && (stderr.contains("unavailable") || stderr.contains("without")),
        "missing solver error must be explicit, stderr={stderr}"
    );
}

#[test]
fn verify_report_without_z3_is_failure_not_false_proof() {
    let program = TempAuraProgram::new();
    let report_path = program.dir.join("proof.json");
    let output = program.run(&["--report", report_path.to_str().expect("UTF-8 temp path")]);
    assert!(
        !output.status.success(),
        "aura verify --report without z3 must fail; got stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let json: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&report_path).expect("failed verify must write a report"),
    )
    .expect("verification report must be valid JSON");
    assert_eq!(json["ok"], false, "not-run cannot be represented as proved");
    assert!(
        json["verify"].is_null(),
        "not-run must have no invented verification evidence"
    );
    assert!(
        json["error"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase()
            .contains("z3"),
        "report must identify missing verifier: {json}"
    );
}
