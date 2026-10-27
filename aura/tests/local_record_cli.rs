//! Q03-001: actual public Aura no-Z3 command path, not just emit_module().
#![cfg(not(feature = "z3"))]
use std::{fs, path::{Path, PathBuf}, process::{Command, Output}, time::{SystemTime, UNIX_EPOCH}};
struct SourceFile { root: PathBuf, file: PathBuf }
impl SourceFile {
    fn new(text: &str) -> Self {
        let tick = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("aura-record-cli-{}-{tick}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let file = root.join("main.aura");
        fs::write(&file, text).unwrap();
        Self { root, file }
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_aura"))
            .arg(args[0]).arg(&self.file).args(&args[1..])
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .output().expect("run real Aura no-Z3 CLI binary")
    }
}
impl Drop for SourceFile {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); }
}
#[test]
fn genuine_local_scalar_record_passes_check_and_c_artifact_build_without_z3() {
    let fixture = SourceFile::new(r#"
type Point = record {
    x: u32,
    y: u32,
}
cell main() ->:
    val p: Point = Point { y: 13, x: 7 }
    yield p.x
"#);
    let check = fixture.run(&["check"]);
    assert!(check.status.success(),
        "source record check failed; stdout={} stderr={}",
        String::from_utf8_lossy(&check.stdout), String::from_utf8_lossy(&check.stderr));
    assert!(String::from_utf8_lossy(&check.stdout).contains("formal verification NOT RUN"),
        "no Z3 check must not assert proof");
    let build = fixture.run(&["build", "--mode", "hybrid", "--backend", "c", "--profile", "dev"]);
    assert!(build.status.success(),
        "source Record C build failed; stdout={} stderr={}",
        String::from_utf8_lossy(&build.stdout), String::from_utf8_lossy(&build.stderr));
}
