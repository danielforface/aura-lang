//! RED gate: these are implementation contracts, NOT a substitute for
//! semantic UAX#9 and OpenType shaping unit tests.
const ENGINE:&str=include_str!("../src/text_shaping.rs");
const RENDERER:&str=include_str!("../src/lib.rs");
#[test]
fn bidi_must_have_real_uax9_visual_runs_not_string_reversal() {
    assert!(ENGINE.contains("BidiInfo::new("),
        "a real unicode-bidi UAX#9 analyzer must exist");
    assert!(ENGINE.contains("visual_runs("),
        "resolved visual runs must be extracted, never reverse an RTL string");
    assert!(ENGINE.contains("grapheme_indices(true)"),
        "must map UTF-8 grapheme boundaries separately from visual runs");
}
#[test]
fn shaping_must_preserve_glyph_ids_clusters_and_position() {
    assert!(ENGINE.contains("rustybuzz::shape("),
        "must use OpenType shaping not Unicode scalar rasterization");
    assert!(ENGINE.contains("glyph_infos()") && ENGINE.contains("glyph_positions()"),
        "glyph-ID + cluster + advance/offset outputs are mandatory");
    assert!(RENDERER.contains("mod text_shaping;"),
        "new engine must compile as part of Lumina");
}
