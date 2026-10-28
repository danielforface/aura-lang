//! Experimental *pure* UAX #9 + OpenType shaping stage. Glyph IDs are NOT
//! Unicode scalar values; Raylib DrawTextEx cannot paint this output directly.
//! Invariant: source stays logical UTF-8, every run is a source byte range,
//! while shaped glyphs are in visual order with advances in native font units.
//! No rendering, visual cursor navigation or IME qualification is implied.
use std::ops::Range;
use unicode_bidi::{BidiInfo, Level};
use unicode_segmentation::UnicodeSegmentation;

pub(crate) const MAX_SHAPE_TEXT_BYTES: usize = 64 * 1024;
pub(crate) const MAX_SHAPE_FONT_BYTES: usize = 16 * 1024 * 1024;
const MAX_SHAPE_GLYPHS: usize = 16 * 1024;
const MAX_VISUAL_RUNS: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParagraphDirection { Auto, Ltr, Rtl }

impl ParagraphDirection {
    pub(crate) fn parse(value: Option<&str>) -> Option<Self> {
        match value.unwrap_or("auto") {
            "auto" => Some(Self::Auto),
            "ltr" => Some(Self::Ltr),
            "rtl" => Some(Self::Rtl),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VisualRun {
    pub(crate) source_bytes: Range<usize>,
    pub(crate) rtl: bool,
    /// First and last grapheme indices in LOGICAL storage, not visual order.
    pub(crate) graphemes: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BidiLayout {
    pub(crate) paragraph_rtl: bool,
    pub(crate) grapheme_boundaries: Vec<usize>,
    /// Runs are ordered LEFT-TO-RIGHT on screen by Unicode UAX #9.
    pub(crate) visual_runs: Vec<VisualRun>,
}

/// Single-line only. Input is never rewritten in order to analyze direction.
/// Restrict allocations and fail closed for multiline and UAX9 explicit
/// directional controls (spoofable hidden overrides are not an app API).
pub(crate) fn analyze(text: &str, direction: ParagraphDirection) -> Option<BidiLayout> {
    if text.len() > MAX_SHAPE_TEXT_BYTES ||
        text.chars().any(|c| c == '\n' || c == '\r' ||
            matches!(c, '\u{202A}'..='\u{202E}' |
                '\u{2066}'..='\u{2069}' | '\u{061C}' | '\u{200E}' | '\u{200F}')) {
        return None;
    }
    let mut boundaries: Vec<usize> = text.grapheme_indices(true).map(|(b, _)| b).collect();
    boundaries.push(text.len());
    if boundaries.len() > MAX_SHAPE_GLYPHS + 1 { return None; }
    let explicit = match direction {
        ParagraphDirection::Auto => None,
        ParagraphDirection::Ltr => Some(Level::ltr()),
        ParagraphDirection::Rtl => Some(Level::rtl()),
    };
    let bidi = BidiInfo::new(text, explicit);
    let para = bidi.paragraphs.first()?;
    if bidi.paragraphs.len() != 1 { return None; }
    let (_, visual) = bidi.visual_runs(para, para.range.clone());
    if visual.len() > MAX_VISUAL_RUNS { return None; }
    let mut runs = Vec::with_capacity(visual.len());
    for bytes in visual {
        if !text.is_char_boundary(bytes.start) || !text.is_char_boundary(bytes.end) {
            return None;
        }
        // Do not treat a shaping boundary in the middle of a grapheme as valid.
        let a = boundaries.binary_search(&bytes.start).ok()?;
        let b = boundaries.binary_search(&bytes.end).ok()?;
        let rtl = bidi.levels.get(bytes.start).is_some_and(Level::is_rtl);
        runs.push(VisualRun {
            source_bytes: bytes,
            rtl,
            graphemes: a..b,
        });
    }
    Some(BidiLayout {
        paragraph_rtl: para.level.is_rtl(),
        grapheme_boundaries: boundaries,
        visual_runs: runs,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShapedGlyph {
    pub(crate) glyph_id: u16,
    /// UTF-8 byte offset in full logical source (not a character index).
    pub(crate) cluster_byte: usize,
    pub(crate) advance_x: f32,
    pub(crate) advance_y: f32,
    pub(crate) offset_x: f32,
    pub(crate) offset_y: f32,
    /// x coordinate within the full visual line, not the logical UTF-8 order.
    pub(crate) pen_x: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShapedRun {
    pub(crate) source_bytes: Range<usize>,
    pub(crate) rtl: bool,
    pub(crate) visual_x: f32,
    pub(crate) width: f32,
    pub(crate) glyphs: Vec<ShapedGlyph>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CaretCandidate {
    /// Logical UTF-8 grapheme boundary. A BiDi junction can have TWO visual
    /// positions; never collapse them without explicit affinity.
    pub(crate) byte: usize,
    pub(crate) visual_x: f32,
    pub(crate) visual_run: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShapedText {
    pub(crate) bidi: BidiLayout,
    pub(crate) visual_runs: Vec<ShapedRun>,
    pub(crate) width: f32,
    /// Only guaranteed at 1:1 grapheme-to-glyph-cluster boundaries.
    /// Ligature interiors require font caret-position APIs, not guessing.
    pub(crate) caret_candidates: Vec<CaretCandidate>,
    pub(crate) caret_map_complete: bool,
    /// False: there is currently no glyph-ID texture upload in Raylib.
    pub(crate) native_paintable: bool,
}

/// Shaping output is measurement/input to a future glyph-ID GPU atlas, never
/// a license to reinterpret glyph_id as a Unicode codepoint. Font units
/// converted to logical pixels, including GPOS/kerning and mark offsets.
pub(crate) fn shape_text(
    source: &str, font_data: &[u8], px: f32, direction: ParagraphDirection,
) -> Option<ShapedText> {
    if font_data.is_empty() || font_data.len() > MAX_SHAPE_FONT_BYTES ||
        !px.is_finite() || !(10.0..=96.0).contains(&px) { return None; }
    let bidi = analyze(source, direction)?;
    let face = rustybuzz::Face::from_slice(font_data, 0)?;
    let units = face.units_per_em();
    if units <= 0 { return None; }
    let scale = px / units as f32;
    let mut visual_runs = Vec::with_capacity(bidi.visual_runs.len());
    let mut pen_x = 0.0_f32;
    let mut count = 0_usize;
    for run in &bidi.visual_runs {
        let slice = &source[run.source_bytes.clone()];
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(slice);
        buffer.guess_segment_properties();
        buffer.set_direction(if run.rtl {
            rustybuzz::Direction::RightToLeft
        } else { rustybuzz::Direction::LeftToRight });
        let output = rustybuzz::shape(&face, &[], buffer);
        if count.saturating_add(output.len()) > MAX_SHAPE_GLYPHS { return None; }
        count += output.len();
        let mut glyphs = Vec::with_capacity(output.len());
        let origin = pen_x;
        for (info, position) in output.glyph_infos().iter().zip(output.glyph_positions()) {
            let cluster_byte = run.source_bytes.start.checked_add(info.cluster as usize)?;
            if cluster_byte >= run.source_bytes.end ||
                !source.is_char_boundary(cluster_byte) || info.glyph_id == 0 {
                // Missing glyph or non-source-aligned cluster: no false paint proof.
                return None;
            }
            let advance_x = position.x_advance as f32 * scale;
            let advance_y = position.y_advance as f32 * scale;
            let offset_x = position.x_offset as f32 * scale;
            let offset_y = position.y_offset as f32 * scale;
            if ![advance_x, advance_y, offset_x, offset_y].iter().all(|n|n.is_finite()) {
                return None;
            }
            glyphs.push(ShapedGlyph {
                glyph_id: info.glyph_id as u16,
                cluster_byte,
                advance_x, advance_y, offset_x, offset_y, pen_x,
            });
            pen_x += advance_x;
        }
        if !pen_x.is_finite() || pen_x < origin { return None; }
        visual_runs.push(ShapedRun {
            source_bytes: run.source_bytes.clone(),
            rtl: run.rtl,
            visual_x: origin,
            width: pen_x - origin,
            glyphs,
        });
    }
    // Construct only unambiguous cluster-aligned stops. A ligature may
    // represent several graphemes with one cluster; mark the map INCOMPLETE,
    // rather than inventing equal-spaced cursor positions inside it.
    let mut caret_candidates = Vec::new();
    let mut complete = true;
    for (run_index, run) in visual_runs.iter().enumerate() {
        let corresponding = &bidi.visual_runs[run_index];
        for g in corresponding.graphemes.clone() {
            let start = bidi.grapheme_boundaries[g];
            let end = bidi.grapheme_boundaries[g + 1];
            let mut left = f32::INFINITY;
            let mut right = f32::NEG_INFINITY;
            for glyph in &run.glyphs {
                if glyph.cluster_byte == start {
                    left = left.min(glyph.pen_x);
                    right = right.max(glyph.pen_x + glyph.advance_x);
                }
            }
            if !left.is_finite() || !right.is_finite() {
                complete = false; // GSUB ligature; cannot deduce interior.
                continue;
            }
            let (a,b) = if run.rtl { (right,left) } else { (left,right) };
            caret_candidates.push(CaretCandidate { byte:start, visual_x:a, visual_run:run_index });
            caret_candidates.push(CaretCandidate { byte:end, visual_x:b, visual_run:run_index });
        }
    }
    caret_candidates.sort_by(|a,b|a.byte.cmp(&b.byte)
        .then(a.visual_run.cmp(&b.visual_run)));
    Some(ShapedText {
        bidi, visual_runs, width: pen_x, caret_candidates,
        caret_map_complete: complete, native_paintable: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn invalid_direction_is_not_silently_downgraded(){
        assert_eq!(ParagraphDirection::parse(None),Some(ParagraphDirection::Auto));
        assert_eq!(ParagraphDirection::parse(Some("rtl")),Some(ParagraphDirection::Rtl));
        assert_eq!(ParagraphDirection::parse(Some("RTL")),None);
        assert_eq!(ParagraphDirection::parse(Some("sideways")),None);
    }
    #[test] fn hebrew_and_english_change_base_direction_without_changing_source(){
        let s="שלום Hello 123 עולם";
        let auto=analyze(s,ParagraphDirection::Auto).unwrap();
        assert!(auto.paragraph_rtl);
        assert!(auto.visual_runs.iter().any(|r|r.rtl));
        assert!(auto.visual_runs.iter().any(|r|!r.rtl));
        assert!(auto.visual_runs.iter().all(|r|s.get(r.source_bytes.clone()).is_some()));
        assert_eq!(auto.grapheme_boundaries.last(),Some(&s.len()));
        let forced=analyze(s,ParagraphDirection::Ltr).unwrap();
        assert!(!forced.paragraph_rtl);
        assert_ne!(auto.visual_runs,forced.visual_runs);
    }
    #[test] fn arabic_and_hebrew_combining_marks_preserve_grapheme_boundary(){
        for s in ["שָׁלוֹם", "السَّلَامُ", "e\u{301} שלום", "👩‍👩‍👧‍👦 אב"] {
            let a=analyze(s,ParagraphDirection::Auto).unwrap();
            assert!(a.visual_runs.iter().all(|r|
                a.grapheme_boundaries.contains(&r.source_bytes.start) &&
                a.grapheme_boundaries.contains(&r.source_bytes.end)));
        }
    }
    #[test] fn spoofing_controls_unbounded_input_and_multiline_fail_closed(){
        assert!(analyze("ab\u{202E}cd",ParagraphDirection::Auto).is_none());
        assert!(analyze("x\nY",ParagraphDirection::Auto).is_none());
        assert!(analyze(&"a".repeat(MAX_SHAPE_TEXT_BYTES+1),ParagraphDirection::Auto).is_none());
    }
    #[test] fn empty_text_and_ascii_have_no_spurious_visual_reversal(){
        assert_eq!(analyze("ABC 123",ParagraphDirection::Auto).unwrap().visual_runs.len(),1);
        let a=analyze("אבג ABC",ParagraphDirection::Auto).unwrap();
        assert!(a.visual_runs.len()>1);
        assert!(a.visual_runs[0].source_bytes.start>0,
            "visual sequence for an RTL paragraph begins with logical trailing LTR run");
    }
    #[test] fn glyph_shaping_with_real_ttf_preserves_visual_runs_and_cluster_offsets(){
        let path="/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
        if !std::path::Path::new(path).is_file(){return;}
        let bytes=std::fs::read(path).unwrap();
        for s in ["שלום עולם", "Hello World", "שלום Hello 123 עולם", "שָׁלוֹם", "السَّلَامُ"] {
            let shaped=shape_text(s,&bytes,24.0,ParagraphDirection::Auto).unwrap();
            assert!(shaped.width>0.0 && !shaped.native_paintable);
            assert!(!shaped.visual_runs.is_empty());
            assert!(shaped.visual_runs.iter().flat_map(|r|&r.glyphs)
                .all(|g|g.glyph_id!=0 && s.is_char_boundary(g.cluster_byte)));
            assert!(shaped.visual_runs.iter().any(|r|!r.glyphs.is_empty()));
        }
    }
    #[test] fn shaped_caret_boundaries_retain_bidi_affinity_without_guessing_ligature_interior(){
        let path="/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
        if !std::path::Path::new(path).is_file(){return;}
        let bytes=std::fs::read(path).unwrap();
        for (text,rtl) in [("Hello",false),("שלום",true)] {
            let shaped=shape_text(text,&bytes,24.0,ParagraphDirection::Auto).unwrap();
            assert!(shaped.caret_map_complete,
                "one glyph cluster per grapheme expected for fixture: {text}");
            let boundaries=&shaped.bidi.grapheme_boundaries;
            let stops:Vec<f32>=boundaries.iter().map(|byte|
                shaped.caret_candidates.iter().find(|s|s.byte==*byte).unwrap().visual_x
            ).collect();
            assert_eq!(stops.len(),text.graphemes(true).count()+1);
            assert!(stops.windows(2).all(|w| if rtl {w[0]>w[1]} else {w[0]<w[1]}),
                "physical caret direction should reflect real bidi levels");
        }
    }
    #[test] fn rejects_missing_font_excessive_font_bytes_and_bad_px(){
        assert!(shape_text("abc",b"not a font",24.0,ParagraphDirection::Auto).is_none());
        assert!(shape_text("abc",&vec![0;MAX_SHAPE_FONT_BYTES+1],24.0,ParagraphDirection::Auto).is_none());
        assert!(shape_text("abc",b"",24.0,ParagraphDirection::Auto).is_none());
        assert!(shape_text("abc",b"bad",f32::NAN,ParagraphDirection::Auto).is_none());
    }
}
