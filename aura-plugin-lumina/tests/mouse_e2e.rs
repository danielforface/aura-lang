#![cfg(feature = "lumina-diagnostics")]
// Real X11/XTest mouse input. No synthetic internal hit-test injection.
// Requires the CI Xvfb environment and xdotool installed explicitly.

use std::process::Command;
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback};
use aura_plugin_lumina::AuraLuminaPlugin;

fn node(kind:&str,props:&[(&str,&str)],children:Vec<UiNode>)->UiNode {
    let mut n=UiNode::new(kind);
    for &(k,v) in props { n.set_prop(k,v); }
    n.children=children;n
}
fn xdotool(args:&[&str])->String {
    let output=Command::new("xdotool").args(args).output().expect("xdotool present");
    assert!(output.status.success(),"xdotool {:?}: {}",args,
        String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}
fn render(plugin:&AuraLuminaPlugin,tree:&UiNode,nexus:&mut NexusContext)->Option<u64>{
    plugin.on_ui_render(tree,nexus).expect("native UI owned").expect("native draw succeeded");
    nexus.get::<UiRuntimeFeedback>().expect("frame feedback").clicked_callback_id
}
fn click(plugin:&AuraLuminaPlugin,tree:&UiNode,nexus:&mut NexusContext,
    window:&str,x:i32,y:i32)->Vec<u64>{
    let _=xdotool(&["mousemove","--window",window,&x.to_string(),&y.to_string()]);
    for _ in 0..3 { render(plugin,tree,nexus); }
    let _=xdotool(&["mousedown","1"]);
    let mut got=Vec::new();
    for _ in 0..6 {
        if let Some(cb)=render(plugin,tree,nexus){got.push(cb);}
    }
    let _=xdotool(&["mouseup","1"]);
    for _ in 0..3{render(plugin,tree,nexus);}
    eprintln!("LUMINA_XTEST_CLICK {x},{y} -> {got:?}");
    got
}

#[test]
fn real_x11_mouse_topmost_click_and_hidden_overflow_guard() {
    let plugin=AuraLuminaPlugin::new();
    let mut nexus=NexusContext::default();
    let tree=node("App",&[("bg","#102030")],vec![
        node("Button",&[("x","20"),("y","20"),("width","90"),
            ("height","45"),("on_click","cb:11"),("label","under")],vec![]),
        node("Button",&[("x","35"),("y","25"),("width","90"),
            ("height","45"),("on_click","cb:22"),("label","top")],vec![]),
        node("Box",&[("x","200"),("y","20"),("width","50"),("height","50"),
            ("overflow","hidden")],vec![
            node("Button",&[("x","270"),("y","25"),("width","70"),
                ("height","45"),("on_click","cb:33")],vec![])
        ])
    ]);
    for _ in 0..3 {render(&plugin,&tree,&mut nexus);}
    assert!(plugin.diagnostic_resize_window(640,420));
    for _ in 0..4 {render(&plugin,&tree,&mut nexus);}
    let windows=xdotool(&["search","--name","Aura Lumina Sentinel"]);
    let id=windows.lines().next().expect("actual native window found");
    eprintln!("LUMINA_XTEST_WINDOW id={id}");
    let hit=click(&plugin,&tree,&mut nexus,id,50,40);
    assert!(hit.contains(&22),"topmost overlapping Button was not dispatched: {hit:?}");
    assert!(!hit.contains(&11),"underneath actionable Button stole the pointer");
    let clipped=click(&plugin,&tree,&mut nexus,id,290,35);
    assert!(clipped.is_empty(),"clipped off-Box Button unexpectedly received click: {clipped:?}");
    drop(plugin);
}
