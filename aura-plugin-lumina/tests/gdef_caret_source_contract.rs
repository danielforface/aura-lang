// LUMINA-GDEF-CARET-015 semantic RED before code implementation.
// OpenType ligature interior positions must come from parsed GDEF and must
// reach actual physical navigation, not estimated equal-width glyph splits.
const SHAPER:&str=include_str!("../src/text_shaping.rs");
const EDITOR:&str=include_str!("../src/bidi_editor.rs");
const PARSER:&str=include_str!("../src/gdef_caret.rs");
#[test]fn real_gdef_lig_caret_coordinates_instead_of_equal_spacing(){
 assert!(PARSER.contains("pub(crate) fn ligature_caret_coords("));
 assert!(PARSER.contains("fn coverage_index("));
 assert!(SHAPER.contains("gdef_caret::ligature_caret_coords("));
 assert!(SHAPER.contains("glyph.font_index"));
}
#[test]fn open_type_caret_positions_reach_native_bidi_navigation_without_fallback(){
 assert!(EDITOR.contains("pub(crate) fn navigate("));
 assert!(SHAPER.contains("caret_map_complete:complete"));
 assert!(PARSER.contains("UnsupportedCaretFormat"));
}
