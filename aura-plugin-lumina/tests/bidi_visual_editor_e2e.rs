#![cfg(feature="lumina-diagnostics")]
// Real X11 events and exact GPU-backed text geometry. Not faked input injection.
use std::process::Command;
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback};
use aura_plugin_lumina::AuraLuminaPlugin;
const FONT:&str="/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
fn tree(text:&str)->UiNode{
 let mut app=UiNode::new("App");app.set_prop("bg","#102030");
 let mut input=UiNode::new("TextInput");
 for (k,v) in [
   ("x","20"),("y","30"),("width","460"),("height","70"),("size","27"),
   ("bg","#0D1117"),("fg","#EE3030"),("radius","0"),("on_change","cb:815"),
   ("font_path",FONT),("shaping","opentype"),("direction","auto"),
   ("value_revision","1"),("value",text)]{input.set_prop(k,v);}
 app.children.push(input);app
}
fn xdo(args:&[&str])->String{
 let out=Command::new("xdotool").args(args).output().unwrap();
 assert!(out.status.success(),"{args:?} {:?}",String::from_utf8_lossy(&out.stderr));
 String::from_utf8_lossy(&out.stdout).trim().to_owned()
}
fn frames(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,n:usize){
 for _ in 0..n{
  p.on_ui_render(t,c).expect("owned").expect("paint");
  assert!(!c.get::<UiRuntimeFeedback>().unwrap().close_requested);
 }
}
fn key(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,k:&str,ctrl:bool,shift:bool){
 if ctrl {xdo(&["keydown","Control_L"]);frames(p,t,c,2);}
 if shift{xdo(&["keydown","Shift_L"]);frames(p,t,c,2);}
 xdo(&["keydown",k]);frames(p,t,c,5);
 xdo(&["keyup",k]);frames(p,t,c,2);
 if shift{xdo(&["keyup","Shift_L"]);frames(p,t,c,2);}
 if ctrl {xdo(&["keyup","Control_L"]);frames(p,t,c,2);}
}
fn click(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,win:&str,x:i32){
 xdo(&["mousemove","--window",win,&x.to_string(),"60"]);
 frames(p,t,c,3);xdo(&["mousedown","1"]);frames(p,t,c,6);
 xdo(&["mouseup","1"]);frames(p,t,c,3);
}
fn visual(p:&AuraLuminaPlugin)->aura_plugin_lumina::DiagnosticTextVisual {
 p.diagnostic_text_visual(815,32.0,436.0).expect("Xvfb shaped visual")
}
#[test]
fn true_x11_hebrew_arrows_and_click_follow_physical_screen_order(){
 assert!(std::path::Path::new(FONT).exists());
 let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
 let t=tree("שלום");frames(&p,&t,&mut c,4);
 assert!(p.diagnostic_resize_window(650,420));frames(&p,&t,&mut c,4);
 let windows=xdo(&["search","--name","Aura Lumina Sentinel"]);
 let win=windows.lines().next().expect("native window");
 xdo(&["windowfocus","--sync",win]);
 key(&p,&t,&mut c,"Tab",false,false);
 let initial=visual(&p);
 eprintln!("BIDI-X11 initial={initial:?}");
 assert!(initial.caret_visible);
 assert_eq!(initial.caret_grapheme,Some(4));
 key(&p,&t,&mut c,"Right",false,false);
 let right=visual(&p);
 eprintln!("BIDI-X11 right={right:?}");
 assert_eq!(right.caret_grapheme,Some(3),"physical Right steps backward through logical RTL graphemes");
 assert!(right.caret_screen_x>initial.caret_screen_x+1.0);
 key(&p,&t,&mut c,"Left",false,false);
 let left=visual(&p);
 assert_eq!(left.caret_grapheme,Some(4));
 assert!((left.caret_screen_x-initial.caret_screen_x).abs()<1.0);
 key(&p,&t,&mut c,"End",false,false);
 let end=visual(&p);
 assert_eq!(end.caret_grapheme,Some(0),"physical End should be the right edge of RTL word");
 assert!(end.caret_screen_x>right.caret_screen_x);
 click(&p,&t,&mut c,win,34);
 let click_left=visual(&p);
 assert_eq!(click_left.caret_grapheme,Some(4),"real pointer left edge is logical end");
 click(&p,&t,&mut c,win,125);
 let click_right=visual(&p);
 assert_eq!(click_right.caret_grapheme,Some(0),"real pointer right edge is logical start");
 assert!(click_right.caret_screen_x>click_left.caret_screen_x);
 drop(p);
}
#[test]
fn true_x11_mixed_direction_select_all_is_split_and_shaded_in_framebuffer(){
 let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
 let t=tree("שלום ABC 123 עולם");frames(&p,&t,&mut c,5);
 assert!(p.diagnostic_resize_window(650,420));frames(&p,&t,&mut c,4);
 let windows=xdo(&["search","--name","Aura Lumina Sentinel"]);
 let win=windows.lines().next().unwrap();xdo(&["windowfocus","--sync",win]);
 key(&p,&t,&mut c,"Tab",false,false);
 let initial=visual(&p);
 assert!(initial.caret_visible,"all cluster boundaries must resolve for mixed fixture");
 assert!(initial.selection_screens.is_empty());
 key(&p,&t,&mut c,"a",true,false);
 let chosen=visual(&p);
 eprintln!("BIDI-X11 mixed select-all={chosen:?}");
 assert_eq!(chosen.logical_selection,Some((0,17)));
 assert!(chosen.selection_screens.len()>=3,
    "UAX9 three visual runs must be three painted selection fragments");
 assert!(chosen.selection_screens.iter().all(|(a,b)|*b>*a && *a>=32.0 && *b<=468.0));
 // Sample inside the top 4px selection area; this is outside rendered
 // glyphs and must be physically blue-tinted, not a synthetic geometry test.
 for &(a,b) in &chosen.selection_screens{
   let mid=((a+b)*.5).floor();
   let pix=p.diagnostic_capture_pixel(mid,35.0).unwrap();
   assert!(pix.b>35 && pix.b>pix.r / 3,
      "selection rectangle must physically paint in native Raylib: span=({a},{b}) pix={pix:?}");
 }
 key(&p,&t,&mut c,"Right",false,false);
 let collapse=visual(&p);
 assert!(collapse.logical_selection.is_none(),"physical Right collapses selection");
 assert!(collapse.caret_visible);
 drop(p);
}
#[test]
fn true_x11_arabic_gsub_ligature_hides_unresolvable_interior_caret(){
 let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
 let t=tree("سلام");frames(&p,&t,&mut c,4);
 assert!(p.diagnostic_resize_window(650,420));frames(&p,&t,&mut c,4);
 let windows=xdo(&["search","--name","Aura Lumina Sentinel"]);
 let win=windows.lines().next().unwrap();xdo(&["windowfocus","--sync",win]);
 key(&p,&t,&mut c,"Tab",false,false);
 let d=visual(&p);
 eprintln!("BIDI-X11 Arabic ligature={d:?}");
 // The renderer is still active; a partial caret map is not a license
 // to assign fabricated x=0 at an internal GSUB ligature boundary.
 assert!(p.diagnostic_shaped_gpu(FONT,27,"auto","سلام").is_some());
 assert!(!d.caret_visible,"ligature interior must not be fabricated");
 let original=d.caret_grapheme;
 key(&p,&t,&mut c,"Left",false,false);
 assert_eq!(visual(&p).caret_grapheme,original,
   "incomplete shaped stop map disables unsafe visual arrow fallback");
 drop(p);
}
