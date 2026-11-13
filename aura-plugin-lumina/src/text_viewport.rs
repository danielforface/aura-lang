//! Pure text viewport geometry. Caller supplies widths measured with the
//! SAME Raylib default-font MeasureText/DrawText pair, outside BeginDrawing.
//! Logical pixels only. Does not shape BiDi, perform IME or detect font fallback.
use unicode_segmentation::UnicodeSegmentation;
use crate::layout::Bounds;

pub(crate) const PAD_X: f32 = 12.0;
const CARET_MARGIN: f32 = 2.0;

#[derive(Clone,Debug,PartialEq)]
pub(crate) struct Visual {
    pub scroll: f32,
    /// Widths in the unscrolled text coordinate system.
    pub caret: f32,
    pub selection: Option<(f32,f32)>,
    /// Real visually separated ranges for an opt-in shaped BiDi editor.
    pub selection_spans: Vec<(f32,f32)>,
    /// Non-authoritative active IME preedit underline fragments, in line px.
    pub preedit_spans: Vec<(f32,f32)>,
    /// Don't paint a fake caret inside an unresolved ligature cluster.
    pub caret_valid: bool,
}
impl Visual {
    pub(crate) fn origin(&self, content: Bounds) -> f32 { content.x-self.scroll }
    pub(crate) fn caret_screen(&self, content: Bounds) -> f32 {
        (self.origin(content)+self.caret).clamp(content.x,content.x+content.width)
    }
    pub(crate) fn selection_screen(&self,content:Bounds)->Option<(f32,f32)> {
        self.selection.and_then(|(a,b)|{
            let x0=(self.origin(content)+a).clamp(content.x,content.x+content.width);
            let x1=(self.origin(content)+b).clamp(content.x,content.x+content.width);
            (x1>x0).then_some((x0,x1))
        })
    }
    pub(crate) fn preedit_screens(&self,content:Bounds)->Vec<(f32,f32)>{
        self.preedit_spans.iter().filter_map(|&(a,b)|{
            let x0=(self.origin(content)+a).clamp(content.x,content.x+content.width);
            let x1=(self.origin(content)+b).clamp(content.x,content.x+content.width);
            (x1>x0).then_some((x0,x1))
        }).collect()
    }
    /// Retain one rectangle per noncontiguous UAX9 visual run segment.
    pub(crate) fn selection_screens(&self,content:Bounds)->Vec<(f32,f32)>{
        if !self.selection_spans.is_empty(){
            return self.selection_spans.iter().filter_map(|&(a,b)|{
                let x0=(self.origin(content)+a).clamp(content.x,content.x+content.width);
                let x1=(self.origin(content)+b).clamp(content.x,content.x+content.width);
                (x1>x0).then_some((x0,x1))
            }).collect();
        }
        self.selection_screen(content).into_iter().collect()
    }
}
pub(crate) fn content_bounds(rect:Bounds)->Bounds {
    // Small top/bottom margin also leaves the rounded widget border intact.
    Bounds::new(rect.x+PAD_X,rect.y+3.0,(rect.width-2.0*PAD_X).max(0.0),
        (rect.height-6.0).max(0.0))
}
pub(crate) fn grapheme_prefix(text:&str, graphemes:usize)->&str {
    let byte=text.grapheme_indices(true).nth(graphemes)
        .map(|(i,_)|i).unwrap_or(text.len());
    &text[..byte]
}
fn clean(value:f32)->f32 { if value.is_finite(){value.max(0.0)}else{0.0} }
/// When focused, keep the native measured caret in the clipping viewport.
/// When blurred, preserve last scroll (clamped) so text does not jump.
pub(crate) fn plan(
    visible_width:f32, full_width:f32,caret_width:f32,
    selection_widths:Option<(f32,f32)>, previous_scroll:f32,focused:bool
)->Visual {
    let width=clean(visible_width);
    let full=clean(full_width);
    let caret=clean(caret_width).min(full);
    let max_scroll=(full-width).max(0.0);
    let mut scroll=clean(previous_scroll).min(max_scroll);
    if focused {
        if caret<scroll+CARET_MARGIN {
            scroll=(caret-CARET_MARGIN).max(0.0);
        } else if caret>scroll+width-CARET_MARGIN {
            scroll=(caret-width+CARET_MARGIN).max(0.0);
        }
        scroll=scroll.min(max_scroll);
    }
    let selection=selection_widths.and_then(|(a,b)| {
        if !a.is_finite() || !b.is_finite(){return None;}
        let lo=clean(a).min(full);
        let hi=clean(b).min(full);
        (hi>lo).then_some((lo,hi))
    });
    Visual {scroll,caret,selection,selection_spans:Vec::new(),preedit_spans:Vec::new(),caret_valid:true}
}
/// Real font prefix widths to UAX29 grapheme stops for the *unshaped*
/// single-line editor. Bounded O(log N) measurements during mouse drags.
/// Shaped BiDi/GSUB text must instead use bidi_editor::hit_test.
pub(crate) fn grapheme_at_x<F>(text:&str,x:f32,mut width:F)->Option<usize>
where F:FnMut(&str)->f32 {
    if !x.is_finite()||text.len()>8192{return None;}
    let boundaries:Vec<usize>=text.grapheme_indices(true)
        .map(|(byte,_)|byte).chain(std::iter::once(text.len())).collect();
    let total=boundaries.len().checked_sub(1)?;
    if x<=0.0||total==0{return Some(0);}
    let full=width(text);
    if !full.is_finite()||full<0.0{return None;}
    if x>=full{return Some(total);}
    let mut low=0usize;
    let mut high=total;
    while low<high {
        let mid=low+(high-low)/2;
        let w=width(&text[..boundaries[mid]]);
        if !w.is_finite()||w<0.0{return None;}
        if w<x{low=mid+1;}else{high=mid;}
    }
    let next=low.min(total);
    let prev=next.saturating_sub(1);
    let a=width(&text[..boundaries[prev]]);
    let b=width(&text[..boundaries[next]]);
    if !a.is_finite()||!b.is_finite()||a<0.0||b<a{return None;}
    Some(if x-a<=b-x{prev}else{next})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c()->Bounds{content_bounds(Bounds::new(20.0,30.0,100.0,40.0))}
    #[test]
    fn native_pointer_hit_uses_measured_grapheme_boundaries(){
        let w=|value:&str|value.graphemes(true).map(|g|
            if g=="W"{20.0}else if g=="👩‍💻"{30.0}else{5.0}).sum::<f32>();
        let s="aW👩‍💻e\u{301}";
        assert_eq!(grapheme_at_x(s,-10.0,w),Some(0));
        assert_eq!(grapheme_at_x(s,4.0,w),Some(1));
        assert_eq!(grapheme_at_x(s,8.0,w),Some(1));
        assert_eq!(grapheme_at_x(s,22.0,w),Some(2));
        assert_eq!(grapheme_at_x(s,42.0,w),Some(3));
        assert_eq!(grapheme_at_x(s,9999.0,w),Some(4));
        assert_eq!(grapheme_at_x("",4.0,|_|0.0),Some(0));
    }
    #[test]
    fn native_pointer_hit_fails_closed_on_nan_and_unbounded_text(){
        assert_eq!(grapheme_at_x("abc",2.0,|s|
            if s.len()==2{f32::NAN}else{s.len() as f32}),None);
        assert_eq!(grapheme_at_x("abc",f32::NAN,|_|5.0),None);
        assert_eq!(grapheme_at_x(&"a".repeat(8193),5.0,|_|5.0),None);
        assert_eq!(grapheme_at_x("👩‍💻",100.0,|s|
            if s.is_empty(){0.0}else{25.0}),Some(1));
    }
    #[test]fn content_clip_is_inside_widget(){assert_eq!(c(),Bounds::new(32.0,33.0,76.0,34.0));}
    #[test]fn preedit_underlines_clip_to_visible_content_and_leave_selection_unmodified(){
        let mut v=plan(76.0,200.0,98.0,None,0.0,true);
        v.preedit_spans=vec![(15.0,32.0),(145.0,200.0)];
        assert_eq!(v.preedit_screens(c()),vec![(32.0,40.0)]);
        v.scroll=10.0;
        assert_eq!(v.preedit_screens(c()),vec![(37.0,54.0)]);
        assert_eq!(v.selection,None);
    }
    #[test]fn short_text_never_scrolls(){let v=plan(76.0,40.0,20.0,None,100.0,true);assert_eq!(v.scroll,0.0);}
    #[test]fn end_caret_scrolls_until_visible(){let v=plan(76.0,200.0,200.0,None,0.0,true);assert_eq!(v.scroll,124.0);assert_eq!(v.caret_screen(c()),108.0);}
    #[test]fn home_returns_to_start(){let v=plan(76.0,250.0,0.0,None,174.0,true);assert_eq!(v.scroll,0.0);}
    #[test]fn blurred_preserves_scroll_but_clamps_when_text_shrinks(){
        let v=plan(76.0,230.0,5.0,None,100.0,false);assert_eq!(v.scroll,100.0);
        let s=plan(76.0,77.0,2.0,None,v.scroll,false);assert_eq!(s.scroll,1.0);
    }
    #[test]fn measured_nonuniform_glyphs_not_grapheme_count_set_caret(){
        let v=plan(76.0,95.0,21.0,None,0.0,true);
        assert_eq!(v.caret,21.0);
        assert_eq!(v.caret_screen(c()),53.0);
    }
    #[test]fn selection_is_clipped_and_normalized_in_visual_coordinates(){
        let v=plan(76.0,200.0,185.0,Some((20.0,195.0)),0.0,true);
        assert_eq!(v.scroll,111.0);
        assert_eq!(v.selection_screen(c()),Some((32.0,108.0)));
    }
    #[test]fn invalid_width_and_grapheme_prefix_are_safe(){
        let v=plan(f32::NAN,f32::INFINITY,f32::NAN,Some((f32::NAN,3.0)),f32::INFINITY,true);
        assert_eq!((v.scroll,v.caret,v.selection),(0.0,0.0,None));
        assert_eq!(grapheme_prefix("A👩‍👩‍👧‍👦B",2),"A👩‍👩‍👧‍👦");
        assert_eq!(grapheme_prefix("abc",usize::MAX),"abc");
    }
    #[test]fn selection_negative_or_beyond_length_is_bounded(){
        let v=plan(50.0,100.0,70.0,Some((-50.0,400.0)),0.0,true);
        assert_eq!(v.selection,Some((0.0,100.0)));
    }
}
