// Phase 22 intentionally RED source contract prior to native engine implementation.
const NATIVE:&str=include_str!("../native/ibus_input_helper.c");
const CLIENT:&str=include_str!("../src/ibus_native.rs");
const HOST:&str=include_str!("../src/lib.rs");
const LIVE:&str=include_str!("ibus_candidate_e2e.rs");
#[test]fn genuine_libibus_surrounding_text_uses_typed_scalar_cursor_and_anchor(){
 assert!(NATIVE.contains("ibus_input_context_set_surrounding_text"));
 assert!(NATIVE.contains("IBUS_CAP_SURROUNDING_TEXT"));
 assert!(NATIVE.contains("delete-surrounding-text"));
 assert!(NATIVE.contains("require-surrounding-text"));
 assert!(CLIENT.contains("fn sync_surrounding_text"));
}
#[test]fn focus_and_authority_guard_surrounding_text_and_reject_unqualified_delete(){
 assert!(CLIENT.contains("surrounding_text_acks"));
 assert!(CLIENT.contains("surrounding_delete_requests"));
 assert!(HOST.contains("diagnostic_native_ibus_surrounding"));
 assert!(LIVE.contains("real_pinyin_surrounding_text_selection_rebase_and_bounds"));
}
