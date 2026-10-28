#![forbid(unsafe_code)]

#[cfg(any(feature = "raylib", test))]
mod viewport;

#[cfg(any(feature = "raylib", test))]
mod layout;

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
    buffer: String,
    caret: usize,
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
impl AuraLuminaPlugin {
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
            let enter = win.rl.is_key_pressed(KeyboardKey::KEY_ENTER)
                || win.rl.is_key_pressed(KeyboardKey::KEY_KP_ENTER);
            let escape = win.rl.is_key_pressed(KeyboardKey::KEY_ESCAPE);

            let mut typed = String::new();
            while let Some(ch) = win.rl.get_char_pressed() {
                // Basic filtering: accept printable chars; keep newline out.
                if ch != '\n' && ch != '\r' {
                    typed.push(ch);
                }
            }

            let (rl, thread, sdf) = (&mut win.rl, &win.thread, &mut win.sdf);

            // Geometry & pointer coordinates stay LOGICAL. The shader alone
            // transforms gl_FragCoord physical bottom-left pixels to logical
            // top-left coordinates. Raylib's scissor mode performs its own DPI
            // transform: do NOT scale scissor rectangles again here.
            let frame = FrameMetrics::from_window(
                rl.get_screen_width(), rl.get_screen_height(),
                rl.get_render_width(), rl.get_render_height(),
            );
            let viewport = frame.logical;
            let valid_frame = frame.scale_x > 0.0 && frame.scale_y > 0.0;
            if valid_frame {
                sdf.shader.set_shader_value(
                    sdf.loc_framebuffer_scale, [frame.scale_x, frame.scale_y]);
                sdf.shader.set_shader_value(
                    sdf.loc_framebuffer_height, frame.render_height as f32);
            }
            // Construct all geometry before beginning the paint pass. Headless
            // tests exercise this exact measure/arrange model without Raylib.
            let planned_layout = layout::arrange(tree, layout::Bounds::new(
                0.0, 0.0, viewport.width, viewport.height,
            ));
            // Resolve a single topmost visible control before paint, using the
            // same arranged bounds and ancestor clips as native drawing.
            let click_cb = if clicked {
                match layout::hit_test(tree, &planned_layout, mouse.x, mouse.y) {
                    Some(layout::HitTarget::Button(id)) => {
                        win.focused_input = None;
                        Some(id)
                    }
                    Some(layout::HitTarget::TextInput { on_change, on_submit, value }) => {
                        let caret = value.chars().count();
                        win.focused_input = Some(FocusedTextInput {
                            on_change, on_submit, buffer: value, caret,
                        });
                        None
                    }
                    None => { win.focused_input = None; None }
                }
            } else { None };
            let mut d = rl.begin_drawing(thread);
            // Allow app-level theming via `App(bg: ...)`.
            let app_bg = parse_color(prop_string(tree, "bg").or_else(|| prop_string(tree, "background")));
            d.clear_background(app_bg);

            if valid_frame {
                render_node(
                    &mut d, tree, &planned_layout, now, sdf,
                    win.click_anim, &win.focused_input, &win.textures,
                );
            }
            fb.clicked_callback_id = click_cb;

            // Apply keyboard edits for the currently focused input and emit events.
            if let Some(fi) = &mut win.focused_input {
                let mut changed = false;

                if escape {
                    win.focused_input = None;
                } else {
                    if left {
                        fi.caret = fi.caret.saturating_sub(1);
                    }
                    if right {
                        fi.caret = (fi.caret + 1).min(fi.buffer.chars().count());
                    }

                    if backspace {
                        if fi.caret > 0 {
                            let mut chars: Vec<char> = fi.buffer.chars().collect();
                            let idx = fi.caret - 1;
                            if idx < chars.len() {
                                chars.remove(idx);
                                fi.buffer = chars.into_iter().collect();
                                fi.caret = fi.caret.saturating_sub(1);
                                changed = true;
                            }
                        }
                    }

                    if delete {
                        let mut chars: Vec<char> = fi.buffer.chars().collect();
                        if fi.caret < chars.len() {
                            chars.remove(fi.caret);
                            fi.buffer = chars.into_iter().collect();
                            changed = true;
                        }
                    }

                    if !typed.is_empty() {
                        let mut chars: Vec<char> = fi.buffer.chars().collect();
                        let insert: Vec<char> = typed.chars().collect();
                        let mut idx = fi.caret.min(chars.len());
                        for ch in insert {
                            chars.insert(idx, ch);
                            idx += 1;
                        }
                        fi.buffer = chars.into_iter().collect();
                        fi.caret = idx;
                        changed = true;
                    }

                    if changed {
                        fb.text_input_events.push(UiTextInputEvent {
                            callback_id: fi.on_change,
                            text: fi.buffer.clone(),
                            submitted: false,
                        });
                    }

                    if enter {
                        if let Some(cb) = fi.on_submit {
                            fb.text_input_events.push(UiTextInputEvent {
                                callback_id: cb,
                                text: fi.buffer.clone(),
                                submitted: true,
                            });
                        }
                    }
                }
            }

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
) {
    for (child, prepared) in node.children.iter().zip(&layout.children) {
        if let Some(prepared) = prepared {
            render_node(d, child, prepared, now, sdf, click_anim, focused_input, textures);
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
) {
    // Each node gets an independent scissor; it is dropped before children
    // draw. Avoid nested raylib scissors (EndScissorMode is not a push/pop).
    let Some((x, y, width, height)) = layout.clip.scissor() else { return };
    {
        let mut clipped = d.begin_scissor_mode(x, y, width, height);
        paint_node(&mut clipped, node, layout, now, sdf, click_anim, focused_input, textures);
    }
    render_children(d, node, layout, now, sdf, click_anim, focused_input, textures);
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

            // Background.
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

            // Display value (controlled input).
            let value = if is_focused {
                focused_input
                    .as_ref()
                    .map(|fi| fi.buffer.as_str())
                    .unwrap_or("")
            } else {
                prop_string(node, "value")
                    .or_else(|| prop_string(node, "text"))
                    .unwrap_or("")
            };

            let placeholder = prop_string(node, "placeholder").unwrap_or("");
            let display = if value.is_empty() { placeholder } else { value };
            let display_color = if value.is_empty() { placeholder_c } else { fg };

            let ts = prop_i32(node, "size").unwrap_or(18);
            let pad_x = 12.0_f32;
            let pad_y = (rect.height - ts as f32) / 2.0;
            d.draw_text(display, (rect.x + pad_x) as i32, (rect.y + pad_y) as i32, ts, display_color);

            // Caret (end-of-text only, MVP).
            if is_focused {
                let est_w = (value.chars().count() as f32) * (ts as f32 * 0.6);
                let cx = rect.x + pad_x + est_w + 1.0;
                let cy0 = rect.y + 10.0;
                let cy1 = rect.y + rect.height - 10.0;
                d.draw_line(cx as i32, cy0 as i32, cx as i32, cy1 as i32, Color::RAYWHITE);
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
