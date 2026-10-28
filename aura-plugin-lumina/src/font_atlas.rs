//! Bounded, renderer-independent Unicode atlas planning. Caller explicitly
//! provides a .ttf/.otf font_path per Text/TextInput; NO font asset is bundled.
//! Raylib font atlas is rasterized in native GL thread after InitWindow.
//! This does not shape scripts or apply bidirectional reordering.
use std::collections::BTreeSet;
pub(crate) const MAX_FONTS:usize=8;
pub(crate) const MAX_CODEPOINTS:usize=1024;
pub(crate) const BASE_ASCII:&str=" !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
pub(crate) fn font_key(path:&str,size:i32)->Option<String> {
    if path.is_empty() || path.len()>512 || path.contains('\0') ||
       !(path.to_ascii_lowercase().ends_with(".ttf") ||
         path.to_ascii_lowercase().ends_with(".otf")) {return None;}
    if !(10..=96).contains(&size) {return None;}
    Some(format!("{size}|{path}"))
}
pub(crate) fn glyph_request(existing:&str,texts:&[&str])->Option<String> {
    let mut set:BTreeSet<char>=existing.chars().chain(BASE_ASCII.chars()).collect();
    if set.len()>MAX_CODEPOINTS {return None;}
    for txt in texts {
        for c in txt.chars() {
            if c.is_control() {continue;}
            set.insert(c);
            if set.len()>MAX_CODEPOINTS {return None;}
        }
    }
    Some(set.into_iter().collect())
}
pub(crate) fn covers(existing:&str,text:&str)->bool {
    text.chars().filter(|c|!c.is_control()).all(|c|existing.contains(c))
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test]fn rejects_invalid_paths_and_font_sizes() {
    assert_eq!(font_key("",24),None);
    assert_eq!(font_key("font.svg",24),None);
    assert_eq!(font_key("bad\0font.ttf",24),None);
    assert_eq!(font_key("x.ttf",0),None);
    assert_eq!(font_key("x.ttf",500),None);
    assert_eq!(font_key(&format!("{}.ttf","a".repeat(513)),24),None);
    assert_eq!(font_key("assets/locale.otf",24),Some("24|assets/locale.otf".into()));
 }
 #[test]fn preserves_real_hebrew_and_ascii_digits_in_unicode_atlas(){
    let a=glyph_request("",&["שלום","אבג 123"]).unwrap();
    assert!(covers(&a,"שלום אבג 123"));
    assert!(covers(&a,"ABC abc"));
 }
 #[test]fn new_dynamic_codepoints_extend_without_deleting_previous_ones(){
    let a=glyph_request("",&["abc שלום"]).unwrap();
    let b=glyph_request(&a,&["界🙂"]).unwrap();
    assert!(covers(&b,"abc שלום 界🙂"));
 }
 #[test]fn control_chars_are_skipped_and_duplicate_chars_deduplicated(){
    let a=glyph_request("",&["aaa\n\tשלום שלום"]).unwrap();
    assert!(!a.contains('\n'));
    assert!(!a.contains('\t'));
    assert_eq!(a.chars().filter(|c|*c=='ש').count(),1);
 }
 #[test]fn huge_distinct_unicode_payload_refuses_incomplete_font_atlas(){
    let payload:String=(0..1030).filter_map(|i|char::from_u32(0x600+i)).collect();
    assert!(glyph_request("",&[&payload]).is_none());
 }
 #[test]fn incomplete_atlas_is_never_reported_as_covering_unknown_character(){
    let a=glyph_request("",&["שלום"]).unwrap();
    assert!(!covers(&a,"שלום界"));
    assert!(covers(&a,"שלום"));
 }
 #[test]fn deterministic_charset_order_does_not_depend_on_input_order(){
    assert_eq!(glyph_request("",&["abc","שלום"]),glyph_request("",&["שלום","abc"]));
 }
}
