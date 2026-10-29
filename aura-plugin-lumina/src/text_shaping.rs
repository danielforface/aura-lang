//! Experimental *pure* UAX #9 + OpenType shaping stage. Glyph IDs are NOT
//! Unicode scalar values; Raylib DrawTextEx cannot paint this output directly.
//! Invariant: source stays logical UTF-8, every run is a source byte range,
//! while shaped glyphs are in visual order with advances in native font units.
//! No rendering, visual cursor navigation or IME qualification is implied.
use std::ops::Range;
use unicode_bidi::{BidiInfo, Level};
use unicode_segmentation::UnicodeSegmentation;
use crate::gdef_caret;

pub(crate) const MAX_SHAPE_TEXT_BYTES: usize = 64 * 1024;
pub(crate) const MAX_SHAPE_FONT_BYTES: usize = 16 * 1024 * 1024;
const MAX_SHAPE_GLYPHS: usize = 16 * 1024;
const MAX_VISUAL_RUNS: usize = 1024;
pub(crate) const MAX_FONT_CHAIN: usize = 2;
const MAX_FONT_SEGMENTS: usize = 512;

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
    /// Index into explicitly provided [primary, fallback] font faces, NEVER a scalar.
    pub(crate) font_index: usize,
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
    shape_text_with_fonts(source, &[font_data], px, direction)
}

/// Explicit, bounded primary+one-secondary font selection. Every UAX29
/// grapheme is covered wholly by ONE cmap; no scalar-by-scalar fallback.
/// Group same-font neighboring graphemes into script-safe shaping segments,
/// then reshape every segment in its UAX9 visual position. Font IDs are
/// carried through to the GPU atlas so GID namespaces never collide.
pub(crate) fn shape_text_with_fonts(
    source: &str, font_data: &[&[u8]], px: f32, direction: ParagraphDirection,
) -> Option<ShapedText> {
    if font_data.is_empty() || font_data.len()>MAX_FONT_CHAIN ||
        !px.is_finite() || !(10.0..=96.0).contains(&px) { return None; }
    let mut faces=Vec::with_capacity(font_data.len());
    let mut scales=Vec::with_capacity(font_data.len());
    for bytes in font_data {
        if bytes.is_empty() || bytes.len()>MAX_SHAPE_FONT_BYTES {return None;}
        let face=rustybuzz::Face::from_slice(bytes,0)?;
        let units=face.units_per_em();
        if units<=0{return None;}
        scales.push(px/units as f32);
        faces.push(face);
    }
    let bidi=analyze(source,direction)?;
    let source_graphemes:Vec<&str>=source.graphemes(true).collect();
    let mut choices=Vec::with_capacity(source_graphemes.len());
    for g in &source_graphemes {
        let selected=faces.iter().position(|face| g.chars().all(|ch|face.glyph_index(ch).is_some()))?;
        choices.push(selected);
    }
    // Never split a cursively joining Arabic/Syriac sequence across fonts.
    // A font transition in such a sequence requires a shaping backend that
    // can negotiate cross-font joining, which this bounded path cannot do.
    fn joining(ch:char)->bool {
        matches!(ch, '\u{0600}'..='\u{06FF}' | '\u{0700}'..='\u{074F}' |
            '\u{0750}'..='\u{077F}' | '\u{08A0}'..='\u{08FF}')
    }
    for i in 1..choices.len() {
        if choices[i]!=choices[i-1] {
            let prev=source_graphemes[i-1].chars().last()?;
            let next=source_graphemes[i].chars().next()?;
            if joining(prev)&&joining(next){return None;}
        }
    }
    let mut visual_runs=Vec::with_capacity(bidi.visual_runs.len());
    let mut pen_x=0.0_f32;
    let mut count=0_usize;
    let mut segment_total=0_usize;
    for run in &bidi.visual_runs {
        let origin=pen_x;
        let mut glyphs=Vec::new();
        // Segment *logical* order at complete graphemes and same font.
        let mut groups=Vec::new();
        let mut i=run.graphemes.start;
        while i<run.graphemes.end {
            let face_idx=choices[i];
            let start=i;
            i+=1;
            while i<run.graphemes.end&&choices[i]==face_idx{i+=1;}
            groups.push((start,i,face_idx));
            segment_total+=1;
            if segment_total>MAX_FONT_SEGMENTS{return None;}
        }
        // UAX9 visual runs are left-to-right, but an RTL run's logical
        // font groups must be processed last-to-first before glyph layout.
        if run.rtl {groups.reverse();}
        for (start,end,face_idx) in groups {
            let first=bidi.grapheme_boundaries[start];
            let last=bidi.grapheme_boundaries[end];
            let mut buffer=rustybuzz::UnicodeBuffer::new();
            buffer.push_str(&source[first..last]);
            buffer.guess_segment_properties();
            buffer.set_direction(if run.rtl {
                rustybuzz::Direction::RightToLeft
            }else{rustybuzz::Direction::LeftToRight});
            let output=rustybuzz::shape(&faces[face_idx],&[],buffer);
            if count.saturating_add(output.len())>MAX_SHAPE_GLYPHS{return None;}
            count+=output.len();
            for (info,pos) in output.glyph_infos().iter().zip(output.glyph_positions()) {
                let cluster_byte=first.checked_add(info.cluster as usize)?;
                if cluster_byte>=last || !source.is_char_boundary(cluster_byte) ||
                    info.glyph_id==0 || info.glyph_id>u16::MAX as u32{return None;}
                let scale=scales[face_idx];
                let advance_x=pos.x_advance as f32*scale;
                let advance_y=pos.y_advance as f32*scale;
                let offset_x=pos.x_offset as f32*scale;
                let offset_y=pos.y_offset as f32*scale;
                if ![advance_x,advance_y,offset_x,offset_y].iter().all(|x|x.is_finite()) {
                    return None;
                }
                glyphs.push(ShapedGlyph{
                    glyph_id:info.glyph_id as u16,font_index:face_idx,
                    cluster_byte,advance_x,advance_y,offset_x,offset_y,pen_x,
                });
                pen_x+=advance_x;
            }
        }
        if !pen_x.is_finite()||pen_x<origin{return None;}
        visual_runs.push(ShapedRun{
            source_bytes:run.source_bytes.clone(),rtl:run.rtl,visual_x:origin,
            width:pen_x-origin,glyphs
        });
    }
    // Derive clusters from *actual* rustybuzz source byte associations.
    // Each cluster's physical boundaries are exact glyph advances. Only a
    // single GSUB ligature GID with matching GDEF LigCaretList coordinates may
    // supply interior grapheme carets. Do not interpolate missing positions.
    let mut caret_candidates=Vec::new();
    let mut complete=true;
    for (run_index,run) in visual_runs.iter().enumerate(){
        let mut cluster_starts:Vec<usize>=run.glyphs.iter()
            .map(|g|g.cluster_byte).collect();
        cluster_starts.sort_unstable();
        cluster_starts.dedup();
        if cluster_starts.first().copied()!=Some(run.source_bytes.start) ||
            (run.glyphs.is_empty()&&run.source_bytes.start!=run.source_bytes.end){
            complete=false;
            continue;
        }
        for (ci,&first) in cluster_starts.iter().enumerate() {
            let last=cluster_starts.get(ci+1).copied()
                .unwrap_or(run.source_bytes.end);
            if last<=first || last>run.source_bytes.end {complete=false;continue;}
            let Ok(lo)=bidi.grapheme_boundaries.binary_search(&first)
                else{complete=false;continue;};
            let Ok(hi)=bidi.grapheme_boundaries.binary_search(&last)
                else{complete=false;continue;};
            let n=hi.saturating_sub(lo);
            if n==0 || n>gdef_caret::MAX_COMPONENTS {complete=false;continue;}
            let parts:Vec<&ShapedGlyph>=run.glyphs.iter()
                .filter(|g|g.cluster_byte==first).collect();
            let left=parts.iter().map(|g|g.pen_x).fold(f32::INFINITY,f32::min);
            let right=parts.iter().map(|g|g.pen_x+g.advance_x)
                .fold(f32::NEG_INFINITY,f32::max);
            if !left.is_finite()||!right.is_finite()||right<left {
                complete=false;continue;
            }
            let mut physical=Vec::with_capacity(n+1);
            physical.push(left);
            if n>1 {
                // Unsupported cluster (several output glyphs) or absent GDEF
                // does not create fake caret coordinates. OpenType design
                // units are converted with the SAME px/upem scale as GSUB.
                let Some(glyph)=parts.first().filter(|_|parts.len()==1)
                    else{complete=false;continue;};
                let Some(bytes)=font_data.get(glyph.font_index) else{
                    complete=false;continue;
                };
                let Ok(coords)=gdef_caret::ligature_caret_coords(
                    bytes,glyph.glyph_id,n-1) else{complete=false;continue;};
                let scale=scales[glyph.font_index];
                let mut prev=left;
                let mut valid=true;
                for x in coords {
                    let pos=glyph.pen_x+x as f32*scale;
                    // Reject out-of-glyph, zero-width and non-monotonic
                    // positions, never snap to a guessed fractional step.
                    if !pos.is_finite()||pos<=prev+0.001||pos>=right-0.001{
                        valid=false;break;
                    }
                    physical.push(pos);
                    prev=pos;
                }
                if !valid || physical.len()!=n {complete=false;continue;}
            }
            physical.push(right);
            for i in 0..=n {
                let byte=bidi.grapheme_boundaries[lo+i];
                let x=if run.rtl{physical[n-i]}else{physical[i]};
                caret_candidates.push(CaretCandidate{
                    byte,visual_x:x,visual_run:run_index
                });
            }
        }
    }
    caret_candidates.sort_by(|a,b|a.byte.cmp(&b.byte)
        .then(a.visual_run.cmp(&b.visual_run)));
    Some(ShapedText{bidi,visual_runs,width:pen_x,caret_candidates,
        caret_map_complete:complete,native_paintable:false})
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

    #[test] fn real_serif_to_sans_hebrew_and_arabic_fallback_owns_actual_glyph_ids(){
        let p="/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf";
        let f="/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
        if !std::path::Path::new(p).is_file()||!std::path::Path::new(f).is_file(){return;}
        let serif=std::fs::read(p).unwrap();
        let sans=std::fs::read(f).unwrap();
        for (text,needle) in [("Hello שלום","שלום"),("ABC سلام","سلام")] {
            assert!(shape_text(text,&serif,28.0,ParagraphDirection::Auto).is_none(),
                "primary alone must fail missing cmap without .notdef substitution");
            let shaped=shape_text_with_fonts(text,&[&serif,&sans],28.0,
                ParagraphDirection::Auto).expect("whole grapheme secondary font");
            let fallback_start=text.find(needle).unwrap();
            let g:Vec<_>=shaped.visual_runs.iter().flat_map(|r|r.glyphs.iter()).collect();
            assert!(g.iter().any(|g|g.font_index==0&&g.cluster_byte<fallback_start));
            assert!(g.iter().any(|g|g.font_index==1&&g.cluster_byte>=fallback_start));
            assert!(g.iter().all(|g|g.glyph_id!=0&&text.is_char_boundary(g.cluster_byte)));
            assert!(shaped.width>10.0);
        }
    }
    #[test] fn fallback_is_bounded_complete_cluster_and_bidi_non_destructive(){
        let p="/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf";
        let f="/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
        if !std::path::Path::new(p).is_file()||!std::path::Path::new(f).is_file(){return;}
        let a=std::fs::read(p).unwrap();let b=std::fs::read(f).unwrap();
        assert!(shape_text_with_fonts("שלום",&[&a,&b],26.0,ParagraphDirection::Auto).is_some());
        assert!(shape_text_with_fonts("שלום",&[&a],26.0,ParagraphDirection::Auto).is_none());
        assert!(shape_text_with_fonts("שלום",&[&a,&b,&b],26.0,ParagraphDirection::Auto).is_none());
        assert!(shape_text_with_fonts("שלום",&[&a,b"no font"],26.0,ParagraphDirection::Auto).is_none());
        assert!(shape_text_with_fonts("שלום",&[&a,&b],f32::NAN,ParagraphDirection::Auto).is_none());
        // The same source UTF-8 grapheme boundaries must survive fallback.
        let t="Aשָׁלום";
        let res=shape_text_with_fonts(t,&[&a,&b],26.0,ParagraphDirection::Auto).unwrap();
        assert_eq!(res.bidi.grapheme_boundaries.last(),Some(&t.len()));
        assert!(res.visual_runs.iter().flat_map(|r|r.glyphs.iter()).all(|g|
            g.cluster_byte<t.len()&&t.is_char_boundary(g.cluster_byte)));
    }
}
