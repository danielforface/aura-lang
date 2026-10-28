#![cfg(feature="lumina-diagnostics")]
// Real X11/XTest tab traversal. Only logical reading-order focus, NOT
// accessibility focus order, assistive technologies, IME or BiDi semantics.
use std::process::Command;
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback,UiTextInputEvent};
use aura_plugin_lumina::AuraLuminaPlugin;

fn node(kind:&str,props:&[(&str,&str)],children:Vec<UiNode>)->UiNode {
    let mut x=UiNode::new(kind);
    for &(k,v) in props { x.set_prop(k,v); }
    x.children=children; x
}
fn xdo(args:&[&str])->String {
    let out=Command::new("xdotool").args(args).output().expect("xdotool installed");
    assert!(out.status.success(),"xdotool {args:?}: {}",String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}
fn render(plugin:&AuraLuminaPlugin,tree:&UiNode,ctx:&mut NexusContext)->Vec<UiTextInputEvent> {
    plugin.on_ui_render(tree,ctx).expect("Lumina active").expect("native draw");
    ctx.get::<UiRuntimeFeedback>().expect("feedback").text_input_events.clone()
}
fn frames(plugin:&AuraLuminaPlugin,tree:&UiNode,ctx:&mut NexusContext,n:usize)->Vec<UiTextInputEvent> {
    (0..n).flat_map(|_| render(plugin,tree,ctx)).collect()
}
fn press(plugin:&AuraLuminaPlugin,tree:&UiNode,ctx:&mut NexusContext,key:&str,shift:bool)->Vec<UiTextInputEvent> {
    if shift { xdo(&["keydown","Shift_L"]); frames(plugin,tree,ctx,2); }
    xdo(&["keydown",key]);
    let mut result=frames(plugin,tree,ctx,5);
    xdo(&["keyup",key]);
    result.extend(frames(plugin,tree,ctx,2));
    if shift { xdo(&["keyup","Shift_L"]); frames(plugin,tree,ctx,2); }
    eprintln!("LUMINA_TAB_XTEST key={key} shift={shift} feedback={result:?}");
    result
}
fn has(e:&[UiTextInputEvent],cb:u64,text:&str)->bool {
    e.iter().any(|x|x.callback_id==cb && x.text==text && !x.submitted)
}
#[test]
fn native_tab_shift_tab_order_wrap_and_skip_hidden_disabled_inputs() {
    let mut ctx=NexusContext::default();
    let plugin=AuraLuminaPlugin::new();
    let tree=node("App",&[("bg","#121212")],vec![
        node("TextInput",&[("x","20"),("y","20"),("width","180"),("height","40"),
            ("value","A"),("on_change","cb:101")],vec![]),
        node("TextInput",&[("x","20"),("y","75"),("width","180"),("height","40"),
            ("value","disabled"),("disabled","true"),("on_change","cb:999")],vec![]),
        node("TextInput",&[("x","20"),("y","130"),("width","180"),("height","40"),
            ("value","B"),("on_change","cb:202")],vec![]),
        node("Box",&[("x","20"),("y","220"),("width","40"),("height","35"),
            ("overflow","hidden")],vec![node("TextInput",
            &[("x","180"),("y","225"),("value","hidden"),("on_change","cb:303")],vec![])]),
        node("TextInput",&[("x","900"),("y","900"),("value","offscreen"),("on_change","cb:404")],vec![]),
    ]);
    frames(&plugin,&tree,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,400));
    frames(&plugin,&tree,&mut ctx,3);
    let windows=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=windows.lines().next().expect("actual Raylib window");
    xdo(&["windowfocus","--sync",id]);
    // From NO focus: Tab selects first visible enabled field.
    assert!(press(&plugin,&tree,&mut ctx,"Tab",false).is_empty());
    let a=press(&plugin,&tree,&mut ctx,"q",false);
    assert!(has(&a,101,"Aq"),"Tab from no focus selects cb101: {a:?}");

    // Tabs skip disabled, clipped-away and offscreen controls.
    assert!(press(&plugin,&tree,&mut ctx,"Tab",false).is_empty());
    let b=press(&plugin,&tree,&mut ctx,"w",false);
    assert!(has(&b,202,"Bw"),"second Tab selects cb202: {b:?}");

    // Reverse navigation must return to first, not disabled cb999.
    assert!(press(&plugin,&tree,&mut ctx,"Tab",true).is_empty());
    let a2=press(&plugin,&tree,&mut ctx,"e",false);
    assert!(has(&a2,101,"Ae"),"Shift-Tab selects cb101: {a2:?}");

    // Reverse from first wraps to last visible field.
    assert!(press(&plugin,&tree,&mut ctx,"Tab",true).is_empty());
    let b2=press(&plugin,&tree,&mut ctx,"z",false);
    assert!(has(&b2,202,"Bz"),"reverse wrap selects cb202: {b2:?}");
    assert!(a.iter().chain(b.iter()).chain(a2.iter()).chain(b2.iter())
        .all(|e| ![999,303,404].contains(&e.callback_id)),
        "disabled, clipped and offscreen fields must not emit input callbacks");
    // Mouse and keyboard must agree: a disabled TextInput must not become
    // editable when clicked, even after another field was focused.
    xdo(&["mousemove","--window",id,"40","95"]);
    frames(&plugin,&tree,&mut ctx,3);
    xdo(&["mousedown","1"]);
    frames(&plugin,&tree,&mut ctx,4);
    xdo(&["mouseup","1"]);
    frames(&plugin,&tree,&mut ctx,2);
    let blocked=press(&plugin,&tree,&mut ctx,"p",false);
    assert!(blocked.is_empty(),"disabled field accepted real pointer+keyboard: {blocked:?}");
    drop(plugin);
    eprintln!("LUMINA_TAB_XTEST: forward/reverse, disabled pointer/Tab, clip+offscreen skip PASS");
}
