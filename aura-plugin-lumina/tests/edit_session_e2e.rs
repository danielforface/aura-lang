#![cfg(feature="lumina-diagnostics")]
// Real native X11, intentionally supplies STALE controlled value props while
// switching inputs. Verifies local drafts, selection, undo, redo and rebase.
use std::process::Command;
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback,UiTextInputEvent};
use aura_plugin_lumina::AuraLuminaPlugin;

fn node(kind:&str,props:&[(&str,&str)],children:Vec<UiNode>)->UiNode{
 let mut n=UiNode::new(kind);
 for &(k,v) in props {n.set_prop(k,v);}
 n.children=children;n
}
fn xdo(args:&[&str])->String{
 let out=Command::new("xdotool").args(args).output().expect("xdotool present");
 assert!(out.status.success(),"xdotool {args:?}: {}",String::from_utf8_lossy(&out.stderr));
 String::from_utf8_lossy(&out.stdout).trim().to_owned()
}
fn frames(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,n:usize)->Vec<UiTextInputEvent>{
 (0..n).flat_map(|_|{
     p.on_ui_render(t,c).expect("UI plugin").expect("render");
     c.get::<UiRuntimeFeedback>().expect("feedback").text_input_events.clone()
 }).collect()
}
fn key(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,key:&str,ctrl:bool)->Vec<UiTextInputEvent>{
 if ctrl {xdo(&["keydown","Control_L"]);frames(p,t,c,2);}
 xdo(&["keydown",key]);
 let mut e=frames(p,t,c,5);
 xdo(&["keyup",key]);e.extend(frames(p,t,c,2));
 if ctrl {xdo(&["keyup","Control_L"]);frames(p,t,c,2);}
 eprintln!("LUMINA_SESSION_XTEST key={key} ctrl={ctrl}: {e:?}");
 e
}
fn eq(e:&[UiTextInputEvent],id:u64,value:&str)->bool{
 e.iter().any(|x|x.callback_id==id&&x.text==value&&!x.submitted)
}
#[test]
fn real_x11_draft_across_focus_switch_ctrl_selection_undo_redo_and_authority_rebase(){
 let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
 let mut t=node("App",&[("bg","#131313")],vec![
  node("TextInput",&[("x","20"),("y","20"),("height","44"),("width","220"),("value","A"),
   ("on_change","cb:101")],vec![]),
  node("TextInput",&[("x","20"),("y","90"),("height","44"),("width","220"),("value","B"),
   ("on_change","cb:202")],vec![])
 ]);
 frames(&p,&t,&mut c,4);assert!(p.diagnostic_resize_window(640,400));frames(&p,&t,&mut c,4);
 let win=xdo(&["search","--name","Aura Lumina Sentinel"]);let id=win.lines().next().unwrap();
 xdo(&["windowfocus","--sync",id]);
 // Value props A/B remain STATIC until external rebase near the end.
 assert!(key(&p,&t,&mut c,"Tab",false).is_empty());
 assert!(eq(&key(&p,&t,&mut c,"q",false),101,"Aq"));
 key(&p,&t,&mut c,"Tab",false);
 assert!(eq(&key(&p,&t,&mut c,"w",false),202,"Bw"));
 key(&p,&t,&mut c,"Tab",true);
 assert!(eq(&key(&p,&t,&mut c,"e",false),101,"Aqe"),
  "previous unsent Aq must persist after returning from another field");
 key(&p,&t,&mut c,"Tab",false);
 assert!(eq(&key(&p,&t,&mut c,"z",false),202,"Bwz"));

 // Grapheme-safe selection: Ctrl+A replaces the complete draft in one step.
 assert!(key(&p,&t,&mut c,"a",true).is_empty());
 assert!(eq(&key(&p,&t,&mut c,"x",false),202,"x"));
 assert!(eq(&key(&p,&t,&mut c,"z",true),202,"Bwz"));
 assert!(eq(&key(&p,&t,&mut c,"y",true),202,"x"));

 // External authoritative update of the SECOND field is deliberate.
 // On next frame, rebase should invalidate old undo and use SERVER.
 t.children[1].set_prop("value","SERVER");
 frames(&p,&t,&mut c,3);
 assert!(eq(&key(&p,&t,&mut c,"k",false),202,"SERVERk"),
  "new host prop must win over stale local edit history");
 assert!(!eq(&key(&p,&t,&mut c,"z",true),202,"Bwz"),
  "undo must not resurrect an older pre-rebase draft");
 drop(p);
 eprintln!("LUMINA_SESSION_XTEST: two-field persistence, ctrl+a/z/y and authoritative rebase PASS");
}

#[test]
fn actual_x11_duplicate_callback_ids_never_share_a_draft_or_receive_pointer_focus(){
 let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
 let t=node("App",&[("bg","#181818")],vec![
  node("TextInput",&[("x","20"),("y","20"),("width","160"),("height","44"),
   ("on_change","cb:71"),("value","FIRST")],vec![]),
  node("TextInput",&[("x","20"),("y","85"),("width","160"),("height","44"),
   ("on_change","cb:71"),("value","SECOND")],vec![]),
  node("TextInput",&[("x","20"),("y","150"),("width","160"),("height","44"),
   ("on_change","cb:72"),("value","C")],vec![])
 ]);
 frames(&p,&t,&mut c,4);p.diagnostic_resize_window(640,400);frames(&p,&t,&mut c,4);
 let w=xdo(&["search","--name","Aura Lumina Sentinel"]);
 let id=w.lines().next().unwrap();
 xdo(&["windowfocus","--sync",id]);
 key(&p,&t,&mut c,"Tab",false);
 assert!(eq(&key(&p,&t,&mut c,"q",false),72,"Cq"),
    "Tab should only focus the uniquely identified field");
 xdo(&["mousemove","--window",id,"40","40"]);
 frames(&p,&t,&mut c,3);
 xdo(&["mousedown","1"]);frames(&p,&t,&mut c,4);
 xdo(&["mouseup","1"]);frames(&p,&t,&mut c,2);
 assert!(key(&p,&t,&mut c,"z",false).is_empty(),
    "ambiguous mouse target must fail closed, not silently edit cb71");
 key(&p,&t,&mut c,"Tab",false);
 assert!(eq(&key(&p,&t,&mut c,"x",false),72,"Cqx"),
    "unique draft should survive refocus after blocked ambiguous click");
 drop(p);
 eprintln!("LUMINA_SESSION_XTEST: duplicate callback fail-closed on Tab and real pointer PASS");
}
