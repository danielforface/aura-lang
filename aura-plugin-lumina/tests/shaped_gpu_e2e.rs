#![cfg(feature="lumina-diagnostics")]
// Actual Xvfb GL readback: requires loaded indexed glyph atlas, tests RTL
// visual painting against the old unshaped codepoint path, not just count.
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback,UiTextInputEvent};
use aura_plugin_lumina::AuraLuminaPlugin;
use std::process::Command;
const FONT:&str="/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
fn node(kind:&str,props:&[(&str,&str)])->UiNode{
 let mut n=UiNode::new(kind);for &(k,v) in props{n.set_prop(k,v);}n
}
fn draw(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext){
 for _ in 0..6 {p.on_ui_render(t,c).expect("runtime").expect("native draw");}
}
#[test]
fn real_gpu_arabic_shaped_glyphs_differ_pixelwise_from_codepoint_fallback(){
 assert!(std::path::Path::new(FONT).is_file(),"DejaVu required");
 let p=AuraLuminaPlugin::new();let mut cx=NexusContext::default();
 let mut app=node("App",&[("bg","#101820")]);
 app.children.push(node("Text",&[("x","30"),("y","120"),("size","36"),
   ("text","سلام"),("font_path",FONT),("color","#EE3030"),
   ("direction","rtl"),("shaping","opentype")]));
 draw(&p,&app,&mut cx);p.diagnostic_resize_window(640,420);draw(&p,&app,&mut cx);
 let (atlas_glyphs,ids,width)=p.diagnostic_shaped_gpu(FONT,36,"rtl","سلام")
     .expect("actual indexed GPU texture uploaded");
 eprintln!("LUMINA_GLYPH_GPU_X11 indexed={atlas_glyphs} ids={ids:?} width={width}");
 assert_eq!(ids.len(),3,"Arabic lam-alef ligature must use actual GSUB glyphs");
 assert!(atlas_glyphs>=2 && atlas_glyphs<=384 && width>10.0);
 let shaped=p.diagnostic_red_signature(20,100,250,100).expect("physical shaped pixels");
 assert!(shaped.0>30,"shaped Arabic paint must have real red pixels: {shaped:?}");
 app.children[0].set_prop("shaping","legacy");
 draw(&p,&app,&mut cx);
 let legacy=p.diagnostic_red_signature(20,100,250,100).expect("legacy pixels");
 eprintln!("LUMINA_GLYPH_GPU_X11 shaped={shaped:?} legacy={legacy:?}");
 assert!(legacy.0>30,"old codepoint fallback must still draw");
 assert_ne!(shaped.1,legacy.1,
     "actual GPU shaped Arabic must have a different spatial silhouette from unshaped glyph order");
 drop(p);
}
#[test]
fn input_shapes_hebrew_and_preserves_native_revision_focus_keyboard(){
 let p=AuraLuminaPlugin::new();let mut cx=NexusContext::default();
 let mut app=node("App",&[("bg","#101820")]);
 app.children.push(node("TextInput",&[
   ("x","20"),("y","20"),("width","280"),("height","62"),
   ("size","28"),("radius","0"),("value","שלום"),("value_revision","1"),
   ("on_change","cb:771"),("font_path",FONT),("fg","#EE3030"),
   ("direction","rtl"),("shaping","opentype")]));
 draw(&p,&app,&mut cx);p.diagnostic_resize_window(640,420);draw(&p,&app,&mut cx);
 let coverage=p.diagnostic_shaped_gpu(FONT,28,"rtl","שלום").expect("GPU prepared");
 assert!(coverage.0>=4 && coverage.2>15.0);
 let pix=p.diagnostic_red_signature(32,24,235,48).expect("live GPU");
 assert!(pix.0>25,"Hebrew shaped input should visibly render: {pix:?}");
 let window=Command::new("xdotool").args(["search","--name","Aura Lumina Sentinel"]).output().unwrap();
 assert!(window.status.success());
 let id=String::from_utf8_lossy(&window.stdout);
 let id=id.lines().next().expect("window id");
 assert!(Command::new("xdotool").args(["windowfocus","--sync",id]).status().unwrap().success());
 assert!(Command::new("xdotool").args(["keydown","Tab"]).status().unwrap().success());
 draw(&p,&app,&mut cx);
 assert!(Command::new("xdotool").args(["keyup","Tab"]).status().unwrap().success());
 draw(&p,&app,&mut cx);
 assert!(Command::new("xdotool").args(["keydown","q"]).status().unwrap().success());
 let mut events=Vec::<UiTextInputEvent>::new();
 for _ in 0..6 {
     p.on_ui_render(&app,&mut cx).expect("feedback").expect("native");
     events.extend(cx.get::<UiRuntimeFeedback>().unwrap().text_input_events.clone());
 }
 assert!(Command::new("xdotool").args(["keyup","q"]).status().unwrap().success());
 assert!(events.iter().any(|e|e.callback_id==771&&e.text=="שלוםq"),
     "real XTest Unicode draft must persist with shaped paint: {events:?}");
 draw(&p,&app,&mut cx);
 assert!(p.diagnostic_shaped_gpu(FONT,28,"rtl","שלוםq").is_some(),
     "newly typed suffix must rebuild GPU glyph IDs before drawing");
 drop(p);
}
