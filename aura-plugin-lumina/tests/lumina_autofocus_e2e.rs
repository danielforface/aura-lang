#![cfg(all(feature="lumina-diagnostics",target_os="linux"))]
// Real X11/XTest end-to-end keyboard acceptance through the PUBLIC Lumina
// on_ui_render -> UiRuntimeFeedback API. No fabricated key events, editor
// callbacks or composition frames. Normal helper only in the live-IME case.
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback,UiTextInputEvent};
use aura_plugin_lumina::AuraLuminaPlugin;
use std::process::Command;

fn node(kind:&str,props:&[(&str,&str)])->UiNode{
    let mut n=UiNode::new(kind);
    for &(k,v) in props{n.set_prop(k,v);}
    n
}
fn frame(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext)->Vec<UiTextInputEvent>{
    p.on_ui_render(t,c).expect("native plugin").expect("render");
    c.get::<UiRuntimeFeedback>().expect("native UI feedback").text_input_events.clone()
}
fn frames(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,n:usize)->Vec<UiTextInputEvent>{
    (0..n).flat_map(|_|frame(p,t,c)).collect()
}
fn xdo(args:&[&str])->String{
    let r=Command::new("xdotool").args(args).output().expect("X11 XTest");
    assert!(r.status.success(),"XTest {args:?}: {}",String::from_utf8_lossy(&r.stderr));
    String::from_utf8_lossy(&r.stdout).trim().to_owned()
}
fn real_window(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext)->String{
    assert!(frames(p,t,c,4).is_empty());
    assert!(p.diagnostic_resize_window(760,520),"native Raylib window");
    assert!(frames(p,t,c,4).is_empty());
    let id=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=id.lines().next().expect("real native X11 window").to_owned();
    xdo(&["windowfocus","--sync",&id]);
    assert!(frames(p,t,c,4).is_empty());
    id
}
fn key(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,k:&str)->Vec<UiTextInputEvent>{
    xdo(&["keydown",k]);let mut out=frames(p,t,c,5);
    xdo(&["keyup",k]);out.extend(frames(p,t,c,5));out
}
fn ctrl_key(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,k:&str)->Vec<UiTextInputEvent>{
    xdo(&["keydown","ctrl"]);frames(p,t,c,4);
    xdo(&["keydown",k]);let mut out=frames(p,t,c,6);
    xdo(&["keyup",k]);out.extend(frames(p,t,c,6));
    xdo(&["keyup","ctrl"]);out.extend(frames(p,t,c,4));out
}

/// Production behavior, no libibus configured: first *eligible* opted field
/// receives initial native focus without Tab or mouse, and user-driven
/// navigation/blur has precedence. Disabled opted input must be skipped.
/// Host-authoritative value revisions remain in control after local edits.
#[test]
fn autofocus_direct_real_x11_first_valid_target_and_no_refocus(){
    assert!(std::env::var_os("LUMINA_IBUS_HELPER_BIN").is_none(),
        "direct fallback test must not inherit an IME helper");
    let p=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut app=node("App",&[("bg","#111722")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","450"),("height","45"),
        ("on_change","cb:37101"),("value","DISABLED"),
        ("autofocus","true"),("disabled","true"),
    ]));
    app.children.push(node("TextInput",&[
        ("x","20"),("y","90"),("width","450"),("height","45"),
        ("on_change","cb:37102"),("value","START"),
        ("value_revision","1"),("autofocus","true"),
    ]));
    app.children.push(node("TextInput",&[
        ("x","20"),("y","170"),("width","450"),("height","45"),
        ("on_change","cb:37103"),("value","NEXT"),
        ("value_revision","1"),("autofocus","true"),
    ]));
    let _window=real_window(&p,&app,&mut ctx);
    // No Tab or mouse click: this key must go to the FIRST enabled,
    // uniquely addressed and actually visible opt-in TextInput.
    let first=key(&p,&app,&mut ctx,"x");
    assert_eq!(first.len(),1,"autofocus must enable immediate typing: {first:?}");
    assert_eq!((first[0].callback_id,first[0].text.as_str()),(37102,"STARTx"));
    assert!(key(&p,&app,&mut ctx,"Tab").is_empty());
    let second=key(&p,&app,&mut ctx,"y");
    assert_eq!(second.len(),1);
    assert_eq!((second[0].callback_id,second[0].text.as_str()),(37103,"NEXTy"),
        "manual Tab must not be stolen back by autofocus");
    app.children[1].set_prop("value","REMOTE");
    app.children[1].set_prop("value_revision","2");
    assert!(frames(&p,&app,&mut ctx,5).is_empty(),
        "host rebase must not create synthetic on_change");
    assert!(key(&p,&app,&mut ctx,"Tab").is_empty());
    let rebound=key(&p,&app,&mut ctx,"z");
    assert_eq!(rebound.len(),1);
    assert_eq!((rebound[0].callback_id,rebound[0].text.as_str()),(37102,"REMOTEz"),
        "the real host revision must rebase a nonfocused TextInput");
    assert!(key(&p,&app,&mut ctx,"Escape").is_empty());
    assert!(key(&p,&app,&mut ctx,"q").is_empty(),
        "after manual Escape, initial autofocus may NOT repeatedly reclaim focus");
    eprintln!("LUMINA_AUTOFOCUS_DIRECT_REAL_X11 initial=true disabled_skipped=true source_first=true tab_preserved=true host_revision=true no_refocus_after_escape=true");
}

/// Same actual public AuraLuminaPlugin entry using the NORMAL production
/// libibus helper and a REAL registered ibus-hangul engine. No diagnostic
/// pump deferral or fabricated C/D/P, unlike the Phase36/37 research tests.
#[test]
fn autofocus_live_normal_ibus_hangul_and_native_undo(){
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"hangul");
    assert!(std::env::var_os("LUMINA_IBUS_HELPER_BIN").is_some());
    assert!(std::env::var_os("LUMINA_IBUS_TEST_HANGUL_SWITCH").is_none());
    let p=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut app=node("App",&[("bg","#111722")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","450"),("height","70"),
        ("on_change","cb:37111"),("value","ANCHOR"),
        ("value_revision","1"),("autofocus","true"),
        ("font_path","/usr/share/fonts/truetype/nanum/NanumGothic.ttf"),
        ("size","28"),
    ]));
    let _window=real_window(&p,&app,&mut ctx);
    let initial=p.diagnostic_native_ibus_surrounding()
        .expect("normal production native helper must ACK first autofocus field");
    assert_eq!(initial.0,"ANCHOR","normal IBus context synchronized without Tab");
    let first=key(&p,&app,&mut ctx,"r");
    assert_eq!(first.len(),1,"genuine real Hangul Commit must edit focused field: {first:?}");
    assert_eq!((first[0].callback_id,first[0].text.as_str()),(37111,"ANCHORㄱ"));
    let second=key(&p,&app,&mut ctx,"k");
    assert_eq!(second.len(),1,"genuine native D+C is one edit");
    assert_eq!(second[0].text,"ANCHOR가");
    let undo=ctrl_key(&p,&app,&mut ctx,"z");
    assert_eq!(undo.len(),1,"one Undo per real D+C");
    assert_eq!(undo[0].text,"ANCHORㄱ");
    let redo=ctrl_key(&p,&app,&mut ctx,"y");
    assert_eq!(redo.len(),1);
    assert_eq!(redo[0].text,"ANCHOR가");
    app.children[0].set_prop("value","HOST-2");
    app.children[0].set_prop("value_revision","2");
    assert!(frames(&p,&app,&mut ctx,5).is_empty());
    let rebase=p.diagnostic_native_ibus_surrounding()
        .expect("real helper must ACK host-rebased autofocus field");
    assert_eq!(rebase.0,"HOST-2");
    let after=key(&p,&app,&mut ctx,"r");
    assert_eq!(after.len(),1);
    assert_eq!((after[0].callback_id,after[0].text.as_str()),(37111,"HOST-2ㄱ"),
        "engine must not reuse old Hangul generation after host rebase");
    eprintln!("LUMINA_AUTOFOCUS_LIVE_NORMAL_HANGUL initial_without_tab=true genuine_DC_atomic=true undo_redo=true host_rebase_safe=true production_helper=true");
}
