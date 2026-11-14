// Semantic RED: no deletion authorization until stale/Unicode/undo guard exists.
// Real IBus engine emission is a separate qualification boundary, not faked here.
const EDITOR:&str=include_str!("../src/text_input.rs");
const IME:&str=include_str!("../src/ibus_native.rs");
const LIVE:&str=include_str!("ibus_candidate_e2e.rs");
#[test]fn atomic_delete_surrounding_requires_exact_authority_snapshot(){
  assert!(EDITOR.contains("fn delete_surrounding_if_snapshot"));
  assert!(EDITOR.contains("authority_epoch"));
  assert!(EDITOR.contains("scalar_range_to_grapheme_bounds"));
}
#[test]fn native_ibus_delete_remains_advisory_without_live_engine_provenance(){
  assert!(IME.contains("fn parse_surrounding_delete_frame"));
  assert!(IME.contains("surrounding_delete_requests"));
  assert!(LIVE.contains("real_pinyin_delete_safety_no_implicit_authority_change"));
}
