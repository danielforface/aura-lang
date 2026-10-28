// Missing real capability: a secondary font must get its OWN glyph ID
// namespace. A glyph ID from font1 rendered against font0 is corruption.
const SHAPE:&str=include_str!("../src/text_shaping.rs");
const GPU:&str=include_str!("../src/shaped_gpu.rs");
const LIB:&str=include_str!("../src/lib.rs");
#[test] fn cluster_fallback_must_resolve_font_per_complete_grapheme(){
    assert!(SHAPE.contains("pub(crate) fn shape_text_with_fonts("));
    assert!(SHAPE.contains("font_index: usize"),"glyphs require their origin font");
    assert!(SHAPE.contains("glyph_index(ch)"),"must check real cmap");
    assert!(SHAPE.contains("graphemes"),"no Unicode scalar split");
}
#[test] fn indexed_gpu_must_use_the_font_selected_by_shaper(){
    assert!(GPU.contains("fallback_font_path"),"explicit fallback API");
    assert!(GPU.contains("glyph.font_index"),"never draw fallback GID from primary atlas");
    assert!(GPU.contains("MAX_FONT_CHAIN"),"no unbounded font loading");
    assert!(LIB.contains("fallback_font_path"),"widget must pass fallback to native load");
}
