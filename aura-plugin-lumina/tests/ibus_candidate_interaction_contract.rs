// Phase 20 source contracts are deliberately RED before implementation.
// Actual XTest/libpinyin engine and GPU pixel tests are separate requirements.
const HOST:&str=include_str!("../src/lib.rs");
const ENGINE:&str=include_str!("../src/ibus_native.rs");
const LIVE:&str=include_str!("ibus_candidate_e2e.rs");
#[test]fn native_candidate_mouse_hit_is_consumed_before_general_blur(){
    assert!(HOST.contains("fn candidate_row_at"));
    assert!(HOST.contains("candidate_click_consumed"));
    assert!(HOST.contains("select_candidate_via_engine"));
}
#[test]fn candidate_click_is_native_engine_owned_and_live_x11_tested(){
    assert!(ENGINE.contains("fn select_candidate_via_engine"));
    assert!(ENGINE.contains("candidate_click_acks"));
    assert!(LIVE.contains("real_pinyin_candidate_mouse_click_commits_selected_engine_text"));
}
#[test]fn native_candidate_paging_is_tested_against_live_engine(){
    assert!(LIVE.contains("real_pinyin_page_navigation"));
    assert!(HOST.contains("diagnostic_native_ibus_candidate_clicks"));
}
