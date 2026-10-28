#![forbid(unsafe_code)]

#[cfg(any(feature = "raylib", test))]
mod viewport;

#[cfg(any(feature = "raylib", test))]
mod layout;

#[cfg(any(feature = "raylib", test))]
mod text_input;

#[cfg(any(feature = "raylib", test))]
mod text_revision;

#[cfg(any(feature = "raylib", test))]
mod text_viewport;

#[cfg(feature = "raylib")]
use viewport::FrameMetrics;

use aura_nexus::{
    AuraPlugin,
    NexusContext,
    NexusDiagnostic,
    PluginCapability,
    UiNode,
    UiRuntimeFeedback,
};

#[cfg(feature = "raylib")]
use aura_nexus::UiTextInputEvent;

#[cfg(not(feature = "raylib"))]
use aura_nexus::format_ui_tree;

#[cfg(feature = "raylib")]
use std::cell::RefCell;

#[cfg(feature = "raylib")]
use std::collections::HashMap;

#[cfg(feature = "raylib")]
use raylib::prelude::*;

#[cfg(feature = "raylib")]
const SDF_ROUNDED_RECT_FS: &str = r#"
#version 330

in vec2 fragTexCoord;
in vec4 fragColor;

out vec4 finalColor;

uniform vec4 rect;        // x, y, w, h (pixels)
uniform float radius;     // px
uniform float softness;   // px AA width
uniform vec4 fillColor;   // rgba 0..1
uniform vec4 borderColor; // rgba 0..1
uniform float borderWidth; // px
// Fragment coordinates are physical GL pixels with a bottom-left origin.
// Rectangle/layout coordinates are logical Raylib pixels with a top-left origin.
uniform vec2 framebufferScale;
uniform float framebufferHeight;

float sdRoundRect(vec2 p, vec2 b, float r) {
    // p is centered coords; b is half-size.
    vec2 q = abs(p) - (b - vec2(r));
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

void main() {
    vec2 p = vec2(gl_FragCoord.x / framebufferScale.x,
                  (framebufferHeight - gl_FragCoord.y) / framebufferScale.y);
    vec2 pos = rect.xy;
    vec2 size = rect.zw;
    vec2 halfSize = size * 0.5;
    vec2 center = pos + halfSize;

    float r = max(radius, 0.0);
    float dist = sdRoundRect(p - center, halfSize, r);

    float aa = max(softness, 0.5);
    float fillAlpha = 1.0 - smoothstep(0.0, aa, dist);

    float bw = max(borderWidth, 0.0);
    float lineMask = (1.0 - smoothstep(bw - aa, bw + aa, abs(dist))) * fillAlpha;

    vec3 rgb = mix(fillColor.rgb, borderColor.rgb, lineMask);
    float a = fillAlpha * fillColor.a;

    finalColor = vec4(rgb, a) * fragColor;
}
"#;

#[cfg(feature = "raylib")]
const INITIAL_WINDOW_W: i32 = 1920;

#[cfg(feature = "raylib")]
const INITIAL_WINDOW_H: i32 = 1080;

#[cfg(feature = "raylib")]
pub struct AuraLuminaPlugin {
    window: RefCell<Option<LuminaWindow>>,
}

#[cfg(feature = "raylib")]
struct LuminaWindow {
    // Rust drops struct fields in declaration order. All GPU objects MUST be
    // released while the native Raylib/OpenGL context is still alive.
    // Keep the window handle LAST; dropping it first caused a SIGSEGV at
    // shutdown in the actual Xvfb Raylib smoke, not in headless unit tests.
    textures: HashMap<String, Texture2D>,
    sdf: RoundedRectShader,

    focused_input: Option<FocusedTextInput>,
    input_sessions: HashMap<u64, FocusedTextInput>,
    last_text_views: HashMap<u64,text_viewport::Visual>,
    click_anim: Option<(u64, f64)>,
    just_opened: bool,
    open_frames: u8,

    thread: RaylibThread,
    rl: RaylibHandle,
}

#[cfg(feature = "raylib")]
#[derive(Clone, Debug)]
struct FocusedTextInput {
    on_change: u64,
    on_submit: Option<u64>,
    revision: text_revision::RevisionTracker,
    session: text_input::EditSession,
    scroll_x: f32,
}

#[cfg(feature = "raylib")]
fn switch_input(win: &mut LuminaWindow, target: Option<(u64, Option<u64>, String, Option<u64>)>) {
    if let (Some(active), Some((new_id, _, _, _))) = (&win.focused_input, &target) {
        if active.on_change == *new_id { return; }
    }
    if let Some(previous) = win.focused_input.take() {
        win.input_sessions.insert(previous.on_change, previous);
    }
    if let Some((on_change, on_submit, source_value, revision)) = target {
        let mut entry = win.input_sessions.remove(&on_change).unwrap_or_else(|| FocusedTextInput {
            on_change, on_submit,
            revision: text_revision::RevisionTracker::new(&source_value, revision),
            session: text_input::EditSession::new(source_value.clone()),
            scroll_x: 0.0,
        });
        if entry.revision.observe(&source_value,revision)
            == text_revision::Decision::Rebase
            && (entry.session.buffer != source_value || revision.is_some()) {
            // Even identical text at a *new* explicit revision starts new
            // authority epoch: old undo history must not resurrect stale data.
            entry.session = text_input::EditSession::new(source_value.clone());
        }
        entry.on_submit = on_submit;
        win.focused_input = Some(entry);
    }
}

/// Only valid revision props for *uniquely identified* visible inputs.
/// A malformed token is never silently downgraded to legacy authority.
#[cfg(feature = "raylib")]
fn input_value_revisions(tree:&UiNode,plan:&layout::LayoutNode,order:&[layout::HitTarget])
    -> (HashMap<u64,Option<u64>>,std::collections::HashSet<u64>)
{
    let valid_ids:std::collections::HashSet<u64>=order.iter().filter_map(|t|match t{
        layout::HitTarget::TextInput{on_change,..}=>Some(*on_change),
        _=>None,
    }).collect();
    fn collect(node:&UiNode,plan:&layout::LayoutNode,
        valid:&std::collections::HashSet<u64>,
        map:&mut HashMap<u64,Option<u64>>,
        invalid:&mut std::collections::HashSet<u64>)
    {
        if plan.bounds.intersect(plan.clip).is_empty() {return;}
        if node.kind=="TextInput" {
            let cb=node.props.iter().find(|(key,_)|key=="on_change")
                .and_then(|(_,v)|parse_callback_id(Some(v.as_str())));
            if let Some(id)=cb.filter(|id|valid.contains(id)) {
                let raw=node.props.iter().find(|(key,_)|key=="value_revision")
                    .map(|(_,v)|v.as_str());
                match raw {
                    None=>{map.insert(id,None);}
                    Some(x)=>match x.parse::<u64>() {
                        Ok(v)=>{map.insert(id,Some(v));}
                        Err(_)=>{invalid.insert(id);}
                    }
                }
            }
        }
        for (child,prepared) in node.children.iter().zip(&plan.children) {
            if let Some(prepared)=prepared {collect(child,prepared,valid,map,invalid);}
        }
    }
    let mut result=HashMap::new();
    let mut invalid=std::collections::HashSet::new();
    collect(tree,plan,&valid_ids,&mut result,&mut invalid);
    (result,invalid)
}

/// Keep local edits when same revision host values echo older locally
/// emitted strings. Strictly increasing value_revision rebases authority.
/// The legacy path (no value_revision) keeps prior changed-prop semantics.
#[cfg(feature = "raylib")]
fn reconcile_input_sessions(
    win:&mut LuminaWindow,order:&[layout::HitTarget],
    revisions:&HashMap<u64,Option<u64>>
) {
    let present:HashMap<u64,(&str,Option<u64>,Option<u64>)> =
        order.iter().filter_map(|item|match item {
            layout::HitTarget::TextInput{on_change,on_submit,value} =>
                revisions.get(on_change).map(|version|
                    (*on_change,(value.as_str(),*on_submit,*version))),
            _=>None,
        }).collect();
    win.input_sessions.retain(|id,_|present.contains_key(id));
    let refresh=|entry:&mut FocusedTextInput,prop:&str,submit:Option<u64>,
        revision:Option<u64>| {
        if entry.revision.observe(prop,revision)==text_revision::Decision::Rebase
            && (entry.session.buffer!=prop || revision.is_some()) {
            entry.session=text_input::EditSession::new(prop.to_owned());
        }
        entry.on_submit=submit;
    };
    for (id,entry) in &mut win.input_sessions {
        if let Some((prop,submit,rev))=present.get(id) {
            refresh(entry,prop,*submit,*rev);
        }
    }
    if let Some(entry)=&mut win.focused_input {
        if let Some((prop,submit,rev))=present.get(&entry.on_change) {
            refresh(entry,prop,*submit,*rev);
        } else {win.focused_input=None;}
    }
}

/// Measure text with the *same* Raylib default-font width used by
/// draw_text, before BeginDrawing. Only a few UAX29 grapheme prefixes per
/// visible input, not an O(n²) full prefix map per frame.
#[cfg(feature = "raylib")]
fn measure_input_visuals(
    rl: &RaylibHandle, tree: &UiNode, plan: &layout::LayoutNode,
    focus_order: &[layout::HitTarget],
    focused: &mut Option<FocusedTextInput>,
    sessions: &mut HashMap<u64,FocusedTextInput>,
) -> HashMap<u64,text_viewport::Visual> {
    let allowed: std::collections::HashSet<u64> = focus_order.iter().filter_map(|x|match x {
        layout::HitTarget::TextInput{on_change,..}=>Some(*on_change),
        _=>None,
    }).collect();
    fn collect(
        rl:&RaylibHandle,node:&UiNode,plan:&layout::LayoutNode,
        allowed:&std::collections::HashSet<u64>,
        focused:&mut Option<FocusedTextInput>,
        sessions:&mut HashMap<u64,FocusedTextInput>,
        result:&mut HashMap<u64,text_viewport::Visual>,
    ) {
        if plan.bounds.intersect(plan.clip).is_empty(){return;}
        if node.kind=="TextInput" {
            if let Some(cb)=parse_callback_id(prop_string(node,"on_change")).filter(|id|allowed.contains(id)) {
                let active=focused.as_ref().is_some_and(|fi|fi.on_change==cb);
                let entry=if active {focused.as_mut()}else{sessions.get_mut(&cb)};
                let value=entry.as_ref().map(|fi|fi.session.buffer.as_str())
                    .or_else(||prop_string(node,"value").or_else(||prop_string(node,"text")))
                    .unwrap_or("");
                let size=prop_i32(node,"size").unwrap_or(18).max(10);
                let width=rl.measure_text(value,size).max(0) as f32;
                let caret=if active {
                    entry.as_ref().map(|fi|rl.measure_text(
                        text_viewport::grapheme_prefix(value,fi.session.caret),size).max(0) as f32)
                }else{None}.unwrap_or(0.0);
                let selection=if active {
                    entry.as_ref().and_then(|fi|fi.session.selection())
                        .map(|(a,b)|(
                            rl.measure_text(text_viewport::grapheme_prefix(value,a),size).max(0) as f32,
                            rl.measure_text(text_viewport::grapheme_prefix(value,b),size).max(0) as f32))
                }else{None};
                let previous=entry.as_ref().map(|fi|fi.scroll_x).unwrap_or(0.0);
                let content=text_viewport::content_bounds(plan.bounds);
                let v=text_viewport::plan(content.width,width,caret,selection,previous,active);
                if let Some(entry)=entry {entry.scroll_x=v.scroll;}
                result.insert(cb,v);
            }
        }
        for (child,slot) in node.children.iter().zip(&plan.children){
            if let Some(child_plan)=slot {
                collect(rl,child,child_plan,allowed,focused,sessions,result);
            }
        }
    }
    let mut result=HashMap::new();
    collect(rl,tree,plan,&allowed,focused,sessions,&mut result);
    result
}

#[cfg(feature = "raylib")]
struct RoundedRectShader {
    shader: Shader,
    loc_rect: i32,
    loc_radius: i32,
    loc_softness: i32,
    loc_fill: i32,
    loc_border: i32,
    loc_border_width: i32,
    loc_framebuffer_scale: i32,
    loc_framebuffer_height: i32,
}

#[cfg(feature = "raylib")]
fn color_to_vec4(c: Color) -> [f32; 4] {
    [
        (c.r as f32) / 255.0,
        (c.g as f32) / 255.0,
        (c.b as f32) / 255.0,
        (c.a as f32) / 255.0,
    ]
}

#[cfg(feature = "raylib")]
impl Default for AuraLuminaPlugin {
    fn default() -> Self {
        Self {
            window: RefCell::new(None),
        }
    }
}

#[cfg(not(feature = "raylib"))]
#[derive(Default)]
pub struct AuraLuminaPlugin;

impl AuraLuminaPlugin {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Opt-in native diagnostic info. This is not enabled for production builds.
#[cfg(feature = "lumina-diagnostics")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiagnosticMetrics {
    pub logical_width: i32,
    pub logical_height: i32,
    pub render_width: i32,
    pub render_height: i32,
    pub scale_x: f32,
    pub scale_y: f32,
}

#[cfg(feature = "lumina-diagnostics")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticPixel {
    pub r: u8, pub g: u8, pub b: u8, pub a: u8,
}

#[cfg(feature = "lumina-diagnostics")]
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct DiagnosticTextVisual {
    pub scroll_x: f32,
    pub measured_caret: f32,
    pub caret_screen_x: f32,
    pub selection_screen: Option<(f32,f32)>,
    pub content_left: f32,
    pub content_right: f32,
}

#[cfg(feature = "lumina-diagnostics")]
impl AuraLuminaPlugin {
    /// Read exact latest frame's default-font logical text geometry.
    pub fn diagnostic_text_visual(&self,on_change:u64,content_left:f32,content_width:f32)
        ->Option<DiagnosticTextVisual> {
        let w=self.window.borrow();
        let v=*w.as_ref()?.last_text_views.get(&on_change)?;
        if !content_left.is_finite() || !content_width.is_finite()
            || content_width<=0.0 { return None; }
        let c=layout::Bounds::new(content_left,0.0,content_width,1.0);
        Some(DiagnosticTextVisual {
            scroll_x:v.scroll,measured_caret:v.caret,
            caret_screen_x:v.caret_screen(c),
            selection_screen:v.selection_screen(c),
            content_left:c.x,content_right:c.x+c.width,
        })
    }

    /// Real native window resize. Runs only in opt-in Xvfb/device tests.
    pub fn diagnostic_resize_window(&self, width: i32, height: i32) -> bool {
        if width <= 0 || height <= 0 { return false; }
        let mut guard = self.window.borrow_mut();
        if let Some(w) = guard.as_mut() {
            w.rl.set_window_size(width, height);
            true
        } else { false }
    }

    /// Samples physical render size independently of logical window geometry.
    pub fn diagnostic_frame_metrics(&self) -> Option<DiagnosticMetrics> {
        let guard = self.window.borrow();
        let w = guard.as_ref()?;
        let m = FrameMetrics::from_window(
            w.rl.get_screen_width(), w.rl.get_screen_height(),
            w.rl.get_render_width(), w.rl.get_render_height());
        Some(DiagnosticMetrics {
            logical_width: m.logical.width as i32,
            logical_height: m.logical.height as i32,
            render_width: m.render_width, render_height: m.render_height,
            scale_x: m.scale_x, scale_y: m.scale_y,
        })
    }

    /// Read back the GPU screen buffer. Use stable solid-color interiors and
    /// repeated frames for virtual X11 because buffer swap timing is platform
    /// dependent. No absolute pixel parity outside that environment is implied.
    pub fn diagnostic_capture_pixel(&self, logical_x: f32, logical_y: f32) -> Option<DiagnosticPixel> {
        let guard = self.window.borrow();
        let w = guard.as_ref()?;
        let m = FrameMetrics::from_window(
            w.rl.get_screen_width(), w.rl.get_screen_height(),
            w.rl.get_render_width(), w.rl.get_render_height());
        let (px,py) = m.logical_to_framebuffer(logical_x,logical_y)?;
        let mut image = w.rl.load_image_from_screen(&w.thread);
        if px >= image.width() || py >= image.height() { return None; }
        let c = image.get_color(px,py);
        Some(DiagnosticPixel { r:c.r,g:c.g,b:c.b,a:c.a })
    }
}

impl AuraPlugin for AuraLuminaPlugin {
    fn name(&self) -> &'static str {
        "aura-lumina"
    }

    fn capabilities(&self) -> &'static [PluginCapability] {
        &[PluginCapability::UiRuntime, PluginCapability::Z3Theories]
    }

    fn on_ui_render(
        &self,
        tree: &UiNode,
        nexus: &mut NexusContext,
    ) -> Option<Result<(), NexusDiagnostic>> {
        #[cfg(not(feature = "raylib"))]
        {
            use std::sync::atomic::{AtomicBool, Ordering};

            static WARNED: AtomicBool = AtomicBool::new(false);
            if !WARNED.swap(true, Ordering::Relaxed) {
                eprintln!(
                    "Aura Lumina UI is running in headless mode (built without the `raylib` feature), so no window can be opened.\n\
Rebuild the `aura` binary with `--features lumina-raylib` (or keep default features enabled)."
                );
            }

            // Fallback: print once (no interactive loop).
            print!("{}", format_ui_tree(tree));

            if nexus.get::<UiRuntimeFeedback>().is_none() {
                nexus.insert(UiRuntimeFeedback::default());
            }
            let fb = nexus.get_mut::<UiRuntimeFeedback>().expect("inserted");
            fb.close_requested = true;
            fb.clicked_callback_id = None;

            return Some(Ok(()));
        }

        #[cfg(feature = "raylib")]
        {
            let mut win_ref = self.window.borrow_mut();
            if win_ref.is_none() {
                let (mut rl, thread) = raylib::init()
                    .size(INITIAL_WINDOW_W, INITIAL_WINDOW_H)
                    .title("Aura Lumina Sentinel")
                    .resizable()
                    .build();
                rl.set_target_fps(60);
                // Keep the AVM-driven UI loop alive; closing should be explicit via the window close button.
                // Raylib defaults to closing on Escape; disable that.
                rl.set_exit_key(None);

                let shader = rl.load_shader_from_memory(&thread, None, Some(SDF_ROUNDED_RECT_FS));
                let sdf = RoundedRectShader {
                    loc_rect: shader.get_shader_location("rect"),
                    loc_radius: shader.get_shader_location("radius"),
                    loc_softness: shader.get_shader_location("softness"),
                    loc_fill: shader.get_shader_location("fillColor"),
                    loc_border: shader.get_shader_location("borderColor"),
                    loc_border_width: shader.get_shader_location("borderWidth"),
                    loc_framebuffer_scale: shader.get_shader_location("framebufferScale"),
                    loc_framebuffer_height: shader.get_shader_location("framebufferHeight"),
                    shader,
                };
                *win_ref = Some(LuminaWindow {
                    rl,
                    thread,
                    just_opened: true,
                    open_frames: 0,
                    sdf,
                    click_anim: None,
                    focused_input: None,
                    input_sessions: HashMap::new(),
                    last_text_views: HashMap::new(),
                    textures: HashMap::new(),
                });
            }

            let win = win_ref.as_mut().expect("window initialized");

            // Preload any image textures before begin_drawing (needs &mut RaylibHandle).
            ensure_textures_loaded(&mut win.rl, &win.thread, &mut win.textures, tree);

            let mut fb = UiRuntimeFeedback::default();
            // Some environments can briefly report a close request right after initialization.
            // Ignore close requests for a few frames; after that, honor them immediately so the
            // window close button (X) works as expected.
            let should_close = win.rl.window_should_close();
            if win.just_opened {
                win.just_opened = false;
                win.open_frames = 0;
            } else {
                win.open_frames = win.open_frames.saturating_add(1);
            }

            let ignore_close = win.open_frames < 5;
            fb.close_requested = should_close && !ignore_close;

            let mouse = win.rl.get_mouse_position();
            let clicked = win.rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);
            let now = win.rl.get_time();

            // Keyboard sampling must happen before begin_drawing (borrow rules).
            let backspace = win.rl.is_key_pressed(KeyboardKey::KEY_BACKSPACE);
            let delete = win.rl.is_key_pressed(KeyboardKey::KEY_DELETE);
            let left = win.rl.is_key_pressed(KeyboardKey::KEY_LEFT);
            let right = win.rl.is_key_pressed(KeyboardKey::KEY_RIGHT);
            let home = win.rl.is_key_pressed(KeyboardKey::KEY_HOME);
            let end = win.rl.is_key_pressed(KeyboardKey::KEY_END);
            let enter = win.rl.is_key_pressed(KeyboardKey::KEY_ENTER)
                || win.rl.is_key_pressed(KeyboardKey::KEY_KP_ENTER);
            let escape = win.rl.is_key_pressed(KeyboardKey::KEY_ESCAPE);
            let ctrl = win.rl.is_key_down(KeyboardKey::KEY_LEFT_CONTROL)
                || win.rl.is_key_down(KeyboardKey::KEY_RIGHT_CONTROL);
            let select_all = ctrl && win.rl.is_key_pressed(KeyboardKey::KEY_A);
            let undo = ctrl && win.rl.is_key_pressed(KeyboardKey::KEY_Z);
            let redo = ctrl && win.rl.is_key_pressed(KeyboardKey::KEY_Y);
            let extend_selection = win.rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT)
                || win.rl.is_key_down(KeyboardKey::KEY_RIGHT_SHIFT);
            let tab = win.rl.is_key_pressed(KeyboardKey::KEY_TAB);
            let reverse_tab = win.rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT)
                || win.rl.is_key_down(KeyboardKey::KEY_RIGHT_SHIFT);

            let mut typed = String::new();
            while let Some(ch) = win.rl.get_char_pressed() {
                // Basic filtering: accept printable chars; keep newline out.
                if ch != '\n' && ch != '\r' {
                    typed.push(ch);
                }
            }

            // Geometry & pointer coordinates stay LOGICAL. The shader alone
            // transforms gl_FragCoord physical bottom-left pixels to logical
            // top-left coordinates. Raylib's scissor mode performs its own DPI
            // transform: do NOT scale scissor rectangles again here.
            let frame = FrameMetrics::from_window(
                win.rl.get_screen_width(), win.rl.get_screen_height(),
                win.rl.get_render_width(), win.rl.get_render_height(),
            );
            let viewport = frame.logical;
            let valid_frame = frame.scale_x > 0.0 && frame.scale_y > 0.0;
            if valid_frame {
                win.sdf.shader.set_shader_value(
                    win.sdf.loc_framebuffer_scale, [frame.scale_x, frame.scale_y]);
                win.sdf.shader.set_shader_value(
                    win.sdf.loc_framebuffer_height, frame.render_height as f32);
            }
            // Construct all geometry before beginning the paint pass. Headless
            // tests exercise this exact measure/arrange model without Raylib.
            let planned_layout = layout::arrange(tree, layout::Bounds::new(
                0.0, 0.0, viewport.width, viewport.height,
            ));
            let mut focus_order = layout::focus_order(tree, &planned_layout);
            let (revisions,invalid)=input_value_revisions(tree,&planned_layout,&focus_order);
            focus_order.retain(|entry|match entry {
                layout::HitTarget::TextInput{on_change,..}=>!invalid.contains(on_change),
                _=>false,
            });
            reconcile_input_sessions(win,&focus_order,&revisions);
            let click_cb = if clicked {
                match layout::hit_test(tree, &planned_layout, mouse.x, mouse.y) {
                    Some(layout::HitTarget::Button(id)) => {
                        switch_input(win, None);Some(id)
                    }
                    Some(layout::HitTarget::TextInput {on_change,on_submit,value}) => {
                        if focus_order.iter().any(|candidate|matches!(candidate,
                            layout::HitTarget::TextInput{on_change:id,..} if *id==on_change)) {
                            switch_input(win,Some((on_change,on_submit,value,
                                revisions.get(&on_change).copied().flatten())));
                        } else {
                            // Duplicate callback IDs are ambiguous: never
                            // grant mouse focus to a field that cannot own a
                            // unique session identity.
                            switch_input(win,None);
                        }
                        None
                    }
                    None => {switch_input(win,None);None}
                }
            } else {None};
            if tab {
                let active=win.focused_input.as_ref().map(|fi|fi.on_change);
                let target=layout::next_focus(&focus_order,active,reverse_tab);
                match target {
                    Some(layout::HitTarget::TextInput {on_change,on_submit,value}) =>
                        switch_input(win,Some((on_change,on_submit,value,
                            revisions.get(&on_change).copied().flatten()))),
                    _=>switch_input(win,None),
                }
            }
            if let Some(fi)=&mut win.focused_input {
                if escape {
                    switch_input(win,None);
                } else {
                    use text_input::{EditCommand,SessionAction};
                    if select_all {fi.session.apply(SessionAction::SelectAll);}
                    if home {fi.session.apply(SessionAction::Move(EditCommand::Home,extend_selection));}
                    if end {fi.session.apply(SessionAction::Move(EditCommand::End,extend_selection));}
                    if left {fi.session.apply(SessionAction::Move(EditCommand::Left,extend_selection));}
                    if right {fi.session.apply(SessionAction::Move(EditCommand::Right,extend_selection));}
                    let mut changed=false;
                    if undo {changed|=fi.session.apply(SessionAction::Undo);}
                    if redo {changed|=fi.session.apply(SessionAction::Redo);}
                    if backspace {changed|=fi.session.apply(SessionAction::Edit(EditCommand::Backspace));}
                    if delete {changed|=fi.session.apply(SessionAction::Edit(EditCommand::Delete));}
                    if !ctrl && !typed.is_empty() {
                        changed|=fi.session.apply(SessionAction::Edit(EditCommand::Insert(typed)));
                    }
                    if changed {
                        fi.revision.record_local_edit(&fi.session.buffer);
                        fb.text_input_events.push(UiTextInputEvent {
                            callback_id:fi.on_change,text:fi.session.buffer.clone(),submitted:false,
                        });
                    }
                    if enter {
                        if let Some(cb)=fi.on_submit {
                            fb.text_input_events.push(UiTextInputEvent {
                                callback_id:cb,text:fi.session.buffer.clone(),submitted:true,
                            });
                        }
                    }
                }
            }
            let text_views=measure_input_visuals(
                &win.rl,tree,&planned_layout,&focus_order,
                &mut win.focused_input,&mut win.input_sessions);
            win.last_text_views=text_views.clone();
            let (rl,thread,sdf)=(&mut win.rl,&win.thread,&mut win.sdf);
            let mut d = rl.begin_drawing(thread);
            // Allow app-level theming via `App(bg: ...)`.
            let app_bg = parse_color(prop_string(tree, "bg").or_else(|| prop_string(tree, "background")));
            d.clear_background(app_bg);

            if valid_frame {
                render_node(
                    &mut d, tree, &planned_layout, now, sdf,
                    win.click_anim, &win.focused_input, &win.textures, &win.input_sessions,
                    &text_views,
                );
            }
            fb.clicked_callback_id = click_cb;

            if let Some(id) = click_cb {
                win.click_anim = Some((id, now));
            } else {
                // Clear once the animation has elapsed.
                if let Some((_id, start)) = win.click_anim {
                    if (now - start) > 0.25 {
                        win.click_anim = None;
                    }
                }
            }

            // Publish feedback for the AVM loop.
            if nexus.get::<UiRuntimeFeedback>().is_none() {
                nexus.insert(UiRuntimeFeedback::default());
            }
            let dst = nexus.get_mut::<UiRuntimeFeedback>().expect("inserted");
            *dst = fb;

            return Some(Ok(()));
        }

        #[allow(unreachable_code)]
        Some(Ok(()))
    }
}

#[cfg(feature = "raylib")]
fn prop<'a>(node: &'a UiNode, k: &str) -> Option<&'a str> {
    node.props.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.as_str())
}

#[cfg(feature = "raylib")]
fn prop_i32(node: &UiNode, k: &str) -> Option<i32> {
    prop(node, k).and_then(|v| v.parse::<i32>().ok())
}

#[cfg(feature = "raylib")]
fn prop_string<'a>(node: &'a UiNode, k: &str) -> Option<&'a str> {
    prop(node, k)
}

#[cfg(feature = "raylib")]
fn parse_color(name: Option<&str>) -> Color {
    let s = name.unwrap_or("White").trim();
    if let Some(hex) = s.strip_prefix('#') {
        // Accept #RRGGBB or #RRGGBBAA
        if hex.len() == 6 || hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok();
            let g = u8::from_str_radix(&hex[2..4], 16).ok();
            let b = u8::from_str_radix(&hex[4..6], 16).ok();
            let a = if hex.len() == 8 {
                u8::from_str_radix(&hex[6..8], 16).ok()
            } else {
                Some(255)
            };
            if let (Some(r), Some(g), Some(b), Some(a)) = (r, g, b, a) {
                return Color::new(r, g, b, a);
            }
        }
    }

    // rgb(r,g,b) / rgba(r,g,b,a) where a can be 0..1 or 0..255
    let lower = s.to_ascii_lowercase();
    if let Some(args) = lower.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
        let mut it = args.split(',').map(|p| p.trim());
        let r = it.next().and_then(|v| v.parse::<u8>().ok());
        let g = it.next().and_then(|v| v.parse::<u8>().ok());
        let b = it.next().and_then(|v| v.parse::<u8>().ok());
        if let (Some(r), Some(g), Some(b)) = (r, g, b) {
            return Color::new(r, g, b, 255);
        }
    }
    if let Some(args) = lower.strip_prefix("rgba(").and_then(|v| v.strip_suffix(')')) {
        let parts: Vec<&str> = args.split(',').map(|p| p.trim()).collect();
        if parts.len() == 4 {
            let r = parts[0].parse::<u8>().ok();
            let g = parts[1].parse::<u8>().ok();
            let b = parts[2].parse::<u8>().ok();
            let a_u8 = if let Ok(a) = parts[3].parse::<u8>() {
                Some(a)
            } else if let Ok(a) = parts[3].parse::<f32>() {
                Some((a.clamp(0.0, 1.0) * 255.0).round() as u8)
            } else {
                None
            };
            if let (Some(r), Some(g), Some(b), Some(a)) = (r, g, b, a_u8) {
                return Color::new(r, g, b, a);
            }
        }
    }

    // A small named palette (case-insensitive). Prefer expanding via hex/rgb.
    match lower.as_str() {
        "gold" => Color::GOLD,
        "white" => Color::WHITE,
        "black" => Color::BLACK,
        "red" => Color::RED,
        "green" => Color::GREEN,
        "blue" => Color::BLUE,
        "raywhite" => Color::RAYWHITE,
        "lightgray" | "lightgrey" => Color::LIGHTGRAY,
        "gray" | "grey" => Color::GRAY,
        "darkgray" | "darkgrey" => Color::DARKGRAY,
        "maroon" => Color::MAROON,
        "orange" => Color::ORANGE,
        "yellow" => Color::YELLOW,
        "purple" => Color::PURPLE,
        "violet" => Color::VIOLET,
        "pink" => Color::PINK,
        "skyblue" => Color::SKYBLUE,
        "lime" => Color::LIME,
        "beige" => Color::BEIGE,
        "brown" => Color::BROWN,
        "transparent" => Color::new(0, 0, 0, 0),
        _ => Color::WHITE,
    }
}

#[cfg(feature = "raylib")]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let t = t.clamp(0.0, 1.0);
    let af = a as f32;
    let bf = b as f32;
    (af + (bf - af) * t).round().clamp(0.0, 255.0) as u8
}

#[cfg(feature = "raylib")]
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        lerp_u8(a.r, b.r, t),
        lerp_u8(a.g, b.g, t),
        lerp_u8(a.b, b.b, t),
        lerp_u8(a.a, b.a, t),
    )
}

#[cfg(feature = "raylib")]
fn parse_callback_id(s: Option<&str>) -> Option<u64> {
    let s = s?;
    let s = s.strip_prefix("cb:")?;
    s.parse::<u64>().ok()
}

#[cfg(feature = "raylib")]
fn ensure_textures_loaded(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    textures: &mut HashMap<String, Texture2D>,
    node: &UiNode,
) {
    if node.kind == "Image" {
        if let Some(src) = prop_string(node, "src").or_else(|| prop_string(node, "path")) {
            let src = src.to_string();
            if !textures.contains_key(&src) {
                if let Ok(tex) = rl.load_texture(thread, &src) {
                    textures.insert(src, tex);
                }
            }
        }
    }

    for child in &node.children {
        ensure_textures_loaded(rl, thread, textures, child);
    }
}

#[cfg(feature = "raylib")]
fn render_children(
    d: &mut RaylibDrawHandle,
    node: &UiNode,
    layout: &layout::LayoutNode,
    now: f64,
    sdf: &mut RoundedRectShader,
    click_anim: Option<(u64, f64)>,
    focused_input: &Option<FocusedTextInput>,
    textures: &HashMap<String, Texture2D>,
    input_sessions: &HashMap<u64, FocusedTextInput>,
    text_views: &HashMap<u64,text_viewport::Visual>,
) {
    for (child, prepared) in node.children.iter().zip(&layout.children) {
        if let Some(prepared) = prepared {
            render_node(d, child, prepared, now, sdf, click_anim, focused_input, textures, input_sessions, text_views);
        }
    }
}

#[cfg(feature = "raylib")]
fn render_node(
    d: &mut RaylibDrawHandle,
    node: &UiNode,
    layout: &layout::LayoutNode,
    now: f64,
    sdf: &mut RoundedRectShader,
    click_anim: Option<(u64, f64)>,
    focused_input: &Option<FocusedTextInput>,
    textures: &HashMap<String, Texture2D>,
    input_sessions: &HashMap<u64, FocusedTextInput>,
    text_views: &HashMap<u64,text_viewport::Visual>,
) {
    // Independent scissor scopes. TextInput draws its frame under inherited
    // clipping, then its text under its inner bounds, never nesting scissors.
    let Some((x, y, width, height)) = layout.clip.scissor() else { return };
    {
        let mut clipped = d.begin_scissor_mode(x, y, width, height);
        paint_node(&mut clipped, node, layout, now, sdf, click_anim,
            focused_input, textures, input_sessions, text_views, false);
    }
    if node.kind=="TextInput" {
        // TextInput content clip: glyphs/selection/caret stay inside padding.
        let inside=layout.clip.intersect(text_viewport::content_bounds(layout.bounds));
        if let Some((x,y,w,h))=inside.scissor() {
            let mut clipped=d.begin_scissor_mode(x,y,w,h);
            paint_node(&mut clipped,node,layout,now,sdf,click_anim,
                focused_input,textures,input_sessions,text_views,true);
        }
    }
    render_children(d, node, layout, now, sdf, click_anim, focused_input, textures, input_sessions, text_views);
}

#[cfg(feature = "raylib")]
fn paint_node<D: RaylibDraw>(
    d: &mut D,
    node: &UiNode,
    layout: &layout::LayoutNode,
    now: f64,
    sdf: &mut RoundedRectShader,
    click_anim: Option<(u64, f64)>,
    focused_input: &Option<FocusedTextInput>,
    textures: &HashMap<String, Texture2D>,
    input_sessions: &HashMap<u64, FocusedTextInput>,
    text_views: &HashMap<u64,text_viewport::Visual>,
    text_only: bool,
) {
    // Paint consumes the already arranged (including absolute overrides) bounds.
    // No geometry measurement or placement occurs during draw recursion.
    let b = layout.bounds;
    let bounds = Rectangle::new(b.x, b.y, b.width, b.height);

    match node.kind.as_str() {
        "Box" | "Grid" => {
            // Draw only; Box/Grid dimensions and children were arranged in
            // the pure layout prepass. No per-frame measurement inside paint.
            let rect = bounds;
            let bg = parse_color(prop_string(node, "bg").or_else(|| prop_string(node, "background")));
            let border = parse_color(prop_string(node, "border").or_else(|| prop_string(node, "stroke")));
            let border_w = prop_i32(node, "border_width")
                .or_else(|| prop_i32(node, "stroke_width"))
                .unwrap_or(0).max(0) as f32;
            let radius = prop_i32(node, "radius").unwrap_or(0).max(0) as f32;

            if radius > 0.0 {
                let min_dim = rect.width.min(rect.height).max(1.0);
                let rect_u = [rect.x, rect.y, rect.width, rect.height];
                sdf.shader.set_shader_value(sdf.loc_rect, rect_u);
                sdf.shader.set_shader_value(sdf.loc_radius, radius.min(min_dim * 0.5));
                sdf.shader.set_shader_value(sdf.loc_softness, 1.25_f32);
                sdf.shader.set_shader_value(sdf.loc_fill, color_to_vec4(bg));
                sdf.shader.set_shader_value(sdf.loc_border, color_to_vec4(border));
                sdf.shader.set_shader_value(sdf.loc_border_width, border_w);
                let mut sd = d.begin_shader_mode(&mut sdf.shader);
                sd.draw_rectangle_rec(rect, Color::WHITE);
            } else {
                if bg.a > 0 { d.draw_rectangle_rec(rect, bg); }
                if border_w > 0.0 { d.draw_rectangle_lines_ex(rect, border_w, border); }
            }
        }
        "App" | "VStack" | "HStack" => {}
        "Text" => {
            let size = prop_i32(node, "size").unwrap_or(20);
            let color = parse_color(prop_string(node, "color").or_else(|| prop_string(node, "fg")));
            let text = prop_string(node, "text")
                .or_else(|| prop_string(node, "content"))
                .unwrap_or("");
            d.draw_text(text, bounds.x as i32, bounds.y as i32, size, color);
        }
        "Image" => {
            let rect = bounds;

            let src = prop_string(node, "src").or_else(|| prop_string(node, "path"));
            let Some(src) = src else {
                d.draw_rectangle_rec(rect, Color::DARKGRAY);
                d.draw_text("Image: missing src", rect.x as i32 + 8, rect.y as i32 + 8, 16, Color::RAYWHITE);
                return;
            };

            if let Some(tex) = textures.get(src) {
                let fit = prop_string(node, "fit").unwrap_or("stretch");
                let tint = parse_color(prop_string(node, "tint").or_else(|| prop_string(node, "color")));

                let src_w = tex.width as f32;
                let src_h = tex.height as f32;
                let mut src_rect = Rectangle::new(0.0, 0.0, src_w, src_h);
                let mut dst_rect = rect;

                if fit == "contain" {
                    let sx = rect.width / src_w;
                    let sy = rect.height / src_h;
                    let s = sx.min(sy);
                    let dw = (src_w * s).max(1.0);
                    let dh = (src_h * s).max(1.0);
                    dst_rect = Rectangle::new(
                        rect.x + (rect.width - dw) / 2.0,
                        rect.y + (rect.height - dh) / 2.0,
                        dw,
                        dh,
                    );
                } else if fit == "cover" {
                    // Crop the source rect to match destination aspect ratio.
                    let src_aspect = src_w / src_h;
                    let dst_aspect = rect.width / rect.height;
                    if src_aspect > dst_aspect {
                        // Source too wide -> crop width.
                        let new_w = src_h * dst_aspect;
                        let x0 = (src_w - new_w) / 2.0;
                        src_rect = Rectangle::new(x0, 0.0, new_w, src_h);
                    } else if src_aspect < dst_aspect {
                        // Source too tall -> crop height.
                        let new_h = src_w / dst_aspect;
                        let y0 = (src_h - new_h) / 2.0;
                        src_rect = Rectangle::new(0.0, y0, src_w, new_h);
                    }
                }

                d.draw_texture_pro(
                    tex,
                    src_rect,
                    dst_rect,
                    Vector2::new(0.0, 0.0),
                    0.0,
                    tint,
                );
            } else {
                d.draw_rectangle_rec(rect, Color::DARKGRAY);
                d.draw_text(
                    &format!("Image not loaded: {}", src),
                    rect.x as i32 + 8,
                    rect.y as i32 + 8,
                    16,
                    Color::RAYWHITE,
                );
            }
        }
        "TextInput" => {
            let rect = bounds;

            let bg = parse_color(prop_string(node, "bg").or_else(|| prop_string(node, "background")).or(Some("#0D1117")));
            let fg = parse_color(prop_string(node, "fg").or_else(|| prop_string(node, "color")).or(Some("#E6EDF3")));
            let placeholder_c = parse_color(Some("#8B949E"));
            let border = parse_color(prop_string(node, "border").or(Some("#30363D")));
            let radius = prop_i32(node, "radius").unwrap_or(12).max(0) as f32;

            // The focus decision was already made by pure hit testing.
            let on_change = parse_callback_id(prop_string(node, "on_change"));
            let is_focused = match (focused_input.as_ref(), on_change) {
                (Some(fi), Some(cb)) => fi.on_change == cb,
                _ => false,
            };

            // Frame/background separate from text clipping.
            if !text_only {
            if radius > 0.0 {
                let min_dim = rect.width.min(rect.height).max(1.0);
                let rect_u = [rect.x, rect.y, rect.width, rect.height];
                let radius_u = radius.min(min_dim * 0.5);
                let softness_u = 1.25_f32;
                let border_w_u = 2.0_f32;

                sdf.shader.set_shader_value(sdf.loc_rect, rect_u);
                sdf.shader.set_shader_value(sdf.loc_radius, radius_u);
                sdf.shader.set_shader_value(sdf.loc_softness, softness_u);
                sdf.shader.set_shader_value(sdf.loc_fill, color_to_vec4(bg));
                sdf.shader.set_shader_value(sdf.loc_border, color_to_vec4(border));
                sdf.shader.set_shader_value(sdf.loc_border_width, border_w_u);

                let mut sd = d.begin_shader_mode(&mut sdf.shader);
                sd.draw_rectangle_rec(rect, Color::WHITE);
            } else {
                d.draw_rectangle_rec(rect, bg);
                d.draw_rectangle_lines_ex(rect, 2.0, border);
            }

            return;
            }
            // Display retained drafts for nonfocused inputs until the host
            // changes their authoritative value. Avoid the old visual reset
            // to stale controlled props on Tab/blur.
            let value = if is_focused {
                focused_input.as_ref().map(|fi|fi.session.buffer.as_str()).unwrap_or("")
            } else {
                on_change.and_then(|cb|input_sessions.get(&cb))
                    .map(|fi|fi.session.buffer.as_str())
                    .or_else(||prop_string(node,"value").or_else(||prop_string(node,"text")))
                    .unwrap_or("")
            };

            let placeholder = prop_string(node, "placeholder").unwrap_or("");
            let display = if value.is_empty() { placeholder } else { value };
            let display_color = if value.is_empty() { placeholder_c } else { fg };

            let ts = prop_i32(node, "size").unwrap_or(18).max(10);
            let pad_y = (rect.height - ts as f32) / 2.0;
            let content=text_viewport::content_bounds(b);
            let visual=on_change.and_then(|cb|text_views.get(&cb));
            let text_x=visual.map(|v|v.origin(content)).unwrap_or(content.x);
            // Native MeasureText widths shared by selection and caret;
            // scissor prevents long text from escaping the input widget.
            if is_focused {
                if let Some((x0,x1))=visual.and_then(|v|v.selection_screen(content)) {
                    d.draw_rectangle_rec(Rectangle::new(x0,rect.y+4.0,
                        x1-x0,(rect.height-8.0).max(0.0)),Color::new(59,130,246,100));
                }
            }
            d.draw_text(display,text_x.floor() as i32,(rect.y+pad_y) as i32,ts,display_color);

            // Exact default-font pixel measurements, not an average cell;
            // separate BiDi/IME/font fallback certification is still needed.
            if is_focused {
                if let Some(v)=visual {
                    let cx=v.caret_screen(content);
                    let cy0=rect.y+10.0;
                    let cy1=rect.y+rect.height-10.0;
                    if cy1>cy0 {
                        d.draw_line(cx.floor() as i32,cy0 as i32,
                            cx.floor() as i32,cy1 as i32,Color::RAYWHITE);
                    }
                }
            }
        }
        "Rect" => {
            let rect = bounds;

            let fill = parse_color(prop_string(node, "color").or_else(|| prop_string(node, "fg")).or_else(|| prop_string(node, "fill")));
            let radius = prop_i32(node, "radius").unwrap_or(0).max(0) as f32;

            if radius > 0.0 {
                let min_dim = rect.width.min(rect.height).max(1.0);
                let rect_u = [rect.x, rect.y, rect.width, rect.height];
                let radius_u = radius.min(min_dim * 0.5);
                let softness_u = 1.25_f32;
                let border_w_u = 0.0_f32;

                sdf.shader.set_shader_value(sdf.loc_rect, rect_u);
                sdf.shader.set_shader_value(sdf.loc_radius, radius_u);
                sdf.shader.set_shader_value(sdf.loc_softness, softness_u);
                sdf.shader.set_shader_value(sdf.loc_fill, color_to_vec4(fill));
                sdf.shader.set_shader_value(sdf.loc_border, color_to_vec4(fill));
                sdf.shader.set_shader_value(sdf.loc_border_width, border_w_u);

                let mut sd = d.begin_shader_mode(&mut sdf.shader);
                sd.draw_rectangle_rec(rect, Color::WHITE);
            } else {
                d.draw_rectangle_rec(rect, fill);
            }

        }
        "Button" => {
            let rect = bounds;

            let base_bg = parse_color(prop_string(node, "bg").or_else(|| prop_string(node, "background")));
            let fg = parse_color(prop_string(node, "fg").or_else(|| prop_string(node, "color")));
            let radius = prop_i32(node, "radius").unwrap_or(0).max(0) as f32;

            // 200ms click tween: brighten the background briefly when clicked.
            let mut bg = base_bg;
            if let Some((id, start)) = click_anim {
                if let Some(cb) = parse_callback_id(prop_string(node, "on_click")) {
                    if cb == id {
                        let t = ((now - start) as f32 / 0.2).clamp(0.0, 1.0);
                        // ease-out
                        let tt = 1.0 - (1.0 - t) * (1.0 - t);
                        bg = lerp_color(base_bg, Color::RAYWHITE, tt * 0.25);
                    }
                }
            }

            // Rounded rect rendering: prefer rounded corners when radius > 0.
            if radius > 0.0 {
                let min_dim = rect.width.min(rect.height).max(1.0);
                let rect_u = [rect.x, rect.y, rect.width, rect.height];
                let radius_u = (radius).min(min_dim * 0.5);
                let softness_u = 1.25_f32;
                let border_w_u = 2.0_f32;

                sdf.shader.set_shader_value(sdf.loc_rect, rect_u);
                sdf.shader.set_shader_value(sdf.loc_radius, radius_u);
                sdf.shader.set_shader_value(sdf.loc_softness, softness_u);
                sdf.shader.set_shader_value(sdf.loc_fill, color_to_vec4(bg));
                sdf.shader
                    .set_shader_value(sdf.loc_border, color_to_vec4(Color::RAYWHITE));
                sdf.shader.set_shader_value(sdf.loc_border_width, border_w_u);

                let mut sd = d.begin_shader_mode(&mut sdf.shader);
                // White is multiplied by shader output (fragColor).
                sd.draw_rectangle_rec(rect, Color::WHITE);
            } else {
                d.draw_rectangle_rec(rect, bg);
                d.draw_rectangle_lines_ex(rect, 2.0, Color::RAYWHITE);
            }

            let label = prop_string(node, "label").unwrap_or("Button");
            let ts = 20;
            // Simple centering with a rough width estimate.
            let est_w = (label.chars().count() as f32) * (ts as f32 * 0.6);
            let tx = rect.x + (rect.width - est_w) / 2.0;
            let ty = rect.y + (rect.height - ts as f32) / 2.0;
            d.draw_text(label, tx as i32, ty as i32, ts, fg);

        }
        _ => {}
    }
}
