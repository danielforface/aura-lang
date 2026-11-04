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
