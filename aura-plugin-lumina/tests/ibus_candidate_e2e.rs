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
    xdo(&["mousemove","--window",id,"700","410"]);
    xdo(&["mousedown","1"]);
    let mut blur_callbacks=frames(&p,&app,&mut c,5);
    xdo(&["mouseup","1"]);
    blur_callbacks.extend(frames(&p,&app,&mut c,5));
    eprintln!("LUMINA_CANDIDATE_LIVE_BLUR callbacks={blur_callbacks:?} editor={:?} lookup={:?}",
        p.diagnostic_composition(941),p.diagnostic_native_ibus_candidates());
    assert!(blur_callbacks.is_empty());
    assert!(p.diagnostic_native_ibus_candidates().is_none(),
        "FocusOut must discard the engine lookup table");
    assert!(p.diagnostic_composition(941).is_none(),
        "FocusOut must also discard engine preedit");
}

#[test]fn real_pinyin_candidate_mouse_click_commits_selected_engine_text(){
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","420"),("height","70"),
        ("value",""),("on_change","cb:942"),("size","28"),
    ]));
    frames(&p,&app,&mut c,4);
    assert!(p.diagnostic_resize_window(760,520));
    frames(&p,&app,&mut c,4);
    let windows=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let window=windows.lines().next().unwrap();
    xdo(&["windowfocus","--sync",window]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    for ch in ["n","i","h","a","o"] {
        assert!(key(&p,&app,&mut c,ch).is_empty(),
            "real Pinyin must not commit a preedit while typing");
    }
    let (cb,cursor,start,_,candidates,_)=p.diagnostic_native_ibus_candidates()
        .expect("live Pinyin engine must emit candidate table");
    assert_eq!(cb,942);
    assert_eq!(cursor,0);
    assert_eq!(start,0);
    assert!(candidates.len()>=2);
    let expected=candidates[1].clone();
    let (px,py,pw,ph)=p.diagnostic_native_ibus_candidate_bounds()
        .expect("real rendered popup");
    assert!(pw>150.0 && ph>70.0);
    let at_x=(px+22.0).round() as i32;
    let at_y=(py+6.0+32.0+14.0).round() as i32;
    xdo(&["mousemove","--window",window,&at_x.to_string(),&at_y.to_string()]);
    xdo(&["mousedown","1"]);
    let mut callbacks=frames(&p,&app,&mut c,5);
    xdo(&["mouseup","1"]);
    callbacks.extend(frames(&p,&app,&mut c,5));
    eprintln!("LUMINA_IBUS_P20_MOUSE clicked=1 selected={expected:?} callbacks={callbacks:?} remaining={:?} ack={:?}",
        p.diagnostic_native_ibus_candidates(),p.diagnostic_native_ibus_candidate_clicks());
    assert_eq!(callbacks.len(),1,"a real candidate mouse click must commit once");
    assert_eq!(callbacks[0].callback_id,942);
    assert_eq!(callbacks[0].text,expected,
        "only the real engine's second candidate may be committed");
    assert_eq!(p.diagnostic_native_ibus_candidate_clicks(),Some(1));
    assert!(p.diagnostic_native_ibus_candidates().is_none());
}
#[test]fn real_pinyin_page_navigation_uses_native_ibus_key_and_updates_lookup(){
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","420"),("height","70"),
        ("value",""),("on_change","cb:943"),("size","28"),
    ]));
    frames(&p,&app,&mut c,4);
    assert!(p.diagnostic_resize_window(760,520));
    frames(&p,&app,&mut c,4);
    let windows=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let window=windows.lines().next().unwrap();
    xdo(&["windowfocus","--sync",window]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    for ch in ["n","i","h","a","o"]{assert!(key(&p,&app,&mut c,ch).is_empty());}
    let (_,old_cursor,old_start,total,old_items,old_events)=
        p.diagnostic_native_ibus_candidates().expect("original live page");
    assert!(total>10&&old_items.len()>=3);
    assert!(key(&p,&app,&mut c,"Next").is_empty(),
        "page navigation must never commit text into editor");
    let (_,new_cursor,new_start,new_total,new_items,new_events)=
        p.diagnostic_native_ibus_candidates().expect("real page-down engine update");
    eprintln!("LUMINA_IBUS_P20_PAGE_DOWN old=({old_cursor},{old_start},{old_items:?},{old_events}) new=({new_cursor},{new_start},{new_items:?},{new_events})");
    assert!(new_total>0 && new_cursor<new_total);
    // The live engine may recompute candidate totals when a page changes.
    // Never require 130 old predictions to remain 130 after PageDown.
    assert!(new_events>old_events,"live engine must update lookup on PageDown");
    assert!(new_start>old_start||new_cursor>old_cursor,
        "physical PageDown must advance real libpinyin candidates");
    assert!(key(&p,&app,&mut c,"Prior").is_empty());
    let (_,back_cursor,back_start,_,_,back_events)=p.diagnostic_native_ibus_candidates()
        .expect("live IBus page-up update");
    eprintln!("LUMINA_IBUS_P20_PAGE_UP cursor={back_cursor} first={back_start} updates={back_events}");
    assert!(back_events>new_events);
    assert_eq!(back_start,old_start,"PageUp should return to first engine page");
    assert!(back_cursor<=old_items.len(),"live cursor returned into first page");
}

#[test]fn real_pinyin_stale_lookup_click_after_host_revision_does_not_commit(){
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","420"),("height","70"),
        ("value","A"),("value_revision","1"),("on_change","cb:944"),("size","28"),
    ]));
    frames(&p,&app,&mut c,4);
    assert!(p.diagnostic_resize_window(760,520));
    frames(&p,&app,&mut c,4);
    let windows=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let window=windows.lines().next().unwrap();
    xdo(&["windowfocus","--sync",window]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    for ch in ["n","i"]{assert!(key(&p,&app,&mut c,ch).is_empty());}
    assert!(p.diagnostic_native_ibus_candidates().is_some());
    let (px,py,_,_)=p.diagnostic_native_ibus_candidate_bounds().unwrap();
    // Authority changes immediately before XTest mouse click is sampled.
    // The prior candidate snapshot is stale and must NEVER commit into B.
    app.children[0].set_prop("value","B");
    app.children[0].set_prop("value_revision","2");
    let xx=(px+22.0).round() as i32;
    let yy=(py+6.0+32.0+14.0).round() as i32;
    xdo(&["mousemove","--window",window,&xx.to_string(),&yy.to_string()]);
    xdo(&["mousedown","1"]);
    let mut events=frames(&p,&app,&mut c,5);
    xdo(&["mouseup","1"]);
    events.extend(frames(&p,&app,&mut c,5));
    eprintln!("LUMINA_IBUS_P20_REBASE_STALE_CLICK events={events:?} lookup={:?} clicks={:?} composition={:?}",
        p.diagnostic_native_ibus_candidates(),p.diagnostic_native_ibus_candidate_clicks(),
        p.diagnostic_composition(944));
    assert!(events.iter().all(|e|e.callback_id!=944));
    assert_eq!(p.diagnostic_native_ibus_candidate_clicks(),Some(0));
    assert!(p.diagnostic_native_ibus_candidates().is_none());
    assert!(p.diagnostic_composition(944).is_none());
}

#[test]fn real_pinyin_caret_location_scroll_and_popup_anchor(){
    assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"libpinyin");
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    let long="W".repeat(100);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","420"),("height","70"),
        ("value",&long),("value_revision","1"),("on_change","cb:945"),("size","28"),
    ]));
    frames(&p,&app,&mut c,4);
    assert!(p.diagnostic_resize_window(760,520));
    frames(&p,&app,&mut c,4);
    let win=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=win.lines().next().unwrap();
    xdo(&["windowfocus","--sync",id]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    for ch in ["n","i"]{assert!(key(&p,&app,&mut c,ch).is_empty());}
    assert!(p.diagnostic_native_ibus_candidates().is_some());
    let (screen,acks)=p.diagnostic_native_ibus_cursor_location()
        .expect("actual IBus set_cursor_location ACK");
    assert!(acks>=1 && screen.2>=1 && screen.3>=1);
    let (cx,cy,cw,ch)=p.diagnostic_native_ibus_caret_logical().unwrap();
    let visual=p.diagnostic_text_visual(945,32.0,396.0).unwrap();
    let (px,py,pw,ph)=p.diagnostic_native_ibus_candidate_bounds().unwrap();
    eprintln!("LUMINA_IBUS_P21_CARET_SCROLL location={screen:?} ack={acks} logical=({cx},{cy},{cw},{ch}) scroll={} measured_caret={} measured_screen={} popup=({px},{py},{pw},{ph})",
        visual.scroll_x,visual.measured_caret,visual.caret_screen_x);
    assert!(visual.scroll_x>100.0,"long native TextInput must horizontally scroll");
    assert!((cx-visual.caret_screen_x).abs()<3.0,
        "popup must use measured scrolled caret not raw grapheme index");
    assert!((px-cx.clamp(0.0,760.0-360.0)).abs()<=2.0);
    assert!((py-(cy+ch+4.0)).abs()<=2.0);
    assert!(p.diagnostic_native_ibus_candidate_glyph_pixels().unwrap()>10);
    // Move the same focused field while composing; IBus screen rectangle
    // and native popup must follow, without authority edit or ghost commit.
    app.children[0].set_prop("y","145");
    assert!(frames(&p,&app,&mut c,4).is_empty());
    let (moved,moved_acks)=p.diagnostic_native_ibus_cursor_location().unwrap();
    let (_,new_y,_,_)=p.diagnostic_native_ibus_caret_logical().unwrap();
    let (_,popup_y,_,_)=p.diagnostic_native_ibus_candidate_bounds().unwrap();
    eprintln!("LUMINA_IBUS_P21_CARET_MOVE previous={screen:?} moved={moved:?} ACKS={acks}->{moved_acks} local_y={cy}->{new_y} popup_y={popup_y}");
    assert!(moved_acks>acks);
    assert!(moved.1>screen.1+100,"real IBus screen Y must follow window-local geometry");
    assert!(new_y>cy+100.0 && popup_y>py+100.0);
    assert!(p.diagnostic_native_ibus_candidates().is_some());
}
#[test]fn real_pinyin_popup_flips_above_visible_caret_near_bottom_edge(){
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","405"),("width","420"),("height","70"),
        ("value",""),("on_change","cb:946"),("size","28"),
    ]));
    frames(&p,&app,&mut c,4);
    assert!(p.diagnostic_resize_window(760,520));
    frames(&p,&app,&mut c,4);
    let window=xdo(&["search","--name","Aura Lumina Sentinel"]);
    xdo(&["windowfocus","--sync",window.lines().next().unwrap()]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    for ch in ["n","i"]{assert!(key(&p,&app,&mut c,ch).is_empty());}
    let (cx,cy,_,_)=p.diagnostic_native_ibus_caret_logical().unwrap();
    let (x,y,w,h)=p.diagnostic_native_ibus_candidate_bounds().unwrap();
    let (screen,ack)=p.diagnostic_native_ibus_cursor_location().unwrap();
    eprintln!("LUMINA_IBUS_P21_BOTTOM_EDGE caret=({cx},{cy}) popup=({x},{y},{w},{h}) native_screen={screen:?} ack={ack}");
    assert!(ack>=1);
    assert!(y+h<cy,"popup must flip above caret with no offscreen rendering");
    assert!(y>=0.0&&x>=0.0&&x+w<=760.0);
    assert!(p.diagnostic_native_ibus_candidate_glyph_pixels().unwrap()>10);
    assert!(key(&p,&app,&mut c,"space").iter()
        .all(|e|e.callback_id==946),"real engine remains functional at bottom edge");
}

#[test]fn real_pinyin_x11_window_move_updates_absolute_ibus_cursor_origin(){
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","420"),("height","70"),
        ("value",""),("on_change","cb:947"),("size","28"),
    ]));
    frames(&p,&app,&mut c,4);
    assert!(p.diagnostic_resize_window(760,520));
    frames(&p,&app,&mut c,4);
    let wins=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=wins.lines().next().unwrap();
    xdo(&["windowfocus","--sync",id]);
    assert!(key(&p,&app,&mut c,"Tab").is_empty());
    for ch in ["n","i"]{assert!(key(&p,&app,&mut c,ch).is_empty());}
    let (before,acks)=p.diagnostic_native_ibus_cursor_location()
        .expect("genuine IBus native cursor location");
    let caret=p.diagnostic_native_ibus_caret_logical().unwrap();
    let popup=p.diagnostic_native_ibus_candidate_bounds().unwrap();
    xdo(&["windowmove","--sync",id,"120","85"]);
    assert!(frames(&p,&app,&mut c,8).is_empty());
    let (after,ack_after)=p.diagnostic_native_ibus_cursor_location().unwrap();
    let caret_after=p.diagnostic_native_ibus_caret_logical().unwrap();
    let popup_after=p.diagnostic_native_ibus_candidate_bounds().unwrap();
    eprintln!("LUMINA_IBUS_P21_WINDOW_MOVE before={before:?} after={after:?} ACK={acks}->{ack_after} caret={caret:?}->{caret_after:?} popup={popup:?}->{popup_after:?}");
    assert!(ack_after>acks,"moving the X11 window must resend real libibus screen coords");
    assert!(after.0>before.0+70 && after.1>before.1+40,
        "IBus cursor origin must include real X11 window translation");
    assert_eq!(caret,caret_after,"window move cannot change logical caret");
    assert_eq!(popup,popup_after,"window move cannot change local popup geometry");
    assert!(p.diagnostic_native_ibus_candidate_glyph_pixels().unwrap()>10);
    // Existing candidate selection remains engine-owned after X11 translation.
    assert!(key(&p,&app,&mut c,"space").iter()
        .any(|e|e.callback_id==947 && !e.text.is_empty()));
}
