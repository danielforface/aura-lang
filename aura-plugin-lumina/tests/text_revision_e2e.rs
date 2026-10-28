#![cfg(feature="lumina-diagnostics")]
// True X11 input and controlled prop updates. value_revision is opt-in:
// unchanged revision cannot authorize a new external value. A strictly
// increasing revision owns the new canonical value. Text-equality echoes
// are *observational* (not protocol-level unique ack receipts).
use std::process::Command;
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback,UiTextInputEvent};
use aura_plugin_lumina::AuraLuminaPlugin;
fn node(k:&str,p:&[(&str,&str)],c:Vec<UiNode>)->UiNode {
 let mut n=UiNode::new(k);
 for &(key,v) in p { n.set_prop(key,v); }
 n.children=c;n
}
fn xdo(args:&[&str])->String {
 let out=Command::new("xdotool").args(args).output().expect("xdotool");
 assert!(out.status.success(), "{args:?}: {}",String::from_utf8_lossy(&out.stderr));
 String::from_utf8_lossy(&out.stdout).trim().to_owned()
}
fn frames(p:&AuraLuminaPlugin,t:&UiNode,ctx:&mut NexusContext,n:usize)->Vec<UiTextInputEvent> {
 (0..n).flat_map(|_|{
   p.on_ui_render(t,ctx).expect("owned").expect("native rendered");
   ctx.get::<UiRuntimeFeedback>().expect("feedback").text_input_events.clone()
 }).collect()
}
fn key(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,k:&str)->Vec<UiTextInputEvent> {
 xdo(&["keydown",k]);let mut e=frames(p,t,c,5);
 xdo(&["keyup",k]);e.extend(frames(p,t,c,2));
 eprintln!("LUMINA_REV_XTEST key={k} -> {e:?}");
 e
}
fn has(e:&[UiTextInputEvent],id:u64,val:&str)->bool {
 e.iter().any(|x|x.callback_id==id && x.text==val && !x.submitted)
}
#[test]
fn real_x11_delayed_intermediate_echo_stale_version_and_new_external_authority() {
 let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
 let mut t=node("App",&[("bg","#131313")],vec![
   node("TextInput",&[("x","20"),("y","20"),("height","44"),
      ("width","220"),("value","A"),("value_revision","7"),("on_change","cb:101")],vec![]),
   node("TextInput",&[("x","20"),("y","90"),("height","44"),
      ("width","220"),("value","B"),("value_revision","2"),("on_change","cb:202")],vec![])
 ]);
 frames(&p,&t,&mut c,4);assert!(p.diagnostic_resize_window(640,400));
 frames(&p,&t,&mut c,4);
 let id=xdo(&["search","--name","Aura Lumina Sentinel"]);
 xdo(&["windowfocus","--sync",id.lines().next().unwrap()]);
 key(&p,&t,&mut c,"Tab");
 assert!(has(&key(&p,&t,&mut c,"q"),101,"Aq"));
 assert!(has(&key(&p,&t,&mut c,"e"),101,"Aqe"));
 // Out-of-order delayed host echo of an old edit (without authority bump).
 t.children[0].set_prop("value","Aq");
 frames(&p,&t,&mut c,3);
 assert!(has(&key(&p,&t,&mut c,"z"),101,"Aqez"),
  "older host echo must not destroy newer unsent edit");
 // A later echo of the newer (but still pre-current) edit is not authority.
 t.children[0].set_prop("value","Aqe");
 frames(&p,&t,&mut c,3);
 assert!(has(&key(&p,&t,&mut c,"x"),101,"Aqezx"));
 // Unknown same-revision value is not authorized to overwrite current draft.
 t.children[0].set_prop("value","INJECTED");
 frames(&p,&t,&mut c,3);
 assert!(has(&key(&p,&t,&mut c,"y"),101,"Aqezxy"));
 // Strictly increasing revision: externally authoritative reset.
 t.children[0].set_prop("value_revision","8");
 t.children[0].set_prop("value","SERVER");
 frames(&p,&t,&mut c,3);
 assert!(has(&key(&p,&t,&mut c,"k"),101,"SERVERk"));
 // A late packet with stale revision must not roll back state.
 t.children[0].set_prop("value_revision","7");
 t.children[0].set_prop("value","Aqe");
 frames(&p,&t,&mut c,3);
 assert!(has(&key(&p,&t,&mut c,"v"),101,"SERVERkv"));
 // Other input must remain independent with its own monotonic revision.
 key(&p,&t,&mut c,"Tab");
 assert!(has(&key(&p,&t,&mut c,"w"),202,"Bw"));
 // Malformed revision fails closed; none of the two should get edited.
 t.children[1].set_prop("value_revision","bad");
 frames(&p,&t,&mut c,3);
 assert!(key(&p,&t,&mut c,"p").is_empty());
 drop(p);
 eprintln!("LUMINA_REV_XTEST: late echo, unknown, stale, newer authority, two fields and malformed revision PASS");
}
