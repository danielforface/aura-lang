// Test-first source boundary for the renderer/geometry split.
// This does not launch a native window or prove pixel-level rendering.
#[test]
fn render_frame_must_prepare_layout_before_begin_drawing() {
    let src = include_str!("../src/lib.rs");
    let call = src.find("layout::arrange(tree,").expect(
        "RED: renderer does not yet build a precomputed layout tree for each frame"
    );
    let begin = src.find("let mut d = rl.begin_drawing(thread);").expect("draw call");
    assert!(call < begin, "layout must be prepared before drawing begins");
    let renderer = src.split("fn render_node(").nth(1).expect("renderer exists");
    assert!(!renderer.contains("measure_node("),
        "paint path must not repeatedly measure children");
}
