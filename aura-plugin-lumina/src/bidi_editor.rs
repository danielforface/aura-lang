//! Pure single-line physical caret model for an already shaped OpenType line.
//! Keep logical UAX29 indices for edits and *separate* UAX9 visual-run
//! affinity for cursor painting/navigation. Never estimate GSUB ligature
//! interior caret positions by dividing glyph advances.
use crate::text_shaping::{ShapedText,ShapedRun};

#[derive(Clone,Copy,Debug,PartialEq)]
pub(crate) struct Stop {
    pub(crate) grapheme: usize,
    pub(crate) run: usize,
    pub(crate) x: f32,
}
/// Constructed only if every grapheme boundary has >=1 exact run-affine
/// stop. Failure means visual caret/selection must NOT be painted or moved.
fn stops(shaped:&ShapedText)->Option<Vec<Stop>>{
    if !shaped.caret_map_complete || !shaped.width.is_finite() ||
        shaped.width<0.0 {return None;}
    let mut out=Vec::new();
    for c in &shaped.caret_candidates {
        if !c.visual_x.is_finite() || c.visual_x<0.0 ||
            c.visual_x>shaped.width+0.125 {return None;}
        let grapheme=shaped.bidi.grapheme_boundaries.binary_search(&c.byte).ok()?;
        if c.visual_run>=shaped.visual_runs.len(){return None;}
        out.push(Stop{grapheme,run:c.visual_run,x:c.visual_x});
    }
    if out.len()>2*(shaped.bidi.grapheme_boundaries.len())+2048 {return None;}
    out.sort_by(|a,b| a.x.total_cmp(&b.x)
        .then(a.run.cmp(&b.run))
        .then(a.grapheme.cmp(&b.grapheme)));
    out.dedup_by(|a,b|a.grapheme==b.grapheme&&a.run==b.run&&a.x==b.x);
    if !shaped.bidi.grapheme_boundaries.iter().enumerate()
        .all(|(i,_)|out.iter().any(|v|v.grapheme==i)){return None;}
    Some(out)
}
fn current<'a>(all:&'a[Stop],grapheme:usize,affinity:Option<usize>)->Option<(usize,&'a Stop)>{
    all.iter().enumerate()
        .find(|(_,v)|v.grapheme==grapheme&&Some(v.run)==affinity)
        .or_else(||all.iter().enumerate().find(|(_,v)|v.grapheme==grapheme))
}
pub(crate) fn caret_position(shaped:&ShapedText,grapheme:usize,
    affinity:Option<usize>)->Option<Stop>{
    let all=stops(shaped)?;
    Some(*current(&all,grapheme,affinity)?.1)
}

/// Visual Left/Right step. The editor still stores a *logical grapheme index*,
/// while an affinity selects which UAX9 run owns an ambiguous boundary.
/// Visual selection collapse chooses an endpoint in actual screen order.
pub(crate) fn navigate(shaped:&ShapedText,grapheme:usize,affinity:Option<usize>,
    right:bool,extend:bool,selection:Option<(usize,usize)>)->Option<Stop>{
    let all=stops(shaped)?;
    if !extend {
        if let Some((a,b))=selection {
            let ends=all.iter().filter(|p|p.grapheme==a||p.grapheme==b);
            let chosen=if right {ends.max_by(|a,b|a.x.total_cmp(&b.x))}
                else {ends.min_by(|a,b|a.x.total_cmp(&b.x))};
            return chosen.copied();
        }
    }
    let (at,_)=current(&all,grapheme,affinity)?;
    let next=if right{at.checked_add(1)?}else{at.checked_sub(1)?};
    all.get(next).copied()
}

/// Physical Home/End without changing the immutable logical string.
pub(crate) fn edge(shaped:&ShapedText,rightmost:bool)->Option<Stop>{
    let all=stops(shaped)?;
    if rightmost{all.last().copied()}else{all.first().copied()}
}

/// Nearest real visual caret stop, with no fabricated ligature or missing
/// cluster position. The caller translates physical pointer x by input scroll.
pub(crate) fn hit_test(shaped:&ShapedText,x:f32)->Option<Stop>{
    if !x.is_finite(){return None;}
    let all=stops(shaped)?;
    all.into_iter().min_by(|a,b|{
        (a.x-x).abs().total_cmp(&(b.x-x).abs())
            .then(a.run.cmp(&b.run)).then(a.grapheme.cmp(&b.grapheme))
    })
}
fn exact(shaped:&ShapedText,run:usize,byte:usize)->Option<f32>{
    shaped.caret_candidates.iter()
        .find(|c|c.visual_run==run&&c.byte==byte).map(|c|c.visual_x)
}
fn range_for_run(shaped:&ShapedText,index:usize,run:&ShapedRun,
    a:usize,b:usize)->Option<Option<(f32,f32)>>{
    let base=&shaped.bidi.visual_runs[index].graphemes;
    let lo=base.start.max(a);
    let hi=base.end.min(b);
    if lo>=hi{return Some(None);}
    let start=shaped.bidi.grapheme_boundaries[lo];
    let end=shaped.bidi.grapheme_boundaries[hi];
    let x0=exact(shaped,index,start)?;
    let x1=exact(shaped,index,end)?;
    if !x0.is_finite()||!x1.is_finite(){return None;}
    let (left,right)=(x0.min(x1),x0.max(x1));
    if left<run.visual_x-0.125 ||
       right>run.visual_x+run.width+0.125{return None;}
    Some((right>left).then_some((left,right)))
}
/// Separate rectangles for source-logical [a,b) intersecting each UAX9 visual
/// run. A selected nonadjacent run must not paint the unselected middle gap.
pub(crate) fn selection_fragments(shaped:&ShapedText,a:usize,b:usize)
    ->Option<Vec<(f32,f32)>>{
    stops(shaped)?;
    let (start,end)=(a.min(b),a.max(b));
    let max=shaped.bidi.grapheme_boundaries.len().checked_sub(1)?;
    if end>max{return None;}
    let mut spans=Vec::new();
    for (i,run) in shaped.visual_runs.iter().enumerate(){
        if let Some(span)=range_for_run(shaped,i,run,start,end)?{
            spans.push(span);
        }
    }
    Some(spans)
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::text_shaping::{shape_text,ParagraphDirection};
    fn sample(text:&str)->ShapedText{
        let bytes=std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf").unwrap();
        shape_text(text,&bytes,24.0,ParagraphDirection::Auto).unwrap()
    }
    #[test]fn pure_hebrew_right_arrow_moves_physically_right_but_logical_index_decreases(){
        if !std::path::Path::new("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf").exists(){return;}
        let s=sample("שלום");let end=4;
        let c=caret_position(&s,end,None).unwrap();
        let r=navigate(&s,end,Some(c.run),true,false,None).unwrap();
        assert!(r.x>c.x);assert_eq!(r.grapheme,3);
        let l=navigate(&s,r.grapheme,Some(r.run),false,false,None).unwrap();
        assert_eq!(l.grapheme,4);
    }
    #[test]fn mixed_uax9_runs_expose_multiple_affinities_and_real_disjoint_selection(){
        if !std::path::Path::new("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf").exists(){return;}
        let text="שלום ABC 123 עולם";
        let s=sample(text);
        assert!(s.visual_runs.len()>=3);
        let all=stops(&s).expect("fixture must be complete or remain explicitly unqualified");
        assert!(all.windows(2).all(|w|w[0].x<=w[1].x));
        let at=hit_test(&s,s.width*.51).unwrap();
        assert!((at.x-s.width*.51).abs()<s.width*.5);
        let end=s.bidi.grapheme_boundaries.len()-1;
        let parts=selection_fragments(&s,0,end).unwrap();
        assert!(parts.len()>=3,"UAX9 split selection by visual runs");
        assert!(parts.iter().all(|(a,b)|b>a));
        let gaps=selection_fragments(&s,2,9).unwrap();
        assert!(gaps.len()>=2,"logical selection straddles reordered runs");
    }
    #[test]fn rejects_incomplete_cluster_and_nonfinite_hit(){
        if !std::path::Path::new("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf").exists(){return;}
        let mut s=sample("שלום");
        s.caret_map_complete=false;
        assert!(hit_test(&s,12.0).is_none());
        assert!(selection_fragments(&s,0,2).is_none());
        assert!(navigate(&s,1,None,true,false,None).is_none());
        s.caret_map_complete=true;
        assert!(hit_test(&s,f32::NAN).is_none());
    }
}
