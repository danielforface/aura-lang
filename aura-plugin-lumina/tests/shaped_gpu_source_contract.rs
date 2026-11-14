// Contract RED: real native glyph-index texture upload, not Unicode scalar DrawTextEx.
const RENDER:&str=include_str!("../src/lib.rs");
#[test]fn experimental_glyph_index_renderer_must_be_real_and_bounded(){
    let source=std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"),"/src/shaped_gpu.rs")).unwrap_or_default();
    assert!(source.contains("rasterize_indexed("), "OpenType glyph IDs must be rasterized directly");
    assert!(source.contains("load_texture_from_image("), "must upload actual glyph bitmap to GPU");
    assert!(source.contains("draw_texture_pro("), "must draw positioned OpenType glyphs, not codepoints");
    assert!(source.contains("MAX_GLYPHS") && source.contains("MAX_ATLAS_SIDE"), "bound GPU memory usage");
}
#[test]fn both_widgets_must_have_opt_in_shaping_paint_path(){
    assert!(RENDER.contains("shaping") && RENDER.contains("opentype"), "explicit shaping opt-in required");
    assert!(RENDER.contains("shaped_gpu::"), "Text and TextInput must use glyph-ID paint");
}
