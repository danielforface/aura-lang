use std::io::Write;
use std::process::{Command, Stdio};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn debug_lines(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|l| l.starts_with("AURA_DEBUG_EVENT "))
        .map(|l| l["AURA_DEBUG_EVENT ".len()..].trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[test]
fn debug_protocol_smoke_hello_and_perf_report() {
    let aura = env!("CARGO_BIN_EXE_aura");

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("aura crate must be in a workspace");

    // Self-contained temp program (no stdlib imports).
    let mut tmp_dir = std::env::temp_dir();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    tmp_dir.push(format!("aura-debug-protocol-smoke-{}-{ts}", std::process::id()));
    std::fs::create_dir_all(&tmp_dir).expect("create temp dir");
    let prog_path = tmp_dir.join("main.aura");
    std::fs::write(
        &prog_path,
        "# debug protocol smoke (self-contained)\n\ncell main() ->:\n    val x: u32 = 1\n    val y: u32 = x + 2\n    val z: u32 = y * 3\n",
    )
    .expect("write temp program");

    let prog_arg = prog_path.to_str().expect("temp program path must be UTF-8");

    let mut child = Command::new(aura)
        .args(["run", prog_arg, "--mode", "avm"])
        .env("AURA_DEBUG_PROTOCOL", "1")
        .env("AURA_AVM_NO_Z3", "1")
        .current_dir(repo_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn aura");

    // Exercise protocol parse hardening: send an invalid line.
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"{not-json}\n");
        let _ = stdin.flush();
    }

    let out = child.wait_with_output().expect("wait aura");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Debug events are transport diagnostics, never evidence of proof.
    // The AVM explicitly reports verified=false when Z3 is disabled, and
    // the public CLI must reject that run while still publishing events.
    assert!(
        !out.status.success(),
        "no-Z3 Aura incorrectly reported verified execution: {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}\n",
        out.status, stdout, stderr
    );
    assert!(
        stderr.contains("avm: Z3 gate rejected execution: verification not run: Z3 gate disabled"),
        "missing explicit no-Z3 fail-closed diagnostic: status {:?}\nstdout: {}\nstderr: {}",
        out.status, stdout, stderr
    );

    let dbg = debug_lines(&stdout);
    assert!(!dbg.is_empty(), "expected debug events on stdout");

    let mut saw_hello = false;
    let mut saw_perf = false;

    for raw in dbg {
        let v: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("debug event must be valid JSON: {e}: {raw}"));
        let ev = v
            .get("event")
            .and_then(|x| x.as_str())
            .unwrap_or("<missing>");

        if ev == "hello" {
            saw_hello = true;
            assert_eq!(v.get("protocol").and_then(|x| x.as_u64()), Some(1));
            let caps = v
                .get("capabilities")
                .and_then(|x| x.as_array())
                .cloned()
                .unwrap_or_default();
            let caps: Vec<String> = caps
                .into_iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect();
            assert!(caps.contains(&"devvm.step".to_string()));
            assert!(caps.contains(&"native.terminate".to_string()));
            assert!(caps.contains(&"perf.timeline".to_string()));
        }

        if ev == "perfReport" {
            saw_perf = true;
        }
    }

    assert!(saw_hello, "expected hello event");
    assert!(saw_perf, "expected perfReport event");

    // The invalid line should not crash Aura; it should show up on stderr.
    assert!(
        stderr.contains("AURA_DEBUG_PROTOCOL: ignored invalid command"),
        "expected parse error log in stderr; got:\n{stderr}"
    );

    // The debug stream must remain parseable despite the expected gate failure.
    assert!(!stdout.contains("Error:"), "unexpected error in debug event stdout");
}


#[test]
fn no_z3_gate_rejects_execution_without_debug_protocol_as_well() {
    let aura = env!("CARGO_BIN_EXE_aura");
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("aura crate must be in a workspace");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let program = std::env::temp_dir().join(format!(
        "aura-no-z3-gate-{}-{nonce}.aura", std::process::id()
    ));
    std::fs::write(&program,
        "cell main() ->:\n    val x: u32 = 1\n    val y: u32 = x + 2\n"
    ).expect("write no-Z3 contract fixture");

    let result = Command::new(aura)
        .args(["run", program.to_str().expect("fixture path UTF-8"), "--mode", "avm"])
        .env("AURA_AVM_NO_Z3", "1")
        .env_remove("AURA_DEBUG_PROTOCOL")
        .current_dir(repo_root)
        .output();
    let _ = std::fs::remove_file(&program);
    let out = result.expect("start no-Z3 Aura without debug protocol");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // Ordinary CLI and debug transport must agree on verification authority:
    // disabling Z3 is NEVER proof of verified execution.
    assert!(!out.status.success(),
        "ordinary AVM run must reject disabled-Z3 verification: {out:?}");
    assert!(stderr.contains(
        "avm: Z3 gate rejected execution: verification not run: Z3 gate disabled"
    ), "ordinary run lost fail-closed gate diagnostic: {stderr}");
    assert!(!stdout.contains("AURA_DEBUG_EVENT "),
        "debug events appeared without enabling the debug protocol: {stdout}");
}
