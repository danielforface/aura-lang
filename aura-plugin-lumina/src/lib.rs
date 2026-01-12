#![forbid(unsafe_code)]

use aura_nexus::{AuraPlugin, NexusContext, NexusDiagnostic, PluginCapability, UiNode, UiRuntimeFeedback};

#[cfg(not(feature = "raylib"))]
use aura_nexus::format_ui_tree;

#[cfg(feature = "raylib")]
use std::cell::RefCell;

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

float sdRoundRect(vec2 p, vec2 b, float r) {
    // p is centered coords; b is half-size.
    vec2 q = abs(p) - (b - vec2(r));
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

void main() {
    vec2 p = gl_FragCoord.xy;
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
const SCREEN_W: i32 = 1920;

#[cfg(feature = "raylib")]
const SCREEN_H: i32 = 1080;

#[cfg(feature = "raylib")]
pub struct AuraLuminaPlugin {
    window: RefCell<Option<LuminaWindow>>,
}

#[cfg(feature = "raylib")]
struct LuminaWindow {
    rl: RaylibHandle,
    thread: RaylibThread,
    just_opened: bool,
    open_frames: u8,

    sdf: RoundedRectShader,

    // Minimal tween state: animate the last-clicked callback for a short duration.
    click_anim: Option<(u64, f64)>,
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
                    .size(SCREEN_W, SCREEN_H)
                    .title("Aura Lumina Sentinel")
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
                    shader,
                };
                *win_ref = Some(LuminaWindow {
                    rl,
                    thread,
                    just_opened: true,
                    open_frames: 0,
                    sdf,
                    click_anim: None,
                });
            }

            let win = win_ref.as_mut().expect("window initialized");

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

            let (rl, thread, sdf) = (&mut win.rl, &win.thread, &mut win.sdf);

            let mut d = rl.begin_drawing(thread);
            // Allow app-level theming via `App(bg: ...)`.
            let app_bg = parse_color(prop_string(tree, "bg").or_else(|| prop_string(tree, "background")));
            d.clear_background(app_bg);

            let click_cb = render_node(
                &mut d,
                tree,
                Rectangle::new(0.0, 0.0, SCREEN_W as f32, SCREEN_H as f32),
                clicked,
                mouse,
                now,
                sdf,
                win.click_anim,
            );

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
fn point_in_rect(p: Vector2, r: Rectangle) -> bool {
    p.x >= r.x && p.x <= r.x + r.width && p.y >= r.y && p.y <= r.y + r.height
}

#[cfg(feature = "raylib")]
fn measure_node(node: &UiNode) -> (f32, f32) {
    match node.kind.as_str() {
        "Button" => {
            let w = prop_i32(node, "width").unwrap_or(200) as f32;
            let h = prop_i32(node, "height").unwrap_or(50) as f32;
            (w, h)
        }
        "Rect" => {
            let w = prop_i32(node, "width").unwrap_or(100) as f32;
            let h = prop_i32(node, "height").unwrap_or(100) as f32;
            (w, h)
        }
        "Text" => {
            let size = prop_i32(node, "size").unwrap_or(20) as f32;
            let text = prop_string(node, "text")
                .or_else(|| prop_string(node, "content"))
                .unwrap_or("");
            // Best-effort estimate (avoids font API differences across raylib-rs versions).
            let w = (text.chars().count() as f32) * (size * 0.6);
            let h = size;
            (w, h)
        }
        _ => {
            // Containers default to available space.
            (0.0, 0.0)
        }
    }
}

#[cfg(feature = "raylib")]
fn render_node(
    d: &mut RaylibDrawHandle,
    node: &UiNode,
    bounds: Rectangle,
    mouse_clicked: bool,
    mouse: Vector2,
    now: f64,
    sdf: &mut RoundedRectShader,
    click_anim: Option<(u64, f64)>,
) -> Option<u64> {
    // Optional absolute positioning: if a node provides `x`/`y` props, render it at that position.
    // This enables simple "game-ish" demos (moving objects) without adding a full canvas API yet.
    let mut bounds = bounds;
    if let Some(x) = prop_i32(node, "x") {
        bounds.x = x as f32;
    }
    if let Some(y) = prop_i32(node, "y") {
        bounds.y = y as f32;
    }

    match node.kind.as_str() {
        "App" => {
            // App is just a root container.
            let mut clicked: Option<u64> = None;
            for child in &node.children {
                clicked = clicked.or(render_node(
                    d,
                    child,
                    bounds,
                    mouse_clicked,
                    mouse,
                    now,
                    sdf,
                    click_anim,
                ));
            }
            clicked
        }
        "VStack" => {
            let spacing = prop_i32(node, "spacing").unwrap_or(0) as f32;
            let padding = prop_i32(node, "padding").unwrap_or(0) as f32;
            let alignment = prop_string(node, "alignment").unwrap_or("start");

            let mut y = bounds.y + padding;
            let mut clicked: Option<u64> = None;

            for child in &node.children {
                let (cw, ch) = measure_node(child);
                let x = if alignment == "center" && cw > 0.0 {
                    bounds.x + (bounds.width - cw) / 2.0
                } else {
                    bounds.x + padding
                };

                let child_bounds = Rectangle::new(x, y, if cw > 0.0 { cw } else { bounds.width }, ch);
                clicked = clicked.or(render_node(
                    d,
                    child,
                    child_bounds,
                    mouse_clicked,
                    mouse,
                    now,
                    sdf,
                    click_anim,
                ));
                y += ch + spacing;
            }
            clicked
        }
        "HStack" => {
            let spacing = prop_i32(node, "spacing").unwrap_or(0) as f32;
            let padding = prop_i32(node, "padding").unwrap_or(0) as f32;

            let mut x = bounds.x + padding;
            let mut clicked: Option<u64> = None;

            for child in &node.children {
                let (cw, ch) = measure_node(child);
                let child_bounds = Rectangle::new(x, bounds.y + padding, cw, ch);
                clicked = clicked.or(render_node(
                    d,
                    child,
                    child_bounds,
                    mouse_clicked,
                    mouse,
                    now,
                    sdf,
                    click_anim,
                ));
                x += cw + spacing;
            }
            clicked
        }
        "Text" => {
            let size = prop_i32(node, "size").unwrap_or(20);
            let color = parse_color(prop_string(node, "color").or_else(|| prop_string(node, "fg")));
            let text = prop_string(node, "text")
                .or_else(|| prop_string(node, "content"))
                .unwrap_or("");
            d.draw_text(text, bounds.x as i32, bounds.y as i32, size, color);
            None
        }
        "Rect" => {
            let w = prop_i32(node, "width").unwrap_or(bounds.width as i32).max(1) as f32;
            let h = prop_i32(node, "height").unwrap_or(bounds.height as i32).max(1) as f32;
            let rect = Rectangle::new(bounds.x, bounds.y, w, h);

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

            None
        }
        "Button" => {
            let w = prop_i32(node, "width").unwrap_or(200) as f32;
            let h = prop_i32(node, "height").unwrap_or(50) as f32;
            let rect = Rectangle::new(bounds.x, bounds.y, w, h);

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

            if mouse_clicked && point_in_rect(mouse, rect) {
                return parse_callback_id(prop_string(node, "on_click"));
            }

            None
        }
        _ => {
            // Unknown nodes: traverse children.
            let mut clicked: Option<u64> = None;
            for child in &node.children {
                clicked = clicked.or(render_node(
                    d,
                    child,
                    bounds,
                    mouse_clicked,
                    mouse,
                    now,
                    sdf,
                    click_anim,
                ));
            }
            clicked
        }
    }
}
