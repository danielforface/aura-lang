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
    let (px,py,pw,ph)=p.diagnostic_native_ibus_candidate_bounds()
        .expect("real candidate page must have a non-overlapping, window-bounded popup");
    eprintln!("LUMINA_CANDIDATE_LIVE_GPU_GEOMETRY x={px} y={py} w={pw} h={ph}");
    assert!(pw>=100.0 && ph>=44.0 && px>=0.0 && py>=0.0);
    assert!(px+pw<=760.0 && py+ph<=520.0);
    if cursor>=start && cursor-start<items.len(){
        let tint=p.diagnostic_capture_pixel(
            px+8.0,py+6.0+(cursor-start) as f32*32.0+14.0)
            .expect("native selection highlight sample");
        eprintln!("LUMINA_CANDIDATE_LIVE_GPU_HIGHLIGHT {tint:?}");
        assert!(tint.b>tint.g && tint.g>tint.r,
            "actual native candidate highlight must be blue");
    }
    let glyphs=p.diagnostic_native_ibus_candidate_glyph_pixels()
        .expect("real CJK font atlas must be available for candidate popup");
    eprintln!("LUMINA_CANDIDATE_LIVE_GPU_GLYPH_PIXELS {glyphs}");
    assert!(glyphs>10,"real CJK candidates must draw visible glyph pixels");

    callbacks.extend(key(&p,&app,&mut c,"space"));
    callbacks.extend(frames(&p,&app,&mut c,5));
    eprintln!("LUMINA_CANDIDATE_LIVE_COMMIT callbacks={callbacks:?}");
    assert_eq!(callbacks.len(),1,"one candidate selection -> one real authoritative commit");
    assert_eq!(callbacks[0].callback_id,940);
    assert!(!callbacks[0].text.is_empty());
    assert!(p.diagnostic_native_ibus_candidates().is_none(),
        "candidate table must be hidden after real commit");
}

#[test]fn real_pinyin_candidate_arrow_selection_and_blur_never_auto_commit(){
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","420"),("height","70"),
        ("value",""),("on_change","cb:941"),("size","28"),
    ]));
    frames(&p,&app,&mut c,4);
    assert!(p.diagnostic_resize_window(760,520));
    frames(&p,&app,&mut c,4);
    let window=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=window.lines().next().unwrap();
    xdo(&["windowfocus","--sync",id]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    assert!(key(&p,&app,&mut c,"n").is_empty());
    assert!(key(&p,&app,&mut c,"i").is_empty());
    let (_,before,_,_,_,events)=p.diagnostic_native_ibus_candidates()
        .expect("actual Pinyin lookup before navigation");
    assert!(events>0);
    assert!(key(&p,&app,&mut c,"Down").is_empty(),
        "native candidate cursor movement is not a text Commit");
    let (_,selected,start,total,items,after_events)=p.diagnostic_native_ibus_candidates()
        .expect("engine must still own lookup after navigation");
    eprintln!("LUMINA_CANDIDATE_LIVE_NAV before={before} selected={selected} start={start} total={total} shown={items:?} engine_events={after_events}");
    assert!(selected<total && selected>=start && selected<start+items.len());
    assert_ne!(selected,before,
        "real physical XTest Down must change Pinyin candidate cursor");
    assert!(after_events>events,"engine candidate update must be real, not local counter");
    // Blur while a different candidate is selected: no automatic commit.
    xdo(&["mousemove","--window",id,"700","410"]);xdo(&["click","1"]);
    assert!(frames(&p,&app,&mut c,6).is_empty());
    assert!(p.diagnostic_native_ibus_candidates().is_none(),
        "FocusOut must discard the engine lookup table");
    assert!(p.diagnostic_composition(941).is_none(),
        "FocusOut must also discard engine preedit");
}
