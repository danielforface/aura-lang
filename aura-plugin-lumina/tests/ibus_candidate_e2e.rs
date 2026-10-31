#![cfg(all(feature="lumina-diagnostics",target_os="linux"))]
// Genuine installed ibus-libpinyin daemon-engine lookup data via X11 XTest.
// No injected CompositionEvent and no fabricated candidate/commit records.
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiTextInputEvent,UiRuntimeFeedback};
use aura_plugin_lumina::AuraLuminaPlugin;
use std::process::Command;

fn node(kind:&str,props:&[(&str,&str)])->UiNode{
    let mut n=UiNode::new(kind);for &(k,v) in props{n.set_prop(k,v);}n
}
fn frame(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext)->Vec<UiTextInputEvent>{
    p.on_ui_render(t,c).expect("native owned").expect("render");
    c.get::<UiRuntimeFeedback>().expect("feedback").text_input_events.clone()
}
fn frames(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,n:usize)->Vec<UiTextInputEvent>{
    (0..n).flat_map(|_|frame(p,t,c)).collect()
}
fn xdo(args:&[&str])->String{
    let x=Command::new("xdotool").args(args).output().unwrap();
    assert!(x.status.success(),"XTest {args:?}: {}",String::from_utf8_lossy(&x.stderr));
    String::from_utf8_lossy(&x.stdout).trim().to_string()
}
fn key(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,k:&str)->Vec<UiTextInputEvent>{
    xdo(&["keydown",k]);let mut out=frames(p,t,c,5);
    xdo(&["keyup",k]);out.extend(frames(p,t,c,5));out
}
#[test]fn genuine_pinyin_engine_emits_bounded_lookup_candidates_without_early_commit(){
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"libpinyin");
    assert!(std::env::var_os("LUMINA_IBUS_HELPER_BIN").is_some());
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","420"),("height","70"),
        ("value",""),("on_change","cb:940"),("size","28"),
    ]));
    frames(&p,&app,&mut c,4);
    assert!(p.diagnostic_resize_window(760,520));
    frames(&p,&app,&mut c,4);
    let window=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=window.lines().next().expect("X11 window");
    xdo(&["windowfocus","--sync",id]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    assert_eq!(p.diagnostic_native_ibus(),Some(0));

    let mut callbacks=Vec::new();
    let mut snapshot=None;
    for k in ["n","i","h","a","o"] {
        callbacks.extend(key(&p,&app,&mut c,k));
        let lookup=p.diagnostic_native_ibus_candidates();
        eprintln!("LUMINA_CANDIDATE_LIVE_STEP key={k} page={lookup:?} composition={:?}",
            p.diagnostic_composition(940));
        if lookup.is_some(){snapshot=lookup;}
        assert!(callbacks.is_empty(),
            "Pinyin candidate preedit must never edit authoritative value before selection");
    }
    let (cb,cursor,start,total,items,events)=snapshot.expect(
        "MUST receive actual engine-originated IBus update-lookup-table candidate signal");
    assert_eq!(cb,940);
    assert!(cursor<total && start<=cursor && events>0);
    assert!(!items.is_empty() && items.len()<=8);
    assert!(items.iter().all(|s|!s.is_empty() && s.len()<=128));
    eprintln!("LUMINA_CANDIDATE_LIVE_PROOF count={total} selected={cursor} page_start={start} items={items:?} engine_updates={events}");

    callbacks.extend(key(&p,&app,&mut c,"space"));
    callbacks.extend(frames(&p,&app,&mut c,5));
    eprintln!("LUMINA_CANDIDATE_LIVE_COMMIT callbacks={callbacks:?}");
    assert_eq!(callbacks.len(),1,"one candidate selection -> one real authoritative commit");
    assert_eq!(callbacks[0].callback_id,940);
    assert!(!callbacks[0].text.is_empty());
    assert!(p.diagnostic_native_ibus_candidates().is_none(),
        "candidate table must be hidden after real commit");
}
