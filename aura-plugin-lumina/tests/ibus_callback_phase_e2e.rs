#![cfg(all(feature="lumina-diagnostics", target_os="linux"))]
// Phase34: REAL registered ibus-hangul and actual libibus helper, no synthetic
// D/C/P, no fake bus, no injected CompositionEvent. This test deliberately
// separates helper-local dispatch timing from external engine provenance.
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

fn request(stdin:&mut ChildStdin, rx:&Receiver<String>, cmd:&str, end:&str)
    ->Vec<String>{
    writeln!(stdin,"{cmd}").expect("real helper stdin");
    stdin.flush().expect("flush real helper");
    let mut lines=Vec::new();
    for _ in 0..48 {
        let line=rx.recv_timeout(Duration::from_secs(8))
            .expect("bounded real native libibus response");
        assert!(line.len()<=4096,"bounded libibus wire frame");
        let done=if end=="A"{line=="A"}else{line=="R\t0"||line=="R\t1"};
        lines.push(line);
        if done{return lines;}
    }
    panic!("native helper missing {end} within bounded line limit");
}
fn content_hex(text:&str)->String{
    text.as_bytes().iter().map(|c|format!("{c:02x}")).collect()
}
fn commit_values(lines:&[String])->Vec<String>{
    lines.iter().filter_map(|line|line.strip_prefix("C\t")).map(|s|{
        assert!(s.len()%2==0 && s.len()<=2048,"bounded commit hex");
        let bytes=s.as_bytes().chunks_exact(2).map(|pair|
            u8::from_str_radix(std::str::from_utf8(pair).expect("ascii"),16)
                .expect("valid hex")).collect::<Vec<_>>();
        String::from_utf8(bytes).expect("valid upstream UTF-8")
    }).collect()
}
#[test]
fn real_registered_hangul_callbacks_have_measured_local_dispatch_phase(){
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"hangul");
    assert_eq!(std::env::var("LUMINA_IBUS_P34_DIAGNOSTIC_PHASE").unwrap(),"1");
    let bin=std::env::var_os("LUMINA_IBUS_HELPER_BIN").expect("real diagnostic C helper");
    let mut child=Command::new(bin).arg("hangul")
        .stdin(Stdio::piped()).stdout(Stdio::piped())
        .stderr(Stdio::piped()).spawn().expect("REAL libibus context");
    let mut input=child.stdin.take().expect("native stdin");
    let stdout=child.stdout.take().expect("native stdout");
    let mut stderr=child.stderr.take().expect("native stderr");
    let (tx,rx)=mpsc::sync_channel(64);
    let stdout_thread=std::thread::spawn(move||{
        for line in BufReader::new(stdout).lines(){
            let Ok(line)=line else{break;};
            if tx.send(line).is_err(){break;}
        }
    });
    let stderr_thread=std::thread::spawn(move||{
        let mut logs=String::new();
        stderr.read_to_string(&mut logs).expect("bounded diagnostic stderr");
        logs
    });
    assert_eq!(rx.recv_timeout(Duration::from_secs(8)).unwrap(),"READY");
    assert!(request(&mut input,&rx,"V1","A").last().is_some_and(|l|l=="A"));
    assert!(request(&mut input,&rx,"F1","A").last().is_some_and(|l|l=="A"));
    assert!(request(&mut input,&rx,&format!("S\t{}\t4\t4",content_hex("BASE")),"A")
        .last().is_some_and(|l|l=="A"));
    // Real libibus ProcessKeyEvent('r'), not a synthetic CommitText.
    let first=request(&mut input,&rx,"K\t114\t0\t0\t0","R");
    eprintln!("LUMINA_IBUS_P34_TRUE_KEY_FIRST frames={first:?}");
    assert_eq!(first.first().map(String::as_str),Some("B"));
    assert_eq!(first.last().map(String::as_str),Some("R\t1"));
    let first_commits=commit_values(&first);
    assert_eq!(first_commits,vec!["ㄱ"],"real registered Hangul first syllable");
    // ACK the host-owned authoritative text, then a second real key: k.
    assert!(request(&mut input,&rx,
        &format!("S\t{}\t5\t5",content_hex("BASEㄱ")),"A")
        .last().is_some_and(|l|l=="A"));
    let second=request(&mut input,&rx,"K\t107\t0\t0\t0","R");
    eprintln!("LUMINA_IBUS_P34_TRUE_KEY_SECOND frames={second:?}");
    assert_eq!(second.first().map(String::as_str),Some("B"));
    assert_eq!(second.last().map(String::as_str),Some("R\t1"));
    assert!(second.iter().any(|line|line=="D\t-1\t1"),
        "genuine upstream DeleteSurroundingText must arrive between B and R");
    assert_eq!(commit_values(&second),vec!["가"],
        "genuine Hangul update to previous committed syllable");
    // Neutral idle health probe only: engine signals, if any, are observed,
    // never treated as a third key or asserted absent at all possible times.
    let idle=request(&mut input,&rx,"T","A");
    eprintln!("LUMINA_IBUS_P34_IDLE_OBSERVED frames={idle:?}");
    drop(input);
    let status=child.wait().expect("real helper exit");
    stdout_thread.join().expect("reader thread");
    let logs=stderr_thread.join().expect("stderr thread");
    assert!(status.success(),"real helper exited: {status:?}");
    // Required only for the Phase34 compile-time diagnostic helper. No
    // source text or secret surrounding data may appear in phase records.
    let phase_lines=logs.lines().filter(|s|s.starts_with("LUMINA_IBUS_P34_CALLBACK ")).collect::<Vec<_>>();
    eprintln!("LUMINA_IBUS_P34_PHASE_OBSERVATIONS {phase_lines:?}");
    assert!(phase_lines.iter().any(|s|s.contains("kind=C ")),
        "true upstream CommitText callback must receive local phase metadata");
    assert!(phase_lines.iter().any(|s|s.contains("kind=D ")),
        "true upstream DeleteSurroundingText callback must receive local phase metadata");
    assert!(phase_lines.iter().all(|s|
        s.contains("phase=CALL")||s.contains("phase=PUMP")||
        s.contains("phase=IDLE")),
        "unknown diagnostic phase markers are invalid");
    eprintln!("LUMINA_IBUS_P34_GENUINE_SIGNAL_PHASE_PROOF first_C={} second_C={} real_D=true phase_records={}",
        first_commits.len(),commit_values(&second).len(),phase_lines.len());
}

/// Phase35: genuine registered engine callbacks across the *post-R* idle
/// windows (0/25/200ms). The new 'I<TAB>ms' command exists ONLY in a
/// separately compiled diagnostic C helper, never in a production helper.
/// Idle C/D callbacks, if observed, are reported rather than assigned a
/// fabricated engine transaction ID or claimed impossible.
#[test]
fn p35_real_registered_hangul_post_r_idle_window_observations(){
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"hangul");
    assert_eq!(std::env::var("LUMINA_IBUS_P35_IDLE_DIAGNOSTIC").unwrap(),"1");
    let bin=std::env::var_os("LUMINA_IBUS_HELPER_BIN").expect("test-only helper");
    let mut child=Command::new(bin).arg("hangul")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("REAL registered IBus child");
    let mut stdin=child.stdin.take().expect("helper input");
    let stdout=child.stdout.take().expect("helper stdout");
    let mut stderr=child.stderr.take().expect("helper stderr");
    let (tx,rx)=mpsc::sync_channel(64);
    let reader=std::thread::spawn(move||{
        for item in BufReader::new(stdout).lines(){
            let Ok(line)=item else{break;};
            if tx.send(line).is_err(){break;}
        }
    });
    let err_reader=std::thread::spawn(move||{
        let mut out=String::new();
        stderr.read_to_string(&mut out).expect("bounded diagnostic text");
        out
    });
    assert_eq!(rx.recv_timeout(Duration::from_secs(8)).unwrap(),"READY");
    assert_eq!(request(&mut stdin,&rx,"V1","A").last().map(String::as_str),Some("A"));
    assert_eq!(request(&mut stdin,&rx,"F1","A").last().map(String::as_str),Some("A"));
    let mut totals=Vec::new();
    for (round,ms) in [0u32,25,200].into_iter().enumerate(){
        // FocusOut must reset the real engine for an independent genuine
        // Hangul r/k pair. Do not fake D/C or inject CompositionEvent.
        if round>0 {
            assert_eq!(request(&mut stdin,&rx,"F0","A").last().map(String::as_str),Some("A"));
            assert_eq!(request(&mut stdin,&rx,"F1","A").last().map(String::as_str),Some("A"));
        }
        assert_eq!(request(&mut stdin,&rx,
            &format!("S\t{}\t4\t4",content_hex("BASE")),"A")
            .last().map(String::as_str),Some("A"));
        let first=request(&mut stdin,&rx,"K\t114\t0\t0\t0","R");
        let first_b=first.iter().position(|s|s=="B").expect("helper BEGIN marker");
        assert_eq!(first.iter().filter(|s|s.as_str()=="B").count(),1);
        assert_eq!(first.last().map(String::as_str),Some("R\t1"));
        let before_first=&first[..first_b];
        let current_first=&first[first_b+1..first.len()-1];
        assert_eq!(commit_values(current_first),vec!["ㄱ"],
            "real first registered Hangul CommitText within bounded B..R");
        assert_eq!(request(&mut stdin,&rx,
            &format!("S\t{}\t5\t5",content_hex("BASEㄱ")),"A")
            .last().map(String::as_str),Some("A"));
        let second=request(&mut stdin,&rx,"K\t107\t0\t0\t0","R");
        let second_b=second.iter().position(|s|s=="B").expect("next helper BEGIN");
        assert_eq!(second.iter().filter(|s|s.as_str()=="B").count(),1);
        assert_eq!(second.last().map(String::as_str),Some("R\t1"));
        let before_second=&second[..second_b];
        let current_second=&second[second_b+1..second.len()-1];
        assert!(current_second.iter().any(|s|s=="D\t-1\t1"),
            "real upstream D(-1,1) must be delivered with next key");
        assert_eq!(commit_values(current_second),vec!["가"],
            "real upstream D/C must remain a single corrected syllable");
        // The helper-owned idle probe only drains REAL GLib events. It does
        // NOT emit a B or R, and never fabricates an upstream D or C.
        let idle=request(&mut stdin,&rx,&format!("I\t{ms}"),"A");
        assert_eq!(idle.last().map(String::as_str),Some("A"));
        assert!(!idle.iter().any(|s|s=="B"||s.starts_with("R\t")),
            "read-only idle probe must never impersonate a key transaction");
        let late=idle.iter().filter(|s|s.starts_with("D\t")||s.starts_with("C\t"))
            .count();
        let pre_b=before_first.iter().chain(before_second.iter())
            .filter(|s|s.starts_with("D\t")||s.starts_with("C\t")).count();
        eprintln!("LUMINA_IBUS_P35_GENUINE_IDLE_ROUND round={round} duration_ms={ms} pre_B_DC={pre_b} post_R_idle_DC={late} idle_frames={} first_key_frames={} second_key_frames={}",
            idle.len(),first.len(),second.len());
        totals.push((ms,pre_b,late));
    }
    drop(stdin);
    assert!(child.wait().expect("genuine helper termination").success());
    reader.join().expect("real helper stdout");
    let diagnostics=err_reader.join().expect("real helper stderr");
    let measured=diagnostics.lines().filter(|s|
        s.starts_with("LUMINA_IBUS_P34_CALLBACK ")).collect::<Vec<_>>();
    assert!(measured.len()>=12,
        "three genuine r/k pairs must produce at least 12 real D/C callback observations");
    assert!(measured.iter().all(|s|
        (s.contains("kind=C ")||s.contains("kind=D ")) &&
        (s.contains("phase=CALL")||s.contains("phase=PUMP")||s.contains("phase=IDLE"))),
        "every diagnostic callback must have a bounded local phase only");
    assert!(!diagnostics.contains("BASE") && !diagnostics.contains("42415345"),
        "diagnostic phase logs must redact test-host authoritative text");
    eprintln!("LUMINA_IBUS_P35_POST_R_WINDOW_SAMPLES {totals:?} actual_callback_records={}",
        measured.len());
    // Deliberately NO 'late == 0' universal assertion: a real late C/D
    // would be reported, not discarded from the research result.
}

/// Diagnostic helper safety gate: an unbounded idle pump MUST be refused.
/// The command is test-only and not recognized in the production binary.
#[test]
fn p35_diagnostic_idle_probe_refuses_over_budget(){
    assert_eq!(std::env::var("LUMINA_IBUS_P35_IDLE_DIAGNOSTIC").unwrap(),"1");
    let binary=std::env::var_os("LUMINA_IBUS_HELPER_BIN").expect("test-only helper");
    let mut child=Command::new(binary).arg("hangul")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("real IBus test helper");
    let mut input=child.stdin.take().expect("stdin");
    writeln!(input,"I\t201").expect("bad >200ms diagnostic request");
    drop(input);
    let out=child.wait_with_output().expect("bounded refusal output");
    let data=String::from_utf8(out.stdout).expect("utf8 wire");
    assert!(!out.status.success(),"over-budget diagnostic must fail closed");
    assert!(data.contains("READY")&&data.contains("FATAL invalid_idle_probe"),
        "invalid diagnostic duration must be rejected without dispatch");
}

