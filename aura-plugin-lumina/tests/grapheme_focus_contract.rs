// Test-first contract: production MUST use UAX #29 extended graphemes,
// and real native UI input MUST traverse the layout-visible focus list.
#[test]
fn editor_uses_actual_unicode_grapheme_boundaries_not_scalar_deletion() {
 let src=include_str!("../src/text_input.rs");
 assert!(src.contains("UnicodeSegmentation"),"RED: editor has no UAX #29 implementation");
 assert!(src.contains(".graphemes(true)")||src.contains(".grapheme_indices(true)"),
   "RED: TextInput is still scalar-based");
}
#[test]
fn native_tab_navigation_is_driven_by_measured_visible_focus_order() {
 let src=include_str!("../src/lib.rs");
 let lay=include_str!("../src/layout.rs");
 assert!(src.contains("KeyboardKey::KEY_TAB"),"RED: no Tab key input");
 assert!(src.contains("layout::focus_order("),"RED: no precomputed live focus plan");
 assert!(lay.contains("pub(crate) fn focus_order("),"RED: layout has no deterministic visible focus ordering");
}
