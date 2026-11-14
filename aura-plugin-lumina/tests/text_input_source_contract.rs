// Source contract only. Native XTest must validate real event flow separately.
#[test]
fn text_input_key_operations_use_tested_pure_edit_engine() {
 let lib=include_str!("../src/lib.rs");
 assert!(lib.contains("mod text_input;"),"RED: production has no isolated editor state machine");
 assert!(lib.contains("use text_input::{EditCommand,SessionAction}"),"RED: native keyboard path bypasses edit engine");
 assert!(lib.contains("fi.session.apply(SessionAction::"),"RED: inline ad-hoc keyboard mutations remain");
 assert!(lib.contains("KeyboardKey::KEY_HOME") && lib.contains("KeyboardKey::KEY_END"),
    "RED: missing Home/End cursor navigation");
 let engine=include_str!("../src/text_input.rs");
 assert!(engine.contains("pub(crate) fn apply_edit("),"RED: no pure editor state transition");
}
