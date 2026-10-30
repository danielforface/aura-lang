#![cfg(all(feature="lumina-diagnostics",target_os="linux"))]
// LIVE X11 XTest -> Raylib -> Rust typed key -> native libibus -> engine.
// These asserts inspect helper-acknowledged key frames; they cannot establish
// physical scancodes, arbitrary XKB layouts, or candidate popup support.
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback};
use aura_plugin_lumina::AuraLuminaPlugin;
use std::process::Command;

fn node(kind:&str,props:&[(&str,&str)])->UiNode{
    let mut n=UiNode::new(kind);for &(k,v) in props{n.set_prop(k,v);}n
}
fn frames(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,n:usize){
    for _ in 0..n {
        p.on_ui_render(t,c).expect("native owned").expect("render");
        let _=c.get::<UiRuntimeFeedback>().expect("feedback");
    }
}
fn xdo(args:&[&str])->String{
    let o=Command::new("xdotool").args(args).output().unwrap();
    assert!(o.status.success(),"XTest {args:?}: {}",String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}
fn key(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,k:&str){
    xdo(&["keydown",k]);frames(p,t,c,5);
    xdo(&["keyup",k]);frames(p,t,c,5);
}
#[test]fn real_ibus_helper_receives_x11_named_keypress_release_and_shifted_text(){
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"hangul");
    assert!(std::env::var_os("LUMINA_IBUS_HELPER_BIN").is_some());
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","360"),("height","60"),
        ("value",""),("on_change","cb:938"),("size","28"),
        ("font_path","/usr/share/fonts/truetype/nanum/NanumGothic.ttf"),
    ]));
    frames(&p,&app,&mut c,4);
    assert!(p.diagnostic_resize_window(640,420));
    frames(&p,&app,&mut c,4);
    let window=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=window.lines().next().expect("X11 window");
    xdo(&["windowfocus","--sync",id]);
    key(&p,&app,&mut c,"Tab");
    assert_eq!(p.diagnostic_native_ibus_keys(),Some((0,0,0)));

    key(&p,&app,&mut c,"Up");
    key(&p,&app,&mut c,"Down");
    let (count,releases,shifted)=p.diagnostic_native_ibus_keys()
        .expect("real live IBus helper");
    eprintln!("IBUS_NATIVE_TYPED_NAMED_KEY_ACK count={count} releases={releases} shifted={shifted}");
    assert!(count>=4, "two physical XTest presses and two releases must reach helper");
    assert!(releases>=2, "IBus must receive actual named-key release frames");

    xdo(&["keydown","Shift_L"]);frames(&p,&app,&mut c,4);
    xdo(&["keydown","r"]);frames(&p,&app,&mut c,5);
    xdo(&["keyup","r"]);frames(&p,&app,&mut c,5);
    xdo(&["keyup","Shift_L"]);frames(&p,&app,&mut c,4);
    let (new_count,new_releases,new_shifted)=p.diagnostic_native_ibus_keys().unwrap();
    eprintln!("IBUS_NATIVE_TYPED_SHIFT_ACK count={new_count} releases={new_releases} shift={new_shifted}");
    assert!(new_count>count,"real XTest shifted printable key must be forwarded");
    assert!(new_shifted>shifted,"modifier shift bit must reach libibus client");
    assert!(new_releases>=releases,"release accounting must be monotonic");
    drop(p);
}
