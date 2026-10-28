#![cfg(feature="lumina-diagnostics")]
// Physical Xvfb GPU pixel proof: DejaVuSerif lacks Hebrew,
// DejaVuSans covers it. This is genuine font-origin resolution.
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback,UiTextInputEvent};
use aura_plugin_lumina::AuraLuminaPlugin;
use std::process::Command;
const PRIMARY:&str="/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf";
const SECONDARY:&str="/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
fn node(kind:&str,props:&[(&str,&str)])->UiNode{
 let mut n=UiNode::new(kind);for &(k,v) in props{n.set_prop(k,v);}n
}
fn frames(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,n:usize){
 for _ in 0..n{
  p.on_ui_render(t,c).expect("owned").expect("native frame");
  assert!(!c.get::<UiRuntimeFeedback>().unwrap().close_requested);
 }
}
fn xdo(args:&[&str]){
 let st=Command::new("xdotool").args(args).status().unwrap();
 assert!(st.success(),"XTest {args:?}");
}
#[test]
fn actual_secondary_font_gids_uploaded_and_missing_font_fails_closed(){
 assert!(std::path::Path::new(PRIMARY).is_file()&&std::path::Path::new(SECONDARY).is_file());
 let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
 let value="ABC שלום";
 let mut app=node("App",&[("bg","#101820")]);
 let mut label=node("Text",&[
  ("x","25"),("y","110"),("size","34"),("color","#EE3030"),
  ("text",value),("font_path",PRIMARY),("fallback_font_path",SECONDARY),
  ("shaping","opentype"),("direction","auto")
 ]);
 app.children.push(label.clone());
 frames(&p,&app,&mut c,4);assert!(p.diagnostic_resize_window(640,420));
 frames(&p,&app,&mut c,4);
 let (cells,ids,width)=p.diagnostic_shaped_gpu_fallback(PRIMARY,SECONDARY,34,"auto",value)
   .expect("real GPU two-font atlas");
 eprintln!("LUMINA_FALLBACK_GPU ids={ids:?} width={width} cells={cells}");
 assert!(ids.iter().any(|g|g.0==0)&&ids.iter().any(|g|g.0==1));
 assert!(ids.iter().all(|g|g.1!=0));
 assert!(cells>=6&&width>70.0);
 let pix=p.diagnostic_red_signature(25,100,280,90).unwrap();
 assert!(pix.0>80,"must render both origins onto native GL framebuffer: {pix:?}");
 // A deliberately invalid secondary file may not produce primary-font
 // tofu or accidental same-index wrong-glyph rendering.
 label.set_prop("fallback_font_path","/tmp/missing_lumina_font_2037.ttf");
 app.children[0]=label;
 frames(&p,&app,&mut c,5);
 let no_font=p.diagnostic_red_signature(25,100,280,90).unwrap();
 assert_eq!(no_font.0,0,"invalid chain must paint no false glyphs");
 drop(p);
}
#[test]
fn fallback_textinput_rebuilds_indexed_atlas_after_real_x11_keyboard_edit(){
 let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
 let mut app=node("App",&[("bg","#101820")]);
 app.children.push(node("TextInput",&[
  ("x","20"),("y","22"),("width","340"),("height","64"),
  ("size","27"),("radius","0"),("value","Hi שלום"),("value_revision","1"),
  ("on_change","cb:816"),("font_path",PRIMARY),("fallback_font_path",SECONDARY),
  ("shaping","opentype"),("direction","auto"),("fg","#EE3030")
 ]));
 frames(&p,&app,&mut c,4);assert!(p.diagnostic_resize_window(640,420));
 frames(&p,&app,&mut c,4);
 let initial=p.diagnostic_shaped_gpu_fallback(PRIMARY,SECONDARY,27,"auto","Hi שלום")
   .expect("native TextInput two-font GPU");
 assert!(initial.1.iter().any(|g|g.0==0)&&initial.1.iter().any(|g|g.0==1));
 let win=Command::new("xdotool").args(["search","--name","Aura Lumina Sentinel"])
   .output().unwrap();
 assert!(win.status.success());
 let win=String::from_utf8_lossy(&win.stdout);
 let id=win.lines().next().expect("X11 window");
 xdo(&["windowfocus","--sync",id]);
 xdo(&["keydown","Tab"]);frames(&p,&app,&mut c,5);
 xdo(&["keyup","Tab"]);frames(&p,&app,&mut c,3);
 xdo(&["keydown","q"]);
 let mut events=Vec::<UiTextInputEvent>::new();
 for _ in 0..6{
  p.on_ui_render(&app,&mut c).expect("render").expect("paint");
  events.extend(c.get::<UiRuntimeFeedback>().unwrap().text_input_events.clone());
 }
 xdo(&["keyup","q"]);
 assert!(events.iter().any(|e|e.callback_id==816&&e.text=="Hi שלוםq"),
   "actual X11 must preserve logical source: {events:?}");
 frames(&p,&app,&mut c,4);
 let updated=p.diagnostic_shaped_gpu_fallback(PRIMARY,SECONDARY,27,"auto","Hi שלוםq")
   .expect("edited value must refresh native GPU atlas");
 assert!(updated.1.iter().any(|g|g.0==0)&&updated.1.iter().any(|g|g.0==1));
 drop(p);
}
