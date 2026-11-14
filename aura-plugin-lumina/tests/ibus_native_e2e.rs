#![cfg(all(feature="lumina-diagnostics",target_os="linux"))]
// REAL live ibus-daemon + ibus-hangul engine proof. No CompositionEvent
// injection and no mocked IBus messages. XTest produces actual GLFW keys,
// Linux libibus ProcessKeyEvent runs engine, and native preedit/commit signal
// data returns into Lumina's focused edit session. Fail closed if no daemon.
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback,UiTextInputEvent};
use aura_plugin_lumina::AuraLuminaPlugin;
use std::process::Command;
fn node(kind:&str,props:&[(&str,&str)])->UiNode{
    let mut n=UiNode::new(kind);for &(k,v) in props{n.set_prop(k,v);}n
}
fn frame(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext)->Vec<UiTextInputEvent>{
    p.on_ui_render(t,c).expect("native owned").expect("render");
    c.get::<UiRuntimeFeedback>().expect("feedback").text_input_events.clone()
}
fn frames(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,n:usize)->Vec<UiTextInputEvent>{
    (0..n).flat_map(|_|frame(p,t,c)).collect()
}
fn xdo(args:&[&str])->String{
    let o=Command::new("xdotool").args(args).output().unwrap();
    assert!(o.status.success(),"XTest {args:?}: {}",String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}
fn key(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,k:&str)
    ->Vec<UiTextInputEvent>{
    xdo(&["keydown",k]);let mut out=frames(p,t,c,4);
    xdo(&["keyup",k]);out.extend(frames(p,t,c,4));out
}
#[test]fn real_ibus_hangul_engine_x11_preedit_and_commit_without_injection(){
    assert!(std::env::var_os("LUMINA_IBUS_HELPER_BIN").is_some(),
        "native IBus proof must compile a real libibus helper first");
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"hangul");
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","380"),("height","65"),
        ("value",""),("on_change","cb:936"),("size","28"),
        ("font_path","/usr/share/fonts/truetype/nanum/NanumGothic.ttf"),
        ("radius","0"),("fg","#EE3030")
    ]));
    assert!(frames(&p,&app,&mut c,4).is_empty());
    assert!(p.diagnostic_resize_window(640,420));
    assert!(frames(&p,&app,&mut c,4).is_empty());
    let window=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=window.lines().next().expect("Xvfb GLFW native window");
    xdo(&["windowfocus","--sync",id]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    assert_eq!(p.diagnostic_native_ibus(),Some(0),
        "the native libibus helper must have been launched and ready");
    let mut seen=Vec::new();
    seen.extend(key(&p,&app,&mut c,"r")); // Hangul ㄱ
    let first=p.diagnostic_composition(936);
    let first_count=p.diagnostic_native_ibus().unwrap();
    eprintln!("IBUS_NATIVE_REAL_ENGINE_AFTER_R first={first:?} signals={first_count} ev={seen:?}");
    assert!(first_count>=1,"MUST receive actual IBus engine preedit signal");
    assert!(first.is_some(),"IBus Hangul 'r' must have real preedit, not Latin keyboard insertion");
    assert!(seen.is_empty(),"preedit must not emit callback before commit");
    seen.extend(key(&p,&app,&mut c,"k")); // Hangul 가
    let updated=p.diagnostic_composition(936);
    eprintln!("IBUS_NATIVE_REAL_ENGINE_AFTER_K preedit={updated:?} signals={:?}",
        p.diagnostic_native_ibus());
    assert!(p.diagnostic_native_ibus().unwrap()>first_count,
        "second physical key must really update IBus preedit");
    assert!(updated.as_ref().is_some_and(|v|v.1.contains('가')),
        "live ibus-hangul must generate composed Korean syllable");
    assert!(seen.is_empty(),"no premature text callback during real Korean preedit");
    let vis=p.diagnostic_text_visual(936,32.0,356.0).unwrap();
    assert!(!vis.preedit_screens.is_empty(),
        "native IBus preedit should have real UI underline");
    let &(ua,ub)=vis.preedit_screens.first().unwrap();
    let underline=p.diagnostic_capture_pixel((ua+ub)*0.5,79.0).unwrap();
    assert!(underline.g>underline.r && underline.b>underline.g,
        "actual IBus-preedit GPU underline must be cyan: {underline:?}");
    eprintln!("IBUS_NATIVE_REAL_ENGINE_GPU_UNDERLINE {underline:?}");
    let paint=p.diagnostic_red_signature(32,24,110,54).unwrap();
    assert!(paint.0>5,"real Korean preedit glyph must be rendered on native GPU: {paint:?}");
    seen.extend(key(&p,&app,&mut c,"space"));
    seen.extend(frames(&p,&app,&mut c,5));
    eprintln!("IBUS_NATIVE_REAL_ENGINE_COMMIT events={seen:?} count={:?}",
        p.diagnostic_native_ibus());
    assert!(seen.iter().any(|e|e.callback_id==936&&e.text.contains('가')),
        "must receive actual engine CommitText, not manually injected text");
    assert!(seen.iter().all(|e|!e.text.contains("rk")),
        "no double raw physical Latin keyboard insertion");
    assert!(p.diagnostic_composition(936).is_none());
    drop(p);
}


/// Phase 26 second behavioral RED: revoking native IME sharing during a live
/// upstream preedit must also abandon the local projected, uncommitted text.
/// No synthetic CompositionEvent or helper D/C frames are injected here.
#[test]fn zzz_live_policy_change_during_active_ibus_preedit_abandons_it(){
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","380"),("height","65"),
        ("value","PUBLIC"),("value_revision","1"),
        ("on_change","cb:937"),("size","28"),
        ("font_path","/usr/share/fonts/truetype/nanum/NanumGothic.ttf")
    ]));
    assert!(frames(&p,&app,&mut c,4).is_empty());
    assert!(p.diagnostic_resize_window(640,420));
    assert!(frames(&p,&app,&mut c,4).is_empty());
    let window=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=window.lines().next().expect("real X11 window");
    xdo(&["windowfocus","--sync",id]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    assert!(key(&p,&app,&mut c,"r").is_empty());
    assert!(p.diagnostic_composition(937).is_some(),
        "registered upstream Hangul engine must create genuine preedit");
    app.children[0].set_prop("surrounding_text","none");
    assert!(frames(&p,&app,&mut c,5).is_empty());
    assert!(p.diagnostic_native_ibus_surrounding().is_none(),
        "revoked policy cannot preserve native surroundings");
    assert!(p.diagnostic_composition(937).is_none(),
        "old preedit must not keep the editor locked after revocation");
    let events=key(&p,&app,&mut c,"z");
    assert_eq!(events.len(),1,"direct local typing should resume after revocation");
    assert_eq!((events[0].callback_id,events[0].text.as_str()),(937,"PUBLICz"));
}

/// C/libibus boundary proof. A private native context starts with its
/// SURROUNDING_TEXT capability OFF and must reject an S frame outright.
/// This is controlled-protocol evidence, NOT engine-generated D/C evidence.
#[test]fn zzz_live_helper_rejects_unpermitted_surrounding_frame(){
    use std::{io::Write,process::Stdio};
    let bin=std::env::var_os("LUMINA_IBUS_HELPER_BIN").expect("built real C helper");
    let mut child=Command::new(bin).arg("hangul")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("registered live IBus");
    child.stdin.take().unwrap()
        .write_all(b"S\t43414e415259\t6\t6\n").unwrap();
    let out=child.wait_with_output().expect("helper must terminate on refusal");
    let stdout=String::from_utf8(out.stdout).unwrap();
    let stderr=String::from_utf8(out.stderr).unwrap();
    assert!(!out.status.success());
    assert!(stdout.contains("FATAL surrounding_not_authorized"),
        "native helper must refuse S before permission, got: {stdout}");
    assert!(!stdout.contains("43414e415259") && !stderr.contains("CANARY"),
        "helper refusal and diagnostics must not disclose even a test canary");
}


/// Phase29 authentic hard crash: actual SIGKILL on the session IBus daemon
/// while upstream Hangul has a live, *uncommitted* Unicode preedit.
#[test]fn zzzz_live_sigkill_daemon_during_active_preedit(){
    use std::time::Duration;
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"hangul");
    assert!(std::env::var_os("LUMINA_IBUS_TEST_HANGUL_SWITCH").is_some(),
        "the fresh upstream engine must start in preedit-capable Hangul mode");
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","380"),("height","65"),
        ("value","HARD-"),("value_revision","1"),("on_change","cb:940"),
        ("size","28"),("font_path","/usr/share/fonts/truetype/nanum/NanumGothic.ttf")
    ]));
    assert!(frames(&p,&app,&mut c,4).is_empty());
    assert!(p.diagnostic_resize_window(640,420));
    frames(&p,&app,&mut c,4);
    let window=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=window.lines().next().expect("real X11 window");
    xdo(&["windowfocus","--sync",id]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    assert_eq!(p.diagnostic_native_ibus_surrounding().unwrap().0,"HARD-");
    assert!(key(&p,&app,&mut c,"r").is_empty(),
        "genuine preedit must not change host authority");
    assert!(p.diagnostic_composition(940).is_some(),
        "true registered Hangul engine must own an uncommitted preedit");

    // Obtain the newest real session daemon PID, not the C helper PID.
    // CI runs this case in a dedicated dbus-run-session under Xvfb.
    let owner=Command::new("pgrep").args(["-n","-x","ibus-daemon"])
        .output().expect("Linux pgrep on real daemon");
    assert!(owner.status.success(),"cannot find live ibus-daemon process");
    let pid=String::from_utf8(owner.stdout).unwrap();
    let pid=pid.trim();
    assert!(!pid.is_empty()&&pid.bytes().all(|b|b.is_ascii_digit()));
    let killed=Command::new("kill").args(["-KILL",pid])
        .output().expect("actual Linux daemon SIGKILL");
    assert!(killed.status.success(),"real daemon must be terminated with SIGKILL");
    eprintln!("LUMINA_IBUS_P29_REAL_DAEMON_SIGKILL (pid redacted)");
    let after=frames(&p,&app,&mut c,20);
    assert!(after.is_empty(),"preedit is not an authoritative host change");
    assert!(p.diagnostic_native_ibus().is_none(),
        "daemon SIGKILL must poison old live C helper and drop authority");
    assert!(p.diagnostic_composition(940).is_none(),
        "old uncommitted preedit must be abandoned, never auto-committed");
    let fallback=key(&p,&app,&mut c,"z");
    assert_eq!(fallback.len(),1,"local typing remains available without daemon");
    assert_eq!((fallback[0].callback_id,fallback[0].text.as_str()),(940,"HARD-z"));
    assert!(p.diagnostic_composition(940).is_none());
    // Genuine delayed daemon return; no mock stream and no restarting the
    // helper directly. Fresh C helper must be created via capped Phase27
    // budget and reauthenticate the host context under Phase28 fencing.
    std::thread::sleep(Duration::from_millis(200));
    let daemon=Command::new("ibus-daemon").args([
        "--replace","--daemonize","--xim","--panel","disable","--cache=refresh"
    ]).output().expect("real ibus-daemon recovery");
    assert!(daemon.status.success(),"real daemon restart failed: {}",
        String::from_utf8_lossy(&daemon.stderr));
    let mut ready=false;
    for _ in 0..25{
        let set=Command::new("ibus").args(["engine","hangul"]).output();
        let check=Command::new("ibus").arg("engine").output();
        if set.as_ref().is_ok_and(|x|x.status.success()) &&
            check.as_ref().is_ok_and(|x|x.status.success() &&
            String::from_utf8_lossy(&x.stdout).trim()=="hangul"){ready=true;break;}
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(ready,"fresh live daemon must expose the requested Hangul engine");
    assert!(frames(&p,&app,&mut c,125).is_empty(),
        "a fresh daemon cannot replay stale preedit or D/C");
    assert_eq!(p.diagnostic_native_ibus(),Some(0),
        "a new native helper must be attached, never the old bus generation");
    let snap=p.diagnostic_native_ibus_surrounding()
        .expect("new daemon context must ACK current authoritative host text");
    assert_eq!(snap.0,"HARD-z");
    assert_eq!(p.diagnostic_native_ibus_replacements(),Some(0));
    assert!(p.diagnostic_native_ibus_deletion().is_none());
    // The first Korean character after recovery is a NEW preedit, not
    // previous uncommitted text or a second local Latin insertion.
    assert!(key(&p,&app,&mut c,"r").is_empty());
    assert!(p.diagnostic_composition(940).is_some(),
        "recovered native Hangul engine must create a NEW preedit");
    assert!(key(&p,&app,&mut c,"k").is_empty());
    let now=p.diagnostic_composition(940).expect("live recovered Hangul syllable");
    assert_eq!(now.0,"HARD-z","old preedit cannot leak into host buffer");
    assert!(now.1.contains('가'));
    let committed=key(&p,&app,&mut c,"space");
    eprintln!("LUMINA_IBUS_P29_REAL_DAEMON_SIGKILL_RESUMED events={committed:?}");
    assert!(committed.iter().any(|e|
        e.callback_id==940&&e.text.starts_with("HARD-z")&&e.text.contains('가')),
        "new daemon must commit real Hangul syllable while preserving fallback");
    assert!(committed.iter().all(|e|!e.text.contains("HARD-ㄱ")),
        "no orphaned pre-crash preedit should ever be committed");
    app.children[0].set_prop("surrounding_text","none");
    assert!(frames(&p,&app,&mut c,5).is_empty());
    assert!(p.diagnostic_native_ibus_surrounding().is_none(),
        "new daemon must still respect phase26 private input policy");
}
