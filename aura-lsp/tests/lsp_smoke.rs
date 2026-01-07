use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

fn file_uri_from_windows_path(path: &std::path::Path) -> String {
    // Convert C:\a\b to file:///C:/a/b
    let s = path
        .to_str()
        .expect("utf-8 path")
        .replace('\\', "/");
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        format!("file:///{s}")
    } else {
        format!("file://{s}")
    }
}

fn write_lsp(stdin: &mut impl Write, msg: &serde_json::Value) {
    let body = msg.to_string();
    let header = format!("Content-Length: {}\r\n\r\n", body.as_bytes().len());
    stdin.write_all(header.as_bytes()).unwrap();
    stdin.write_all(body.as_bytes()).unwrap();
    stdin.flush().unwrap();
}

fn read_lsp(reader: &mut BufReader<impl Read>) -> Option<serde_json::Value> {
    let mut content_len: Option<usize> = None;

    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).unwrap();
        if n == 0 {
            return None;
        }
        let line_trim = line.trim_end_matches(['\r', '\n']);
        if line_trim.is_empty() {
            break;
        }

        let lower = line_trim.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            let v = rest.trim();
            content_len = Some(v.parse::<usize>().unwrap());
        }
    }

    let len = content_len.expect("missing Content-Length header");
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).unwrap();
    Some(serde_json::from_slice(&buf).unwrap())
}

fn wait_for_response(
    rx: &mpsc::Receiver<serde_json::Value>,
    id: i64,
    timeout: Duration,
) -> serde_json::Value {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if std::time::Instant::now() >= deadline {
            panic!("timed out waiting for response id={id}");
        }
        let msg = rx.recv_timeout(Duration::from_millis(250)).unwrap();
        if msg.get("id").and_then(|v| v.as_i64()) == Some(id) {
            return msg;
        }
    }
}

fn wait_for_proofs_stream_done(
    rx: &mpsc::Receiver<serde_json::Value>,
    stream_id: i64,
    timeout: Duration,
) -> (Vec<String>, serde_json::Value) {
    let deadline = std::time::Instant::now() + timeout;
    let mut phases: Vec<String> = Vec::new();
    loop {
        if std::time::Instant::now() >= deadline {
            panic!("timed out waiting for proofs stream done id={stream_id}; phases={phases:?}");
        }
        let msg = rx.recv_timeout(Duration::from_millis(250)).unwrap();
        if msg.get("method").and_then(|m| m.as_str()) != Some("aura/proofsStream") {
            continue;
        }
        let params = msg.get("params").cloned().unwrap_or(serde_json::Value::Null);
        if params.get("id").and_then(|v| v.as_i64()) != Some(stream_id) {
            continue;
        }
        if params.get("state").and_then(|v| v.as_str()) == Some("phase") {
            if let Some(p) = params.get("phase").and_then(|v| v.as_str()) {
                phases.push(p.to_string());
            }
        }
        if params.get("state").and_then(|v| v.as_str()) == Some("done") {
            return (phases, params);
        }
        if params.get("state").and_then(|v| v.as_str()) == Some("error") {
            panic!("proofs stream error: {params:?}");
        }
    }
}

fn position_of_nth(haystack: &str, needle: &str, n: usize) -> (u32, u32) {
    assert!(!needle.is_empty());
    let mut count = 0usize;
    let mut idx = 0usize;
    loop {
        let Some(rel) = haystack[idx..].find(needle) else {
            panic!("needle not found: {needle}");
        };
        let at = idx + rel;
        if count == n {
            let before = &haystack[..at];
            let line = before.chars().filter(|&c| c == '\n').count() as u32;
            let col = before
                .chars()
                .rev()
                .take_while(|&c| c != '\n')
                .count() as u32;
            return (line, col);
        }
        count += 1;
        idx = at + needle.len();
    }
}

#[test]
fn aura_lsp_publishes_diagnostics_on_did_open() {
    // Create a minimal project so manifest discovery does not fail.
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("aura.toml"), "[project]\nname = \"Test\"\n").unwrap();

    let file_path = tmp.path().join("main.aura");
    std::fs::write(&file_path, "!!!\n").unwrap();
    let uri = file_uri_from_windows_path(&file_path);

    let exe = env!("CARGO_BIN_EXE_aura-lsp");
    let mut child = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    // Read loop on a thread so we can time out.
    let (tx, rx) = mpsc::channel::<serde_json::Value>();
    std::thread::spawn(move || {
        let mut r = BufReader::new(stdout);
        loop {
            match read_lsp(&mut r) {
                Some(msg) => {
                    let _ = tx.send(msg);
                }
                None => break,
            }
        }
    });

    // initialize
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": null,
                "capabilities": {}
            }
        }),
    );

    // Wait for initialize response (ignore any notifications).
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let now = std::time::Instant::now();
        if now >= deadline {
            panic!("timed out waiting for initialize response");
        }
        let msg = rx.recv_timeout(Duration::from_millis(250)).unwrap();
        if msg.get("id").and_then(|v| v.as_i64()) == Some(1) {
            break;
        }
    }

    // initialized notification
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }),
    );

    // didOpen
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "aura",
                    "version": 1,
                    "text": "!!!\n"
                }
            }
        }),
    );

    // Find publishDiagnostics notification.
    let mut published = None;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let msg = match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if msg.get("method").and_then(|m| m.as_str()) == Some("textDocument/publishDiagnostics") {
            published = Some(msg);
            break;
        }
    }

    let published = published.expect("did not receive publishDiagnostics");
    let params = published.get("params").expect("publishDiagnostics params");
    let diags = params
        .get("diagnostics")
        .and_then(|v| v.as_array())
        .expect("diagnostics array");

    assert!(!diags.is_empty(), "expected at least one diagnostic");

    // Clean shutdown.
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "shutdown",
            "params": null
        }),
    );

    // Wait for shutdown response (ignore any notifications).
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if std::time::Instant::now() >= deadline {
            panic!("timed out waiting for shutdown response");
        }
        let msg = rx.recv_timeout(Duration::from_millis(250)).unwrap();
        if msg.get("id").and_then(|v| v.as_i64()) == Some(2) {
            break;
        }
    }

    // Close the transport; tower-lsp servers typically exit when stdin closes.
    drop(stdin);

    let status = child.wait_timeout(Duration::from_secs(5)).unwrap();
    if !status.success() {
        let mut err = String::new();
        let _ = stderr.read_to_string(&mut err);
        panic!("aura-lsp exited non-zero: {status:?}\n--- stderr ---\n{err}");
    }
}

#[test]
fn aura_lsp_language_features_smoke() {
    // Create a minimal project so manifest discovery does not fail.
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("aura.toml"), "[project]\nname = \"Test\"\n").unwrap();

    let file_path = tmp.path().join("main.aura");
    let src = r#"cell main():
    val mut counter: u32 = 0
    while counter < 3 invariant counter < 1000:
        counter = counter + 1
"#;
    std::fs::write(&file_path, src).unwrap();
    let uri = file_uri_from_windows_path(&file_path);

    let exe = env!("CARGO_BIN_EXE_aura-lsp");
    let mut child = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let (tx, rx) = mpsc::channel::<serde_json::Value>();
    std::thread::spawn(move || {
        let mut r = BufReader::new(stdout);
        while let Some(msg) = read_lsp(&mut r) {
            let _ = tx.send(msg);
        }
    });

    // initialize
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": null,
                "capabilities": {}
            }
        }),
    );
    let _ = wait_for_response(&rx, 1, Duration::from_secs(5));

    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }),
    );

    // didOpen
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "aura",
                    "version": 1,
                    "text": src
                }
            }
        }),
    );

    // Go to definition: place cursor on 2nd occurrence of `counter` (in `while counter < 3`).
    let (line, col) = position_of_nth(src, "counter", 1);
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/definition",
            "params": {
                "textDocument": {"uri": uri},
                "position": {"line": line, "character": col}
            }
        }),
    );
    let def = wait_for_response(&rx, 2, Duration::from_secs(5));
    assert!(def.get("result").is_some(), "definition response has result");

    // References: should find multiple occurrences.
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "textDocument/references",
            "params": {
                "textDocument": {"uri": uri},
                "position": {"line": line, "character": col},
                "context": {"includeDeclaration": true}
            }
        }),
    );
    let refs = wait_for_response(&rx, 3, Duration::from_secs(5));
    let arr = refs
        .get("result")
        .and_then(|v| v.as_array())
        .expect("references array");
    assert!(arr.len() >= 2, "expected >=2 references");

    // Prepare rename.
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "textDocument/prepareRename",
            "params": {
                "textDocument": {"uri": uri},
                "position": {"line": line, "character": col}
            }
        }),
    );
    let prep = wait_for_response(&rx, 4, Duration::from_secs(5));
    assert!(prep.get("result").is_some(), "prepareRename result exists");

    // Rename.
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "textDocument/rename",
            "params": {
                "textDocument": {"uri": uri},
                "position": {"line": line, "character": col},
                "newName": "counter2"
            }
        }),
    );
    let ren = wait_for_response(&rx, 5, Duration::from_secs(5));
    let changes = ren
        .get("result")
        .and_then(|r| r.get("changes"))
        .and_then(|c| c.as_object())
        .expect("rename returns WorkspaceEdit.changes");
    assert!(!changes.is_empty(), "rename has edits");

    // Inlay hints.
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "textDocument/inlayHint",
            "params": {
                "textDocument": {"uri": uri},
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 10, "character": 0}
                }
            }
        }),
    );
    let ih = wait_for_response(&rx, 6, Duration::from_secs(5));
    assert!(ih.get("result").is_some(), "inlayHint returns a result");

    // Semantic tokens.
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "textDocument/semanticTokens/full",
            "params": {
                "textDocument": {"uri": uri}
            }
        }),
    );
    let st = wait_for_response(&rx, 7, Duration::from_secs(5));
    let data = st
        .get("result")
        .and_then(|r| r.get("data"))
        .and_then(|d| d.as_array())
        .expect("semanticTokens result.data");
    assert!(!data.is_empty(), "semanticTokens has some tokens");

    // Clean shutdown.
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "shutdown",
            "params": null
        }),
    );
    let _ = wait_for_response(&rx, 8, Duration::from_secs(5));
    drop(stdin);

    let status = child.wait_timeout(Duration::from_secs(5)).unwrap();
    if !status.success() {
        let mut err = String::new();
        let _ = stderr.read_to_string(&mut err);
        panic!("aura-lsp exited non-zero: {status:?}\n--- stderr ---\n{err}");
    }
}

#[test]
fn aura_lsp_proofs_stream_phases_include_normalize() {
    // Create a minimal project so manifest discovery does not fail.
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("aura.toml"), "[project]\nname = \"Test\"\n").unwrap();

    let file_path = tmp.path().join("main.aura");
    // A small, valid program.
    let src = r#"cell main():
    val mut counter: u32 = 0
    while counter < 3 invariant counter < 1000:
        counter = counter + 1
"#;
    std::fs::write(&file_path, src).unwrap();
    let uri = file_uri_from_windows_path(&file_path);

    let exe = env!("CARGO_BIN_EXE_aura-lsp");
    let mut child = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let (tx, rx) = mpsc::channel::<serde_json::Value>();
    std::thread::spawn(move || {
        let mut r = BufReader::new(stdout);
        loop {
            match read_lsp(&mut r) {
                Some(msg) => {
                    let _ = tx.send(msg);
                }
                None => break,
            }
        }
    });

    // initialize
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": null,
                "capabilities": {}
            }
        }),
    );
    let _ = wait_for_response(&rx, 1, Duration::from_secs(5));
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }),
    );

    // didOpen
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "aura",
                    "version": 1,
                    "text": src
                }
            }
        }),
    );

    // Start proofs stream.
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "aura/proofsStreamStart",
            "params": {"uri": uri}
        }),
    );

    let resp = wait_for_response(&rx, 2, Duration::from_secs(10));
    let stream_id = resp
        .get("result")
        .and_then(|r| r.get("id"))
        .and_then(|v| v.as_i64())
        .expect("proofsStreamStart result id");

    let (phases, done_params) = wait_for_proofs_stream_done(&rx, stream_id, Duration::from_secs(20));
    assert!(phases.contains(&"parse".to_string()), "missing parse phase: {phases:?}");
    assert!(phases.contains(&"sema".to_string()), "missing sema phase: {phases:?}");
    assert!(phases.contains(&"normalize".to_string()), "missing normalize phase: {phases:?}");
    assert!(phases.contains(&"z3".to_string()), "missing z3 phase: {phases:?}");
    assert_eq!(done_params.get("uri").and_then(|u| u.as_str()), Some(uri.as_str()));

    // Clean shutdown.
    write_lsp(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "shutdown",
            "params": null
        }),
    );
    let _ = wait_for_response(&rx, 3, Duration::from_secs(5));
    drop(stdin);

    let status = child.wait_timeout(Duration::from_secs(5)).unwrap();
    if !status.success() {
        let mut err = String::new();
        let _ = stderr.read_to_string(&mut err);
        panic!("aura-lsp exited non-zero: {status:?}\n--- stderr ---\n{err}");
    }
}

trait WaitTimeout {
    fn wait_timeout(&mut self, timeout: Duration) -> std::io::Result<std::process::ExitStatus>;
}

impl WaitTimeout for std::process::Child {
    fn wait_timeout(&mut self, timeout: Duration) -> std::io::Result<std::process::ExitStatus> {
        let start = std::time::Instant::now();
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
            if start.elapsed() >= timeout {
                // Best-effort kill to avoid hanging CI.
                let _ = self.kill();
                return self.wait();
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}
