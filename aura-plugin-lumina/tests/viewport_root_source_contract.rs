// Source-level RED guard: this is deliberately not an interactive UI test.
// Its purpose is to detect the existing fixed-root-rectangle contract before
// extracting a headless geometry helper and testing that helper separately.
#[test]
fn raylib_root_must_use_current_frame_dimensions() {
    let source = include_str!("../src/lib.rs");
    assert!(
        source.contains("rl.get_screen_width()") && source.contains("rl.get_screen_height()"),
        "Raylib root must sample the live window size on every UI frame"
    );
    assert!(
        !source.contains("Rectangle::new(0.0, 0.0, SCREEN_W as f32, SCREEN_H as f32)"),
        "Raylib root must not keep drawing into the initial fixed-size viewport"
    );
}
