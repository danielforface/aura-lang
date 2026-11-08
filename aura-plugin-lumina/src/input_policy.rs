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

#[cfg(test)]
mod tests{
    use super::*;
    fn field(props:&[(&str,&str)])->UiNode{
        let mut n=UiNode::new("TextInput");
        for (k,v) in props {n.set_prop(k,v);}
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
