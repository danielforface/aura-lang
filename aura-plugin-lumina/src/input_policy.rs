//! Opt-in native read-only TextInput contract.
//!
//! Editable by default. Explicit read_only="true" keeps text selectable,
//! focusable and eligible for Host form validation, but forbids native edits,
//! submission callbacks and IME preedit/commit/replace transactions.
//! Malformed/duplicate policy props fail closed (no "first value wins").
use aura_nexus::UiNode;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum EditMode { Editable, ReadOnly, Invalid }
impl EditMode {
    pub(crate) fn editable(self)->bool{self==Self::Editable}
    pub(crate) fn locked(self)->bool{!self.editable()}
}
pub(crate) fn edit_mode(node:&UiNode)->EditMode{
    if node.kind!="TextInput"{return EditMode::Invalid;}
    let mut tokens=node.props.iter().filter(|(key,_)|key=="read_only");
    match (tokens.next(),tokens.next()){
        (None,_)=>EditMode::Editable,
        (Some((_,v)),None) if v=="false"=>EditMode::Editable,
        (Some((_,v)),None) if v=="true"=>EditMode::ReadOnly,
        _=>EditMode::Invalid,
    }
}

/// Maximum number of UAX29 grapheme clusters accepted by local mutations.
/// Absent means existing unlimited editor behavior. The form validator uses
/// the same 0..=4096 syntax; invalid/duplicated host policy fails closed.
pub(crate) fn grapheme_limit(node:&UiNode)->Result<Option<usize>,()>{
    if node.kind!="TextInput"{return Err(());}
    let mut props=node.props.iter().filter(|(key,_)|key=="max_graphemes");
    let first=props.next();
    if props.next().is_some(){return Err(());}
    let Some((_,value))=first else{return Ok(None);};
    if value.is_empty() || value.len()>4 ||
        !value.bytes().all(|b|b.is_ascii_digit()){return Err(());}
    let value=value.parse::<usize>().map_err(|_|())?;
    if value>4096{return Err(());}
    Ok(Some(value))
}

/// Native TextInput paint privacy boundary. Any explicit sensitive flag,
/// unknown input type, unsupported surrounding-text policy or *ambiguous*
/// duplicate declaration must never be rendered as clear text. This is a
/// display-only decision: the Host's value/event contract stays unchanged.
pub(crate) fn private_display(node:&UiNode)->bool {
    if node.kind!="TextInput"{return false;}
    for name in ["sensitive","type","input_type","surrounding_text"] {
        let mut props=node.props.iter().filter(|(key,_)|key==name);
        let value=props.next().map(|(_,v)|v.as_str());
        if props.next().is_some(){return true;}
        match name {
            "sensitive" if !matches!(value,None|Some("false")|Some("0"))=>
                return true,
            "type"|"input_type" if value.is_some_and(|kind|
                !matches!(kind,"text"|"search"|"email"|"number"|"url"|"tel"))=>
                return true,
            "surrounding_text" if !matches!(value,None|Some("full"))=>
                return true,
            _=>{},
        }
    }
    false
}

/// Use one plain ASCII asterisk per UAX29 extended grapheme, never per byte,
/// scalar or UTF-16 code unit. This string alone is allowed to enter glyph
/// caches, text shaping, font measurements and native drawing for private
/// fields. Oversized Host values remain secret; display length is capped.
/// This is visual redaction, NOT encrypted storage or a secure input buffer.
pub(crate) const MAX_MASK_GLYPHS:usize=8192;
pub(crate) fn masked_display(value:&str)->String {
    use unicode_segmentation::UnicodeSegmentation;
    value.graphemes(true).take(MAX_MASK_GLYPHS).map(|_|'*').collect()
}

/// Native OS clipboard is an explicit, per-field capability, not implied
/// by selection support. A read-only editor may export a selection but may
/// never cut/paste. Unknown, duplicated or conflicting Host policy fails closed.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum ClipboardAccess { Disabled, CopyOnly, ReadWrite }
impl ClipboardAccess {
    pub(crate) fn can_copy(self)->bool{self!=Self::Disabled}
    pub(crate) fn can_write(self)->bool{self==Self::ReadWrite}
}
pub(crate) fn clipboard_access(node:&UiNode)->Result<ClipboardAccess,()> {
    if node.kind!="TextInput"{return Err(());}
    fn unique<'a>(node:&'a UiNode,key:&str)->Result<Option<&'a str>,()>{
        let mut props=node.props.iter().filter(|(k,_)|k==key);
        let value=props.next().map(|(_,v)|v.as_str());
        if props.next().is_some(){return Err(());}
        Ok(value)
    }
    let requested=match unique(node,"clipboard")?{
        None|Some("false")=>ClipboardAccess::Disabled,
        Some("true")=>ClipboardAccess::ReadWrite,
        Some("copy")=>ClipboardAccess::CopyOnly,
        _=>return Err(()),
    };
    if requested==ClipboardAccess::Disabled{return Ok(requested);}
    let sensitive=unique(node,"sensitive")?;
    let surrounding=unique(node,"surrounding_text")?;
    let explicit_type=unique(node,"type")?;
    let input_type=unique(node,"input_type")?;
    let edit=edit_mode(node);
    let valid_type=|v:Option<&str>|v.is_none_or(|v|matches!(
        v,"text"|"search"|"email"|"number"|"url"|"tel"
    ));
    if edit==EditMode::Invalid || grapheme_limit(node).is_err()
        || !valid_type(explicit_type) || !valid_type(input_type)
        || !matches!(sensitive,None|Some("false")|Some("0"))
        || !matches!(surrounding,None|Some("full")) {
        return Ok(ClipboardAccess::Disabled);
    }
    if edit==EditMode::ReadOnly || requested==ClipboardAccess::CopyOnly {
        Ok(ClipboardAccess::CopyOnly)
    }else{
        Ok(ClipboardAccess::ReadWrite)
    }
}
/// Untrusted clipboard bytes must not introduce multi-line text, embedded
/// controls, nulls or Unicode bidi formatting controls into one-line fields.
/// Whole-transaction rejection avoids silent truncation and forged display.
pub(crate) const MAX_NATIVE_CLIPBOARD_BYTES:usize=4096;
pub(crate) fn safe_clipboard_text(value:&str)->bool{
    !value.is_empty() && value.len()<=MAX_NATIVE_CLIPBOARD_BYTES
        && !value.chars().any(|ch|ch.is_control() ||
            matches!(ch,'\u{202A}'..='\u{202E}'|'\u{2066}'..='\u{2069}'))
}

/// A separate explicit pointer-selection grant, not a clipboard grant.
/// Invalid, duplicated or private text-field properties fail closed.
pub(crate) fn pointer_selection_enabled(node:&UiNode)->Result<bool,()> {
    if node.kind!="TextInput"{return Err(());}
    fn unique<'a>(node:&'a UiNode,key:&str)->Result<Option<&'a str>,()>{
        let mut props=node.props.iter().filter(|(k,_)|k==key);
        let result=props.next().map(|(_,v)|v.as_str());
        if props.next().is_some(){return Err(());}
        Ok(result)
    }
    let enabled=match unique(node,"pointer_selection")?{
        None|Some("false")=>false,
        Some("true")=>true,
        _=>return Err(()),
    };
    if !enabled{return Ok(false);}
    let allowable=|v:Option<&str>|v.is_none_or(|v|
        matches!(v,"text"|"search"|"email"|"number"|"url"|"tel"));
    Ok(edit_mode(node)!=EditMode::Invalid && grapheme_limit(node).is_ok()
        && allowable(unique(node,"type")?)
        && allowable(unique(node,"input_type")?)
        && matches!(unique(node,"sensitive")?,None|Some("false")|Some("0"))
        && matches!(unique(node,"surrounding_text")?,None|Some("full")))
}

#[cfg(test)]
mod tests{
    use super::*;
    fn field(props:&[(&str,&str)])->UiNode{
        let mut n=UiNode::new("TextInput");
        for (k,v) in props {n.set_prop(*k,*v);}
        n
    }
    #[test]
    fn native_private_display_masks_password_otp_sensitive_and_uncertain_policy(){
        for (key,value) in [
            ("type","password"),("input_type","password"),
            ("type","otp"),("input_type","otp"),("sensitive","true"),
            ("sensitive","1"),("sensitive","broken"),
            ("type","PASSWORD"),("input_type","mystery"),
            ("surrounding_text","none"),("surrounding_text","untrusted"),
        ] {
            assert!(private_display(&field(&[(key,value)])),
                "{key}={value} cannot render raw input");
        }
        for props in [
            vec![],vec![("type","text")],vec![("input_type","email")],
            vec![("sensitive","false")],vec![("sensitive","0")],
            vec![("surrounding_text","full")],
        ] {
            assert!(!private_display(&field(&props)), "ordinary field remains visible");
        }
        for key in ["type","input_type","sensitive","surrounding_text"] {
            let mut duplicate=field(&[(key,"text")]);
            duplicate.props.push((key.into(),"text".into()));
            assert!(private_display(&duplicate),"duplicate {key} must never show raw");
        }
    }
    #[test]
    fn mask_is_bounded_by_graphemes_not_utf8_or_scalar_count(){
        assert_eq!(masked_display(""),"");
        assert_eq!(masked_display("שלום"),"****");
        assert_eq!(masked_display("🇮🇱é👩‍💻"),"***");
        assert_eq!(masked_display("visible private"),"*".repeat(15));
        assert_eq!(masked_display(&"x".repeat(MAX_MASK_GLYPHS+100)).len(),MAX_MASK_GLYPHS);
        assert!(!masked_display("סודי👩‍💻").contains('ס'));
    }
    #[test]
    fn pointer_grant_requires_explicit_safe_unambiguous_field(){
        assert_eq!(pointer_selection_enabled(&field(&[])),Ok(false));
        assert_eq!(pointer_selection_enabled(&field(&[("pointer_selection","true")])),Ok(true));
        assert_eq!(pointer_selection_enabled(&field(&[("pointer_selection","false")])),Ok(false));
        assert_eq!(pointer_selection_enabled(&field(&[("pointer_selection","true"),
            ("read_only","true")])),Ok(true));
        for (key,v) in [("sensitive","true"),("type","password"),
            ("input_type","otp"),("surrounding_text","none"),
            ("read_only","invalid"),("max_graphemes","5000")] {
            assert_eq!(pointer_selection_enabled(&field(&[("pointer_selection","true"),
                (key,v)])),Ok(false),"unsafe {key}");
        }
        for v in ["","TRUE","1","copy"," true","true "]{
            assert_eq!(pointer_selection_enabled(&field(&[("pointer_selection",v)])),Err(()));
        }
        let mut duplicate=field(&[("pointer_selection","true")]);
        duplicate.props.push(("pointer_selection".into(),"false".into()));
        assert_eq!(pointer_selection_enabled(&duplicate),Err(()));
        let mut duplicated_sensitive=field(&[("pointer_selection","true"),("sensitive","false")]);
        duplicated_sensitive.props.push(("sensitive".into(),"false".into()));
        assert_eq!(pointer_selection_enabled(&duplicated_sensitive),Err(()));
    }
    #[test]
    fn clipboard_requires_explicit_safe_opt_in_and_keeps_readonly_copy_only(){
        assert_eq!(clipboard_access(&field(&[])),Ok(ClipboardAccess::Disabled));
        assert_eq!(clipboard_access(&field(&[("clipboard","true")])),
            Ok(ClipboardAccess::ReadWrite));
        assert_eq!(clipboard_access(&field(&[("clipboard","copy")])),
            Ok(ClipboardAccess::CopyOnly));
        assert_eq!(clipboard_access(&field(&[("clipboard","false")])),
            Ok(ClipboardAccess::Disabled));
        assert_eq!(clipboard_access(&field(&[("clipboard","true"),("read_only","true")])),
            Ok(ClipboardAccess::CopyOnly));
        for (key,value) in [("sensitive","true"),("type","password"),
            ("input_type","otp"),("surrounding_text","none"),
            ("read_only","broken"),("max_graphemes","4097")] {
            assert_eq!(clipboard_access(&field(&[("clipboard","true"),(key,value)])),
                Ok(ClipboardAccess::Disabled),"{key} must deny OS clipboard");
        }
        for malformed in ["", "TRUE", "1", " true", "paste", "false "]{
            assert_eq!(clipboard_access(&field(&[("clipboard",malformed)])),Err(()));
        }
        let mut duplicate=field(&[("clipboard","true")]);
        duplicate.props.push(("clipboard".into(),"false".into()));
        assert_eq!(clipboard_access(&duplicate),Err(()));
        let mut duplicate_sensitive=field(&[("clipboard","true"),("sensitive","false")]);
        duplicate_sensitive.props.push(("sensitive".into(),"false".into()));
        assert_eq!(clipboard_access(&duplicate_sensitive),Err(()));
    }
    #[test]
    fn clipboard_bytes_are_bounded_and_bidi_controls_and_multiline_rejected(){
        for ok in ["שלום 👩‍💻", "🇮🇱e\u{301}","0"] {
            assert!(safe_clipboard_text(ok));
        }
        for bad in ["","hello\nworld","x\0y","x\u{202e}","x\u{2066}"]{
            assert!(!safe_clipboard_text(bad));
        }
        assert!(safe_clipboard_text(&"a".repeat(MAX_NATIVE_CLIPBOARD_BYTES)));
        assert!(!safe_clipboard_text(&"a".repeat(MAX_NATIVE_CLIPBOARD_BYTES+1)));
    }
    #[test]
    fn absent_and_false_are_editable_and_true_blocks_every_edit_path(){
        assert_eq!(edit_mode(&field(&[])),EditMode::Editable);
        assert_eq!(edit_mode(&field(&[("read_only","false")])),EditMode::Editable);
        assert_eq!(edit_mode(&field(&[("read_only","true")])),EditMode::ReadOnly);
        assert!(!edit_mode(&field(&[("read_only","true")])).editable());
    }
    #[test]
    fn invalid_policy_values_fail_closed_and_do_not_silently_restore_editing(){
        for v in ["","True","0","1","TRUE","yes"," false","false ",
            "read-only","\n","unknown","false\u{202E}"] {
            assert_eq!(edit_mode(&field(&[("read_only",v)])),
                EditMode::Invalid, "malformed value {v:?}");
        }
        assert_eq!(edit_mode(&UiNode::new("Button")),EditMode::Invalid);
    }
    #[test]
    fn duplicated_policy_never_chooses_first_value(){
        let mut n=field(&[("read_only","false")]);
        n.props.push(("read_only".into(),"true".into()));
        assert_eq!(edit_mode(&n),EditMode::Invalid);
        n.props.reverse();
        assert_eq!(edit_mode(&n),EditMode::Invalid);
        let mut n=field(&[("read_only","false")]);
        n.props.push(("read_only".into(),"false".into()));
        assert_eq!(edit_mode(&n),EditMode::Invalid);
    }
    #[test]
    fn max_graphemes_parser_matches_form_constraints_and_rejects_duplicates(){
        let mut field=field(&[]);
        assert_eq!(grapheme_limit(&field),Ok(None));
        for (v,n) in [("0",0),("2",2),("4096",4096),("002",2)]{
            field.set_prop("max_graphemes",v);
            assert_eq!(grapheme_limit(&field),Ok(Some(n)));
        }
        for value in ["","-1","4097","5000","99999","2 "," 2","2.0","٢","①","\n"]{
            field.set_prop("max_graphemes",value);
            assert_eq!(grapheme_limit(&field),Err(()),"malformed={value:?}");
        }
        field.set_prop("max_graphemes","2");
        field.props.push(("max_graphemes".into(),"3".into()));
        assert_eq!(grapheme_limit(&field),Err(()));
        field.props.reverse();
        assert_eq!(grapheme_limit(&field),Err(()));
    }
    #[test]
    fn dynamic_host_transition_is_pure_and_has_no_hidden_native_cache(){
        let mut node=field(&[("read_only","true")]);
        assert_eq!(edit_mode(&node),EditMode::ReadOnly);
        node.set_prop("read_only","false");
        assert_eq!(edit_mode(&node),EditMode::Editable);
        node.set_prop("read_only","broken");
        assert_eq!(edit_mode(&node),EditMode::Invalid);
        node.set_prop("read_only","true");
        assert_eq!(edit_mode(&node),EditMode::ReadOnly);
    }
}
