// Test-only contract; no GUI claim. A native Xvfb test must exercise this
// through a separately opt-in feature, not a production public API.
#[test]
fn sampled_frame_uses_logical_and_render_dimensions_without_fixed_assumptions() {
    let source = include_str!("../src/lib.rs");
    assert!(source.contains("FrameMetrics::from_window"),
        "RED: actual frame does not sample logical and framebuffer pixel sizes");
    assert!(source.contains("get_render_width()") && source.contains("get_render_height()"),
        "RED: screen-only dimensions cannot establish framebuffer/DPI truth");
}
#[test]
fn native_e2e_has_explicit_window_resize_and_pixel_capture_hooks() {
    let source = include_str!("../src/lib.rs");
    assert!(source.contains("pub fn diagnostic_resize_window("),
        "RED: real window resize is untested in current native flow");
    assert!(source.contains("pub fn diagnostic_capture_pixel("),
        "RED: no actual renderer framebuffer pixel readback test seam");
}
