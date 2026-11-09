// Phase 21 semantic RED: native IBus cursor/IME screen geometry is missing.
// Real libibus Xvfb and GPU tests must back source contracts in a separate suite.
const NATIVE:&str=include_str!("../native/ibus_input_helper.c");
const ADAPTER:&str=include_str!("../src/ibus_native.rs");
const HOST:&str=include_str!("../src/lib.rs");
const LIVE:&str=include_str!("ibus_candidate_e2e.rs");
#[test]fn libibus_receives_a_bounded_real_screen_space_caret_rectangle(){
    assert!(NATIVE.contains("ibus_input_context_set_cursor_location"));
    assert!(NATIVE.contains("LUMINA_IBUS_CURSOR_LOCATION"));
    assert!(ADAPTER.contains("fn set_cursor_location"));
    assert!(ADAPTER.contains("cursor_location_acks"));
}
#[test]fn popup_tracks_actual_measured_caret_and_scrolled_editor(){
    assert!(HOST.contains("fn native_ibus_caret_rect"));
    assert!(HOST.contains("diagnostic_native_ibus_cursor_location"));
    assert!(HOST.contains("candidate_caret_anchor"));
    assert!(LIVE.contains("real_pinyin_caret_location_scroll_and_popup_anchor"));
}
