#![cfg(feature="lumina-diagnostics")]
// True X11 Raylib native frame/pixel tests of an EXPLICIT adapter event
// injection. These are NOT native XIM/IBus/preedit callbacks; raylib only
// exposes committed GetCharPressed Unicode codepoints.
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback,UiTextInputEvent};
use aura_plugin_lumina::{AuraLuminaPlugin,CompositionEvent};
use std::process::Command;
const FONT:&str="/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
fn node(kind:&str,props:&[(&str,&str)])->UiNode{
    let mut n=UiNode::new(kind);for &(k,v) in props{n.set_prop(k,v);}n
}
fn frame(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext)->Vec<UiTextInputEvent>{
    p.on_ui_render(t,c).expect("owned").expect("native frame");
    c.get::<UiRuntimeFeedback>().expect("feedback").text_input_events.clone()
}
fn frames(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,n:usize)->Vec<UiTextInputEvent>{
    (0..n).flat_map(|_|frame(p,t,c)).collect()
}
fn xdo(args:&[&str])->String {
    let o=Command::new("xdotool").args(args).output().unwrap();
    assert!(o.status.success(),"XTest {args:?}: {}",String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}
fn key(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,k:&str)->Vec<UiTextInputEvent>{
    xdo(&["keydown",k]);let mut ev=frames(p,t,c,5);
    xdo(&["keyup",k]);ev.extend(frames(p,t,c,3));ev
}
#[test]fn explicit_composition_bridge_native_gpu_preview_and_atomic_undo(){
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","340"),("height","64"),
        ("radius","0"),("value","Hi"),("value_revision","1"),
        ("on_change","cb:916"),("size","30"),("fg","#EE3030"),
        ("font_path",FONT),("shaping","opentype"),("direction","ltr")
    ]));
    frames(&p,&app,&mut c,4);assert!(p.diagnostic_resize_window(640,420));
    frames(&p,&app,&mut c,4);
    let wins=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let win=wins.lines().next().unwrap();
    xdo(&["windowfocus","--sync",win]);key(&p,&app,&mut c,"Tab");
    assert!(p.submit_composition_event(CompositionEvent::Start{callback_id:916,sequence:1}));
    assert!(p.submit_composition_event(CompositionEvent::Update{
        callback_id:916,sequence:2,text:"שלום".into(),caret:4}));
    assert!(frames(&p,&app,&mut c,5).is_empty(),
        "preedit cannot emit any on_change before commit");
    let (authoritative,projected,caret,undo)=p.diagnostic_composition(916).unwrap();
    assert_eq!((authoritative.as_str(),projected.as_str(),caret,undo),("Hi","Hiשלום",6,0));
    let gpu=p.diagnostic_shaped_gpu(FONT,30,"ltr","Hiשלום")
        .expect("physical GPU glyph atlas must use temporary Unicode projection");
    assert!(gpu.0>=5,"composed Unicode painted: {gpu:?}");
    let vis=p.diagnostic_text_visual(916,32.0,316.0).unwrap();
    assert!(vis.caret_visible);
    assert_eq!(vis.preedit_screens.len(),1,"one physical rtl run underline");
    let &(start,end)=vis.preedit_screens.first().unwrap();
    assert!(end>start+15.0,"nonzero physical underline width: {vis:?}");
    let sample=p.diagnostic_capture_pixel((start+end)*0.5,77.0).unwrap();
    eprintln!("LUMINA_EXPLICIT_COMPOSITION_X11 preedit={vis:?} underline={sample:?}");
    assert!(sample.b>sample.r&&sample.g>sample.r,
        "real GPU preedit underline should be cyan: {sample:?}");
    // Real XTest key events cannot mutate the authoritative value or undo
    // while a platform-adapter composition is active.
    assert!(key(&p,&app,&mut c,"q").is_empty());
    assert_eq!(p.diagnostic_composition(916).unwrap().0,"Hi");
    // Stale and wrong-field events must not move the in-progress preedit.
    assert!(p.submit_composition_event(CompositionEvent::Update{
        callback_id:916,sequence:2,text:"REPLAY".into(),caret:6}));
    assert!(p.submit_composition_event(CompositionEvent::Commit{
        callback_id:927,sequence:3,text:"wrong".into()}));
    assert!(frames(&p,&app,&mut c,3).is_empty());
    assert_eq!(p.diagnostic_composition(916).unwrap().1,"Hiשלום");
    assert!(!p.submit_composition_event(CompositionEvent::Update{
        callback_id:916,sequence:3,text:"invalid\nline".into(),caret:2}));
    assert!(p.submit_composition_event(CompositionEvent::Commit{
        callback_id:916,sequence:4,text:"שלום".into()}));
    let committed=frames(&p,&app,&mut c,4);
    assert_eq!(committed.iter().filter(|e|e.callback_id==916).count(),1,
        "one OS adapter Commit must yield exactly one callback: {committed:?}");
    assert!(committed.iter().any(|e|e.callback_id==916&&e.text=="Hiשלום"));
    assert!(p.diagnostic_composition(916).is_none(),"no uncommitted draft after Commit");
    assert!(p.diagnostic_text_visual(916,32.0,316.0).unwrap().preedit_screens.is_empty());
    // Native Ctrl+Z/Y actually returns to one pre-composition snapshot.
    xdo(&["keydown","ctrl"]);
    let undo=key(&p,&app,&mut c,"z");
    xdo(&["keyup","ctrl"]);frames(&p,&app,&mut c,2);
    assert!(undo.iter().any(|e|e.callback_id==916&&e.text=="Hi"),
        "whole IME commit is one Undo: {undo:?}");
    xdo(&["keydown","ctrl"]);
    let redo=key(&p,&app,&mut c,"y");
    xdo(&["keyup","ctrl"]);frames(&p,&app,&mut c,2);
    assert!(redo.iter().any(|e|e.callback_id==916&&e.text=="Hiשלום"));
    assert!(p.submit_composition_event(CompositionEvent::Start{callback_id:916,sequence:5}));
    assert!(p.submit_composition_event(CompositionEvent::Update{
        callback_id:916,sequence:6,text:"אב".into(),caret:2}));
    assert!(frames(&p,&app,&mut c,4).is_empty());
    assert_eq!(p.diagnostic_composition(916).unwrap().0,"Hiשלום");
    assert!(p.submit_composition_event(CompositionEvent::Cancel{callback_id:916,sequence:7}));
    assert!(frames(&p,&app,&mut c,3).is_empty());
    assert!(p.diagnostic_composition(916).is_none());
    assert_eq!(p.diagnostic_shaped_gpu(FONT,30,"ltr","Hiשלום").unwrap().0,gpu.0);
    drop(p);
}

#[test]fn adapter_preedit_blur_and_authoritative_version_rebase_have_no_stale_commit(){
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","340"),("height","64"),
        ("radius","0"),("value","seed"),("value_revision","1"),
        ("on_change","cb:917"),("size","28"),("fg","#EE3030"),
        ("font_path",FONT),("shaping","opentype"),("direction","ltr")
    ]));
    app.children.push(node("Button",&[
        ("x","390"),("y","20"),("width","90"),("height","50"),
        ("label","blur"),("on_click","cb:444")
    ]));
    frames(&p,&app,&mut c,4);assert!(p.diagnostic_resize_window(640,420));
    frames(&p,&app,&mut c,4);
    let wins=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let win=wins.lines().next().unwrap();
    xdo(&["windowfocus","--sync",win]);key(&p,&app,&mut c,"Tab");
    assert!(p.submit_composition_event(CompositionEvent::Start{callback_id:917,sequence:100}));
    assert!(p.submit_composition_event(CompositionEvent::Update{
        callback_id:917,sequence:101,text:"אב".into(),caret:2}));
    assert!(frames(&p,&app,&mut c,3).is_empty());
    assert_eq!(p.diagnostic_composition(917).unwrap().1,"seedאב");
    // Real pointer blur must abandon transient preedit and keep original value.
    xdo(&["mousemove","--window",win,"420","40"]);frames(&p,&app,&mut c,3);
    xdo(&["mousedown","1"]);frames(&p,&app,&mut c,3);
    xdo(&["mouseup","1"]);frames(&p,&app,&mut c,3);
    assert!(p.diagnostic_composition(917).is_none());
    key(&p,&app,&mut c,"Tab");
    assert!(p.diagnostic_composition(917).is_none());
    assert!(p.submit_composition_event(CompositionEvent::Commit{
        callback_id:917,sequence:102,text:"stale".into()}));
    assert!(frames(&p,&app,&mut c,3).is_empty(),
        "blurred preedit commit must not survive re-focus");
    assert!(p.submit_composition_event(CompositionEvent::Start{callback_id:917,sequence:103}));
    assert!(p.submit_composition_event(CompositionEvent::Update{
        callback_id:917,sequence:104,text:"שלום".into(),caret:4}));
    assert!(frames(&p,&app,&mut c,4).is_empty());
    assert!(p.diagnostic_composition(917).is_some());
    // A newer host authority revision rebases editor, cancels preedit,
    // clears Undo and retains replay-barrier serial across rebase.
    app.children[0].set_prop("value","server");
    app.children[0].set_prop("value_revision","2");
    assert!(frames(&p,&app,&mut c,4).is_empty());
    assert!(p.diagnostic_composition(917).is_none());
    assert!(p.submit_composition_event(CompositionEvent::Start{callback_id:917,sequence:104}));
    assert!(p.submit_composition_event(CompositionEvent::Commit{
        callback_id:917,sequence:105,text:"phantom".into()}));
    assert!(frames(&p,&app,&mut c,4).is_empty());
    assert!(p.diagnostic_composition(917).is_none());
    assert!(p.submit_composition_event(CompositionEvent::Start{callback_id:917,sequence:106}));
    assert!(frames(&p,&app,&mut c,3).is_empty());
    let draft=p.diagnostic_composition(917).unwrap();
    assert_eq!((draft.0.as_str(),draft.1.as_str(),draft.3),("server","server",0));
    drop(p);
}
