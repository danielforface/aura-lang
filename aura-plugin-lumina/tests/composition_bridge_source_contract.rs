// Test-first semantic capability contract: prior editor accepts only
// committed Unicode characters, never transient OS-adapter preedit.
const INPUT:&str=include_str!("../src/text_input.rs");
const HOST:&str=include_str!("../src/lib.rs");
const VIEW:&str=include_str!("../src/text_viewport.rs");
#[test]fn composition_update_must_not_mutate_authoritative_editor(){
 assert!(INPUT.contains("pub(crate) fn start_composition("));
 assert!(INPUT.contains("pub(crate) fn update_composition("));
 assert!(INPUT.contains("pub(crate) fn commit_composition("));
 assert!(INPUT.contains("pub(crate) fn cancel_composition("));
 assert!(INPUT.contains("pub(crate) fn projected("));
}
#[test]fn native_adapter_bridge_must_be_real_and_preview_visible(){
 assert!(HOST.contains("pub fn submit_composition_event("));
 assert!(HOST.contains("pub enum CompositionEvent"));
 assert!(VIEW.contains("preedit_spans"));
 assert!(HOST.contains("preedit_screens("));
}
