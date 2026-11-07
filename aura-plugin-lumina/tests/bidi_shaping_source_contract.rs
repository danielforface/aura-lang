//! RED gate: these are implementation contracts, NOT a substitute for
//! semantic UAX#9 and OpenType shaping unit tests.


#[test]
fn bidi_must_have_real_uax9_visual_runs_not_string_reversal() {
    let engine=std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/text_shaping.rs")).unwrap_or_default();
    assert!(engine.contains("BidiInfo::new("),
        "a real unicode-bidi UAX#9 analyzer must exist");
    assert!(engine.contains("visual_runs("),
        "resolved visual runs must be extracted, never reverse an RTL string");
    assert!(engine.contains("grapheme_indices(true)"),
        "must map UTF-8 grapheme boundaries separately from visual runs");
}
#[test]
fn shaping_must_preserve_glyph_ids_clusters_and_position() {
    let engine=std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/text_shaping.rs")).unwrap_or_default();
    let renderer=std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")).unwrap_or_default();
    assert!(engine.contains("rustybuzz::shape("),
        "must use OpenType shaping not Unicode scalar rasterization");
    assert!(engine.contains("glyph_infos()") && engine.contains("glyph_positions()"),
        "glyph-ID + cluster + advance/offset outputs are mandatory");
    assert!(renderer.contains("mod text_shaping;"),
        "new engine must compile as part of Lumina");
}
