#![cfg(feature="lumina-diagnostics")]
// True X11/XTest + framebuffer readback and measured caret from the actual
// Raylib default font. Only ASCII and virtual Linux 1x here; no Unicode
// shaping, BiDi, IME or physical hardware-DPI claims.
use std::process::Command;
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback};
use aura_plugin_lumina::{AuraLuminaPlugin,DiagnosticPixel};
fn node(kind:&str,props:&[(&str,&str)],children:Vec<UiNode>)->UiNode {
 let mut n=UiNode::new(kind);for &(k,v) in props{n.set_prop(k,v);}n.children=children;n
}
fn xdo(args:&[&str])->String {
 let output=Command::new("xdotool").args(args).output().expect("xdotool installed");
 assert!(output.status.success(),"{args:?}: {}",String::from_utf8_lossy(&output.stderr));
 String::from_utf8_lossy(&output.stdout).trim().to_owned()
}
fn frames(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,count:usize){
 for _ in 0..count {
  p.on_ui_render(t,c).expect("UI owned").expect("render ok");
  assert!(!c.get::<UiRuntimeFeedback>().expect("feedback").close_requested);
 }
}
fn key(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,k:&str,ctrl:bool){
 if ctrl{xdo(&["keydown","Control_L"]);frames(p,t,c,2);}
 xdo(&["keydown",k]);frames(p,t,c,5);
 xdo(&["keyup",k]);frames(p,t,c,2);
 if ctrl{xdo(&["keyup","Control_L"]);frames(p,t,c,2);}
}
fn color(r:u8,g:u8,b:u8)->DiagnosticPixel {DiagnosticPixel{r,g,b,a:255}}
#[test]
fn live_native_long_text_caret_tracks_measured_width_scroll_and_scissors(){
 let p=AuraLuminaPlugin::new();let mut ctx=NexusContext::default();
 let long="WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW";
 let root=node("App",&[("bg","#102030")],vec![
  node("TextInput",&[("x","20"),("y","20"),("width","120"),("height","44"),
   ("radius","0"),("bg","#0D1117"),("size","18"),
   ("value",long),("on_change","cb:77")],vec![])
 ]);
 frames(&p,&root,&mut ctx,4);
 assert!(p.diagnostic_resize_window(640,400));
 frames(&p,&root,&mut ctx,4);
 let window=xdo(&["search","--name","Aura Lumina Sentinel"]);
 xdo(&["windowfocus","--sync",window.lines().next().expect("X11 window found")]);
 let unselected=p.diagnostic_capture_pixel(48.0,27.0).expect("GPU before selection");
 // This pixel is beyond the narrow TextInput but within the unpainted App.
 assert_eq!(p.diagnostic_capture_pixel(160.0,42.0),Some(color(16,32,48)),
    "long value must not paint outside TextInput scissor");

 key(&p,&root,&mut ctx,"Tab",false);
 let end=p.diagnostic_text_visual(77,32.0,96.0).expect("actual default-font metrics");
 eprintln!("LUMINA_VIEW_XTEST end={end:?}");
 assert!(end.scroll_x>0.0,"long text MUST horizontally scroll to end caret");
 assert!(end.measured_caret>96.0);
 assert!((32.0..=128.0).contains(&end.caret_screen_x));

 key(&p,&root,&mut ctx,"Home",false);
 let home=p.diagnostic_text_visual(77,32.0,96.0).expect("Home measurements");
 eprintln!("LUMINA_VIEW_XTEST home={home:?}");
 assert_eq!(home.scroll_x,0.0);
 assert_eq!(home.measured_caret,0.0);
 assert_eq!(home.caret_screen_x,32.0);

 key(&p,&root,&mut ctx,"End",false);
 let end2=p.diagnostic_text_visual(77,32.0,96.0).unwrap();
 assert!(end2.scroll_x>0.0);
 assert!((32.0..=128.0).contains(&end2.caret_screen_x));
 key(&p,&root,&mut ctx,"a",true);
 let selected=p.diagnostic_text_visual(77,32.0,96.0).unwrap();
 eprintln!("LUMINA_VIEW_XTEST select={selected:?}");
 let (x0,x1)=selected.selection_screen.expect("Ctrl+A measured range");
 assert!((32.0..=128.0).contains(&x0) && (32.0..=128.0).contains(&x1));
 assert!(x1>x0);
 let over=p.diagnostic_capture_pixel(48.0,27.0).expect("selection GPU");
 assert_ne!(over,unselected,"Ctrl+A selection overlay must change actual physical pixels");
 assert_eq!(p.diagnostic_capture_pixel(160.0,42.0),Some(color(16,32,48)),
   "highlight must not escape the content clipping rectangle");

 // Real window resize with long active selection must not stale-scissor.
 assert!(p.diagnostic_resize_window(320,240));
 frames(&p,&root,&mut ctx,4);
 assert_eq!(p.diagnostic_capture_pixel(160.0,42.0),Some(color(16,32,48)));
 drop(p);
 eprintln!("LUMINA_VIEW_XTEST: actual GPU clip and measured cursor Home/End/selection/resize PASS");
}
