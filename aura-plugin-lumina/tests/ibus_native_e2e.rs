#![cfg(all(feature="lumina-diagnostics",target_os="linux"))]
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback,UiTextInputEvent};
use aura_plugin_lumina::AuraLuminaPlugin;
use std::process::Command;
const FONT:&str="/usr/share/fonts/truetype/nanum/NanumGothic.ttf";
const HELPER:&str=concat!(env!("CARGO_MANIFEST_DIR"),"/native/ibus_bridge.py");
fn node(k:&str, props:&[(&str,&str)])->UiNode {
    let mut n=UiNode::new(k);
    for &(p,v) in props {n.set_prop(p,v);}
    n
}
fn frame(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext)->Vec<UiTextInputEvent>{
    p.on_ui_render(t,c).expect("owned").expect("native frame");
    c.get::<UiRuntimeFeedback>().unwrap().text_input_events.clone()
}
fn frames(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,n:usize)->Vec<UiTextInputEvent>{
    (0..n).flat_map(|_|frame(p,t,c)).collect()
}
fn xdo(a:&[&str]){
    let z=Command::new("xdotool").args(a).output().unwrap();
    assert!(z.status.success(),"{a:?}: {}",String::from_utf8_lossy(&z.stderr));
}
fn key(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,k:&str)->Vec<UiTextInputEvent>{
    xdo(&["keydown",k]);
    let mut out=frames(p,t,c,5);
    xdo(&["keyup",k]);
    out.extend(frames(p,t,c,8));
    out
}
#[test]fn real_ibus_hangul_signal_to_raylib_gpu(){
    assert!(std::path::Path::new(FONT).is_file());
    assert!(std::path::Path::new(HELPER).is_file());
    assert_eq!(std::env::var("AURA_LUMINA_IBUS_HELPER").as_deref(),Ok(HELPER),
        "host must configure the audited absolute helper path, not the UI node");
    assert_eq!(std::env::var("AURA_LUMINA_IBUS_ENGINE").as_deref(),Ok("hangul"));
    let p=AuraLuminaPlugin::new();
    let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","390"),("height","66"),
        ("radius","0"),("value","Hi"),("value_revision","1"),
        ("on_change","cb:918"),("size","31"),("fg","#DD3030"),
        ("font_path",FONT),("shaping","opentype"),("direction","ltr"),
        ("native_ime","ibus")
    ]));
    frames(&p,&app,&mut c,4);
    assert!(p.diagnostic_resize_window(640,420));
    frames(&p,&app,&mut c,4);
    let ids=Command::new("xdotool").args(["search","--name","Aura Lumina Sentinel"])
        .output().unwrap();
    let windows=String::from_utf8(ids.stdout).unwrap();
    let id=windows.lines().next().unwrap();
    xdo(&["windowfocus","--sync",id]);
    key(&p,&app,&mut c,"Tab");
    let mut ready=false;
    for _ in 0..150 {
        frames(&p,&app,&mut c,1);
        if p.diagnostic_native_ibus_ready(918){ready=true;break;}
    }
    assert!(ready,"real IBus InputContext must ACK focus");
    assert!(key(&p,&app,&mut c,"r").is_empty());
    assert!(key(&p,&app,&mut c,"k").is_empty());
    let mut preview=None;
    for _ in 0..150 {
        frames(&p,&app,&mut c,1);
        if let Some(state)=p.diagnostic_composition(918){
            if state.1.chars().any(|ch|matches!(ch,'\u{AC00}'..='\u{D7A3}')){
                preview=Some(state);
                break;
            }
        }
    }
    let state=preview.expect("genuine Hangul engine must send Korean preedit");
    assert_eq!(state.0,"Hi");
    assert_eq!(state.3,0);
    let v=p.diagnostic_text_visual(918,32.0,366.0).unwrap();
    assert!(!v.preedit_screens.is_empty(),"no native IBus underline");
    let &(a,b)=v.preedit_screens.first().unwrap();
    assert!(b>a+5.0);
    // Underline is actually at y=(widget.y+height-7)=79, NOT y77.
    // Reject the dark background RGB(13,17,23), which trivially satisfies
    // blue > red and would be a false positive. Require real cyan ink.
    let pix=p.diagnostic_capture_pixel((a+b)*0.5,79.0).unwrap();
    eprintln!("REAL_IBUS_PREEDIT {:?} {v:?} {pix:?}",state.1);
    assert!(pix.b>180 && pix.g>130 && pix.r<120,
        "real IBus preedit underline needs actual cyan framebuffer ink, not background");
    let mut committed=key(&p,&app,&mut c,"space");
    for _ in 0..150 {
        committed.extend(frames(&p,&app,&mut c,1));
        if !committed.is_empty(){break;}
    }
    eprintln!("REAL_IBUS_COMMIT {committed:?}");
    assert_eq!(committed.iter().filter(|x|x.callback_id==918).count(),1);
    assert!(committed.iter().any(|x|x.callback_id==918 &&
        x.text.starts_with("Hi") &&
        x.text.chars().any(|ch|matches!(ch,'\u{AC00}'..='\u{D7A3}'))));
    assert!(p.diagnostic_composition(918).is_none());
}