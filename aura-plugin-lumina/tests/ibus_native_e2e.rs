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
