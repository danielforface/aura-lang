//! Deterministic renderer-independent UAX #29 *extended grapheme cluster*
//! editor. Cursor is a grapheme index, not a byte or scalar index.
//! This protects combining marks, variation selectors, flags and emoji ZWJ
//! sequences. Visual bidi caret placement and IME preedit are NOT solved here.

use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EditCommand {
    Left, Right, Home, End, Backspace, Delete, Insert(String),
}

pub(crate) fn grapheme_count(text: &str) -> usize {
    text.graphemes(true).count()
}

fn byte_offset(text: &str, index: usize) -> usize {
    text.grapheme_indices(true)
        .nth(index)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

/// After insertion the inserted byte offset may fall *inside* a combined
/// grapheme (e.g. new combining accent, ZWJ joining an existing emoji).
/// In that case snap the caret to the first boundary at or after that byte.
fn grapheme_index_at_or_after_byte(text: &str, byte: usize) -> usize {
    if byte == 0 { return 0; }
    for (i, (start, cluster)) in text.grapheme_indices(true).enumerate() {
        if start + cluster.len() >= byte { return i + 1; }
    }
    grapheme_count(text)
}

pub(crate) fn apply_edit(buffer: &mut String, caret: &mut usize, cmd: EditCommand) -> bool {
    let len = grapheme_count(buffer);
    *caret = (*caret).min(len);
    match cmd {
        EditCommand::Left => *caret = caret.saturating_sub(1),
        EditCommand::Right => *caret = caret.saturating_add(1).min(len),
        EditCommand::Home => *caret = 0,
        EditCommand::End => *caret = len,
        EditCommand::Backspace if *caret > 0 => {
            let start = byte_offset(buffer, *caret - 1);
            let end = byte_offset(buffer, *caret);
            buffer.replace_range(start..end, "");
            *caret -= 1;
            return true;
        }
        EditCommand::Delete if *caret < len => {
            let start = byte_offset(buffer, *caret);
            let end = byte_offset(buffer, *caret + 1);
            buffer.replace_range(start..end, "");
            return true;
        }
        EditCommand::Insert(value) => {
            let filtered: String = value.chars().filter(|c| !c.is_control()).collect();
            if filtered.is_empty() { return false; }
            let pos = byte_offset(buffer, *caret);
            let boundary = pos + filtered.len();
            buffer.insert_str(pos, &filtered);
            *caret = grapheme_index_at_or_after_byte(buffer, boundary);
            return true;
        }
        EditCommand::Backspace | EditCommand::Delete => {}
    }
    false
}


/// Per-widget ephemeral editor state, including grapheme selection and
/// bounded undo/redo. This is independent of rendering and host callbacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EditSession {
    pub buffer: String,
    pub caret: usize,
    pub anchor: usize,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    preedit: Option<Preedit>,
    last_composition_sequence: u64,
    /// Monotonic barrier for authority rebases, edits and selection history.
    authority_epoch: u64,
    /// Changes only when host authority rebases (not local editing).
    rebase_generation: u64,
    /// Optional Host-authored UAX29 capacity; never truncates existing values.
    max_graphemes:Option<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot { buffer: String, caret: usize, anchor: usize }

pub(crate) const MAX_PREEDIT_BYTES: usize = 1024;
pub(crate) const MAX_PROJECTED_TEXT_BYTES: usize = 8192;
/// An OS input adapter may supply this sequence, but the editor owns the
/// authoritative string; only Commit crosses that boundary.
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct Preedit { pub(crate) text:String, pub(crate) caret:usize }
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct Projection {
    pub(crate) display: String,
    pub(crate) caret: usize,
    pub(crate) preedit_graphemes: std::ops::Range<usize>,
}


#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SessionAction {
    Move(EditCommand, bool), // second arg extends selection
    Edit(EditCommand),
    SelectAll,
    Undo,
    Redo,
}
const MAX_HISTORY: usize = 64;
fn valid_preedit(text:&str)->bool{
    text.len()<=MAX_PREEDIT_BYTES &&
    text.chars().all(|c|!c.is_control() && !matches!(c,
        '\u{202A}'..='\u{202E}'|'\u{2066}'..='\u{2069}'))
}

impl EditSession {
    pub(crate) fn new(text: String) -> Self {
        let end=grapheme_count(&text);
        Self { buffer:text, caret:end, anchor:end, undo:Vec::new(), redo:Vec::new(),
            preedit:None, last_composition_sequence:0,
            authority_epoch:0,rebase_generation:0,max_graphemes:None }
    }
    /// Applied on every focused native frame after Host revision reconciliation.
    /// A newer Host revision remains authoritative even if it exceeds the cap.
    pub(crate) fn set_max_graphemes(&mut self,max:Option<usize>){
        self.max_graphemes=max;
    }
    /// A bounded editor never increases an over-limit Host value; deletion or
    /// replacement that preserves/decreases its grapheme count stays usable.
    /// Count the FULL Unicode result, not inserted UTF-8 bytes or the sum of
    /// separately counted fragments (graphemes can merge across boundaries).
    fn permits_grapheme_result(&self,before:&str,after:&str)->bool{
        let Some(cap)=self.max_graphemes else{return true;};
        let before_count=grapheme_count(before);
        let after_count=grapheme_count(after);
        after_count<=cap || (before_count>cap && after_count<=before_count)
    }
    /// Count all retained dynamic text, including Undo/Redo, before a route
    /// can keep this session alive outside the visible widget tree.
    pub(crate) fn retained_bytes(&self)->usize {
        let dynamic=|items:&Vec<Snapshot>|items.iter()
            .fold(0usize,|sum,s|sum.saturating_add(s.buffer.len()));
        self.buffer.len()
            .saturating_add(dynamic(&self.undo))
            .saturating_add(dynamic(&self.redo))
            .saturating_add(self.preedit.as_ref().map_or(0,|p|p.text.len()))
    }
    pub(crate) fn authority_epoch(&self)->u64{self.authority_epoch}
    pub(crate) fn rebase_generation(&self)->u64{self.rebase_generation}
    /// Shaped/BiDi GPU hit-testing must pass through this method rather
    /// than assigning caret/anchor directly, so stale IME deletion tokens
    /// are invalidated even if the user navigates away and back.
    pub(crate) fn set_visual_caret(&mut self,grapheme:usize,extend:bool)->bool{
        if self.composing(){return false;}
        let next=grapheme.min(grapheme_count(&self.buffer));
        let old=(self.caret,self.anchor);
        self.caret=next;
        if !extend{self.anchor=next;}
        if old!=(self.caret,self.anchor){self.advance_authority_epoch();}
        old!=(self.caret,self.anchor)
    }

    fn advance_authority_epoch(&mut self){
        self.authority_epoch=self.authority_epoch.saturating_add(1);
    }
    /// Translate IBus Unicode *scalar* positions to strict UAX29 grapheme
    /// boundaries. No partial regional-indicator flag, combining sequence,
    /// emoji ZWJ cluster or UTF-8 codepoint can ever be removed.
    fn scalar_range_to_grapheme_bounds(
        text:&str,cursor_grapheme:usize,offset:i32,count:u32
    )->Option<(usize,usize,usize,usize)>{
        if count==0||count>1024||!(-1024..=1024).contains(&offset){return None;}
        let cursor_byte=byte_offset(text,cursor_grapheme);
        let cursor_scalar=text[..cursor_byte].chars().count();
        let start_scalar=(cursor_scalar as i64).checked_add(i64::from(offset))?;
        let end_scalar=start_scalar.checked_add(i64::from(count))?;
        if start_scalar<0||end_scalar>text.chars().count() as i64{return None;}
        let byte_at=|scalar:usize|{
            text.char_indices().nth(scalar).map(|(byte,_)|byte)
                .unwrap_or(text.len())
        };
        let a=byte_at(start_scalar as usize);
        let b=byte_at(end_scalar as usize);
        if a>=b{return None;}
        let boundaries:Vec<usize>=text.grapheme_indices(true)
            .map(|(byte,_)|byte).chain(std::iter::once(text.len())).collect();
        let g0=boundaries.binary_search(&a).ok()?;
        let g1=boundaries.binary_search(&b).ok()?;
        Some((a,b,g0,g1))
    }
    /// PREPARED ONLY: this atomic deletion primitive is NOT wired to
    /// engine-origin DeleteSurroundingText in production. Such a request
    /// is advisory until a separate real-engine provenance gate is proven.
    /// Snapshot and epoch must match after host value_revision reconciliation.
    pub(crate) fn delete_surrounding_if_snapshot(
        &mut self,epoch:u64,expected:&str,expected_caret:usize,
        expected_anchor:usize,offset:i32,count:u32
    )->bool{
        if self.composing()||self.authority_epoch!=epoch||
            self.buffer!=expected || self.caret!=expected_caret ||
            self.anchor!=expected_anchor || self.caret!=self.anchor ||
            self.buffer.len()>1024{return false;}
        let Some((a,b,g0,g1))=Self::scalar_range_to_grapheme_bounds(
            &self.buffer,self.caret,offset,count)else{return false;};
        let old=self.snapshot();
        self.buffer.replace_range(a..b,"");
        self.caret=if self.caret>=g1{
            self.caret-(g1-g0)
        }else if self.caret>=g0{g0}else{self.caret};
        self.anchor=self.caret;
        Self::push(&mut self.undo,old);
        self.redo.clear();
        self.advance_authority_epoch();
        true
    }
    /// Exactly one committed D→C replacement, not a standalone engine delete.
    /// The native adapter supplies a focus-owned, engine-key-local D then C
    /// proof; edit authority independently checks the exact snapshot + epoch.
    /// IBus offsets count Unicode scalars; deletion must align to BOTH UAX29
    /// grapheme boundaries, otherwise reject the complete transaction.
    pub(crate) fn replace_surrounding_if_snapshot(
        &mut self,epoch:u64,expected:&str,
        expected_caret_scalar:u32,expected_anchor_scalar:u32,
        offset:i32,count:u32,replacement:&str,sequence:u64
    )->Option<bool>{
        if sequence==0||sequence<=self.last_composition_sequence ||
            self.composing()||self.authority_epoch!=epoch||
            self.buffer!=expected||self.caret!=self.anchor||
            self.buffer.len()>1024||!valid_preedit(replacement){
            return None;
        }
        let caret_byte=byte_offset(&self.buffer,self.caret);
        let anchor_byte=byte_offset(&self.buffer,self.anchor);
        if self.buffer[..caret_byte].chars().count()!=expected_caret_scalar as usize||
            self.buffer[..anchor_byte].chars().count()!=expected_anchor_scalar as usize{
            return None;
        }
        let (a,b,_,_)=Self::scalar_range_to_grapheme_bounds(
            &self.buffer,self.caret,offset,count)?;
        let next_size=self.buffer.len().checked_sub(b-a)?
            .checked_add(replacement.len())?;
        if next_size>MAX_PROJECTED_TEXT_BYTES{return None;}
        let mut next=self.buffer.clone();
        next.replace_range(a..b,replacement);
        if !self.permits_grapheme_result(&self.buffer,&next){return None;}
        self.last_composition_sequence=sequence;
        if next==self.buffer{return Some(false);}
        let old=self.snapshot();
        self.buffer=next;
        self.caret=grapheme_index_at_or_after_byte(&self.buffer,a+replacement.len());
        self.anchor=self.caret;
        Self::push(&mut self.undo,old);
        self.redo.clear();
        self.advance_authority_epoch();
        Some(true)
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot { buffer:self.buffer.clone(),caret:self.caret,anchor:self.anchor }
    }
    fn restore(&mut self, s: Snapshot) {
        self.buffer=s.buffer;
        self.caret=s.caret.min(grapheme_count(&self.buffer));
        self.anchor=s.anchor.min(grapheme_count(&self.buffer));
    }
    /// Begin a new composition epoch on a focused field only. The sequence
    /// must be monotonically increasing, including after Cancel and Commit.
    pub(crate) fn start_composition(&mut self,sequence:u64)->bool {
        if sequence==0 || sequence<=self.last_composition_sequence || self.preedit.is_some(){return false;}
        self.last_composition_sequence=sequence;
        self.preedit=Some(Preedit{text:String::new(),caret:0});
        self.advance_authority_epoch();
        true
    }
    pub(crate) fn update_composition(&mut self,sequence:u64,text:&str,caret:usize)->bool {
        if sequence<=self.last_composition_sequence || sequence==0 ||
            self.preedit.is_none() || !valid_preedit(text) ||
            caret>grapheme_count(text){return false;}
        self.last_composition_sequence=sequence;
        self.preedit=Some(Preedit{text:text.to_owned(),caret});
        self.advance_authority_epoch();
        true
    }
    /// Exactly one committed Edit action: one Undo snapshot, one on_change.
    pub(crate) fn commit_composition(&mut self,sequence:u64,text:&str)->Option<bool> {
        if sequence==0 || sequence<=self.last_composition_sequence ||
           self.preedit.is_none() || !valid_preedit(text){return None;}
        self.last_composition_sequence=sequence;
        self.preedit=None;
        self.advance_authority_epoch();
        Some(if text.is_empty(){false}else{
            self.apply(SessionAction::Edit(EditCommand::Insert(text.to_owned())))
        })
    }
    pub(crate) fn cancel_composition(&mut self,sequence:u64)->bool {
        if sequence==0 || sequence<=self.last_composition_sequence || self.preedit.is_none(){return false;}
        self.last_composition_sequence=sequence;
        self.preedit=None;
        self.advance_authority_epoch();
        true
    }
    pub(crate) fn abandon_composition(&mut self) {
        if self.preedit.take().is_some(){self.advance_authority_epoch();}
    }
    pub(crate) fn composing(&self)->bool{self.preedit.is_some()}
    /// A new authoritative value revision clears transient composition and
    /// undo, yet must not reset the adapter replay barrier for this widget.
    pub(crate) fn rebase_authority(&mut self,text:String){
        let last=self.last_composition_sequence;
        let next=self.authority_epoch.saturating_add(1);
        let generation=self.rebase_generation.saturating_add(1);
        *self=Self::new(text);
        self.last_composition_sequence=last;
        self.authority_epoch=next;
        self.rebase_generation=generation;
    }
    /// Non-authoritative projection used by GPU and measured viewport only.
    /// The source buffer, caret, selection, undo and redo remain unchanged.
    pub(crate) fn projected(&self)->Option<Projection>{
        let p=self.preedit.as_ref()?;
        let (start,end)=self.selection().unwrap_or((self.caret,self.caret));
        let lo=byte_offset(&self.buffer,start);
        let hi=byte_offset(&self.buffer,end);
        let bytes=self.buffer.len().checked_sub(hi-lo)?.checked_add(p.text.len())?;
        if bytes>MAX_PROJECTED_TEXT_BYTES{return None;}
        let mut display=String::with_capacity(bytes);
        display.push_str(&self.buffer[..lo]);
        display.push_str(&p.text);
        display.push_str(&self.buffer[hi..]);
        let begin=grapheme_index_at_or_after_byte(&display,lo);
        let end_at=grapheme_index_at_or_after_byte(&display,lo+p.text.len());
        let caret_byte=lo+byte_offset(&p.text,p.caret);
        let caret=grapheme_index_at_or_after_byte(&display,caret_byte);
        Some(Projection{display,caret,preedit_graphemes:begin..end_at})
    }
    pub(crate) fn selection(&self) -> Option<(usize,usize)> {
        (self.anchor!=self.caret).then(||(self.anchor.min(self.caret),self.anchor.max(self.caret)))
    }
    fn push(undo:&mut Vec<Snapshot>,snapshot:Snapshot) {
        if undo.len()==MAX_HISTORY {undo.remove(0);}
        undo.push(snapshot);
    }
    pub(crate) fn apply(&mut self, action: SessionAction) -> bool {
        // Raw navigation, delete, insert and Undo are owned by the IME until
        // its explicit Commit or Cancel; no partial editor mutation.
        if self.preedit.is_some(){return false;}
        let len=grapheme_count(&self.buffer);
        self.caret=self.caret.min(len); self.anchor=self.anchor.min(len);
        match action {
            SessionAction::SelectAll => {
                if self.anchor!=0||self.caret!=len{self.advance_authority_epoch();}
                self.anchor=0;self.caret=len;false
            }
            SessionAction::Move(cmd,extend) => {
                let before=(self.caret,self.anchor);
                if !extend {
                    if let Some((start,end))=self.selection() {
                        match cmd {
                            EditCommand::Left=>{self.caret=start;self.anchor=start;
                                if before!=(self.caret,self.anchor){self.advance_authority_epoch();}
                                return false;}
                            EditCommand::Right=>{self.caret=end;self.anchor=end;
                                if before!=(self.caret,self.anchor){self.advance_authority_epoch();}
                                return false;}
                            _=>{}
                        }
                    }
                }
                let old=self.caret;
                // Navigation only: never create undo entries or emit on_change.
                let _=apply_edit(&mut self.buffer,&mut self.caret,cmd);
                if !extend {self.anchor=self.caret;}
                if self.caret==old && !extend {self.anchor=self.caret;}
                if before!=(self.caret,self.anchor){self.advance_authority_epoch();}
                false
            }
            SessionAction::Undo => {
                if self.undo.last().is_some_and(|previous|
                    !self.permits_grapheme_result(&self.buffer,&previous.buffer)){
                    return false; // Keep the undo stack intact for future changes.
                }
                if let Some(previous)=self.undo.pop() {
                    let now=self.snapshot();
                    Self::push(&mut self.redo,now);
                    self.restore(previous);self.advance_authority_epoch();return true;
                }
                false
            }
            SessionAction::Redo => {
                if self.redo.last().is_some_and(|next|
                    !self.permits_grapheme_result(&self.buffer,&next.buffer)){
                    return false; // Never consume an inadmissible redo snapshot.
                }
                if let Some(next)=self.redo.pop() {
                    let now=self.snapshot();
                    Self::push(&mut self.undo,now);
                    self.restore(next);self.advance_authority_epoch();return true;
                }
                false
            }
            SessionAction::Edit(cmd) => {
                let insertion=match &cmd { EditCommand::Insert(s) =>
                    !s.chars().any(|c| !c.is_control()), _=>false };
                if insertion {return false;}
                let old=self.snapshot();
                let mut changed=false;
                if let Some((start,end))=self.selection() {
                    match cmd {
                        EditCommand::Backspace|EditCommand::Delete|EditCommand::Insert(_) => {
                            let a=byte_offset(&self.buffer,start);
                            let z=byte_offset(&self.buffer,end);
                            self.buffer.replace_range(a..z,"");
                            self.caret=start;self.anchor=start;
                            changed=true;
                        }
                        _=>{}
                    }
                }
                // Do not delete an extra grapheme after replacing a selection.
                changed |= match cmd {
                    EditCommand::Backspace|EditCommand::Delete if changed => false,
                    other => apply_edit(&mut self.buffer,&mut self.caret,other),
                };
                self.anchor=self.caret;
                if changed && !self.permits_grapheme_result(&old.buffer,&self.buffer){
                    self.restore(old);
                    return false; // Full rollback: caret, selection, Undo, Redo.
                }
                if changed {
                    Self::push(&mut self.undo,old);
                    self.redo.clear();
                    self.advance_authority_epoch();
                }
                changed
            }
        }
    }
    pub(crate) fn undo_len(&self) -> usize {self.undo.len()}
}

#[cfg(test)]
mod tests {

    #[test]fn retained_size_accounts_for_committed_text_and_undo_redo(){
        let mut s=EditSession::new("A".into());
        assert_eq!(s.retained_bytes(),1);
        assert!(s.apply(SessionAction::Edit(EditCommand::Insert("xy".into()))));
        assert!(s.retained_bytes()>=4,
            "live three-byte string plus previous one-byte snapshot");
        assert!(s.apply(SessionAction::Undo));
        assert!(s.retained_bytes()>=4,
            "Redo also counts toward retained memory budget");
        s.rebase_authority("host".into());
        assert_eq!(s.retained_bytes(),4,
            "host rebase must discard old undo/redo snapshots");
    }
    #[test]fn real_engine_d_then_c_replacement_is_one_undo_not_two_edits(){
        let mut s=EditSession::new("ANCHORㄱ".into());
        let first=s.authority_epoch();
        assert_eq!(s.replace_surrounding_if_snapshot(
            first,"ANCHORㄱ",7,7,-1,1,"가",101),Some(true));
        assert_eq!((s.buffer.as_str(),s.caret,s.anchor),("ANCHOR가",7,7));
        assert_eq!(s.undo_len(),1);
        assert_eq!(s.replace_surrounding_if_snapshot(
            first,"ANCHORㄱ",7,7,-1,1,"가",101),None);
        assert!(s.apply(SessionAction::Undo));
        assert_eq!(s.buffer,"ANCHORㄱ");
        assert_eq!(s.replace_surrounding_if_snapshot(
            first,"ANCHORㄱ",7,7,-1,1,"가",102),None,
            "an old host snapshot cannot be replayed after Undo restores its bytes");
        assert!(s.apply(SessionAction::Redo));
        assert_eq!(s.buffer,"ANCHOR가");
    }
    #[test]fn real_engine_replace_rejects_half_flag_zwj_and_combining(){
        for text in ["A🇮🇱Z","Ae\u{301}Z","A👨‍👩‍👦Z"]{
            let mut s=EditSession::new(text.into());
            let epoch=s.authority_epoch();
            let total=text.chars().count() as i32;
            assert_eq!(s.replace_surrounding_if_snapshot(
                epoch,text,total as u32,total as u32,-2,1,"x",1),None);
            assert_eq!(s.buffer,text);
            assert_eq!(s.undo_len(),0);
        }
    }
    #[test]fn real_engine_replace_rejects_rebase_selection_epoch_and_preedit(){
        let mut s=EditSession::new("ABㄱ".into());
        let epoch=s.authority_epoch();
        s.rebase_authority("ABㄱ".into());
        assert_eq!(s.rebase_generation(),1);
        assert_eq!(s.replace_surrounding_if_snapshot(
            epoch,"ABㄱ",3,3,-1,1,"가",1),None);
        let current=s.authority_epoch();
        assert!(s.set_visual_caret(1,true));
        assert_eq!(s.replace_surrounding_if_snapshot(
            current,"ABㄱ",3,3,-1,1,"가",1),None);
        assert!(s.start_composition(1));
        let cur=s.authority_epoch();
        assert_eq!(s.replace_surrounding_if_snapshot(
            cur,"ABㄱ",1,3,-1,1,"가",2),None);
        assert_eq!(s.undo_len(),0);
    }
    #[test]fn real_engine_replacement_checks_unicode_scalar_caret_not_graphemes(){
        let mut s=EditSession::new("🇮🇱Z".into());
        let epoch=s.authority_epoch();
        assert_eq!(s.replace_surrounding_if_snapshot(
            epoch,"🇮🇱Z",2,2,-1,1,"X",1),None);
        assert_eq!(s.replace_surrounding_if_snapshot(
            epoch,"🇮🇱Z",3,3,-1,1,"X",1),Some(true));
        assert_eq!(s.buffer,"🇮🇱X");
        assert_eq!(s.undo_len(),1);
    }

    #[test]fn gpu_visual_caret_roundtrip_cannot_reuse_stale_delete_snapshot(){
        let mut s=EditSession::new("ABCDE".into());
        let (old,original)=(s.authority_epoch(),s.buffer.clone());
        assert!(s.set_visual_caret(2,false));
        assert!(s.set_visual_caret(5,false));
        assert!(!s.delete_surrounding_if_snapshot(old,&original,5,5,-1,1),
            "same old caret after visual roundtrip does not revive old token");
        assert_eq!(s.buffer,"ABCDE");
        assert_eq!(s.undo_len(),0);
        assert!(s.set_visual_caret(3,true));
        assert!(!s.delete_surrounding_if_snapshot(s.authority_epoch(),&original,
            s.caret,s.anchor,-1,1),"selection must reject deletion");
        assert_eq!(s.buffer,"ABCDE");
    }
    #[test]fn native_delete_exact_epoch_is_single_atomic_undo(){
        let mut s=EditSession::new("A🇮🇱e\u{301}Z".into());
        let prior=s.buffer.clone();
        let (epoch,caret,anchor)=(s.authority_epoch(),s.caret,s.anchor);
        // A(1), flag(2), e+combining(2), Z(1): remove exactly
        // the two scalar-codepoint regional indicator flag as one grapheme.
        assert!(s.delete_surrounding_if_snapshot(epoch,&prior,caret,anchor,-5,2));
        assert_eq!(s.buffer,"Ae\u{301}Z");
        assert_eq!((s.caret,s.anchor),(3,3));
        assert_eq!(s.undo_len(),1);
        assert!(!s.delete_surrounding_if_snapshot(epoch,&prior,caret,anchor,-5,2),
            "same engine request replay cannot delete again");
        assert!(s.apply(SessionAction::Undo));
        assert_eq!(s.buffer,prior);
        assert_eq!(s.undo_len(),0);
        assert!(!s.delete_surrounding_if_snapshot(epoch,&prior,caret,anchor,-5,2),
            "undo restoring same bytes must NOT restore old authority token");
    }
    #[test]fn native_delete_never_splits_combining_zwj_or_flag_cluster(){
        for text in ["A🇮🇱Z","Ae\u{301}Z","A👨‍👩‍👦Z"] {
            let mut s=EditSession::new(text.into());
            let original=s.buffer.clone();
            let epoch=s.authority_epoch();
            let scalar_count=text.chars().count();
            // One internal scalar from the multi-scalar grapheme is forbidden.
            assert!(!s.delete_surrounding_if_snapshot(epoch,&original,
                s.caret,s.anchor,-(scalar_count as i32-2),1));
            assert_eq!(s.buffer,original);
            assert_eq!(s.undo_len(),0);
        }
    }
    #[test]fn native_delete_rejects_old_rebase_selected_composing_or_bad_ranges(){
        let mut s=EditSession::new("XY界Z".into());
        let snapshot=s.buffer.clone();
        let epoch=s.authority_epoch();
        assert!(!s.delete_surrounding_if_snapshot(epoch,&snapshot,s.caret,s.anchor,-500,3));
        assert!(!s.delete_surrounding_if_snapshot(epoch,&snapshot,s.caret,s.anchor,-1,0));
        assert!(!s.delete_surrounding_if_snapshot(epoch,&snapshot,s.caret,s.anchor,-1,1025));
        assert!(!s.delete_surrounding_if_snapshot(epoch,&snapshot,s.caret,s.anchor,1,1));
        assert_eq!(s.undo_len(),0);
        s.apply(SessionAction::SelectAll);
        assert!(!s.delete_surrounding_if_snapshot(epoch,&snapshot,4,4,-1,1));
        assert!(!s.delete_surrounding_if_snapshot(s.authority_epoch(),&snapshot,
            s.caret,s.anchor,-1,1),"selected region requires explicit policy");
        s.rebase_authority("XY界Z".into());
        assert!(!s.delete_surrounding_if_snapshot(epoch,&snapshot,4,4,-1,1),
            "identical text under higher host revision must reject stale request");
        let new_epoch=s.authority_epoch();
        assert!(s.start_composition(1));
        assert!(!s.delete_surrounding_if_snapshot(new_epoch,&snapshot,4,4,-1,1),
            "engine cannot delete authoritative text while composition active");
        assert_eq!(s.buffer,snapshot);
    }

    use super::*;

    fn edit(text: &str, index: usize, ops: Vec<EditCommand>) -> (String, usize, Vec<bool>) {
        let mut s=text.to_owned(); let mut caret=index;
        let diffs=ops.into_iter().map(|op| apply_edit(&mut s,&mut caret,op)).collect();
        (s,caret,diffs)
    }

    #[test]
    fn select_all_replace_graphemes_and_restore_undo_redo() {
        let mut s=EditSession::new("👩‍👩‍👧‍👦 A".into());
        assert!(!s.apply(SessionAction::SelectAll));
        assert_eq!(s.selection(),Some((0,3)));
        assert!(s.apply(SessionAction::Edit(EditCommand::Insert("B".into()))));
        assert_eq!((s.buffer.as_str(),s.caret,s.selection()),("B",1,None));
        assert!(s.apply(SessionAction::Undo));
        assert_eq!(s.buffer,"👩‍👩‍👧‍👦 A");
        assert_eq!(s.selection(),Some((0,3)));
        assert!(s.apply(SessionAction::Redo));
        assert_eq!(s.buffer,"B");
    }
    #[test]
    fn shift_left_selects_whole_zwj_and_backspace_erases_only_selection() {
        let mut s=EditSession::new("A👩‍👩‍👧‍👦".into());
        s.apply(SessionAction::Move(EditCommand::Left,true));
        assert_eq!(s.selection(),Some((1,2)));
        assert!(s.apply(SessionAction::Edit(EditCommand::Backspace)));
        assert_eq!(s.buffer,"A");
        assert_eq!(s.caret,1);
        assert!(s.apply(SessionAction::Undo));
        assert_eq!(s.buffer,"A👩‍👩‍👧‍👦");
    }
    #[test]
    fn left_and_right_collapse_existing_selection_without_mutating_text() {
        let mut s=EditSession::new("abcd".into());
        s.apply(SessionAction::Move(EditCommand::Left,true));
        s.apply(SessionAction::Move(EditCommand::Left,true));
        assert_eq!(s.selection(),Some((2,4)));
        assert!(!s.apply(SessionAction::Move(EditCommand::Left,false)));
        assert_eq!((s.caret,s.anchor),(2,2));
        s.apply(SessionAction::SelectAll);
        s.apply(SessionAction::Move(EditCommand::Right,false));
        assert_eq!((s.caret,s.anchor),(4,4));
        assert_eq!(s.undo_len(),0);
    }
    #[test]
    fn external_rebase_new_session_has_no_cross_text_history() {
        let mut s=EditSession::new("old".into());
        s.apply(SessionAction::Edit(EditCommand::Insert("!".into())));
        s=EditSession::new("server".into());
        assert_eq!(s.buffer,"server");
        assert!(!s.apply(SessionAction::Undo));
    }
    #[test]
    fn undo_is_bounded_and_new_edit_invalidates_redo() {
        let mut s=EditSession::new(String::new());
        for _ in 0..80 { assert!(s.apply(SessionAction::Edit(EditCommand::Insert("X".into())))); }
        assert_eq!(s.undo_len(),64);
        assert!(s.apply(SessionAction::Undo));
        assert!(s.apply(SessionAction::Edit(EditCommand::Insert("!".into()))));
        assert!(!s.apply(SessionAction::Redo));
    }
    #[test]
    fn selection_delete_is_one_undo_step_and_control_chars_are_noop() {
        let mut s=EditSession::new("abc".into());
        s.apply(SessionAction::SelectAll);
        assert!(!s.apply(SessionAction::Edit(EditCommand::Insert("\n".into()))));
        assert_eq!(s.buffer,"abc");
        assert!(s.apply(SessionAction::Edit(EditCommand::Delete)));
        assert_eq!(s.buffer,"");
        assert_eq!(s.undo_len(),1);
        assert!(s.apply(SessionAction::Undo));
        assert_eq!(s.buffer,"abc");
    }
    #[test]
    fn ascii_home_right_insert_then_end_backspace() {
        let r=edit("abc",3,vec![EditCommand::Home,EditCommand::Right,
            EditCommand::Insert("X".into()),EditCommand::End,EditCommand::Backspace]);
        assert_eq!(r,("aXb".into(),3,vec![false,false,true,false,true]));
    }

    #[test]
    fn unicode_scalars_never_split_utf8_and_do_not_confuse_bytes_with_caret() {
        let r=edit("aשלום🙂z",7,vec![EditCommand::Left,EditCommand::Backspace,
            EditCommand::Insert("界".into())]);
        assert_eq!(r.0,"aשלום界z");
        assert_eq!(r.1,6);
    }

    #[test]
    fn delete_removes_current_char_without_moving_cursor() {
        assert_eq!(edit("a🙂z",1,vec![EditCommand::Delete]),
            ("az".into(),1,vec![true]));
    }

    #[test]
    fn backspace_at_start_and_delete_at_end_are_noops() {
        assert_eq!(edit("abc",0,vec![EditCommand::Backspace]),
            ("abc".into(),0,vec![false]));
        assert_eq!(edit("abc",3,vec![EditCommand::Delete]),
            ("abc".into(),3,vec![false]));
    }

    #[test]
    fn navigation_reports_no_text_mutation() {
        let ops=vec![EditCommand::Left,EditCommand::Left,EditCommand::Home,
            EditCommand::Left,EditCommand::End,EditCommand::Right];
        assert_eq!(edit("abc",3,ops),("abc".into(),3,vec![false;6]));
    }

    #[test]
    fn unsafe_caret_is_clamped_to_available_scalars() {
        assert_eq!(edit("é🙂",usize::MAX,vec![EditCommand::Backspace]),
            ("é".into(),1,vec![true]));
    }

    #[test]
    fn control_chars_are_not_inserted_into_single_line_fields() {
        assert_eq!(edit("",0,vec![EditCommand::Insert("\n\r\t\0".into())]),
            ("".into(),0,vec![false]));
        assert_eq!(edit("a",1,vec![EditCommand::Insert("B\nC".into())]),
            ("aBC".into(),3,vec![true]));
    }

    #[test]
    fn inserted_hebrew_and_emoji_have_scalar_count_not_utf8_byte_length() {
        assert_eq!(edit("a",1,vec![EditCommand::Insert("שלום🙂".into())]),
            ("aשלום🙂".into(),6,vec![true]));
    }

    #[test]
    fn empty_and_multichar_edits_remain_deterministic() {
        let r=edit("",0,vec![EditCommand::Delete,EditCommand::Backspace,
            EditCommand::Insert("abc".into()),EditCommand::Left,EditCommand::Insert("Z".into())]);
        assert_eq!(r,("abZc".into(),3,vec![false,false,true,false,true]));
    }

    #[test]
    fn decomposed_accent_is_one_grapheme_for_backspace() {
        assert_eq!(grapheme_count("e\u{301}"),1);
        assert_eq!(edit("Ae\u{301}B",2,vec![EditCommand::Backspace]),
            ("AB".into(),1,vec![true]));
    }

    #[test]
    fn family_zwj_emoji_is_deleted_atomically() {
        let family="👩‍👩‍👧‍👦";
        assert_eq!(grapheme_count(family),1);
        assert_eq!(edit(&format!("A{family}B"),2,vec![EditCommand::Backspace]),
            ("AB".into(),1,vec![true]));
    }

    #[test]
    fn regional_indicator_flag_behaves_as_one_visual_character() {
        let flag="🇮🇱";
        assert_eq!(grapheme_count(flag),1);
        assert_eq!(edit(&format!("A{flag}B"),1,vec![EditCommand::Delete]),
            ("AB".into(),1,vec![true]));
    }

    #[test]
    fn emoji_variation_selector_and_skin_tone_stay_together() {
        assert_eq!(grapheme_count("✌️"),1);
        assert_eq!(grapheme_count("👍🏽"),1);
        assert_eq!(edit("👍🏽x",1,vec![EditCommand::Backspace]),
            ("x".into(),0,vec![true]));
    }

    #[test]
    fn accent_insert_merges_previous_cluster_and_does_not_drift_caret() {
        let r=edit("e",1,vec![EditCommand::Insert("\u{301}".into())]);
        assert_eq!(r,("e\u{301}".into(),1,vec![true]));
        let r=edit(&r.0,r.1,vec![EditCommand::Backspace]);
        assert_eq!(r,("".into(),0,vec![true]));
    }

    #[test]
    fn cluster_cursor_move_crosses_entire_family_in_single_step() {
        let family="👩‍👩‍👧‍👦";
        let x=format!("a{family}b");
        assert_eq!(edit(&x,3,vec![EditCommand::Left,EditCommand::Left]),
            (x,1,vec![false,false]));
    }

    #[test]
    fn emoji_insert_between_two_graphemes_and_delete_atomic() {
        let s="hi".to_string();
        let r=edit(&s,1,vec![EditCommand::Insert("👩‍👩‍👧‍👦".into())]);
        assert_eq!(r,("h👩‍👩‍👧‍👦i".into(),2,vec![true]));
        assert_eq!(edit(&r.0,r.1,vec![EditCommand::Backspace]),
            ("hi".into(),1,vec![true]));
    }

    #[test]
    fn erase_and_rewrite_handles_arbitrary_sequences_without_panicking() {
        let mut s="ב🐝X".to_string();let mut caret=3;
        for _ in 0..5 { apply_edit(&mut s,&mut caret,EditCommand::Backspace); }
        assert_eq!((s.clone(),caret),(String::new(),0));
        for _ in 0..10 { assert!(!apply_edit(&mut s,&mut caret,EditCommand::Delete)); }
        assert!(apply_edit(&mut s,&mut caret,EditCommand::Insert("Hi".into())));
        assert_eq!((s,caret),("Hi".into(),2));
    }

    #[test]fn ime_preedit_update_cancel_keeps_authoritative_and_undo_untouched(){
        let mut e=EditSession::new("Aשלום".into());
        e.apply(SessionAction::Move(EditCommand::Home,false));
        e.apply(SessionAction::Move(EditCommand::Right,false));
        assert!(e.start_composition(101));
        assert!(e.update_composition(102,"界に",1));
        let preview=e.projected().expect("temporary render projection");
        assert_eq!(preview.display,"A界にשלום");
        assert_eq!(preview.caret,2);
        assert_eq!(preview.preedit_graphemes,1..3);
        assert_eq!(e.buffer,"Aשלום");
        assert_eq!(e.undo_len(),0);
        assert!(!e.apply(SessionAction::Edit(EditCommand::Insert("x".into()))));
        assert!(!e.apply(SessionAction::Undo));
        assert!(!e.update_composition(102,"replay",1));
        assert!(!e.update_composition(103,"bad\nvalue",1));
        assert!(e.cancel_composition(104));
        assert!(e.projected().is_none());
        assert_eq!(e.buffer,"Aשלום");
        assert!(!e.apply(SessionAction::Undo));
    }
    #[test]fn ime_commit_is_one_authoritative_edit_and_one_undo_step(){
        let mut e=EditSession::new("ab".into());
        assert!(e.start_composition(11));
        assert!(e.update_composition(12,"k",1));
        assert!(e.update_composition(13,"か",1));
        assert_eq!(e.buffer,"ab");
        assert_eq!(e.commit_composition(14,"漢"),Some(true));
        assert_eq!(e.buffer,"ab漢");
        assert_eq!(e.undo_len(),1);
        assert_eq!(e.commit_composition(15,"duplicate"),None);
        assert_eq!(e.buffer,"ab漢");
        assert!(e.apply(SessionAction::Undo));
        assert_eq!(e.buffer,"ab");
        assert!(e.apply(SessionAction::Redo));
        assert_eq!(e.buffer,"ab漢");
        assert!(!e.start_composition(14),"old serial never starts a new composition");
        assert!(e.start_composition(16));
    }
    #[test]fn ime_selection_replacement_and_zwj_grapheme_anchor_is_stable(){
        let mut e=EditSession::new("B👩‍👩‍👧‍👦C".into());
        e.apply(SessionAction::SelectAll);
        assert!(e.start_composition(5));
        assert!(e.update_composition(6,"日",1));
        let p=e.projected().unwrap();
        assert_eq!(p.display,"日");
        assert_eq!(p.preedit_graphemes,0..1);
        assert_eq!(e.buffer,"B👩‍👩‍👧‍👦C");
        assert_eq!(e.selection(),Some((0,3)));
        assert_eq!(e.commit_composition(7,"שלום"),Some(true));
        assert_eq!(e.buffer,"שלום");
        assert_eq!(e.undo_len(),1);
        assert!(e.apply(SessionAction::Undo));
        assert_eq!(e.buffer,"B👩‍👩‍👧‍👦C");
    }
    #[test]fn newer_host_value_revision_cancels_preedit_without_reopening_old_serial(){
        let mut e=EditSession::new("original".into());
        assert!(e.start_composition(40));
        assert!(e.update_composition(41,"漢",1));
        assert!(e.projected().is_some());
        e.rebase_authority("server".into());
        assert!(e.projected().is_none());
        assert_eq!(e.buffer,"server");
        assert_eq!(e.undo_len(),0);
        assert!(!e.start_composition(41));
        assert!(!e.commit_composition(42,"stale").is_some());
        assert!(e.start_composition(43));
    }
    #[test]fn max_graphemes_atomic_unicode_clusters_and_selection_replacement(){
        let mut s=EditSession::new("a👩‍💻".into());
        s.set_max_graphemes(Some(2));
        assert!(!s.apply(SessionAction::Edit(EditCommand::Insert("🇮🇱".into()))));
        assert_eq!(s.buffer,"a👩‍💻");
        assert_eq!((s.caret,s.anchor,s.undo_len()),(2,2,0));
        s.apply(SessionAction::SelectAll);
        assert!(s.apply(SessionAction::Edit(EditCommand::Insert("e\u{301}🇮🇱".into()))));
        assert_eq!(s.buffer,"e\u{301}🇮🇱");
        assert_eq!(grapheme_count(&s.buffer),2);
        assert_eq!(s.undo_len(),1);
        assert!(s.apply(SessionAction::Undo));
        assert_eq!(s.buffer,"a👩‍💻");
    }
    #[test]fn combining_mark_merging_at_cap_is_not_rejected_by_scalar_count(){
        let mut s=EditSession::new("e".into());
        s.set_max_graphemes(Some(1));
        assert!(s.apply(SessionAction::Edit(EditCommand::Insert("\u{301}".into()))));
        assert_eq!(s.buffer,"e\u{301}");
        assert_eq!(grapheme_count(&s.buffer),1);
        assert!(!s.apply(SessionAction::Edit(EditCommand::Insert("x".into()))));
        assert_eq!(s.buffer,"e\u{301}");
        assert!(s.apply(SessionAction::Undo));
        assert_eq!(s.buffer,"e");
    }
    #[test]fn host_oversized_value_is_not_truncated_but_cannot_grow(){
        let mut s=EditSession::new("ABCD".into());
        s.set_max_graphemes(Some(2));
        assert_eq!(s.buffer,"ABCD","Host input stays byte-exact");
        assert!(!s.apply(SessionAction::Edit(EditCommand::Insert("x".into()))));
        assert!(s.apply(SessionAction::Edit(EditCommand::Backspace)));
        assert_eq!(s.buffer,"ABC");
        assert!(!s.apply(SessionAction::Undo),
            "undo would grow an already-over-limit buffer");
        assert_eq!(s.buffer,"ABC");
        assert!(s.apply(SessionAction::Edit(EditCommand::Backspace)));
        assert_eq!(s.buffer,"AB");
        assert!(!s.apply(SessionAction::Undo),
            "undo cannot raise a compliant buffer over the cap");
        s.rebase_authority("ServerOwned".into());
        s.set_max_graphemes(Some(2));
        assert_eq!(s.buffer,"ServerOwned");
        assert_eq!(s.undo_len(),0);
    }
    #[test]fn redo_over_limit_is_blocked_without_consuming_redo(){
        let mut s=EditSession::new("ab".into());
        assert!(s.apply(SessionAction::Edit(EditCommand::Insert("c".into()))));
        assert!(s.apply(SessionAction::Undo));
        assert_eq!(s.buffer,"ab");
        s.set_max_graphemes(Some(2));
        assert!(!s.apply(SessionAction::Redo));
        assert_eq!(s.buffer,"ab");
        s.set_max_graphemes(Some(3));
        assert!(s.apply(SessionAction::Redo),
            "previous rejected redo must remain available");
        assert_eq!(s.buffer,"abc");
    }
    #[test]fn ime_commit_enforces_cap_but_preedit_is_never_authoritative(){
        let mut s=EditSession::new("a".into());
        s.set_max_graphemes(Some(2));
        assert!(s.start_composition(1));
        assert!(s.update_composition(2,"👩‍💻🇮🇱",2));
        assert_eq!(s.buffer,"a");
        assert_eq!(s.commit_composition(3,"👩‍💻🇮🇱"),Some(false));
        assert_eq!(s.buffer,"a");
        assert_eq!(s.undo_len(),0);
        assert!(s.start_composition(4));
        assert!(s.update_composition(5,"👩‍💻",1));
        assert_eq!(s.commit_composition(6,"👩‍💻"),Some(true));
        assert_eq!(s.buffer,"a👩‍💻");
        assert_eq!(s.undo_len(),1);
        assert!(s.apply(SessionAction::Undo));
        assert_eq!(s.buffer,"a");
    }
    #[test]fn ibus_atomic_surrounding_replacement_obeys_grapheme_capacity(){
        let mut s=EditSession::new("abcd".into());
        s.set_max_graphemes(Some(4));
        let epoch=s.authority_epoch();
        assert_eq!(s.replace_surrounding_if_snapshot(
            epoch,"abcd",4,4,-1,1,"🙂🙂",1),None);
        assert_eq!(s.buffer,"abcd");
        assert_eq!(s.undo_len(),0);
        assert_eq!(s.replace_surrounding_if_snapshot(
            epoch,"abcd",4,4,-1,1,"🇮🇱",2),Some(true));
        assert_eq!(s.buffer,"abc🇮🇱");
        assert_eq!(grapheme_count(&s.buffer),4);
        assert_eq!(s.undo_len(),1);
        assert!(s.apply(SessionAction::Undo));
        assert_eq!(s.buffer,"abcd");
    }
    #[test]fn zero_cap_disallows_growth_but_still_allows_deletion(){
        let mut s=EditSession::new("A".into());
        s.set_max_graphemes(Some(0));
        assert!(!s.apply(SessionAction::Edit(EditCommand::Insert("z".into()))));
        assert!(s.apply(SessionAction::Edit(EditCommand::Backspace)));
        assert_eq!(s.buffer,"");
        assert!(!s.apply(SessionAction::Edit(EditCommand::Insert("z".into()))));
    }
    #[test]fn ime_limits_invalid_cursor_and_controls_rejected_without_sequence_advance(){
        let mut e=EditSession::new(String::new());
        assert!(!e.start_composition(0));
        assert!(e.start_composition(1));
        assert!(!e.update_composition(2,&"X".repeat(MAX_PREEDIT_BYTES+1),0));
        assert!(!e.update_composition(2,"e\u{301}",2));
        assert!(!e.update_composition(2,"evil\u{202E}",1));
        assert!(e.update_composition(2,"e\u{301}",1));
        assert_eq!(e.projected().unwrap().preedit_graphemes,0..1);
        assert_eq!(e.commit_composition(3,""),Some(false));
        assert_eq!(e.buffer,"");
        assert_eq!(e.undo_len(),0);
    }
}
