#![cfg(feature="lumina-diagnostics")]
// Real X11/XTest tab traversal. Only logical reading-order focus, NOT
// accessibility focus order, assistive technologies, IME or BiDi semantics.
use std::process::Command;
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback,UiTextInputEvent};
use aura_plugin_lumina::AuraLuminaPlugin;

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
