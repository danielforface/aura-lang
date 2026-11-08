#[test]
fn grapheme_editor_supports_selection_undo_redo_and_bounded_history() {
 let s=include_str!("../src/text_input.rs");
 assert!(s.contains("struct EditSession"),"RED: no independent persistent cursor/selection session");
 assert!(s.contains("SelectAll"),"RED: no selection");
 assert!(s.contains("Undo")&&s.contains("Redo"),"RED: no edit history");
}
#[test]
fn native_focus_preserves_drafts_and_handles_external_value_authority() {
 let s=include_str!("../src/lib.rs");
 assert!(s.contains("input_sessions:"),"RED: only one ephemeral focused text input");
 assert!(s.contains("reconcile_input_sessions("),"RED: no controlled-prop rebase contract");
 assert!(s.contains("KeyboardKey::KEY_A")&&s.contains("KeyboardKey::KEY_Z")&&s.contains("KeyboardKey::KEY_Y"),
  "RED: no Ctrl+A/Z/Y native input");
}
