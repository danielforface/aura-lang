#![cfg(feature = "lumina-diagnostics")]
// Real X11 keyboard/character events delivered to an actual Raylib window.
// The pure UTF-8 editor is covered separately in src/text_input.rs tests.
// This proves ASCII XTest events, not IME/RTL grapheme input or accessibility.
use std::process::Command;
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback,UiTextInputEvent};
use aura_plugin_lumina::AuraLuminaPlugin;

fn node(kind: &str, props: &[(&str,&str)]) -> UiNode {
    let mut n=UiNode::new(kind);
    for &(k,v) in props { n.set_prop(k,v); }
    n
}
fn cmd(args:&[&str])->String {
    let o=Command::new("xdotool").args(args).output().expect("xdotool installed");
    assert!(o.status.success(),"xdotool {args:?}: {}",String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}
fn frame(plugin:&AuraLuminaPlugin,root:&UiNode,ctx:&mut NexusContext)->Vec<UiTextInputEvent>{
    plugin.on_ui_render(root,ctx).expect("Lumina owns UI").expect("render ok");
    ctx.get::<UiRuntimeFeedback>().expect("UI feedback").text_input_events.clone()
}
fn frames(plugin:&AuraLuminaPlugin,root:&UiNode,ctx:&mut NexusContext,count:usize)
    ->Vec<UiTextInputEvent>{
    (0..count).flat_map(|_|frame(plugin,root,ctx)).collect()
}
fn key(plugin:&AuraLuminaPlugin,root:&UiNode,ctx:&mut NexusContext,name:&str)
    ->Vec<UiTextInputEvent>{
    cmd(&["keydown",name]);
    let mut ev=frames(plugin,root,ctx,3);
    cmd(&["keyup",name]);
    ev.extend(frames(plugin,root,ctx,2));
    eprintln!("LUMINA_KEY {name} -> {ev:?}");
    ev
}
fn click(plugin:&AuraLuminaPlugin,root:&UiNode,ctx:&mut NexusContext,
    window:&str,x:i32,y:i32){
    cmd(&["mousemove","--window",window,&x.to_string(),&y.to_string()]);
    frames(plugin,root,ctx,3);
    cmd(&["mousedown","1"]);
    frames(plugin,root,ctx,3);
    cmd(&["mouseup","1"]);
    frames(plugin,root,ctx,2);
}
#[test]
fn real_x11_text_focus_typing_caret_navigation_submit_and_blur() {
    let plugin=AuraLuminaPlugin::new();let mut ctx=NexusContext::default();
    let mut root=node("App",&[("bg","#111111")]);
    root.children.push(node("TextInput",&[("x","20"),("y","20"),
        ("width","280"),("height","45"),("radius","0"),("value","AB"),
        ("on_change","cb:44"),("on_submit","cb:45")]));
    root.children.push(node("Button",&[("x","330"),("y","20"),("width","80"),
        ("height","40"),("label","blur"),("on_click","cb:55")]));
    frames(&plugin,&root,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,400));
    frames(&plugin,&root,&mut ctx,4);
    let w=cmd(&["search","--name","Aura Lumina Sentinel"]);
    let id=w.lines().next().expect("native Raylib window");
    cmd(&["windowfocus","--sync",id]);
    click(&plugin,&root,&mut ctx,id,40,40);
    let insert=key(&plugin,&root,&mut ctx,"q");
    assert!(insert.iter().any(|e|e.callback_id==44&&e.text=="ABq"&&!e.submitted),
        "focused TextInput must emit exact typed content, got {insert:?}");
    assert!(key(&plugin,&root,&mut ctx,"Home").is_empty(),"Home is navigation only");
    let d=key(&plugin,&root,&mut ctx,"Delete");
    assert!(d.iter().any(|e|e.callback_id==44&&e.text=="Bq"),"Home+Delete must delete first char: {d:?}");
    assert!(key(&plugin,&root,&mut ctx,"End").is_empty(),"End must not emit changes");
    let back=key(&plugin,&root,&mut ctx,"BackSpace");
    assert!(back.iter().any(|e|e.callback_id==44&&e.text=="B"),"End+Backspace must remove last char: {back:?}");
    let submitted=key(&plugin,&root,&mut ctx,"Return");
    assert!(submitted.iter().any(|e|e.callback_id==45&&e.text=="B"&&e.submitted),
        "Return should submit final edited buffer without changing it: {submitted:?}");
    click(&plugin,&root,&mut ctx,id,350,35);
    assert!(key(&plugin,&root,&mut ctx,"z").is_empty(),
        "blurred TextInput must not receive edits");
    drop(plugin);
    eprintln!("LUMINA_KEYBOARD_E2E: XTest edit navigation + submit + blur PASS");
}
