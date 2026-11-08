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

#[cfg(test)]
mod tests{
    use super::*;
    fn field(props:&[(&str,&str)])->UiNode{
        let mut n=UiNode::new("TextInput");
        for (k,v) in props {n.set_prop(*k,*v);}
        n
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
