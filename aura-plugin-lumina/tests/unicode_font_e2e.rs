#![cfg(feature="lumina-diagnostics")]
// Actual Linux virtual X11 + Raylib GPU glyph atlas pixel readback.
// DejaVuSans is installed by the CI job, NOT bundled in this repository.
// This tests Hebrew codepoint rasterization in *logical* order, not BiDi,
// shaping, IME, mark positioning, screenreaders or real-device DPI.
use std::process::Command;
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback,UiTextInputEvent};
use aura_plugin_lumina::AuraLuminaPlugin;
const FONT:&str="/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
fn node(kind:&str,props:&[(&str,&str)],children:Vec<UiNode>)->UiNode{
 let mut n=UiNode::new(kind);for &(k,v) in props{n.set_prop(k,v);}n.children=children;n
}
fn xdo(args:&[&str])->String {
 let out=Command::new("xdotool").args(args).output().expect("xdotool");
 assert!(out.status.success(),"{args:?}: {}",String::from_utf8_lossy(&out.stderr));
 String::from_utf8_lossy(&out.stdout).trim().to_owned()
}
fn frames(p:&AuraLuminaPlugin,root:&UiNode,ctx:&mut NexusContext,n:usize)->Vec<UiTextInputEvent> {
 (0..n).flat_map(|_|{
    p.on_ui_render(root,ctx).expect("UI available").expect("native draw");
    ctx.get::<UiRuntimeFeedback>().expect("feedback").text_input_events.clone()
 }).collect()
}
fn key(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,k:&str)->Vec<UiTextInputEvent> {
 xdo(&["keydown",k]);let mut e=frames(p,t,c,5);
 xdo(&["keyup",k]);e.extend(frames(p,t,c,2));e
}
#[test]
fn native_hebrew_external_ttf_raster_text_and_textinput_dynamic_glyphs(){
 assert!(std::path::Path::new(FONT).is_file(),"CI apt fonts-dejavu-core required");
 let p=AuraLuminaPlugin::new();let mut ctx=NexusContext::default();
 let mut tree=node("App",&[("bg","#101820")],vec![
    node("TextInput",&[("x","20"),("y","20"),("width","250"),("height","56"),
        ("radius","0"),("size","24"),("font_path",FONT),("fg","#EE3030"),
        ("value","שלום"),("value_revision","1"),("on_change","cb:77")],vec![]),
    node("Text",&[("x","20"),("y","105"),("size","28"),("text","אבג"),
        ("font_path",FONT),("color","#EE3030")],vec![])
 ]);
 frames(&p,&tree,&mut ctx,5);p.diagnostic_resize_window(640,420);frames(&p,&tree,&mut ctx,5);
 let width=p.diagnostic_font_coverage(FONT,24,"שלום").expect("TextInput Unicode font atlas is GPU loaded");
 let width2=p.diagnostic_font_coverage(FONT,28,"אבג").expect("Text Unicode font loaded");
 assert!(width>10.0 && width2>10.0,"measured Hebrew glyph widths must be nonzero");
 let text_pixels=p.diagnostic_red_pixels_in_rect(20,100,125,60).expect("native GPU frame");
 let input_pixels=p.diagnostic_red_pixels_in_rect(32,25,220,42).expect("native input glyphs");
 eprintln!("LUMINA_UNICODE_XTEST initial widths={width} {width2} text_red={text_pixels} input_red={input_pixels}");
 assert!(text_pixels>12,"actual Hebrew Text widget must rasterize red visible glyphs");
 assert!(input_pixels>12,"actual Hebrew TextInput must rasterize red visible glyphs");
 assert_eq!(p.diagnostic_red_pixels_in_rect(300,30,75,50),Some(0),
    "no Unicode glyph overdraw outside widget");
 let w=xdo(&["search","--name","Aura Lumina Sentinel"]);
 xdo(&["windowfocus","--sync",w.lines().next().expect("native window")]);
 key(&p,&tree,&mut ctx,"Tab");
 let ev=key(&p,&tree,&mut ctx,"q");
 assert!(ev.iter().any(|e|e.callback_id==77 && e.text=="שלוםq"),
    "real X11 keyboard edit must retain Hebrew value: {ev:?}");
 // The font contains ASCII baseline; updated native caret must measure through
 // the actual custom loaded font, not the default font or grapheme estimates.
 let visual=p.diagnostic_text_visual(77,32.0,226.0).expect("measured caret");
 let expected=p.diagnostic_font_coverage(FONT,24,"שלוםq").expect("glyph atlas coverage");
 assert!((visual.measured_caret-expected).abs()<0.5,
    "caret and draw font diverged: {visual:?} expected={expected}");
 tree.children[0].set_prop("value_revision","2");
 tree.children[0].set_prop("value","שלום עולם");
 frames(&p,&tree,&mut ctx,5);
 assert!(p.diagnostic_font_coverage(FONT,24,"שלום עולם").is_some(),
    "new host text forces bounded dynamic glyph atlas refresh");
 let e=key(&p,&tree,&mut ctx,"x");
 assert!(e.iter().any(|e|e.callback_id==77 && e.text=="שלום עולםx"),
    "new authoritative Hebrew content must survive further real key edits");
 assert_eq!(p.diagnostic_red_pixels_in_rect(300,30,75,50),Some(0));
 drop(p);
 eprintln!("LUMINA_UNICODE_XTEST: real font atlas, Hebrew widgets, X11 editing, dynamic Unicode and GPU teardown PASS");
}
#[test]
fn nonexistent_external_font_fails_to_legacy_default_without_gpu_failure(){
 let p=AuraLuminaPlugin::new();let mut ctx=NexusContext::default();
 let root=node("App",&[("bg","#101820")],vec![
  node("Text",&[("x","20"),("y","90"),("size","24"),
    ("text","HELLO"),("color","#EE3030"),("font_path","/not-real/lumina.ttf")],vec![])
 ]);
 frames(&p,&root,&mut ctx,5);
 assert!(p.diagnostic_font_coverage("/not-real/lumina.ttf",24,"HELLO").is_none());
 assert!(p.diagnostic_red_pixels_in_rect(20,85,100,42).unwrap()>5,
    "missing TTF should preserve old default-font DrawText fallback");
 drop(p);
 eprintln!("LUMINA_UNICODE_XTEST: missing TTF falls back to default rendering PASS");
}
