// Test-first contracts for the production native text view.
#[test]
fn text_caret_must_use_actual_raylib_font_metrics() {
    let lib=include_str!("../src/lib.rs");
    assert!(lib.contains("mod text_viewport;"),"RED: no measured text view");
    assert!(lib.contains(".measure_text("),"RED: same default-font DrawText metrics not measured");
    assert!(lib.contains("text_viewport::plan("),"RED: no pure clip/scroll planning");
}
#[test]
fn input_text_has_separate_scissors_from_frame_to_prevent_overdraw() {
    let lib=include_str!("../src/lib.rs");
    assert!(lib.contains("TextInput content clip") &&
            lib.contains("text_viewport::content_bounds("),
            "RED: input text can draw beyond its own frame bounds");
}
