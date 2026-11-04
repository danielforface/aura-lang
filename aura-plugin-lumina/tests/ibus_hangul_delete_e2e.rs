#![cfg(all(feature="lumina-diagnostics",target_os="linux"))]
// Third-party REGISTERED ibus-hangul engine running under the REAL ibus-daemon.
// The upstream engine's "none" preedit mode calls genuine
// ibus_engine_delete_surrounding_text() when composing a following syllable.
// Physical X11 XTest -> Raylib -> libibus input-context -> IBus daemon ->
// upstream Hangul engine -> libibus DeleteSurroundingText signal -> helper ->
// bounded D frame -> read-only native diagnostics. NEVER inject D frames.
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback,UiTextInputEvent};
use aura_plugin_lumina::AuraLuminaPlugin;
use std::process::Command;

fn node(kind:&str,props:&[(&str,&str)])->UiNode{
    let mut n=UiNode::new(kind);
    for &(k,v) in props{n.set_prop(k,v);} n
}
fn frame(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext)->Vec<UiTextInputEvent>{
    p.on_ui_render(t,c).expect("native owned").expect("render");
    c.get::<UiRuntimeFeedback>().expect("feedback").text_input_events.clone()
}
fn frames(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,n:usize)->Vec<UiTextInputEvent>{
    (0..n).flat_map(|_|frame(p,t,c)).collect()
}
fn xdo(args:&[&str])->String{
    let x=Command::new("xdotool").args(args).output().unwrap();
    assert!(x.status.success(),"XTest {args:?}: {}",String::from_utf8_lossy(&x.stderr));
    String::from_utf8_lossy(&x.stdout).trim().to_string()
}
fn key(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,k:&str)->Vec<UiTextInputEvent>{
    xdo(&["keydown",k]);let mut out=frames(p,t,c,5);
    xdo(&["keyup",k]);out.extend(frames(p,t,c,5));out
}
#[test]fn real_hangul_none_preedit_engine_emits_delete_surrounding(){
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"hangul");
    assert!(std::env::var_os("LUMINA_IBUS_HELPER_BIN").is_some());
    assert!(std::env::var_os("LUMINA_IBUS_TEST_HANGUL_SWITCH").is_none(),
        "the real Hangul engine must start in native Korean input mode");
    let p=AuraLuminaPlugin::new();
    let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","440"),("height","70"),
        ("value","ANCHOR"),("value_revision","1"),("on_change","cb:950"),
        ("size","28"),("font_path","/usr/share/fonts/truetype/nanum/NanumGothic.ttf"),
    ]));
    frames(&p,&app,&mut c,4);
    assert!(p.diagnostic_resize_window(760,520));
    frames(&p,&app,&mut c,4);
    let window=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=window.lines().next().expect("real X11 native window");
    xdo(&["windowfocus","--sync",id]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    let before=p.diagnostic_native_ibus_surrounding()
        .expect("real IBus surrounding text ACK before Hangul keys");
    assert_eq!((before.0.as_str(),before.1,before.2),("ANCHOR",6,6));
    assert_eq!(before.5,0);
    let mut callbacks=Vec::new();
    let mut first_delete=None;
    for k in ["r","k","s","k","r","k","t","k"] {
        callbacks.extend(key(&p,&app,&mut c,k));
        let deletions=p.diagnostic_native_ibus_deletion();
        let surrounding=p.diagnostic_native_ibus_surrounding();
        eprintln!("LUMINA_IBUS_P24_REGISTERED_HANGUL key={k:?} delete={deletions:?} context={surrounding:?} callbacks={callbacks:?}");
        if first_delete.is_none(){first_delete=deletions;}
    }
    let (count,offset,nchars)=p.diagnostic_native_ibus_deletion()
        .expect("MUST receive actual upstream Hangul DeleteSurroundingText via live daemon, not injected D");
    eprintln!("LUMINA_IBUS_P24_NATIVE_D_PROOF count={count} offset={offset} nchars={nchars} first={first_delete:?} callbacks={callbacks:?}");
    assert!(count>=1,"actual IBus engine must emit a bounded D signal");
    assert!(offset<0&&nchars>=1&&nchars<=1024,
        "upstream Hangul replacement must delete prior composed scalar(s)");
    assert!(callbacks.iter().any(|x|x.text.starts_with("ANCHOR")),
        "only genuine CommitText, not D, may emit an authoritative callback");
    assert!(callbacks.iter().all(|x|x.callback_id==950&&x.text.starts_with("ANCHOR")),
        "advisory-only D request must never delete committed ANCHOR prefix");
    assert!(p.diagnostic_composition(950).is_none(),
        "none preedit mode must not fabricate a CompositionEvent preview");
    // The D record must NOT survive loss of native engine focus even if a
    // new field later reuses the same keyboard. No stale cached delete token.
    xdo(&["mousemove","--window",id,"710","470"]);
    xdo(&["mousedown","1"]);
    assert!(frames(&p,&app,&mut c,5).is_empty());
    xdo(&["mouseup","1"]);
    assert!(frames(&p,&app,&mut c,5).is_empty());
    assert!(p.diagnostic_native_ibus_deletion().is_none(),
        "the last engine-origin D request must be discarded on FocusOut");
    eprintln!("LUMINA_IBUS_P24_FOCUS_OUT_CLEARED real_deletion_advisory_only");
}

#[test]fn real_hangul_none_preedit_atomic_replace_and_undo(){
    // The installed upstream engine sends genuine D(-1,1) before each C.
    // Each real physical key must cause exactly ONE host change and one
    // grapheme-safe EditSession undo snapshot, never duplicate old syllables.
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"hangul");
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","440"),("height","70"),
        ("value","ANCHOR"),("value_revision","1"),("on_change","cb:951"),
        ("size","28"),("font_path","/usr/share/fonts/truetype/nanum/NanumGothic.ttf"),
    ]));
    assert!(frames(&p,&app,&mut c,4).is_empty());
    assert!(p.diagnostic_resize_window(760,520));
    assert!(frames(&p,&app,&mut c,4).is_empty());
    let w=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=w.lines().next().expect("X11 window");
    xdo(&["windowfocus","--sync",id]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    let keys=["r","k","s","k","r","k","t","k"];
    let expected=["ANCHORㄱ","ANCHOR가","ANCHOR간","ANCHOR가나",
        "ANCHOR가낙","ANCHOR가나가","ANCHOR가나갓","ANCHOR가나가사"];
    for (i,k) in keys.iter().enumerate(){
        let callbacks=key(&p,&app,&mut c,k);
        let live=p.diagnostic_native_ibus_deletion();
        let replacements=p.diagnostic_native_ibus_replacements();
        eprintln!("LUMINA_IBUS_P25_ATOMIC_REPLACE key={k:?} expected={:?} callbacks={callbacks:?} engine_delete={live:?} paired={replacements:?}",
            expected[i]);
        assert_eq!(callbacks.len(),1,"each real engine D+C key must create ONE callback");
        assert_eq!(callbacks[0].callback_id,951);
        assert_eq!(callbacks[0].text,expected[i],"no duplicate Hangul syllables");
        if i>0{
            assert!(replacements.unwrap_or(0)>=i,
                "same-key genuine IBus D→C replacement provenance required");
        }
    }
    assert_eq!(p.diagnostic_native_ibus_replacements(),Some(7));
    let last=p.diagnostic_native_ibus_surrounding().unwrap();
    assert_eq!(last.0,expected[7]);
    assert_eq!(last.1 as usize,last.0.chars().count());
    let mut press_ctrl=|letter:&str|{
        xdo(&["keydown","ctrl"]);frames(&p,&app,&mut c,4);
        xdo(&["keydown",letter]);let mut events=frames(&p,&app,&mut c,6);
        xdo(&["keyup",letter]);events.extend(frames(&p,&app,&mut c,6));
        xdo(&["keyup","ctrl"]);events.extend(frames(&p,&app,&mut c,4));
        events
    };
    let undone=press_ctrl("z");
    eprintln!("LUMINA_IBUS_P25_ONE_UNDO {undone:?}");
    assert_eq!(undone.len(),1);
    assert_eq!(undone[0].text,expected[6],
        "one Ctrl+Z must revert the ENTIRE D+Commit pair, not only insertion");
    let redone=press_ctrl("y");
    eprintln!("LUMINA_IBUS_P25_ONE_REDO {redone:?}");
    assert_eq!(redone.len(),1);
    assert_eq!(redone[0].text,expected[7]);
    // External authority change while the Hangul engine retains its prior
    // syllable must reset native generation BEFORE the next physical key.
    app.children[0].set_prop("value","HOST-2");
    app.children[0].set_prop("value_revision","2");
    assert!(frames(&p,&app,&mut c,5).is_empty());
    let rebased=p.diagnostic_native_ibus_surrounding().unwrap();
    assert_eq!(rebased.0,"HOST-2");
    let next=key(&p,&app,&mut c,"r");
    eprintln!("LUMINA_IBUS_P25_REBASE_NO_STALE_DELETE {next:?}");
    assert_eq!(next.len(),1);
    assert_eq!(next[0].text,"HOST-2ㄱ");
    assert!(p.diagnostic_native_ibus_deletion().is_none(),
        "first new-generation Hangul key has only D(0,0), not a stale D(-1,1)");
}


/// Phase 26 semantic RED: privacy is an outbound native protocol property,
/// not a screen mask or a suppressed diagnostic. Values here are nonsecret
/// test canaries. This uses the REAL ibus-hangul daemon and X11 window.
#[test]fn live_surrounding_policy_revokes_context_without_cross_field_reuse(){
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"hangul");
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut tree=node("App",&[("bg","#101820")]);
    tree.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","400"),("height","65"),
        ("value","PUBLIC-CANARY"),("value_revision","1"),("on_change","cb:953"),
    ]));
    tree.children.push(node("TextInput",&[
        ("x","20"),("y","110"),("width","400"),("height","65"),
        ("value","PRIVATE-CANARY"),("value_revision","1"),("on_change","cb:954"),
        ("surrounding_text","full"),("type","password"),
    ]));
    frames(&plugin,&tree,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(760,520));
    frames(&plugin,&tree,&mut ctx,4);
    let window=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=window.lines().next().expect("real X11 window");
    xdo(&["windowfocus","--sync",id]);
    assert!(key(&plugin,&tree,&mut ctx,"Tab").is_empty());
    let ordinary=plugin.diagnostic_native_ibus_surrounding()
        .expect("ordinary field must preserve phase-25 protocol");
    assert_eq!(ordinary.0,"PUBLIC-CANARY");
    // Change policy while still focused. A stale helper ACK or host revision
    // must not make the previous published value authoritative again.
    tree.children[0].set_prop("surrounding_text","none");
    frames(&plugin,&tree,&mut ctx,5);
    assert!(plugin.diagnostic_native_ibus_surrounding().is_none(),
        "sensitive TextInput must not retain a native surrounding snapshot");
    let typed=key(&plugin,&tree,&mut ctx,"z");
    assert_eq!(typed.len(),1,"ordinary direct typing remains available without IME");
    assert_eq!(typed[0].callback_id,953);
    assert!(typed[0].text.ends_with('z'));
    assert!(plugin.diagnostic_native_ibus_surrounding().is_none(),
        "local edits cannot silently re-enable native sharing");
    // The second field must never inherit the first field's text.
    assert!(key(&plugin,&tree,&mut ctx,"Tab").is_empty());
    assert!(plugin.diagnostic_native_ibus_surrounding().is_none(),
        "different private field must not inherit previous context");
    tree.children[0].set_prop("surrounding_text","full");
    assert!(key(&plugin,&tree,&mut ctx,"Tab").is_empty());
    let restored=plugin.diagnostic_native_ibus_surrounding()
        .expect("explicit normal mode should restore ordinary IBus");
    assert_eq!(restored.0,"PUBLIC-CANARYz");
    tree.children[0].set_prop("surrounding_text","unknown-mode");
    frames(&plugin,&tree,&mut ctx,5);
    assert!(plugin.diagnostic_native_ibus_surrounding().is_none(),
        "unknown explicit policy must fail closed even on a normal field");
}


/// LUMINA-IBUS-RECONNECT-ISOLATION-027: RED until the real helper can be
/// re-created safely. This terminates the actual libibus C child process;
/// no synthetic D/C or scripted composition signals are used.
#[test]fn zzz_real_ibus_helper_death_recovers_with_new_authority(){
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"hangul");
    let p=AuraLuminaPlugin::new();let mut ctx=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","440"),("height","70"),
        ("value","RECOVER-"),("value_revision","1"),("on_change","cb:955"),
        ("size","28"),("font_path","/usr/share/fonts/truetype/nanum/NanumGothic.ttf"),
    ]));
    assert!(frames(&p,&app,&mut ctx,4).is_empty());
    assert!(p.diagnostic_resize_window(760,520));
    assert!(frames(&p,&app,&mut ctx,4).is_empty());
    let window=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=window.lines().next().expect("genuine X11 window");
    xdo(&["windowfocus","--sync",id]);
    assert!(key(&p,&app,&mut ctx,"Tab").is_empty());
    assert_eq!(p.diagnostic_native_ibus_surrounding().unwrap().0,"RECOVER-");
    let first=key(&p,&app,&mut ctx,"r");
    assert_eq!(first.len(),1,"first real Hangul CommitText");
    assert_eq!(first[0].text,"RECOVER-ㄱ");
    assert!(p.diagnostic_native_ibus().unwrap_or(0)>=1);
    assert!(p.diagnostic_terminate_native_ibus_helper(),
        "must terminate a real live IBus C helper process");
    assert!(frames(&p,&app,&mut ctx,2).is_empty(),
        "dead helper cannot issue stale host edit events");
    assert_eq!(p.diagnostic_native_ibus(),None,
        "dead generation must be fully dropped");
    assert_eq!(p.diagnostic_composition(955),None,
        "abandoned helper must not retain active IME composition");
    // During the bounded reconnect delay, normal local editing is usable.
    let fallback=key(&p,&app,&mut ctx,"z");
    assert_eq!(fallback.len(),1,"one direct fallback change, no old engine C");
    assert_eq!(fallback[0].text,"RECOVER-ㄱz");
    let during=frames(&p,&app,&mut ctx,100);
    assert!(during.is_empty(),"recovering cannot replay stale native commits");
    assert_eq!(p.diagnostic_native_ibus(),Some(0),
        "a fresh real helper must have reconnected, with new counters");
    let snap=p.diagnostic_native_ibus_surrounding()
        .expect("fresh helper must ACK updated host value before any new key");
    assert_eq!(snap.0,"RECOVER-ㄱz");
    assert_eq!(p.diagnostic_native_ibus_replacements(),Some(0),
        "old native replacement proofs must not survive process death");
    assert!(p.diagnostic_native_ibus_deletion().is_none(),
        "old engine D requests must not cross process generations");
    let resumed=key(&p,&app,&mut ctx,"r");
    eprintln!("LUMINA_IBUS_P27_RECONNECT_REAL_ENGINE resumed={resumed:?}");
    assert_eq!(resumed.len(),1,"new upstream Hangul key still has one edit");
    assert_eq!(resumed[0].callback_id,955);
    assert_eq!(resumed[0].text,"RECOVER-ㄱzㄱ");
    assert!(p.diagnostic_native_ibus_deletion().is_none(),
        "new engine first key cannot reuse old syllable D");
    // Reconnection must not bypass the phase26 privacy revocation.
    app.children[0].set_prop("surrounding_text","none");
    assert!(frames(&p,&app,&mut ctx,5).is_empty());
    assert!(p.diagnostic_native_ibus_surrounding().is_none(),
        "recovered helper must honor sensitive field opt-out");
    let private=key(&p,&app,&mut ctx,"x");
    assert_eq!(private.len(),1);
    assert_eq!(private[0].text,"RECOVER-ㄱzㄱx");
    assert!(p.diagnostic_native_ibus_surrounding().is_none());
}


/// Phase 28 semantic RED: replacing the REAL bus-owning ibus-daemon while
/// the prior libibus C helper is still running must invalidate that stale
/// daemon generation without waiting for a user key or fabricating signals.
/// The child-process crash proof in Phase27 is not sufficient for this case.
#[test]fn zzzz_real_ibus_daemon_replacement_invalidates_running_helper(){
    use std::time::Duration;
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"hangul");
    assert!(std::env::var_os("LUMINA_IBUS_TEST_HANGUL_SWITCH").is_none());
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","440"),("height","70"),
        ("value","DAEMON-"),("value_revision","1"),("on_change","cb:956"),
        ("size","28"),("font_path","/usr/share/fonts/truetype/nanum/NanumGothic.ttf"),
    ]));
    assert!(frames(&p,&app,&mut c,4).is_empty());
    assert!(p.diagnostic_resize_window(760,520));
    frames(&p,&app,&mut c,4);
    let window=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=window.lines().next().expect("genuine X11 window");
    xdo(&["windowfocus","--sync",id]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    assert_eq!(p.diagnostic_native_ibus_surrounding().unwrap().0,"DAEMON-");
    let first=key(&p,&app,&mut c,"r");
    assert_eq!(first.len(),1);
    assert_eq!(first[0].text,"DAEMON-ㄱ");
    assert!(p.diagnostic_native_ibus().unwrap_or(0)>=1);
    // Restart the actual daemon, NOT the helper. The old helper remains
    // alive here; only a bus-generation liveness check can revoke it.
    let swap=Command::new("ibus-daemon").args([
        "--replace","--daemonize","--xim","--panel","disable","--cache=refresh"
    ]).output().expect("real ibus-daemon --replace");
    assert!(swap.status.success(),"real daemon replace failed: {}",
        String::from_utf8_lossy(&swap.stderr));
    let mut active=false;
    for _ in 0..25{
        let set=Command::new("ibus").args(["engine","hangul"]).output();
        let check=Command::new("ibus").arg("engine").output();
        if set.as_ref().is_ok_and(|x|x.status.success()) &&
            check.as_ref().is_ok_and(|x|x.status.success()
                && String::from_utf8_lossy(&x.stdout).trim()=="hangul"){
            active=true;break;
        }
        std::thread::sleep(Duration::from_millis(120));
    }
    assert!(active,"the new REAL daemon must have registered hangul");
    let idle=frames(&p,&app,&mut c,110);
    assert!(idle.is_empty(),"no old native CommitText on daemon restart");
    assert_eq!(p.diagnostic_native_ibus(),Some(0),
        "a replaced daemon must discard the OLD live helper and start new real libibus generation before another key");
    let snapshot=p.diagnostic_native_ibus_surrounding()
        .expect("new native helper must independently ACK current surrounding");
    assert_eq!(snapshot.0,"DAEMON-ㄱ");
    assert_eq!(p.diagnostic_native_ibus_replacements(),Some(0));
    assert!(p.diagnostic_native_ibus_deletion().is_none());
    assert_eq!(p.diagnostic_composition(956),None);
    let next=key(&p,&app,&mut c,"r");
    eprintln!("LUMINA_IBUS_P28_DAEMON_REPLACE_REAL_ENGINE next={next:?}");
    assert_eq!(next.len(),1,"new daemon can emit exactly one real key Commit");
    assert_eq!(next[0].callback_id,956);
    assert_eq!(next[0].text,"DAEMON-ㄱㄱ",
        "no old-engine D(-1,1) may delete before new daemon first syllable");
    app.children[0].set_prop("surrounding_text","none");
    assert!(frames(&p,&app,&mut c,5).is_empty());
    assert!(p.diagnostic_native_ibus_surrounding().is_none(),
        "daemon recovery cannot resurrect a private TextInput policy");
}


// Phase29 never injects engine C/D/P. This sends SIGKILL to the real daemon
// process inside the dedicated Xvfb + dbus-run-session test job; the C helper
// is NOT killed. The old libibus bus signal must invalidate its generation.
fn phase29_hard_kill_real_daemon(){
    let out=Command::new("pkill").args(["-KILL","-x","ibus-daemon"])
        .output().expect("procps pkill required for genuine daemon SIGKILL");
    assert!(out.status.success(),
        "must actually SIGKILL the live ibus-daemon process (not helper): {}",
        String::from_utf8_lossy(&out.stderr));
}
fn phase29_start_real_daemon_hangul(){
    use std::time::Duration;
    let out=Command::new("ibus-daemon").args([
        "--daemonize","--xim","--panel","disable","--cache=refresh"
    ]).output().expect("start actual ibus-daemon after hard crash");
    assert!(out.status.success(),
        "restarting genuine daemon failed: {}",String::from_utf8_lossy(&out.stderr));
    let mut active=false;
    for _ in 0..30{
        let set=Command::new("ibus").args(["engine","hangul"]).output();
        let check=Command::new("ibus").arg("engine").output();
        if set.as_ref().is_ok_and(|x|x.status.success()) &&
            check.as_ref().is_ok_and(|x|x.status.success() &&
                String::from_utf8_lossy(&x.stdout).trim()=="hangul"){
            active=true;break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(active,"restarted daemon must activate registered REAL hangul engine");
}
fn phase29_live_field(prefix:&str,cb:u64)->(AuraLuminaPlugin,UiNode,NexusContext){
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    let cb_name=format!("cb:{cb}");
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","440"),("height","70"),
        ("value",prefix),("value_revision","1"),("on_change",&cb_name),
        ("size","28"),("font_path","/usr/share/fonts/truetype/nanum/NanumGothic.ttf"),
    ]));
    assert!(frames(&p,&app,&mut c,4).is_empty());
    assert!(p.diagnostic_resize_window(760,520));
    assert!(frames(&p,&app,&mut c,4).is_empty());
    let window=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=window.lines().next().expect("real X11 window");
    xdo(&["windowfocus","--sync",id]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    assert_eq!(p.diagnostic_native_ibus_surrounding().unwrap().0,prefix);
    (p,app,c)
}
/// RED: authentic SIGKILL must abandon the old libibus generation without
/// applying stale edits, permit fallback, then recover when daemon returns
/// while remaining within the strict lifetime retry budget.
#[test]fn zzzzz_real_ibus_daemon_sigkill_delayed_recovery(){
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"hangul");
    let (p,app,mut c)=phase29_live_field("HARD-",957);
    let first=key(&p,&app,&mut c,"r");
    assert_eq!(first.len(),1);
    assert_eq!(first[0].text,"HARD-ㄱ");
    phase29_hard_kill_real_daemon();
    assert!(frames(&p,&app,&mut c,50).is_empty(),
        "real hard-crash cannot replay old CommitText during idle frames");
    assert_eq!(p.diagnostic_native_ibus(),None,
        "the old daemon generation must lose its native helper");
    let (tries,exhausted)=p.diagnostic_native_ibus_recovery_budget().unwrap();
    assert!(tries>=1 && !exhausted,
        "a failed real offline attempt must be observable without sensitive text: tries={tries}");
    let local=key(&p,&app,&mut c,"z");
    assert_eq!(local.len(),1,"offline direct typing must remain available");
    assert_eq!(local[0].text,"HARD-ㄱz");
    phase29_start_real_daemon_hangul();
    assert!(frames(&p,&app,&mut c,210).is_empty(),
        "old engine events must not replay after delayed daemon restart");
    assert_eq!(p.diagnostic_native_ibus(),Some(0),
        "remaining bounded attempt must connect a FRESH real helper");
    assert_eq!(p.diagnostic_native_ibus_surrounding().unwrap().0,"HARD-ㄱz");
    assert_eq!(p.diagnostic_native_ibus_replacements(),Some(0));
    assert!(p.diagnostic_native_ibus_deletion().is_none());
    let next=key(&p,&app,&mut c,"r");
    eprintln!("LUMINA_IBUS_P29_HARD_KILL_RECOVER next={next:?}");
    assert_eq!(next.len(),1);
    assert_eq!(next[0].text,"HARD-ㄱzㄱ");
}

/// A second genuine hard crash persists across ALL three limited process
/// respawn attempts. When the daemon later returns, exhaustion forbids
/// unlimited automatic reconnect; direct local typing remains available.
#[test]fn zzzzz_real_ibus_daemon_sigkill_retry_exhaustion(){
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"hangul");
    let (p,app,mut c)=phase29_live_field("EXHAUST-",958);
    let first=key(&p,&app,&mut c,"r");
    assert_eq!(first.len(),1);
    assert_eq!(first[0].text,"EXHAUST-ㄱ");
    phase29_hard_kill_real_daemon();
    assert!(frames(&p,&app,&mut c,430).is_empty(),
        "no stale engine callback may escape during long offline render");
    assert_eq!(p.diagnostic_native_ibus(),None,
        "no offline helper can remain trusted after genuine hard crash");
    assert_eq!(p.diagnostic_native_ibus_recovery_budget(),Some((3,true)),
        "three failed live native reconnect attempts must exhaust budget");
    phase29_start_real_daemon_hangul();
    assert!(frames(&p,&app,&mut c,180).is_empty());
    assert_eq!(p.diagnostic_native_ibus(),None,
        "automatic helper spawning must STOP at lifetime cap even after daemon returns");
    assert_eq!(p.diagnostic_native_ibus_recovery_budget(),Some((3,true)),
        "late daemon return must not silently reset budget");
    let fallback=key(&p,&app,&mut c,"z");
    assert_eq!(fallback.len(),1);
    assert_eq!(fallback[0].text,"EXHAUST-ㄱz");
    assert!(p.diagnostic_native_ibus_surrounding().is_none(),
        "no cached prior engine surrounding after exhausted retry");
    eprintln!("LUMINA_IBUS_P29_GENUINE_OFFLINE_EXHAUSTED attempted=3 no_auto_restart");
}
