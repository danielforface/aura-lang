// Real behavior also runs in Linux Xvfb; these contracts are test-first RED.
#[test]
fn actual_external_font_must_be_loaded_and_retained_for_native_text() {
 let lib=include_str!("../src/lib.rs");
 assert!(lib.contains("mod font_atlas;"),"RED: no font atlas planning");
 assert!(lib.contains("load_font_ex("),"RED: font_path never loads a Unicode atlas");
 assert!(lib.contains("fonts: HashMap"),"RED: no GPU font lifecycle cache");
 assert!(lib.contains("font_path"),"RED: no opt-in font_path");
}
#[test]
fn same_glyph_font_must_measure_caret_and_draw_text() {
 let lib=include_str!("../src/lib.rs");
 assert!(lib.contains("font.measure_text("),"RED: caret metrics still always default font");
 assert!(lib.contains("d.draw_text_ex("),"RED: no custom UTF-8 glyph drawing");
}
