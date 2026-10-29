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
            preedit:None, last_composition_sequence:0 }
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
        true
    }
    pub(crate) fn update_composition(&mut self,sequence:u64,text:&str,caret:usize)->bool {
        if sequence<=self.last_composition_sequence || sequence==0 ||
            self.preedit.is_none() || !valid_preedit(text) ||
            caret>grapheme_count(text){return false;}
        self.last_composition_sequence=sequence;
        self.preedit=Some(Preedit{text:text.to_owned(),caret});
        true
    }
    /// Exactly one committed Edit action: one Undo snapshot, one on_change.
    pub(crate) fn commit_composition(&mut self,sequence:u64,text:&str)->Option<bool> {
        if sequence==0 || sequence<=self.last_composition_sequence ||
           self.preedit.is_none() || !valid_preedit(text){return None;}
        self.last_composition_sequence=sequence;
        self.preedit=None;
        Some(if text.is_empty(){false}else{
            self.apply(SessionAction::Edit(EditCommand::Insert(text.to_owned())))
        })
    }
    pub(crate) fn cancel_composition(&mut self,sequence:u64)->bool {
        if sequence==0 || sequence<=self.last_composition_sequence || self.preedit.is_none(){return false;}
        self.last_composition_sequence=sequence;
        self.preedit=None;
        true
    }
    pub(crate) fn abandon_composition(&mut self) {self.preedit=None;}
    pub(crate) fn composing(&self)->bool{self.preedit.is_some()}
    /// A new authoritative value revision clears transient composition and
    /// undo, yet must not reset the adapter replay barrier for this widget.
    pub(crate) fn rebase_authority(&mut self,text:String){
        let last=self.last_composition_sequence;
        *self=Self::new(text);
        self.last_composition_sequence=last;
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
            SessionAction::SelectAll => {self.anchor=0;self.caret=len;false}
            SessionAction::Move(cmd,extend) => {
                if !extend {
                    if let Some((start,end))=self.selection() {
                        match cmd {
                            EditCommand::Left=>{self.caret=start;self.anchor=start;return false;}
                            EditCommand::Right=>{self.caret=end;self.anchor=end;return false;}
                            _=>{}
                        }
                    }
                }
                let old=self.caret;
                // Navigation only: never create undo entries or emit on_change.
                let _=apply_edit(&mut self.buffer,&mut self.caret,cmd);
                if !extend {self.anchor=self.caret;}
                if self.caret==old && !extend {self.anchor=self.caret;}
                false
            }
            SessionAction::Undo => {
                if let Some(previous)=self.undo.pop() {
                    let now=self.snapshot();
                    Self::push(&mut self.redo,now);
                    self.restore(previous);return true;
                }
                false
            }
            SessionAction::Redo => {
                if let Some(next)=self.redo.pop() {
                    let now=self.snapshot();
                    Self::push(&mut self.undo,now);
                    self.restore(next);return true;
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
                if changed {
                    Self::push(&mut self.undo,old);
                    self.redo.clear();
                }
                changed
            }
        }
    }
    pub(crate) fn undo_len(&self) -> usize {self.undo.len()}
}

#[cfg(test)]
mod tests {
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
