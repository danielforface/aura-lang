#![cfg(feature="lumina-diagnostics")]
// Real X11/XTest tab traversal. Only logical reading-order focus, NOT
// accessibility focus order, assistive technologies, IME or BiDi semantics.
use std::process::Command;
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback,UiTextInputEvent};
use aura_plugin_lumina::{AuraLuminaPlugin,CompositionEvent};

fn node(kind:&str,props:&[(&str,&str)],children:Vec<UiNode>)->UiNode {
    let mut x=UiNode::new(kind);
    for &(k,v) in props { x.set_prop(k,v); }
    x.children=children; x
}
fn xdo(args:&[&str])->String {
    let out=Command::new("xdotool").args(args).output().expect("xdotool installed");
    assert!(out.status.success(),"xdotool {args:?}: {}",String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}
fn render(plugin:&AuraLuminaPlugin,tree:&UiNode,ctx:&mut NexusContext)->Vec<UiTextInputEvent> {
    plugin.on_ui_render(tree,ctx).expect("Lumina active").expect("native draw");
    ctx.get::<UiRuntimeFeedback>().expect("feedback").text_input_events.clone()
}
fn frames(plugin:&AuraLuminaPlugin,tree:&UiNode,ctx:&mut NexusContext,n:usize)->Vec<UiTextInputEvent> {
    (0..n).flat_map(|_| render(plugin,tree,ctx)).collect()
}
fn press(plugin:&AuraLuminaPlugin,tree:&UiNode,ctx:&mut NexusContext,key:&str,shift:bool)->Vec<UiTextInputEvent> {
    if shift { xdo(&["keydown","Shift_L"]); frames(plugin,tree,ctx,2); }
    xdo(&["keydown",key]);
    let mut result=frames(plugin,tree,ctx,5);
    xdo(&["keyup",key]);
    result.extend(frames(plugin,tree,ctx,2));
    if shift { xdo(&["keyup","Shift_L"]); frames(plugin,tree,ctx,2); }
    eprintln!("LUMINA_TAB_XTEST key={key} shift={shift} feedback={result:?}");
    result
}
fn has(e:&[UiTextInputEvent],cb:u64,text:&str)->bool {
    e.iter().any(|x|x.callback_id==cb && x.text==text && !x.submitted)
}
#[test]
fn native_tab_shift_tab_order_wrap_and_skip_hidden_disabled_inputs() {
    let mut ctx=NexusContext::default();
    let plugin=AuraLuminaPlugin::new();
    let tree=node("App",&[("bg","#121212")],vec![
        node("TextInput",&[("x","20"),("y","20"),("width","180"),("height","40"),
            ("value","A"),("on_change","cb:101")],vec![]),
        node("TextInput",&[("x","20"),("y","75"),("width","180"),("height","40"),
            ("value","disabled"),("disabled","true"),("on_change","cb:999")],vec![]),
        node("TextInput",&[("x","20"),("y","130"),("width","180"),("height","40"),
            ("value","B"),("on_change","cb:202")],vec![]),
        node("Box",&[("x","20"),("y","220"),("width","40"),("height","35"),
            ("overflow","hidden")],vec![node("TextInput",
            &[("x","180"),("y","225"),("value","hidden"),("on_change","cb:303")],vec![])]),
        node("TextInput",&[("x","900"),("y","900"),("value","offscreen"),("on_change","cb:404")],vec![]),
    ]);
    frames(&plugin,&tree,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,400));
    frames(&plugin,&tree,&mut ctx,3);
    let windows=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=windows.lines().next().expect("actual Raylib window");
    xdo(&["windowfocus","--sync",id]);
    // From NO focus: Tab selects first visible enabled field.
    assert!(press(&plugin,&tree,&mut ctx,"Tab",false).is_empty());
    let a=press(&plugin,&tree,&mut ctx,"q",false);
    assert!(has(&a,101,"Aq"),"Tab from no focus selects cb101: {a:?}");

    // Tabs skip disabled, clipped-away and offscreen controls.
    assert!(press(&plugin,&tree,&mut ctx,"Tab",false).is_empty());
    let b=press(&plugin,&tree,&mut ctx,"w",false);
    assert!(has(&b,202,"Bw"),"second Tab selects cb202: {b:?}");

    // Reverse navigation must return to first, not disabled cb999.
    assert!(press(&plugin,&tree,&mut ctx,"Tab",true).is_empty());
    let a2=press(&plugin,&tree,&mut ctx,"e",false);
    assert!(has(&a2,101,"Aqe"),"Shift-Tab selects cb101: {a2:?}");

    // Reverse from first wraps to last visible field.
    assert!(press(&plugin,&tree,&mut ctx,"Tab",true).is_empty());
    let b2=press(&plugin,&tree,&mut ctx,"z",false);
    assert!(has(&b2,202,"Bwz"),"reverse wrap selects cb202: {b2:?}");
    assert!(a.iter().chain(b.iter()).chain(a2.iter()).chain(b2.iter())
        .all(|e| ![999,303,404].contains(&e.callback_id)),
        "disabled, clipped and offscreen fields must not emit input callbacks");
    // Mouse and keyboard must agree: a disabled TextInput must not become
    // editable when clicked, even after another field was focused.
    xdo(&["mousemove","--window",id,"40","95"]);
    frames(&plugin,&tree,&mut ctx,3);
    xdo(&["mousedown","1"]);
    frames(&plugin,&tree,&mut ctx,4);
    xdo(&["mouseup","1"]);
    frames(&plugin,&tree,&mut ctx,2);
    let blocked=press(&plugin,&tree,&mut ctx,"p",false);
    assert!(blocked.is_empty(),"disabled field accepted real pointer+keyboard: {blocked:?}");
    drop(plugin);
    eprintln!("LUMINA_TAB_XTEST: retained drafts, forward/reverse, disabled pointer/Tab, clip+offscreen skip PASS");
}

/// Actual Raylib/XTest mixed Button+TextInput keyboard navigation. Uses the
/// public on_ui_render feedback surface, not injected internal focus state.
fn button_frames(p:&AuraLuminaPlugin,root:&UiNode,ctx:&mut NexusContext,n:usize)->Vec<u64>{
    let mut callbacks=Vec::new();
    for _ in 0..n {
        render(p,root,ctx);
        if let Some(id)=ctx.get::<UiRuntimeFeedback>()
            .expect("native feedback").clicked_callback_id{callbacks.push(id);}
    }
    callbacks
}
fn button_key(p:&AuraLuminaPlugin,root:&UiNode,ctx:&mut NexusContext,key:&str)->Vec<u64>{
    xdo(&["keydown",key]);
    let mut ids=button_frames(p,root,ctx,5);
    xdo(&["keyup",key]);
    ids.extend(button_frames(p,root,ctx,3));
    ids
}
#[test]
fn native_x11_mixed_buttons_inputs_enter_space_dynamic_disable_and_escape() {
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut root=node("App",&[("bg","#121212")],vec![
        node("TextInput",&[("x","20"),("y","20"),("width","180"),("height","40"),
            ("on_change","cb:101"),("on_submit","cb:102"),("value","A")],vec![]),
        node("Button",&[("x","20"),("y","80"),("width","180"),("height","44"),
            ("on_click","cb:201"),("label","Save")],vec![]),
        node("Button",&[("x","20"),("y","140"),("width","180"),("height","44"),
            ("on_click","cb:202"),("disabled","true")],vec![]),
        node("TextInput",&[("x","20"),("y","200"),("width","180"),("height","40"),
            ("on_change","cb:301"),("value","B")],vec![]),
        node("Button",&[("x","220"),("y","20"),("width","120"),("height","44"),
            ("on_click","cb:401"),("label","Reset")],vec![]),
    ]);
    frames(&plugin,&root,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,400));
    frames(&plugin,&root,&mut ctx,3);
    let w=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=w.lines().next().expect("real native X11 window");
    xdo(&["windowfocus","--sync",id]);

    // First Tab focuses first field; Enter retains its original submit,
    // without invoking Button on_click.
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    let submitted=press(&plugin,&root,&mut ctx,"Return",false);
    assert!(submitted.iter().any(|e|e.submitted && e.callback_id==102),
        "TextInput Enter must preserve submit semantics: {submitted:?}");

    // Next Tab focuses Button 201. Held Return/Space can fire exactly once.
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![201]);
    assert_eq!(button_key(&plugin,&root,&mut ctx,"space"),vec![201],
        "Space must activate a Button once, not type in an old input");

    // Tab skips disabled Button 202 and reaches TextInput 301.
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    let inserted=press(&plugin,&root,&mut ctx,"x",false);
    assert!(has(&inserted,301,"Bx"),"Tab must skip disabled Button: {inserted:?}");

    // Shift+Tab back to Button, then first field with retained local draft.
    assert!(press(&plugin,&root,&mut ctx,"Tab",true).is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![201]);
    assert!(press(&plugin,&root,&mut ctx,"Tab",true).is_empty());
    let a=press(&plugin,&root,&mut ctx,"q",false);
    assert!(has(&a,101,"Aq"),"reverse traversal must preserve input draft: {a:?}");

    // Reverse wrap from the first field reaches Button 401.
    assert!(press(&plugin,&root,&mut ctx,"Tab",true).is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"space"),vec![401]);

    // A dynamic disabled transition immediately revokes keyboard authority.
    root.children[4].set_prop("disabled","true");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "disabled Button must never fire a stale keyboard callback");
    root.children[4].set_prop("disabled","false");
    assert!(button_frames(&plugin,&root,&mut ctx,2).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "re-enabled Button must not reclaim revoked focus without user action");

    // Explicit native pointer click restores focus and a subsequent key works.
    xdo(&["mousemove","--window",id,"250","40"]);
    button_frames(&plugin,&root,&mut ctx,3);
    xdo(&["mousedown","1"]);
    let by_pointer=button_frames(&plugin,&root,&mut ctx,5);
    xdo(&["mouseup","1"]);
    button_frames(&plugin,&root,&mut ctx,2);
    assert_eq!(by_pointer,vec![401],"original native pointer click: {by_pointer:?}");
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![401]);
    assert!(button_key(&plugin,&root,&mut ctx,"Escape").is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "Escape must blur Button and block following Enter");
    eprintln!("LUMINA_MIXED_KEYBOARD_X11 submit=true enter_once=true space_once=true disabled_skipped=true reverse_wrap=true revoked_focus=true pointer_focus=true escape=true");
}


/// Real X11 keyboard-only controlled selectors coexist with TextInput
/// without claiming host state toggling or screen-reader role semantics.
#[test]
fn native_x11_checkbox_and_switch_keyboard_host_authority_and_invalid_revocation() {
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut root=node("App",&[("bg","#121212")],vec![
        node("TextInput",&[("x","20"),("y","20"),("width","200"),("height","40"),
            ("on_change","cb:10"),("on_submit","cb:11"),("value","A")],vec![]),
        node("Button",&[("x","20"),("y","80"),("width","220"),("height","45"),
            ("control","checkbox"),("checked","false"),("label","Agree"),
            ("on_click","cb:101")],vec![]),
        node("Button",&[("x","20"),("y","140"),("width","220"),("height","45"),
            ("control","switch"),("checked","true"),("label","Alerts"),
            ("on_click","cb:102")],vec![]),
        node("Button",&[("x","20"),("y","200"),("width","220"),("height","45"),
            ("control","checkbox"),("checked","false"),("disabled","true"),
            ("on_click","cb:103")],vec![]),
        node("Button",&[("x","20"),("y","260"),("width","220"),("height","45"),
            ("control","switch"),("checked","not-bool"),
            ("on_click","cb:104")],vec![]),
        node("Button",&[("x","20"),("y","320"),("width","220"),("height","45"),
            ("label","Save"),("on_click","cb:105")],vec![]),
    ]);
    frames(&plugin,&root,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,420));
    frames(&plugin,&root,&mut ctx,3);
    let w=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=w.lines().next().expect("real X11 window");
    xdo(&["windowfocus","--sync",id]);

    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    let edit=press(&plugin,&root,&mut ctx,"q",false);
    assert!(has(&edit,10,"Aq"),"first Tab must select the TextInput: {edit:?}");
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![101]);
    assert_eq!(button_key(&plugin,&root,&mut ctx,"space"),vec![101],
        "Space and Enter share the same controlled on_click channel");
    root.children[1].set_prop("checked","true");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty(),
        "host checked changes are renders, not synthetic callbacks");
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![101],
        "host checked update must not unexpectedly revoke keyboard focus");

    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"space"),vec![102],
        "next selector receives keyboard Space while retaining host checked");
    // Skip both disabled Checkbox and malformed Switch; reach ordinary Button.
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![105]);
    assert!(press(&plugin,&root,&mut ctx,"Tab",true).is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![102]);

    // An active native selector must immediately lose focus if the host
    // supplies invalid selection state. Fixing it must not resurrect focus.
    root.children[2].set_prop("checked","invalid");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty());
    root.children[2].set_prop("checked","false");
    assert!(button_frames(&plugin,&root,&mut ctx,2).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"space").is_empty(),
        "correcting the host prop must not auto-focus the switch");

    // User pointer explicitly restores native focus after authority repair.
    xdo(&["mousemove","--window",id,"60","162"]);
    button_frames(&plugin,&root,&mut ctx,3);
    xdo(&["mousedown","1"]);
    let pointer=button_frames(&plugin,&root,&mut ctx,5);
    xdo(&["mouseup","1"]);
    button_frames(&plugin,&root,&mut ctx,2);
    assert_eq!(pointer,vec![102]);
    assert_eq!(button_key(&plugin,&root,&mut ctx,"space"),vec![102]);
    assert!(button_key(&plugin,&root,&mut ctx,"Escape").is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty());
    eprintln!("LUMINA_SELECTION_KEYBOARD_X11 checkbox_enter_space=true switch_space=true skip_invalid_disabled=true checked_host_update_keeps_focus=true invalid_revokes_focus=true pointer_restore=true escape_blurs=true");
}


/// X11 keyboard-only radio group navigation. Left/Up and Right/Down select a
/// target callback, never local checked; unlike Tab, arrows stay in one group.
#[test]
fn native_x11_radio_arrows_group_bounds_host_authority_and_invalid_revoke() {
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut root=node("App",&[("bg","#121212")],vec![
        node("TextInput",&[("x","20"),("y","20"),("width","190"),("height","40"),
            ("on_change","cb:10"),("value","A")],vec![]),
        node("Button",&[("x","20"),("y","75"),("width","230"),("height","48"),
            ("control","radio"),("group","plan"),("checked","true"),
            ("on_click","cb:101"),("label","Standard")],vec![]),
        node("Button",&[("x","20"),("y","135"),("width","230"),("height","48"),
            ("control","radio"),("group","plan"),("checked","false"),
            ("on_click","cb:102"),("label","Premium")],vec![]),
        node("Button",&[("x","20"),("y","195"),("width","230"),("height","48"),
            ("control","radio"),("group","plan"),("checked","false"),
            ("disabled","true"),("on_click","cb:103")],vec![]),
        node("Button",&[("x","20"),("y","255"),("width","230"),("height","48"),
            ("control","radio"),("group","other"),("checked","true"),
            ("on_click","cb:201")],vec![]),
        node("Button",&[("x","300"),("y","75"),("width","190"),("height","48"),
            ("on_click","cb:301"),("label","Submit")],vec![]),
    ]);
    frames(&plugin,&root,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,420));
    frames(&plugin,&root,&mut ctx,3);
    let windows=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=windows.lines().next().expect("real X11 window");
    xdo(&["windowfocus","--sync",id]);

    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&root,&mut ctx,"q",false),10,"Aq"));
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Right"),vec![102],
        "Right arrow selects next enabled radio callback");
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Down"),vec![101],
        "Down wraps and skips disabled third option");
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Left"),vec![102],
        "Left wraps to previous radio within the same group");
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Up"),vec![101],
        "Up selects previous radio without leaving group");
    // No host update: Enter on the currently focused radio remains valid.
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![101]);

    // Host changes selection atomically, existing focus remains stable.
    root.children[1].set_prop("checked","false");
    root.children[2].set_prop("checked","true");
    assert!(button_frames(&plugin,&root,&mut ctx,4).is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Right"),vec![102]);
    // Tab leaves plan group for the *other* group, not another plan option.
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Right").is_empty(),
        "a single-member other group cannot move arrows to plan/Submit");
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![201]);
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![301],
        "ordinary Button stays actionable, radio arrows do not affect it");
    assert!(button_key(&plugin,&root,&mut ctx,"Left").is_empty(),
        "ordinary Button must not navigate radio groups");

    // Focus radio 102 via mouse, then inject conflicting host selections.
    xdo(&["mousemove","--window",id,"70","155"]);
    button_frames(&plugin,&root,&mut ctx,3);
    xdo(&["mousedown","1"]);
    let clicked=button_frames(&plugin,&root,&mut ctx,5);
    xdo(&["mouseup","1"]);
    button_frames(&plugin,&root,&mut ctx,2);
    assert_eq!(clicked,vec![102]);
    root.children[1].set_prop("checked","true");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "two selected radios must revoke stale keyboard focus");
    assert!(button_key(&plugin,&root,&mut ctx,"Right").is_empty(),
        "conflicted group arrow callbacks must be blocked");

    root.children[1].set_prop("checked","false");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "repair does not silently reacquire user keyboard focus");
    // Only a new genuine Tab or mouse action is allowed to re-acquire.
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    // From no focus, Tab starts at TextInput again (not radio).
    assert!(has(&press(&plugin,&root,&mut ctx,"z",false),10,"Aqz"));
    eprintln!("LUMINA_RADIO_ARROW_X11 four_directions=true wraps_skips_disabled=true host_checked_authority=true separate_group=true nonradio_no_arrow=true conflict_revokes_focus=true repair_no_autofocus=true");
}


/// Native Tab/Enter-only opt-in form. Invalid submissions must send no app
/// click, focus the first correctable input, and preserve live EditSession text.
#[test]
fn native_x11_form_keyboard_submit_validation_and_host_rule_recovery(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut root=node("App",&[("bg","#121212")],vec![
        node("TextInput",&[("x","20"),("y","20"),("width","230"),("height","45"),
            ("form","registration"),("required","true"),
            ("on_change","cb:701"),("value","")],vec![]),
        node("TextInput",&[("x","20"),("y","88"),("width","230"),("height","45"),
            ("form","registration"),("min_graphemes","2"),
            ("on_change","cb:702"),("on_submit","cb:703"),("value","A")],vec![]),
        node("Button",&[("x","20"),("y","162"),("width","230"),("height","48"),
            ("form_submit","registration"),("label","Submit"),
            ("on_click","cb:790")],vec![]),
        node("Button",&[("x","290"),("y","162"),("width","220"),("height","48"),
            ("label","Ordinary"),("on_click","cb:791")],vec![]),
    ]);
    frames(&plugin,&root,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,410));
    frames(&plugin,&root,&mut ctx,3);
    let id=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=id.lines().next().expect("real native X11 window");
    xdo(&["windowfocus","--sync",id]);

    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    let enter_in_input=press(&plugin,&root,&mut ctx,"Return",false);
    assert!(enter_in_input.iter().any(|e|e.callback_id==703&&e.submitted),
        "TextInput on_submit remains unchanged and bypasses Button form validation");
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "invalid native form keyboard submit must NEVER emit 790");
    // First field now focused; Enter and Space should remain TextInput text,
    // and the input's local draft must be used on later submit attempts.
    assert!(has(&press(&plugin,&root,&mut ctx,"x",false),701,"x"),
        "blocked submit must transfer focus to first required input");
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&root,&mut ctx,"b",false),702,"Ab"),
        "second input should keep the source A and append only one b");
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![790],
        "valid committed drafts permit exactly one host button callback");

    // Malformed host rule revokes submit even when current drafts satisfy
    // the earlier contract. Native runtime cannot silently ignore it.
    root.children[1].set_prop("min_graphemes","bogus");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "malformed rule must block app callback, not be treated as absent");
    // Restore host rule; callback remains gated until a new user activation.
    root.children[1].set_prop("min_graphemes","2");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "focus now on invalid field, corrected rule must not auto-activate submit");
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![790]);

    // Ordinary Button must still function even if any form becomes invalid.
    root.children[0].set_prop("required","TRUE");
    assert!(button_frames(&plugin,&root,&mut ctx,2).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![791],
        "non-form actions must remain independent of form configuration errors");
    eprintln!("LUMINA_FORM_KEYBOARD_X11 submit_blocked=true first_error_focus=true second_error_repair=true session_draft_valid=true malformed_rule_fail_closed=true no_silent_autosubmit=true ordinary_button=true");
}


/// Only the host may make a one-of-N radio group valid or transition a Form
/// Submit from busy to retryable server error. Real XTest uses keyboard only.
#[test]
fn native_x11_required_radio_then_form_busy_failure_and_retry_keyboard(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut root=node("App",&[("bg","#121212")],vec![
        node("Button",&[("x","20"),("y","20"),("width","235"),("height","48"),
            ("form","order"),("control","radio"),("group","plan"),
            ("checked","false"),("required","true"),("on_click","cb:101")],vec![]),
        node("Button",&[("x","20"),("y","84"),("width","235"),("height","48"),
            ("form","order"),("control","radio"),("group","plan"),
            ("checked","false"),("on_click","cb:102")],vec![]),
        node("Button",&[("x","20"),("y","158"),("width","235"),("height","54"),
            ("form_submit","order"),("form_status","ready"),
            ("on_click","cb:300"),("label","Submit")],vec![]),
        node("Button",&[("x","315"),("y","158"),("width","210"),("height","54"),
            ("on_click","cb:900"),("label","Independent")],vec![]),
    ]);
    frames(&plugin,&root,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,400));
    frames(&plugin,&root,&mut ctx,3);
    let found=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=found.lines().next().expect("actual X11 window");
    xdo(&["windowfocus","--sync",id]);
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // Radio A
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Right"),vec![102]);
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // Submit
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "radio activation sends selection intent, but without host checked no valid submit");
    assert_eq!(button_key(&plugin,&root,&mut ctx,"space"),vec![101],
        "failed submit focuses first invalid radio in mixed source order");
    root.children[0].set_prop("checked","true");
    assert!(button_frames(&plugin,&root,&mut ctx,4).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // Radio B
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // Submit
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![300]);

    // Busy state revokes a focused Button and blocks keyboard reactivation,
    // including after status changes back until a *new* user focus action.
    root.children[2].set_prop("form_status","busy");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"space").is_empty());
    root.children[2].set_prop("form_status","error");
    root.children[2].set_prop("form_error","Retry from service");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "remote failed state must never auto-reactivate revoked focus");

    // Explicit Tab walk to the retryable Submit (no synthetic auto-click).
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // Radio A
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // Radio B
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // Submit
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![300],
        "new user activation retries only when server publishes error");
    root.children[2].set_prop("form_error","bad\u{202E}error");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "unsafe remote bidi override must revoke Submit keyboard focus");
    root.children[2].props.retain(|(key,_)|key!="form_error");
    root.children[2].set_prop("form_status","ready");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "a repaired host state alone never steals focus back");
    // Still must allow independent ordinary Button after invalid form.
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![900],
        "independent Button remains keyboard-accessible after form recovery");
    eprintln!("LUMINA_FORM_ASYNC_KEYBOARD_X11 required_group_gates=true first_invalid_radio_focus=true host_checked_authority=true busy_revokes_focus=true failure_no_auto_retry=true explicit_retry=true bidi_error_fail_closed=true repair_no_focus_steal=true unrelated_button=true");
}


/// XTest keyboard-only cancel request: the cancel button gets focus only when
/// the host reports one busy submission with a fresh canonical revision.
#[test]
fn native_x11_cancel_request_tab_enter_once_bounded_generations_and_focus(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut root=node("App",&[("bg","#121212")],vec![
        node("Button",&[("x","20"),("y","20"),("width","235"),("height","48"),
            ("form_submit","signup"),("form_status","busy"),
            ("form_request_revision","12"),("on_click","cb:100")],vec![]),
        node("Button",&[("x","20"),("y","90"),("width","235"),("height","48"),
            ("form_cancel","signup"),("on_click","cb:200"),
            ("label","Cancel")],vec![]),
        node("Button",&[("x","20"),("y","170"),("width","235"),("height","48"),
            ("on_click","cb:300"),("label","Independent")],vec![]),
    ]);
    frames(&plugin,&root,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,380));
    frames(&plugin,&root,&mut ctx,3);
    let found=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=found.lines().next().expect("native X11 window");
    xdo(&["windowfocus","--sync",id]);
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![200],
        "Tab skips busy Submit and activates cancel control");
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "same cancellation cannot be reissued while key remains focused");
    assert!(button_key(&plugin,&root,&mut ctx,"space").is_empty(),
        "a second activation method cannot replay the same host generation");
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![300],
        "from revoked focus Tab skips blocked cancel and visits next Button");
    root.children[0].set_prop("form_status","ready");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "ready form without any fields cannot submit; cancel stays ineligible");
    root.children[0].set_prop("form_status","busy");
    root.children[0].set_prop("form_request_revision","13");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![200],
        "higher host request generation grants a new cancellation");
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty());
    root.children[0].set_prop("form_request_revision","012");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![300],
        "malformed revision removes cancel from keyboard focus order");
    eprintln!("LUMINA_CANCEL_KEYBOARD_X11 busy_skip_submit=true tab_cancel=true once_per_revision=true enter_space_no_replay=true revokes_stale_focus=true ordinary_button=true ready_blocks_cancel=true new_host_revision_rearms=true noncanonical_generation_fail_closed=true");
}


/// Cancellation is a host request, NOT a local edit-session reset. Real X11
/// typing and Ctrl+Z must still operate on exactly the retained local draft.
#[test]
fn native_x11_form_cancel_preserves_edit_session_and_undo_history(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let root=node("App",&[("bg","#101820")],vec![
        node("TextInput",&[("x","20"),("y","20"),("width","220"),("height","48"),
            ("on_change","cb:401"),("form","signup"),("value","A")],vec![]),
        node("Button",&[("x","20"),("y","95"),("width","220"),("height","50"),
            ("form_submit","signup"),("form_status","busy"),
            ("form_request_revision","1"),("on_click","cb:100")],vec![]),
        node("Button",&[("x","20"),("y","170"),("width","220"),("height","50"),
            ("form_cancel","signup"),("on_click","cb:200")],vec![]),
    ]);
    frames(&plugin,&root,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,350));
    frames(&plugin,&root,&mut ctx,3);
    let id=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=id.lines().next().expect("Native X11 editor");
    xdo(&["windowfocus","--sync",id]);
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&root,&mut ctx,"x",false),401,"Ax"),
        "native first local edit must emit a full committed draft");
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![200]);
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty());
    // Cancel focus is revoked; the next Tab reacquires TextInput (not busy
    // Submit and not the already requested cancel).
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&root,&mut ctx,"y",false),401,"Axy"),
        "cancel must not revert or overwrite the retained EditSession");
    xdo(&["keydown","Control_L"]);
    button_frames(&plugin,&root,&mut ctx,3);
    let undone=press(&plugin,&root,&mut ctx,"z",false);
    xdo(&["keyup","Control_L"]);
    button_frames(&plugin,&root,&mut ctx,3);
    assert!(has(&undone,401,"Ax"),
        "Ctrl+Z after cancel must undo only the latest local text edit, not reset session: {undone:?}");
    eprintln!("LUMINA_CANCEL_EDITOR_X11 cancel_callback_once=true draft_persisted_after_focus_revoke=true undo_history_retained=true busy_submit_skipped=true");
}


/// A terminal form acknowledgement is host-authored and strictly tied to a
/// cancellation that this X11 window actually sent. It is not a synthetic
/// timer completion and must not restore forbidden keyboard actions.
#[test]
fn native_x11_form_cancel_acknowledged_terminal_focus_and_ready_recovery(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut root=node("App",&[("bg","#121212")],vec![
        node("TextInput",&[("x","20"),("y","20"),("width","240"),("height","45"),
            ("form","signup"),("on_change","cb:401"),("value","A")],vec![]),
        node("Button",&[("x","20"),("y","85"),("width","240"),("height","48"),
            ("form_submit","signup"),("form_status","busy"),
            ("form_request_revision","7"),("on_click","cb:100"),
            ("label","Submit")],vec![]),
        node("Button",&[("x","20"),("y","158"),("width","240"),("height","48"),
            ("form_cancel","signup"),("on_click","cb:200"),
            ("label","Cancel")],vec![]),
        node("Button",&[("x","300"),("y","158"),("width","200"),("height","48"),
            ("on_click","cb:300"),("label","Other")],vec![]),
    ]);
    frames(&plugin,&root,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,380));
    frames(&plugin,&root,&mut ctx,3);
    let found=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=found.lines().next().expect("real native keyboard window");
    xdo(&["windowfocus","--sync",id]);
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // TextInput
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // Cancel; busy Submit skipped
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![200]);
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "held/fresh Return cannot duplicate a cancellation generation");
    root.children[1].set_prop("form_status","cancelled");
    root.children[1].set_prop("form_cancel_ack_revision","7");
    root.children[1].set_prop("form_cancel_notice","Request closed");
    assert!(button_frames(&plugin,&root,&mut ctx,4).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "host acknowledgement does not synthesize a callback or restore focus");
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // TextInput
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // Other
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![300],
        "terminal Submit/Cancel are both excluded from keyboard order");

    // Stale metadata is rejected. A host must explicitly clear cancellation
    // props when returning to ready; no implicit unfocusing/refresh-click.
    root.children[1].set_prop("form_status","ready");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // TextInput
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // Other, stale ack blocks Submit
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![300]);
    root.children[1].props.retain(|(k,_)|
        k!="form_cancel_ack_revision" && k!="form_cancel_notice");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // TextInput
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // Submit
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![100],
        "explicit ready and cleared metadata reenable submission via new user action");

    root.children[1].set_prop("form_status","busy");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty()); // revoked Submit focus
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // TextInput
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // Other, ABA canceller blocked
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![300]);
    root.children[1].set_prop("form_request_revision","8");
    assert!(button_frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // TextInput
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // Cancel
    assert_eq!(button_key(&plugin,&root,&mut ctx,"Return"),vec![200],
        "new higher host generation supports one more cancellation");
    root.children[1].set_prop("form_status","cancelled");
    root.children[1].set_prop("form_request_revision","7");
    root.children[1].set_prop("form_cancel_ack_revision","7");
    assert!(button_frames(&plugin,&root,&mut ctx,4).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "stale terminal acknowledgement must never restore keyboard focus");
    root.children[1].set_prop("form_request_revision","8");
    root.children[1].set_prop("form_cancel_ack_revision","8");
    assert!(button_frames(&plugin,&root,&mut ctx,4).is_empty());
    assert!(button_key(&plugin,&root,&mut ctx,"Return").is_empty(),
        "valid terminal acknowledgement has no synthetic Submit/Cancel callbacks");
    eprintln!("LUMINA_CANCEL_ACK_KEYBOARD_X11 host_terminal_no_callback=true submit_and_cancel_not_focusable=true unrelated_button=true stale_ack_metadata_rejects_ready=true explicit_ready_recovery=true aba_busy_revision_blocks=true new_revision_cancellable=true stale_completion_no_focus=true valid_completion_no_autoclick=true");
}


/// Genuine X11 route transitions use the actual render loop, not a fake
/// injected editor state. The source props stay at old revision 1 throughout
/// the first navigation cycle: only an opted-in retained EditSession can make
/// the form valid and preserve Ctrl+Z after the route is reinserted.
#[test]
fn native_x11_route_draft_retention_validates_live_text_undo_and_host_rebase(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut screen=node("App",&[("bg","#121212")],vec![
        node("TextInput",&[("x","20"),("y","20"),("width","240"),
            ("height","48"),("value","A"),("value_revision","1"),
            ("retain_draft","route"),("draft_scope","window-session-1"),("draft_key","checkout.customer"),
            ("form","checkout"),("required","true"),("min_graphemes","3"),
            ("on_change","cb:401")],vec![]),
        node("Button",&[("x","20"),("y","95"),("width","220"),
            ("height","52"),("form_submit","checkout"),
            ("on_click","cb:500"),("label","Complete")],vec![]),
    ]);
    let other=node("App",&[("bg","#121212")],vec![
        node("Text",&[("x","20"),("y","20"),("text","Review route")],vec![]),
        node("Button",&[("x","20"),("y","95"),("width","220"),
            ("height","52"),("on_click","cb:800"),("label","Other")],vec![]),
    ]);
    frames(&plugin,&screen,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,360));
    frames(&plugin,&screen,&mut ctx,3);
    let id=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=id.lines().next().expect("native X11 editor");
    xdo(&["windowfocus","--sync",id]);
    assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&screen,&mut ctx,"x",false),401,"Ax"));
    assert!(has(&press(&plugin,&screen,&mut ctx,"y",false),401,"Axy"));
    assert!(button_frames(&plugin,&other,&mut ctx,4).is_empty(),
        "route removal must not send accidental callbacks");
    // The original host props still say A@revision1, without a state echo.
    assert!(button_frames(&plugin,&screen,&mut ctx,4).is_empty());
    assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
    assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
    assert_eq!(button_key(&plugin,&screen,&mut ctx,"Return"),vec![500],
        "form validation must read the retained Axy draft, not stale host A");
    assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&screen,&mut ctx,"z",false),401,"Axyz"),
        "native committed draft survived full tree removal");
    xdo(&["keydown","Control_L"]);
    button_frames(&plugin,&screen,&mut ctx,3);
    let undone=press(&plugin,&screen,&mut ctx,"z",false);
    xdo(&["keyup","Control_L"]);
    button_frames(&plugin,&screen,&mut ctx,3);
    assert!(has(&undone,401,"Axy"),
        "Undo history must survive route transitions: {undone:?}");

    // A strictly higher authoritative host revision discards the cached edit
    // lineage and restores a fresh undo boundary, even with the same key.
    screen.children[0].set_prop("value","Host");
    screen.children[0].set_prop("value_revision","2");
    assert!(button_frames(&plugin,&screen,&mut ctx,4).is_empty());
    assert!(has(&press(&plugin,&screen,&mut ctx,"q",false),401,"Hostq"),
        "host revision 2 must win over retained revision 1");
    xdo(&["keydown","Control_L"]);
    button_frames(&plugin,&screen,&mut ctx,3);
    let undo_host=press(&plugin,&screen,&mut ctx,"z",false);
    xdo(&["keyup","Control_L"]);
    button_frames(&plugin,&screen,&mut ctx,3);
    assert!(has(&undo_host,401,"Host"),
        "new host revision clears old route's undo snapshots: {undo_host:?}");
    eprintln!("LUMINA_ROUTE_DRAFT_X11 retained_committed_draft=true form_reads_retained_value=true same_callback_key=true undo_across_unmount=true host_higher_revision_rebase=true no_stale_undo=true no_auto_focus_restore=true");
}

/// Reusing a callback ID for a different logical field or sensitive input
/// must not resurrect another route's retained text.
#[test]
fn native_x11_route_draft_key_collision_sensitive_field_and_revision_downgrade(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut root=node("App",&[("bg","#101820")],vec![
        node("TextInput",&[("x","20"),("y","20"),("width","260"),
            ("height","48"),("value","M"),("value_revision","4"),
            ("retain_draft","route"),("draft_scope","window-session-1"),("draft_key","account.public"),
            ("on_change","cb:401")],vec![]),
    ]);
    let blank=node("App",&[("bg","#101820")],vec![
        node("Text",&[("x","20"),("y","20"),("text","Route absent")],vec![]),
    ]);
    frames(&plugin,&root,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,360));
    frames(&plugin,&root,&mut ctx,3);
    let id=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=id.lines().next().expect("native X11 editor");
    xdo(&["windowfocus","--sync",id]);
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&root,&mut ctx,"x",false),401,"Mx"));
    assert!(frames(&plugin,&blank,&mut ctx,4).is_empty());
    root.children[0].set_prop("value_revision","3"); // Host regressed.
    frames(&plugin,&root,&mut ctx,4);
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&root,&mut ctx,"q",false),401,"Mq"),
        "downgraded host revision must refuse restoration");
    frames(&plugin,&blank,&mut ctx,4);
    root.children[0].set_prop("draft_key","other-route.public");
    frames(&plugin,&root,&mut ctx,4);
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&root,&mut ctx,"z",false),401,"Mz"),
        "same callback but different key cannot load prior route draft");
    frames(&plugin,&blank,&mut ctx,4);
    root.children[0].set_prop("draft_scope","window-session-2");
    frames(&plugin,&root,&mut ctx,4);
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&root,&mut ctx,"v",false),401,"Mv"),
        "rotating host session scope must reject previous session draft");
    frames(&plugin,&blank,&mut ctx,4);
    root.children[0].set_prop("type","password");
    frames(&plugin,&root,&mut ctx,4);
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&root,&mut ctx,"w",false),401,"Mw"),
        "sensitive input must never restore another retained route session");
    frames(&plugin,&blank,&mut ctx,4);
    frames(&plugin,&root,&mut ctx,4);
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&root,&mut ctx,"v",false),401,"Mv"),
        "password field local edits must not persist across removal");
    eprintln!("LUMINA_ROUTE_DRAFT_GUARDS_X11 host_revision_downgrade_refuses_restore=true different_draft_key_isolated=true callback_reuse_not_identity=true scope_rotation_rejects_old_draft=true sensitive_type_not_cached=true sensitive_unmount_discards=true");
}


/// Actual X11 route transition retaining two independent editors, then host
/// rotating a single root-wide session scope while the route is unmounted.
/// Old drafts must be purged, not merely ignored until some later reuse.
#[test]
fn native_x11_multifield_route_retention_and_host_session_revocation(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let screen=node("App",&[("bg","#121212"),("draft_active_scope","session-a")],vec![
        node("TextInput",&[("x","20"),("y","20"),("width","250"),("height","46"),
            ("value","A"),("value_revision","1"),("on_change","cb:401"),
            ("retain_draft","route"),("draft_scope","session-a"),
            ("draft_key","checkout.first"),("form","checkout")],vec![]),
        node("TextInput",&[("x","20"),("y","95"),("width","250"),("height","46"),
            ("value","B"),("value_revision","1"),("on_change","cb:402"),
            ("retain_draft","route"),("draft_scope","session-a"),
            ("draft_key","checkout.second"),("form","checkout")],vec![]),
    ]);
    let mut alternate=node("App",&[("bg","#121212"),("draft_active_scope","session-a")],
        vec![node("Text",&[("x","20"),("y","20"),("text","Review route")],vec![])]);
    frames(&plugin,&screen,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,340));
    frames(&plugin,&screen,&mut ctx,4);
    let found=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=found.lines().next().expect("actual X11 window");
    xdo(&["windowfocus","--sync",id]);
    assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&screen,&mut ctx,"x",false),401,"Ax"));
    assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&screen,&mut ctx,"y",false),402,"By"));
    assert!(frames(&plugin,&alternate,&mut ctx,4).is_empty(),
        "temporary route change cannot synthesize editor callbacks");
    assert!(frames(&plugin,&screen,&mut ctx,4).is_empty());
    assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&screen,&mut ctx,"z",false),401,"Axz"),
        "first field kept its own committed unsynced draft");
    assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&screen,&mut ctx,"q",false),402,"Byq"),
        "second field restored independently without callback/session mix-up");
    xdo(&["keydown","Control_L"]);
    frames(&plugin,&screen,&mut ctx,3);
    let undone=press(&plugin,&screen,&mut ctx,"z",false);
    xdo(&["keyup","Control_L"]);
    frames(&plugin,&screen,&mut ctx,3);
    assert!(has(&undone,402,"By"),
        "second field must retain its own Undo history after route return: {undone:?}");
    assert!(has(&press(&plugin,&screen,&mut ctx,"q",false),402,"Byq"),
        "fresh edit after Undo must not corrupt the retained first field");

    assert!(frames(&plugin,&alternate,&mut ctx,4).is_empty());
    alternate.set_prop("draft_active_scope","session-b"); // logout while absent
    assert!(frames(&plugin,&alternate,&mut ctx,4).is_empty(),
        "explicit host session rotation must not invent text callbacks");
    // A stale navigation back to the original scope cannot resurrect data
    // already purged by the logout boundary, even at the old host revision.
    alternate.set_prop("draft_active_scope","session-a");
    assert!(frames(&plugin,&alternate,&mut ctx,4).is_empty());
    assert!(frames(&plugin,&screen,&mut ctx,4).is_empty());
    assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&screen,&mut ctx,"n",false),401,"An"),
        "revoked first draft Axz must never survive session switch");
    assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&screen,&mut ctx,"m",false),402,"Bm"),
        "revoked second draft Byq must never survive session switch");
    eprintln!("LUMINA_ROUTE_MULTI_SCOPE_X11 two_distinct_drafts=true undo_authority_preserved=true root_scope_logout_while_unmounted=true both_old_drafts_purged=true stale_scope_reentry_clean=true no_focus_steal=true");
}

/// An ineligible editor claiming a cached logical key with another callback
/// revokes old ownership; otherwise stale private drafts could survive
/// through disabled/password/hidden interstitial screens.
#[test]
fn native_x11_route_draft_foreign_ineligible_key_claim_revokes_private_cache(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let screen=node("App",&[("bg","#121212"),("draft_active_scope","session-a")],vec![
        node("TextInput",&[("x","20"),("y","20"),("width","240"),("height","46"),
            ("value","M"),("value_revision","4"),("on_change","cb:401"),
            ("retain_draft","route"),("draft_scope","session-a"),
            ("draft_key","profile.name"),("form","profile")],vec![]),
    ]);
    let empty=node("App",&[("bg","#121212"),("draft_active_scope","session-a")],
        vec![node("Text",&[("x","20"),("y","20"),("text","Empty route")],vec![])]);
    let mut alien=node("App",&[("bg","#121212"),("draft_active_scope","session-a")],vec![
        node("TextInput",&[("x","9000"),("y","9000"),("width","200"),
            ("disabled","true"),("value","private"),("value_revision","4"),
            ("on_change","cb:903"),("retain_draft","route"),
            ("draft_scope","session-a"),("draft_key","profile.name"),
            ("form","profile")],vec![]),
    ]);
    frames(&plugin,&screen,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,330));
    frames(&plugin,&screen,&mut ctx,4);
    let found=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=found.lines().next().expect("actual X11 window");
    xdo(&["windowfocus","--sync",id]);
    assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&screen,&mut ctx,"x",false),401,"Mx"));
    assert!(frames(&plugin,&empty,&mut ctx,4).is_empty());
    assert!(frames(&plugin,&alien,&mut ctx,4).is_empty(),
        "hidden disabled foreign field cannot synthesize a text callback");
    assert!(frames(&plugin,&screen,&mut ctx,4).is_empty());
    assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&screen,&mut ctx,"q",false),401,"Mq"),
        "foreign hidden key claim must purge Mx and its Undo chain");

    assert!(frames(&plugin,&empty,&mut ctx,4).is_empty());
    alien.children[0].set_prop("on_change","malformed");
    alien.children[0].set_prop("type","password");
    assert!(frames(&plugin,&alien,&mut ctx,4).is_empty());
    assert!(frames(&plugin,&screen,&mut ctx,4).is_empty());
    assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&screen,&mut ctx,"z",false),401,"Mz"),
        "malformed sensitive foreign key claim must also purge Mq");
    eprintln!("LUMINA_ROUTE_FOREIGN_CLAIM_X11 hidden_disabled_claim_revokes=true malformed_password_claim_revokes=true original_host_value_wins=true no_private_draft_replay=true");
}


/// Genuine native X11/XTest acceptance for the deterministic bounded route
/// cache: on one screen the user edits nine different opted-in TextInputs.
/// Only eight can survive unmount; the eight most recently focused do, not
/// an arbitrary subset selected by HashMap randomized iteration order.
#[test]
fn native_x11_ninth_retained_draft_evicts_least_recently_focused_field(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut screen=node("App",&[("bg","#121212"),
        ("draft_active_scope","window-session-1")],vec![]);
    for i in 0..9usize{
        let mut field=node("TextInput",&[
            ("x","20"),("width","245"),("height","40"),
            ("value","A"),("value_revision","1"),
            ("retain_draft","route"),("draft_scope","window-session-1"),
            ("form","checkout"),
        ],vec![]);
        field.set_prop("y",&format!("{}",14+i*53));
        field.set_prop("draft_key",&format!("checkout.field.{i:02}"));
        field.set_prop("on_change",&format!("cb:{}",401+i));
        screen.children.push(field);
    }
    let away=node("App",&[("bg","#121212"),
        ("draft_active_scope","window-session-1")],
        vec![node("Text",&[("x","20"),("y","20"),
            ("text","Review nine inputs")],vec![])]);
    frames(&plugin,&screen,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,560));
    frames(&plugin,&screen,&mut ctx,4);
    let found=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=found.lines().next().expect("real X11 native window");
    xdo(&["windowfocus","--sync",id]);
    for i in 0..9usize{
        assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
        let edited=press(&plugin,&screen,&mut ctx,"x",false);
        assert!(has(&edited,(401+i) as u64,"Ax"),
            "all nine fields must accept a real focused edit: field={i} {edited:?}");
    }
    assert!(frames(&plugin,&away,&mut ctx,4).is_empty(),
        "route removal cannot emit synthetic text callbacks");
    assert!(frames(&plugin,&screen,&mut ctx,4).is_empty(),
        "route reentry cannot steal focus or send callbacks");
    let evicted=plugin.diagnostic_capture_pixel(21.0,34.0)
        .expect("native evicted draft border pixel");
    let retained=plugin.diagnostic_capture_pixel(21.0,87.0)
        .expect("native retained draft border pixel");
    assert!(evicted.r>retained.r.saturating_add(65)
            && evicted.g>retained.g.saturating_add(40),
        "only the evicted unsynced draft should show a visible amber warning: \
         evicted={evicted:?}, retained={retained:?}");
    for i in 0..9usize{
        assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
        let edit=press(&plugin,&screen,&mut ctx,"q",false);
        let expected=if i==0{"Aq"}else{"Axq"};
        if i==0{
            frames(&plugin,&screen,&mut ctx,3);
            let dismissed=plugin.diagnostic_capture_pixel(21.0,34.0)
                .expect("native warning dismissal pixel");
            assert!(evicted.r>dismissed.r.saturating_add(65),
                "the first real committed user edit dismisses the warning: \
                 before={evicted:?}, after={dismissed:?}");
        }
        assert!(has(&edit,(401+i) as u64,expected),
            "field={i} must follow last-focus capacity ordering \
             (evict oldest only): expected={expected:?} feedback={edit:?}");
    }
    eprintln!("LUMINA_ROUTE_CAPACITY_X11 nine_fields_edited=true \
        last_eight_retained=true least_recent_evicted=true \
        no_random_hashmap_survivors=true host_scope_unchanged=true \
        visible_amber_eviction_warning=true no_warning_for_retained=true \
        warning_dismisses_on_real_edit=true \
        focus_not_stolen=true normal_fields_still_editable=true");
}


/// A newer Host revision is authoritative even when the older local draft
/// would otherwise have been dropped because it was ninth in the cache.
#[test]
fn native_x11_capacity_notice_never_overrides_newer_host_revision(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut screen=node("App",&[("bg","#121212"),
        ("draft_active_scope","session-a")],vec![]);
    for i in 0..9usize{
        let mut field=node("TextInput",&[
            ("x","20"),("width","245"),("height","40"),
            ("value","A"),("value_revision","1"),
            ("retain_draft","route"),("draft_scope","session-a"),
            ("form","checkout"),
        ],vec![]);
        field.set_prop("y",&format!("{}",14+i*53));
        field.set_prop("draft_key",&format!("checkout.{i:02}"));
        field.set_prop("on_change",&format!("cb:{}",401+i));
        screen.children.push(field);
    }
    let away=node("App",&[("bg","#121212"),("draft_active_scope","session-a")],
        vec![node("Text",&[("text","Next page")],vec![])]);
    frames(&plugin,&screen,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,560));
    frames(&plugin,&screen,&mut ctx,4);
    let windows=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=windows.lines().next().expect("actual Native X11 window");
    xdo(&["windowfocus","--sync",id]);
    for i in 0..9usize{
        assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
        assert!(has(&press(&plugin,&screen,&mut ctx,"x",false),
            (401+i) as u64,"Ax"));
    }
    assert!(frames(&plugin,&away,&mut ctx,4).is_empty());
    screen.children[0].set_prop("value","Authoritative");
    screen.children[0].set_prop("value_revision","2");
    assert!(frames(&plugin,&screen,&mut ctx,4).is_empty());
    let host=plugin.diagnostic_capture_pixel(21.0,34.0)
        .expect("revised input frame");
    let retained=plugin.diagnostic_capture_pixel(21.0,87.0)
        .expect("retained input frame");
    assert!(host.r.abs_diff(retained.r)<32 &&
            host.g.abs_diff(retained.g)<32,
        "new Host revision must suppress amber loss warning: {host:?} vs {retained:?}");
    assert!(button_key(&plugin,&screen,&mut ctx,"Tab").is_empty());
    assert!(has(&press(&plugin,&screen,&mut ctx,"q",false),401,"Authoritativeq"),
        "new host value wins and remains editable");
    eprintln!("LUMINA_ROUTE_CAPACITY_REBASE_X11 newer_host_revision_suppresses_warning=true no_stale_local_replay=true editable_authoritative_value=true other_fields_retained=true");
}


/// Real X11 keyboard navigation and authoritative runtime policy transitions.
/// Unlike disabled inputs, native read-only TextInput keeps its cursor/focus
/// available while every local edit/submit and composition transaction is
/// rejected before it can change EditSession or emit UiRuntimeFeedback.
#[test]
fn native_x11_read_only_textinput_ime_undo_focus_and_host_rebase(){
    let plugin=AuraLuminaPlugin::new();
    let mut ctx=NexusContext::default();
    let mut root=node("App",&[("bg","#121212")],vec![
        node("TextInput",&[("x","20"),("y","20"),("width","250"),("height","48"),
            ("radius","0"),("value","Locked"),("value_revision","1"),
            ("read_only","true"),("on_change","cb:901"),("on_submit","cb:911")],vec![]),
        node("TextInput",&[("x","20"),("y","102"),("width","250"),("height","48"),
            ("radius","0"),("value","Open"),("value_revision","1"),
            ("on_change","cb:902"),("on_submit","cb:912")],vec![]),
    ]);
    frames(&plugin,&root,&mut ctx,4);
    assert!(plugin.diagnostic_resize_window(640,360));
    frames(&plugin,&root,&mut ctx,4);
    let windows=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let win=windows.lines().next().expect("real native X11 window");
    xdo(&["windowfocus","--sync",win]);
    let locked_border=plugin.diagnostic_capture_pixel(21.0,42.0).unwrap();
    let editable_border=plugin.diagnostic_capture_pixel(21.0,122.0).unwrap();
    assert!(locked_border.b>editable_border.b.saturating_add(75),
        "native readonly state must be visibly distinguished: {locked_border:?}, {editable_border:?}");
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // readonly focus
    for k in ["q","BackSpace","Delete","Return","Left","Right"]{
        assert!(press(&plugin,&root,&mut ctx,k,false).is_empty(),
            "readonly must never send any edit or on_submit callback: {k}");
    }
    xdo(&["keydown","Control_L"]);
    frames(&plugin,&root,&mut ctx,2);
    for k in ["a","z","y"]{
        assert!(press(&plugin,&root,&mut ctx,k,false).is_empty(),
            "select/Undo/Redo in readonly cannot change value: {k}");
    }
    xdo(&["keyup","Control_L"]);
    frames(&plugin,&root,&mut ctx,2);
    assert!(plugin.submit_composition_event(CompositionEvent::Start{
        callback_id:901,sequence:1}));
    assert!(plugin.submit_composition_event(CompositionEvent::Update{
        callback_id:901,sequence:2,text:"אב".into(),caret:2}));
    assert!(plugin.submit_composition_event(CompositionEvent::Commit{
        callback_id:901,sequence:3,text:"שלום".into()}));
    assert!(frames(&plugin,&root,&mut ctx,4).is_empty(),
        "explicit adapter cannot insert preedit into a read-only session");
    assert!(plugin.diagnostic_composition(901).is_none());
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // editable
    let typed=press(&plugin,&root,&mut ctx,"x",false);
    assert!(has(&typed,902,"Openx"),"normal sibling remains editable: {typed:?}");
    assert!(button_key(&plugin,&root,&mut ctx,"Tab").is_empty()); // readonly wraps
    root.children[0].set_prop("read_only","false");
    assert!(frames(&plugin,&root,&mut ctx,3).is_empty());
    let enabled=press(&plugin,&root,&mut ctx,"q",false);
    assert!(has(&enabled,901,"Lockedq"),"Host unlocks without focus theft: {enabled:?}");
    assert!(plugin.submit_composition_event(CompositionEvent::Start{
        callback_id:901,sequence:10}));
    assert!(plugin.submit_composition_event(CompositionEvent::Update{
        callback_id:901,sequence:11,text:"אב".into(),caret:2}));
    assert!(frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(plugin.diagnostic_composition(901).is_some(),
        "writable field supports normal composition");
    root.children[0].set_prop("read_only","true");
    assert!(plugin.submit_composition_event(CompositionEvent::Commit{
        callback_id:901,sequence:12,text:"אב".into()}));
    assert!(frames(&plugin,&root,&mut ctx,4).is_empty(),
        "mid-composition lock must discard queued CommitText");
    assert!(plugin.diagnostic_composition(901).is_none(),
        "locking revokes transient preedit without committing");
    assert!(press(&plugin,&root,&mut ctx,"z",false).is_empty());
    root.children[0].set_prop("value","Server");
    root.children[0].set_prop("value_revision","2");
    assert!(frames(&plugin,&root,&mut ctx,3).is_empty(),
        "Host may authoritatively replace read-only content");
    root.children[0].set_prop("read_only","false");
    assert!(frames(&plugin,&root,&mut ctx,2).is_empty());
    let changed=press(&plugin,&root,&mut ctx,"r",false);
    assert!(has(&changed,901,"Serverr"),
        "new Host value wins over old preedit and local echo: {changed:?}");
    root.children[0].set_prop("read_only","invalid");
    assert!(frames(&plugin,&root,&mut ctx,2).is_empty());
    assert!(press(&plugin,&root,&mut ctx,"p",false).is_empty(),
        "malformed policy fails closed");
    root.children[0].set_prop("read_only","false");
    root.children[0].props.push(("read_only".into(),"false".into()));
    assert!(frames(&plugin,&root,&mut ctx,3).is_empty());
    assert!(press(&plugin,&root,&mut ctx,"p",false).is_empty(),
        "ambiguous duplicate readonly policy fails closed");
    eprintln!("LUMINA_READ_ONLY_X11 focusable=true visible_blue_native_border=true no_mutation_or_submit=true no_undo_redo=true bridge_preedit_rejected=true normal_sibling_editable=true host_unlock=true lock_cancels_live_preedit=true new_host_revision_rebases=true malformed_and_duplicate_fail_closed=true");
}
