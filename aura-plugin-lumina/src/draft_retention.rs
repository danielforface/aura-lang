//! Native-only opt-in retained TextInput identities across temporarily
//! unmounted route subtrees. This is NOT host persistence, user auth, secure
//! storage, or a replacement for explicit controlled value_revision rebases.
use std::collections::{HashMap,HashSet};
use aura_nexus::UiNode;
use crate::layout::HitTarget;

pub(crate) const MAX_DRAFTS:usize=8;
pub(crate) const MAX_DRAFT_BYTES:usize=128*1024;
pub(crate) const MAX_ENTRY_BYTES:usize=32*1024;
/// Dynamic payload accounting is conservative; no unbounded cache insertion.
pub(crate) fn within_budget(bytes:usize,entries:usize,already:usize)->bool{
    bytes<=MAX_ENTRY_BYTES && entries<MAX_DRAFTS
        && already<=MAX_DRAFT_BYTES
        && bytes<=MAX_DRAFT_BYTES-already
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct Identity{
    pub(crate) scope:String,
    pub(crate) key:String,
    pub(crate) form:Option<String>,
    pub(crate) type_kind:String,
    pub(crate) revision:u64,
}
impl Identity{
    /// Host revisions are monotonic; a higher revision is a mandatory rebase,
    /// but it is still the same logical field identity.
    pub(crate) fn same_field(&self,other:&Self)->bool{
        self.scope==other.scope && self.key==other.key
            && self.form==other.form
            && self.type_kind==other.type_kind
    }
}
#[derive(Default)]
pub(crate) struct Catalog{
    pub(crate) active:HashMap<u64,Identity>,
    /// Even hidden/disabled/malformed fields are present and MUST NOT be
    /// silently treated as an unmounted route with retained private data.
    pub(crate) present:HashSet<u64>,
}
fn prop<'a>(n:&'a UiNode,k:&str)->Option<&'a str>{
    n.props.iter().find(|(key,_)|key==k).map(|(_,v)|v.as_str())
}
fn valid_key(key:&str)->bool {
    !key.is_empty() && key.len()<=128 && key.trim()==key &&
        !key.chars().any(|c|c.is_control()||
            matches!(c,'\u{202A}'..='\u{202E}'|'\u{2066}'..='\u{2069}'))
}
fn canonical_revision(raw:&str)->Option<u64>{
    if raw.is_empty()||raw.len()>20||!raw.bytes().all(|b|b.is_ascii_digit()){
        return None;
    }
    let value=raw.parse::<u64>().ok()?;
    (raw==value.to_string()).then_some(value)
}
fn callback(node:&UiNode)->Option<u64>{
    let raw=prop(node,"on_change")?.strip_prefix("cb:")?;
    if raw.is_empty()||raw.len()>20||!raw.bytes().all(|c|c.is_ascii_digit()){
        return None;
    }
    raw.parse().ok()
}
/// Opt-in is explicit. Password/secret/unknown text field kinds and disabled
/// surrounding-text policy cannot be retained in long-lived native memory.
pub(crate) fn identity(node:&UiNode)->Option<Identity>{
    if node.kind!="TextInput" || prop(node,"retain_draft")!=Some("route"){return None;}
    let scope=prop(node,"draft_scope").filter(|s|valid_key(s))?;
    let key=prop(node,"draft_key").filter(|s|valid_key(s))?;
    let revision=canonical_revision(prop(node,"value_revision")?)?;
    if prop(node,"sensitive").is_some_and(|v|!matches!(v,"false"|"0")){
        return None;
    }
    if prop(node,"surrounding_text").is_some_and(|v|v!="full"){
        return None;
    }
    let type_kind=prop(node,"type").or_else(||prop(node,"input_type"))
        .unwrap_or("text");
    if !matches!(type_kind,"text"|"search"|"email"|"number"|"url"|"tel"){
        return None;
    }
    if prop(node,"type").zip(prop(node,"input_type"))
        .is_some_and(|(a,b)|a!=b){return None;}
    let form=match prop(node,"form"){
        Some(value) if valid_key(value)=>Some(value.to_owned()),
        Some(_)=>return None,
        None=>None,
    };
    Some(Identity{scope:scope.to_owned(),key:key.to_owned(),form,
        type_kind:type_kind.to_owned(),revision})
}

/// Source-tree identity and layout eligibility are independent. A duplicate
/// key anywhere in the tree (including hidden branches) is ambiguous and
/// blocks retention rather than guessing which editor owns stored content.
pub(crate) fn catalog(root:&UiNode,order:&[HitTarget])->Catalog{
    let allowed:HashSet<u64>=order.iter().filter_map(|hit|match hit{
        HitTarget::TextInput{on_change,..}=>Some(*on_change),
        _=>None,
    }).collect();
    let mut by_id:HashMap<u64,(Identity,usize)>=HashMap::new();
    let mut key_counts:HashMap<(String,String,Option<String>),usize>=HashMap::new();
    let mut callback_counts:HashMap<u64,usize>=HashMap::new();
    let mut result=Catalog::default();
    fn scan(node:&UiNode,by_id:&mut HashMap<u64,(Identity,usize)>,
        counts:&mut HashMap<(String,String,Option<String>),usize>,
        callbacks:&mut HashMap<u64,usize>,present:&mut HashSet<u64>){
        if node.kind=="TextInput"{
            if let Some(id)=callback(node){
                present.insert(id);
                *callbacks.entry(id).or_default()+=1;
            }
            // Even a duplicate with a broken callback still occupies the
            // logical key and prevents the other input from retaining it.
            if let Some(identity)=identity(node){
                *counts.entry((identity.scope.clone(),
                    identity.key.clone(),identity.form.clone())).or_default()+=1;
                if let Some(id)=callback(node){
                    by_id.entry(id).and_modify(|(_,n)|*n+=1)
                        .or_insert((identity,1));
                }
            }
        }
        for child in &node.children{scan(child,by_id,counts,callbacks,present);}
    }
    scan(root,&mut by_id,&mut key_counts,&mut callback_counts,&mut result.present);
    for (id,(value,multiplicity)) in by_id{
        if allowed.contains(&id) && multiplicity==1
            && callback_counts.get(&id)==Some(&1)
            && key_counts.get(&(value.scope.clone(),value.key.clone(),
                value.form.clone()))==Some(&1){
            result.active.insert(id,value);
        }
    }
    result
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::layout::{self,Bounds};
    fn n(kind:&str,fields:&[(&str,&str)],children:Vec<UiNode>)->UiNode{
        let mut out=UiNode::new(kind);
        for (k,v) in fields{out.set_prop(k,v);}
        out.children=children;out
    }
    fn field(fields:&[(&str,&str)])->UiNode{
        let mut node=n("TextInput",&[
            ("on_change","cb:10"),("x","20"),("y","20"),
            ("retain_draft","route"),("draft_scope","window-session-1"),
            ("draft_key","registration.email"),
            ("value_revision","1"),("form","registration"),
        ],vec![]);
        for (k,v) in fields{node.set_prop(k,v);}
        node
    }
    #[test]
    fn bounded_cache_rejects_exact_overruns_and_excess_entries(){
        assert!(within_budget(0,0,0));
        assert!(within_budget(MAX_ENTRY_BYTES,0,0));
        assert!(within_budget(MAX_ENTRY_BYTES,MAX_DRAFTS-1,
            MAX_DRAFT_BYTES-MAX_ENTRY_BYTES));
        assert!(!within_budget(MAX_ENTRY_BYTES+1,0,0));
        assert!(!within_budget(1,MAX_DRAFTS,0));
        assert!(!within_budget(1,0,MAX_DRAFT_BYTES));
        assert!(!within_budget(0,0,MAX_DRAFT_BYTES+1));
        assert!(!within_budget(usize::MAX,usize::MAX,usize::MAX));
    }
    #[test]
    fn only_explicit_public_text_revision_and_stable_key_are_eligible(){
        let input=field(&[]);
        assert_eq!(identity(&input).unwrap().revision,1);
        for (key,val) in [
            ("retain_draft","true"),("draft_key",""),("draft_key"," bad"),
            ("draft_scope",""),("draft_scope"," bad"),
            ("draft_key","bad\nkey"),("draft_key","bad\u{202E}key"),
            ("value_revision","01"),("value_revision","-1"),
            ("value_revision","18446744073709551616"),
            ("type","password"),("input_type","otp"),
            ("sensitive","true"),("surrounding_text","none"),("form"," bad"),
        ]{
            let mut x=input.clone();x.set_prop(key,val);
            assert!(identity(&x).is_none(),"malformed/private {key}={val:?}");
        }
        let mut x=input.clone();x.set_prop("value_revision","0");
        assert_eq!(identity(&x).unwrap().revision,0);
        x.set_prop("type","email");assert!(identity(&x).is_some());
        x.set_prop("input_type","url");assert!(identity(&x).is_none(),
            "conflicting input kind aliases cannot restore the same draft");
    }
    #[test]
    fn stable_identity_uses_form_namespace_not_host_revision(){
        let one=identity(&field(&[])).unwrap();
        let mut f=field(&[]);f.set_prop("value_revision","2");
        let changed=identity(&f).unwrap();
        assert!(one.same_field(&changed));
        f.set_prop("form","other");
        assert!(!one.same_field(&identity(&f).unwrap()));
        f.set_prop("form","registration");
        f.set_prop("draft_scope","window-session-2");
        assert!(!one.same_field(&identity(&f).unwrap()),
            "a different host session cannot access prior user's draft");
        f.set_prop("draft_scope","window-session-1");
        f.set_prop("draft_key","another");
        assert!(!one.same_field(&identity(&f).unwrap()));
        f.set_prop("draft_key","registration.email");
        f.set_prop("type","search");
        assert!(!one.same_field(&identity(&f).unwrap()),
            "changing the allowed input purpose is still a logical boundary");
    }
    #[test]
    fn duplicate_visible_or_hidden_key_never_owns_a_retained_session(){
        let mut first=field(&[]);first.set_prop("on_change","cb:10");
        let mut second=field(&[]);second.set_prop("on_change","cb:20");
        second.set_prop("x","70");second.set_prop("y","75");
        let root=n("App",&[],vec![first.clone(),second.clone()]);
        let plan=layout::arrange(&root,Bounds::new(0.,0.,600.,400.));
        let visible=layout::focus_order(&root,&plan);
        let c=catalog(&root,&visible);
        assert!(c.active.is_empty(),"duplicate logical key blocks both");
        assert_eq!(c.present.len(),2);
        // Hidden duplicate still occupies the key namespace.
        second.set_prop("x","5000");
        let root=n("App",&[],vec![first,second]);
        let p=layout::arrange(&root,Bounds::new(0.,0.,600.,400.));
        assert!(catalog(&root,&layout::focus_order(&root,&p)).active.is_empty());
    }
    #[test]
    fn duplicate_key_with_malformed_callback_still_blocks_retention(){
        let first=field(&[]);
        let mut duplicate=field(&[]);
        duplicate.set_prop("on_change","not-a-callback");
        duplicate.set_prop("x","8500");
        let root=n("App",&[],vec![first,duplicate]);
        let plan=layout::arrange(&root,Bounds::new(0.,0.,600.,400.));
        let c=catalog(&root,&layout::focus_order(&root,&plan));
        assert!(c.active.is_empty(),
            "broken callback must not bypass draft key uniqueness");
        assert_eq!(c.present.len(),1);
    }
    #[test]
    fn no_visible_eligible_input_may_claim_an_unmounted_draft(){
        let input=field(&[]);
        let root=n("App",&[],vec![input.clone()]);
        let p=layout::arrange(&root,Bounds::new(0.,0.,600.,400.));
        assert_eq!(catalog(&root,&layout::focus_order(&root,&p)).active.len(),1);
        let gone=n("App",&[],vec![n("Text",&[("text","Next screen")],vec![])]);
        assert!(catalog(&gone,&[]).active.is_empty());
        assert!(catalog(&gone,&[]).present.is_empty());
    }
    #[test]
    fn ineligible_sensitive_or_disabled_field_stays_present_no_hidden_cache(){
        let mut input=field(&[]);
        input.set_prop("disabled","true");
        let root=n("App",&[],vec![input.clone()]);
        let p=layout::arrange(&root,Bounds::new(0.,0.,600.,400.));
        let c=catalog(&root,&layout::focus_order(&root,&p));
        assert!(c.active.is_empty());
        assert!(c.present.contains(&10));
        input.set_prop("disabled","false");
        let root=n("App",&[],vec![input]);
        let p=layout::arrange(&root,Bounds::new(0.,0.,600.,400.));
        assert!(catalog(&root,&layout::focus_order(&root,&p)).active.contains_key(&10));
    }
}
