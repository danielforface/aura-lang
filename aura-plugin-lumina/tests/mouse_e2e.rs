#![cfg(feature = "lumina-diagnostics")]
// Real X11/XTest mouse input. No synthetic internal hit-test injection.
// Requires the CI Xvfb environment and xdotool installed explicitly.

use std::process::Command;
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback};
use aura_plugin_lumina::AuraLuminaPlugin;

fn node(kind:&str,props:&[(&str,&str)],children:Vec<UiNode>)->UiNode {
    let mut n=UiNode::new(kind);
    for &(k,v) in props { n.set_prop(k,v); }
    n.children=children;n
}
fn xdotool(args:&[&str])->String {
    let output=Command::new("xdotool").args(args).output().expect("xdotool present");
    assert!(output.status.success(),"xdotool {:?}: {}",args,
        String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}
fn render(plugin:&AuraLuminaPlugin,tree:&UiNode,nexus:&mut NexusContext)->Option<u64>{
    plugin.on_ui_render(tree,nexus).expect("native UI owned").expect("native draw succeeded");
    nexus.get::<UiRuntimeFeedback>().expect("frame feedback").clicked_callback_id
}
fn click(plugin:&AuraLuminaPlugin,tree:&UiNode,nexus:&mut NexusContext,
    window:&str,x:i32,y:i32)->Vec<u64>{
    let _=xdotool(&["mousemove","--window",window,&x.to_string(),&y.to_string()]);
    for _ in 0..3 { render(plugin,tree,nexus); }
    let _=xdotool(&["mousedown","1"]);
    let mut got=Vec::new();
    for _ in 0..6 {
        if let Some(cb)=render(plugin,tree,nexus){got.push(cb);}
    }
    let _=xdotool(&["mouseup","1"]);
    for _ in 0..3{render(plugin,tree,nexus);}
    eprintln!("LUMINA_XTEST_CLICK {x},{y} -> {got:?}");
    got
}

#[test]
fn real_x11_mouse_topmost_click_and_hidden_overflow_guard() {
    let plugin=AuraLuminaPlugin::new();
    let mut nexus=NexusContext::default();
    let tree=node("App",&[("bg","#102030")],vec![
        node("Button",&[("x","20"),("y","20"),("width","90"),
            ("height","45"),("on_click","cb:11"),("label","under")],vec![]),
        node("Button",&[("x","35"),("y","25"),("width","90"),
            ("height","45"),("on_click","cb:22"),("label","top")],vec![]),
        node("Box",&[("x","200"),("y","20"),("width","50"),("height","50"),
            ("overflow","hidden")],vec![
            node("Button",&[("x","270"),("y","25"),("width","70"),
                ("height","45"),("on_click","cb:33")],vec![])
        ])
    ]);
    for _ in 0..3 {render(&plugin,&tree,&mut nexus);}
    assert!(plugin.diagnostic_resize_window(640,420));
    for _ in 0..4 {render(&plugin,&tree,&mut nexus);}
    let windows=xdotool(&["search","--name","Aura Lumina Sentinel"]);
    let id=windows.lines().next().expect("actual native window found");
    eprintln!("LUMINA_XTEST_WINDOW id={id}");
    let hit=click(&plugin,&tree,&mut nexus,id,50,40);
    assert!(hit.contains(&22),"topmost overlapping Button was not dispatched: {hit:?}");
    assert!(!hit.contains(&11),"underneath actionable Button stole the pointer");
    let clipped=click(&plugin,&tree,&mut nexus,id,290,35);
    assert!(clipped.is_empty(),"clipped off-Box Button unexpectedly received click: {clipped:?}");
    drop(plugin);
}

#[test]
fn real_x11_disabled_button_blocks_click_through_and_tracks_host_state() {
    let plugin=AuraLuminaPlugin::new();
    let mut nexus=NexusContext::default();
    let mut tree=node("App",&[("bg","#102030")],vec![
        node("Button",&[("x","20"),("y","20"),("width","120"),
            ("height","70"),("on_click","cb:41"),("label","back")],vec![]),
        node("Button",&[("x","35"),("y","25"),("width","90"),
            ("height","45"),("on_click","cb:42"),("label","top"),
            ("disabled","true")],vec![]),
        node("Button",&[("x","200"),("y","20"),("width","90"),
            ("height","45"),("on_click","cb:43"),("label","enabled")],vec![]),
    ]);
    for _ in 0..3 {render(&plugin,&tree,&mut nexus);}
    assert!(plugin.diagnostic_resize_window(640,420));
    for _ in 0..4 {render(&plugin,&tree,&mut nexus);}
    let windows=xdotool(&["search","--name","Aura Lumina Sentinel"]);
    let id=windows.lines().next().expect("actual native X11 window");
    let disabled=click(&plugin,&tree,&mut nexus,id,50,40);
    assert!(disabled.is_empty(),
        "disabled topmost Button must neither dispatch nor activate the lower Button: {disabled:?}");
    let under=click(&plugin,&tree,&mut nexus,id,25,30);
    assert!(under.contains(&41),"uncovered enabled Button must remain clickable: {under:?}");
    let other=click(&plugin,&tree,&mut nexus,id,230,40);
    assert!(other.contains(&43),"independent enabled Button must stay actionable: {other:?}");
    tree.children[1].set_prop("disabled","false");
    let enabled=click(&plugin,&tree,&mut nexus,id,50,40);
    assert!(enabled.contains(&42),"host enabling must restore topmost click: {enabled:?}");
    assert!(!enabled.contains(&41),"topmost enabled Button must not pass click through");
    tree.children[1].set_prop("disabled","true");
    let disabled_again=click(&plugin,&tree,&mut nexus,id,50,40);
    assert!(disabled_again.is_empty(),
        "host re-disabling must remove native callback immediately: {disabled_again:?}");
    eprintln!("LUMINA_DISABLED_BUTTON_X11 disabled_blocks=true uncovered_enabled=true dynamic_reenable=true disabled_again_blocks=true");
}


/// Native controlled selection contract: a click emits a callback, but the
/// visual state stays unchanged until the Aura host supplies a new checked
/// prop. Real software-OpenGL X11 framebuffers are sampled for both widgets.
#[test]
fn real_x11_controlled_checkbox_and_switch_host_state_pixel_contract() {
    let plugin=AuraLuminaPlugin::new();
    let mut nexus=NexusContext::default();
    let mut tree=node("App",&[("bg","#101820")],vec![
        node("Button",&[("x","20"),("y","20"),("width","210"),("height","50"),
            ("control","checkbox"),("checked","false"),("label","Remember choice"),
            ("on_click","cb:71")],vec![]),
        node("Button",&[("x","20"),("y","100"),("width","210"),("height","50"),
            ("control","switch"),("checked","false"),("label","Enable alerts"),
            ("on_click","cb:72")],vec![]),
        node("Button",&[("x","280"),("y","20"),("width","175"),("height","56"),
            ("on_click","cb:73"),("label","Behind invalid")],vec![]),
        node("Button",&[("x","290"),("y","25"),("width","150"),("height","48"),
            ("control","checkbox"),("checked","sometimes"),
            ("on_click","cb:74")],vec![]),
        node("Button",&[("x","280"),("y","100"),("width","175"),("height","50"),
            ("control","switch"),("on_click","cb:75")],vec![]),
    ]);
    for _ in 0..5 {render(&plugin,&tree,&mut nexus);}
    assert!(plugin.diagnostic_resize_window(640,400));
    for _ in 0..5 {render(&plugin,&tree,&mut nexus);}
    let windows=xdotool(&["search","--name","Aura Lumina Sentinel"]);
    let id=windows.lines().next().expect("native X11 window");
    let initial_checkbox=plugin.diagnostic_capture_pixel(38.0,38.0)
        .expect("checkbox GPU pixel before host update");
    let initial_switch=plugin.diagnostic_capture_pixel(55.0,117.0)
        .expect("switch GPU track pixel before host update");

    assert_eq!(click(&plugin,&tree,&mut nexus,id,44,45),vec![71],
        "click must use the established on_click channel");
    let unchanged_checkbox=plugin.diagnostic_capture_pixel(38.0,38.0).unwrap();
    assert_eq!(initial_checkbox,unchanged_checkbox,
        "native click must not optimistically mutate host-owned checked");
    tree.children[0].set_prop("checked","true");
    for _ in 0..5 {render(&plugin,&tree,&mut nexus);}
    let checked_checkbox=plugin.diagnostic_capture_pixel(38.0,38.0).unwrap();
    assert!(checked_checkbox.g>initial_checkbox.g.saturating_add(65),
        "checked host prop must produce visible green checkbox indicator: off={initial_checkbox:?}, on={checked_checkbox:?}");

    assert_eq!(click(&plugin,&tree,&mut nexus,id,52,125),vec![72]);
    let unchanged_switch=plugin.diagnostic_capture_pixel(55.0,117.0).unwrap();
    assert_eq!(initial_switch,unchanged_switch,
        "switch must not toggle a local copy before authoritative host update");
    tree.children[1].set_prop("checked","true");
    for _ in 0..5 {render(&plugin,&tree,&mut nexus);}
    let checked_switch=plugin.diagnostic_capture_pixel(55.0,117.0).unwrap();
    assert!(checked_switch.g>initial_switch.g.saturating_add(65),
        "host checked must visibly toggle native switch track: off={initial_switch:?}, on={checked_switch:?}");

    // Unknown control boolean and missing checked fail closed, including
    // click-through to an underlying ordinary Button.
    assert!(click(&plugin,&tree,&mut nexus,id,320,45).is_empty(),
        "invalid checkbox must block own callback and lower overlapping button");
    assert!(click(&plugin,&tree,&mut nexus,id,325,125).is_empty(),
        "missing mandatory checked must fail closed");

    tree.children[1].set_prop("disabled","true");
    assert!(click(&plugin,&tree,&mut nexus,id,52,125).is_empty(),
        "disabled controlled switch cannot fire");
    tree.children[1].set_prop("disabled","false");
    assert_eq!(click(&plugin,&tree,&mut nexus,id,52,125),vec![72],
        "re-enabled switch becomes actionable without stale state copy");
    tree.children[1].set_prop("checked","false");
    for _ in 0..5 {render(&plugin,&tree,&mut nexus);}
    let turned_off=plugin.diagnostic_capture_pixel(55.0,117.0).unwrap();
    assert_eq!(turned_off,initial_switch,
        "host false update must restore exact off-track color");

    eprintln!("LUMINA_CONTROLLED_SELECTION_X11 checkbox_click=true switch_click=true host_only_visuals=true malformed_blocks_underlay=true missing_checked_blocks=true disable_reenable=true pixel_off_on_off=true");
}


/// A single-select radio group is host-owned. The native renderer samples
/// actual GPU pixels, while real XTest mouse clicks reach public callbacks.
#[test]
fn real_x11_radio_group_exclusivity_conflict_recovery_and_gpu_state() {
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut tree=node("App",&[("bg","#101820")],vec![
        node("Button",&[("x","20"),("y","20"),("width","220"),("height","55"),
            ("on_click","cb:10"),("label","Underlay")],vec![]),
        node("Button",&[("x","30"),("y","25"),("width","200"),("height","50"),
            ("control","radio"),("group","plan"),("checked","true"),
            ("label","Standard"),("on_click","cb:21")],vec![]),
        node("Button",&[("x","30"),("y","100"),("width","200"),("height","50"),
            ("control","radio"),("group","plan"),("checked","false"),
            ("label","Premium"),("on_click","cb:22")],vec![]),
        node("Button",&[("x","30"),("y","175"),("width","200"),("height","50"),
            ("control","radio"),("group","other"),("checked","false"),
            ("label","Other"),("on_click","cb:31")],vec![]),
        node("Button",&[("x","30"),("y","250"),("width","200"),("height","50"),
            ("control","radio"),("checked","false"),
            ("label","Missing group"),("on_click","cb:41")],vec![]),
        node("Button",&[("x","280"),("y","25"),("width","180"),("height","50"),
            ("control","checkbox"),("checked","false"),
            ("label","Stay active"),("on_click","cb:51")],vec![]),
    ]);
    for _ in 0..5{render(&plugin,&tree,&mut ctx);}
    assert!(plugin.diagnostic_resize_window(640,420));
    for _ in 0..5{render(&plugin,&tree,&mut ctx);}
    let windows=xdotool(&["search","--name","Aura Lumina Sentinel"]);
    let id=windows.lines().next().expect("native X11 window");
    let selected=plugin.diagnostic_capture_pixel(55.0,50.0)
        .expect("radio A selected pixel");
    let off=plugin.diagnostic_capture_pixel(55.0,125.0)
        .expect("radio B off pixel");
    assert!(selected.g>off.g.saturating_add(65),
        "native radio dot must be green only for host selected: selected={selected:?} off={off:?}");

    assert_eq!(click(&plugin,&tree,&mut ctx,id,65,125),vec![22],
        "radio pointer emits original callback, no synthetic selection change");
    let unchanged=plugin.diagnostic_capture_pixel(55.0,125.0).unwrap();
    assert_eq!(unchanged,off,"click alone must not mutate checked");
    tree.children[1].set_prop("checked","false");
    tree.children[2].set_prop("checked","true");
    for _ in 0..5{render(&plugin,&tree,&mut ctx);}
    let host_selected=plugin.diagnostic_capture_pixel(55.0,125.0).unwrap();
    assert!(host_selected.g>off.g.saturating_add(65),
        "host atomic group update must produce actual checked radio dot");
    let first_off=plugin.diagnostic_capture_pixel(55.0,50.0).unwrap();
    assert_eq!(first_off,off,"both radio options use same off-dot paint");

    // Host bug: conflicting selected values. Block all conflicted members,
    // including click-through into an overlapped ordinary Button.
    tree.children[1].set_prop("checked","true");
    for _ in 0..4{render(&plugin,&tree,&mut ctx);}
    assert!(click(&plugin,&tree,&mut ctx,id,65,50).is_empty(),
        "invalid top radio must not activate the ordinary underlying Button");
    assert!(click(&plugin,&tree,&mut ctx,id,65,125).is_empty(),
        "second checked radio must fail closed until authoritative repair");
    assert_eq!(click(&plugin,&tree,&mut ctx,id,65,200),vec![31],
        "a separate radio group must keep working");
    assert!(click(&plugin,&tree,&mut ctx,id,65,275).is_empty(),
        "missing mandatory group token must fail closed");
    assert_eq!(click(&plugin,&tree,&mut ctx,id,310,50),vec![51],
        "independent Checkbox must not be blocked by radio group conflict");

    tree.children[1].set_prop("checked","false");
    for _ in 0..4{render(&plugin,&tree,&mut ctx);}
    assert_eq!(click(&plugin,&tree,&mut ctx,id,65,50),vec![21],
        "after host repair radio callback is actionable again");
    tree.children[1].set_prop("disabled","true");
    assert!(click(&plugin,&tree,&mut ctx,id,65,50).is_empty());
    tree.children[1].set_prop("disabled","false");
    assert_eq!(click(&plugin,&tree,&mut ctx,id,65,50),vec![21]);
    eprintln!("LUMINA_RADIO_GROUP_MOUSE_X11 host_checked_pixels=true no_optimistic_toggle=true duplicate_selected_fail_closed=true underlay_block=true independent_groups=true malformed_group_block=true repair=true disable_reenable=true");
}


/// Real X11 form submission is gated by native Unicode-aware field rules.
/// Every on_click is observed via Nexus feedback; invalid submits never leak.
#[test]
fn real_x11_form_submit_required_min_graphemes_error_border_and_first_focus(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let tree=node("App",&[("bg","#101820")],vec![
        node("TextInput",&[("x","20"),("y","20"),("width","230"),("height","46"),
            ("radius","0"),("form","register"),("required","true"),
            ("on_change","cb:501"),("value","")],vec![]),
        node("TextInput",&[("x","20"),("y","95"),("width","230"),("height","46"),
            ("radius","0"),("form","register"),("min_graphemes","2"),
            ("on_change","cb:502"),("value","x")],vec![]),
        node("Button",&[("x","20"),("y","175"),("width","220"),("height","50"),
            ("form_submit","register"),("on_click","cb:590"),("label","Save")],vec![]),
        node("TextInput",&[("x","310"),("y","95"),("width","180"),("height","46"),
            ("form","other"),("required","true"),
            ("on_change","cb:601"),("value","")],vec![]),
        node("Button",&[("x","310"),("y","175"),("width","180"),("height","50"),
            ("on_click","cb:690"),("label","Independent")],vec![]),
    ]);
    for _ in 0..4{render(&plugin,&tree,&mut ctx);}
    assert!(plugin.diagnostic_resize_window(640,420));
    for _ in 0..4{render(&plugin,&tree,&mut ctx);}
    let id=xdotool(&["search","--name","Aura Lumina Sentinel"]);
    let id=id.lines().next().expect("native form X11 window");
    let before=plugin.diagnostic_capture_pixel(21.0,43.0).unwrap();
    assert!(click(&plugin,&tree,&mut ctx,id,100,195).is_empty(),
        "invalid required/min-graphemes must block callback 590");
    let red=plugin.diagnostic_capture_pixel(21.0,43.0).unwrap();
    assert!(red.r>before.r.saturating_add(75),
        "error must visibly mark first native TextInput border: before={before:?}, after={red:?}");

    // Invalid submit must have focused first required field; type without
    // manually clicking it. The committed EditSession, not stale host echo,
    // must be considered for the following submit.
    xdotool(&["keydown","a"]);
    let mut first=Vec::new();
    for _ in 0..6{
        render(&plugin,&tree,&mut ctx);
        first.extend(ctx.get::<UiRuntimeFeedback>().unwrap().text_input_events.clone());
    }
    xdotool(&["keyup","a"]);
    for _ in 0..3{render(&plugin,&tree,&mut ctx);}
    assert!(first.iter().any(|e|e.callback_id==501 && e.text=="a"),
        "first invalid field gets keyboard focus after blocked submit: {first:?}");
    assert!(click(&plugin,&tree,&mut ctx,id,100,195).is_empty(),
        "second short field must still block submit after first local repair");
    xdotool(&["keydown","z"]);
    let mut second=Vec::new();
    for _ in 0..6{
        render(&plugin,&tree,&mut ctx);
        second.extend(ctx.get::<UiRuntimeFeedback>().unwrap().text_input_events.clone());
    }
    xdotool(&["keyup","z"]);
    for _ in 0..3{render(&plugin,&tree,&mut ctx);}
    assert!(second.iter().any(|e|e.callback_id==502 && e.text=="xz"),
        "next invalid field receives focus and retained host value: {second:?}");
    let clear=plugin.diagnostic_capture_pixel(21.0,43.0).unwrap();
    assert_eq!(clear,before,"successful local correction removes red error on subsequent frame");
    assert_eq!(click(&plugin,&tree,&mut ctx,id,100,195),vec![590],
        "all corrected current-session values permit original callback once");
    assert_eq!(click(&plugin,&tree,&mut ctx,id,340,195),vec![690],
        "independent ordinary button remains functional");
    eprintln!("LUMINA_FORM_SUBMIT_X11 required_blocks=true min_graphemes_blocks=true first_invalid_focus=true committed_draft_authority=true error_border=true error_clears=true valid_callback_once=true independent_button=true");
}


/// Real X11 single-selection form + app-owned async submit lifecycle.
/// Neither busy nor remote error status may manufacture another callback.
#[test]
fn real_x11_required_radio_form_busy_error_retry_and_native_pixels(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut tree=node("App",&[("bg","#101820")],vec![
        node("Button",&[("x","20"),("y","20"),("width","245"),("height","50"),
            ("control","radio"),("group","tier"),("form","checkout"),
            ("required","true"),("checked","false"),
            ("on_click","cb:101"),("label","Standard")],vec![]),
        node("Button",&[("x","20"),("y","90"),("width","245"),("height","50"),
            ("control","radio"),("group","tier"),("form","checkout"),
            ("checked","false"),("on_click","cb:102"),("label","Plus")],vec![]),
        node("Button",&[("x","20"),("y","220"),("width","240"),("height","60"),
            ("on_click","cb:999"),("label","Behind")],vec![]),
        node("Button",&[("x","20"),("y","220"),("width","240"),("height","60"),
            ("form_submit","checkout"),("on_click","cb:300"),
            ("form_status","ready"),("label","Place order")],vec![]),
        node("Button",&[("x","315"),("y","220"),("width","200"),("height","55"),
            ("on_click","cb:900"),("label","Independent")],vec![]),
    ]);
    for _ in 0..4{render(&plugin,&tree,&mut ctx);}
    assert!(plugin.diagnostic_resize_window(640,420));
    for _ in 0..4{render(&plugin,&tree,&mut ctx);}
    let wins=xdotool(&["search","--name","Aura Lumina Sentinel"]);
    let id=wins.lines().next().expect("native X11 form");
    let unchecked_border=plugin.diagnostic_capture_pixel(21.0,45.0).unwrap();
    assert!(click(&plugin,&tree,&mut ctx,id,100,245).is_empty(),
        "required Radio group must block normal Submit with no selection");
    let invalid_border=plugin.diagnostic_capture_pixel(21.0,45.0).unwrap();
    assert!(invalid_border.r>unchecked_border.r.saturating_add(60),
        "native radio shows group validation error in pixels: {unchecked_border:?} -> {invalid_border:?}");
    assert_eq!(click(&plugin,&tree,&mut ctx,id,60,45),vec![101]);
    assert!(click(&plugin,&tree,&mut ctx,id,100,245).is_empty(),
        "radio callback alone cannot mutate host-owned selected state");
    tree.children[0].set_prop("checked","true");
    for _ in 0..4{render(&plugin,&tree,&mut ctx);}
    assert_eq!(plugin.diagnostic_capture_pixel(21.0,45.0).unwrap(),unchecked_border,
        "host selection clears the red radio border");
    assert_eq!(click(&plugin,&tree,&mut ctx,id,100,245),vec![300]);
    tree.children[3].set_prop("form_status","busy");
    for _ in 0..4{render(&plugin,&tree,&mut ctx);}
    assert!(click(&plugin,&tree,&mut ctx,id,100,245).is_empty(),
        "in-flight Submit blocks repeat and underlying ordinary Button");
    assert_eq!(click(&plugin,&tree,&mut ctx,id,360,245),vec![900],
        "independent Button can still work while another form is busy");

    // The remote completion is an authoritative host transition.
    tree.children[3].set_prop("form_status","error");
    tree.children[3].set_prop("form_error","Gateway unavailable");
    for _ in 0..4{render(&plugin,&tree,&mut ctx);}
    let failed_stroke=plugin.diagnostic_capture_pixel(21.0,245.0).unwrap();
    assert!(failed_stroke.r>failed_stroke.g.saturating_add(80),
        "server error must be shown as red button stroke, not a silent failure: {failed_stroke:?}");
    for _ in 0..5{
        render(&plugin,&tree,&mut ctx);
        assert_eq!(ctx.get::<UiRuntimeFeedback>().unwrap().clicked_callback_id,None,
            "failure state cannot auto retry in absence of new physical user click");
    }
    assert_eq!(click(&plugin,&tree,&mut ctx,id,100,245),vec![300],
        "failed state permits a fresh explicit user retry");
    tree.children[3].set_prop("form_error","bad\nmessage");
    for _ in 0..3{render(&plugin,&tree,&mut ctx);}
    assert!(click(&plugin,&tree,&mut ctx,id,100,245).is_empty(),
        "malformed server message must fail closed without leaking to underlay");
    tree.children[3].props.retain(|(key,_)|key!="form_error");
    tree.children[3].set_prop("form_status","ready");
    for _ in 0..3{render(&plugin,&tree,&mut ctx);}
    assert_eq!(click(&plugin,&tree,&mut ctx,id,100,245),vec![300],
        "valid authoritative ready after failed request restores Submit");
    eprintln!("LUMINA_RADIO_ASYNC_FORM_X11 group_required_blocks=true radio_error_pixel=true host_selected_authority=true busy_no_double_send=true underlay_block=true remote_error_visual=true explicit_retry_only=true malformed_error_fail_closed=true independent_button=true");
}


/// Genuine XTest: a cancel button is actionable only for a *live* host
/// submission generation and cannot duplicate cancellation or click through.
/// The callback requests cancellation; Lumina does NOT cancel a network task.
#[test]
fn real_x11_form_cancel_once_per_busy_generation_underlay_and_host_recovery(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut tree=node("App",&[("bg","#101820")],vec![
        node("Button",&[("x","20"),("y","20"),("width","230"),("height","50"),
            ("form_submit","signup"),("form_status","busy"),
            ("form_request_revision","7"),
            ("on_click","cb:100"),("label","Submit")],vec![]),
        node("Button",&[("x","20"),("y","110"),("width","235"),("height","55"),
            ("on_click","cb:999"),("label","Underlay")],vec![]),
        node("Button",&[("x","30"),("y","115"),("width","210"),("height","45"),
            ("form_cancel","signup"),("on_click","cb:200"),
            ("label","Cancel request")],vec![]),
        node("Button",&[("x","330"),("y","115"),("width","180"),("height","50"),
            ("on_click","cb:300"),("label","Independent")],vec![]),
    ]);
    for _ in 0..5{render(&plugin,&tree,&mut ctx);}
    assert!(plugin.diagnostic_resize_window(640,380));
    for _ in 0..4{render(&plugin,&tree,&mut ctx);}
    let wins=xdotool(&["search","--name","Aura Lumina Sentinel"]);
    let id=wins.lines().next().expect("actual Native X11 form window");
    // Submit itself is disabled by host busy, before cancel.
    assert!(click(&plugin,&tree,&mut ctx,id,85,45).is_empty());
    let available_pixel=plugin.diagnostic_capture_pixel(31.0,135.0)
        .expect("cancel native border pixel");
    assert_eq!(click(&plugin,&tree,&mut ctx,id,80,135),vec![200],
        "first physical click requests cancellation exactly once");
    let requested_pixel=plugin.diagnostic_capture_pixel(31.0,135.0).unwrap();
    assert!(available_pixel.r>requested_pixel.r.saturating_add(35),
        "cancel button should visually disable while host request pending: {available_pixel:?} -> {requested_pixel:?}");
    assert!(click(&plugin,&tree,&mut ctx,id,80,135).is_empty(),
        "a second click for unchanged host revision must be blocked");
    assert_eq!(click(&plugin,&tree,&mut ctx,id,365,140),vec![300],
        "an unrelated Button remains live during cancellation");

    tree.children[0].set_prop("form_status","ready");
    for _ in 0..3{render(&plugin,&tree,&mut ctx);}
    assert!(click(&plugin,&tree,&mut ctx,id,80,135).is_empty(),
        "cancel after host completion is not a valid request");
    tree.children[0].set_prop("form_status","busy");
    for _ in 0..3{render(&plugin,&tree,&mut ctx);}
    assert!(click(&plugin,&tree,&mut ctx,id,80,135).is_empty(),
        "ABA: reused previously cancelled revision must not unlock cancel");
    tree.children[0].set_prop("form_request_revision","8");
    for _ in 0..3{render(&plugin,&tree,&mut ctx);}
    assert_eq!(click(&plugin,&tree,&mut ctx,id,80,135),vec![200],
        "new strictly greater busy generation allows one new request");
    assert!(click(&plugin,&tree,&mut ctx,id,80,135).is_empty());
    tree.children[0].set_prop("form_request_revision","08");
    for _ in 0..3{render(&plugin,&tree,&mut ctx);}
    assert!(click(&plugin,&tree,&mut ctx,id,80,135).is_empty(),
        "noncanonical request revision is invalid");
    tree.children[0].set_prop("form_request_revision","9");
    tree.children[2].set_prop("form_cancel","foreign");
    for _ in 0..3{render(&plugin,&tree,&mut ctx);}
    assert!(click(&plugin,&tree,&mut ctx,id,80,135).is_empty(),
        "a cancel control cannot target a different form");
    assert_eq!(click(&plugin,&tree,&mut ctx,id,365,140),vec![300]);
    eprintln!("LUMINA_CANCEL_MOUSE_X11 busy_submit_blocked=true single_cancel_per_generation=true cancel_disabled_pixel=true no_underlay_leak=true unrelated_button=true ready_blocks=true aba_revision_guard=true higher_revision_rearms=true malformed_revision_blocks=true foreign_form_blocks=true");
}
