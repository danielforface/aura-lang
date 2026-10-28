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

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(text: &str, index: usize, ops: Vec<EditCommand>) -> (String, usize, Vec<bool>) {
        let mut s=text.to_owned(); let mut caret=index;
        let diffs=ops.into_iter().map(|op| apply_edit(&mut s,&mut caret,op)).collect();
        (s,caret,diffs)
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
}
