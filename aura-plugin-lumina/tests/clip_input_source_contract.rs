// Regression guard for the new geometry-only input and clipping boundary.
// This test does NOT assert that a physical window, GPU scissor or DPI was exercised.
#[test]
fn event_dispatch_must_use_prepared_hit_test_before_paint() {
    let source = include_str!("../src/lib.rs");
    assert!(source.contains("layout::hit_test("),
        "RED: input callback selection is still performed while drawing widgets");
    assert!(!source.contains("point_in_rect(mouse, rect)"),
        "per-widget paint must not dispatch input behind clipped or covered widgets");
    let layout = include_str!("../src/layout.rs");
    assert!(layout.contains("pub(crate) fn hit_test("),
        "hit testing must be renderer-independent and unit-testable");
}

#[test]
fn paint_must_apply_precomputed_clip_to_real_render_path() {
    let source = include_str!("../src/lib.rs");
    assert!(source.contains("begin_scissor_mode("),
        "RED: planned clipping has no real native drawing restriction");
    assert!(source.contains("layout.clip"),
        "rendering must consume exactly the clipping bounds used by hit testing");
}
