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

/// Stable, deterministic admission independent of HashMap iteration order.
/// Prefer editors most recently focused, including initial opt-in autofocus. On equal
/// focus epochs, compare logical scope/key/form and then callback ID.
/// Admission remains fail-closed: no partial Undo truncation or payload edits.
pub(crate) struct RetentionCandidate<'a>{
    pub(crate) callback:u64,
    pub(crate) last_focus:u64,
    pub(crate) bytes:usize,
    pub(crate) identity:&'a Identity,
}
pub(crate) fn select_retained(candidates:&[RetentionCandidate<'_>])
    ->HashSet<u64>{
    let mut sorted:Vec<_>=candidates.iter().collect();
    sorted.sort_by(|a,b|
        b.last_focus.cmp(&a.last_focus)
            .then_with(||a.identity.scope.cmp(&b.identity.scope))
            .then_with(||a.identity.key.cmp(&b.identity.key))
            .then_with(||a.identity.form.cmp(&b.identity.form))
            .then_with(||a.callback.cmp(&b.callback)));
    let mut selected=HashSet::new();
    let mut bytes=0usize;
    for candidate in sorted{
        if !selected.contains(&candidate.callback)
            && within_budget(candidate.bytes,selected.len(),bytes){
            bytes+=candidate.bytes; // checked by within_budget
            selected.insert(candidate.callback);
        }
    }
    selected
}

/// A privacy-minimized, bounded *notice*, not a backup of discarded text.
/// Only user-committed text that differed from the most recently observed
/// host value may generate this notice when capacity forces an eviction.
#[derive(Clone,Debug)]
pub(crate) struct LostDraft {
    pub(crate) identity:Identity,
    pub(crate) last_focus:u64,
}
pub(crate) fn lost_draft_matches(notice:&LostDraft,current:&Identity)->bool{
    notice.identity.same_field(current)
        && notice.identity.revision==current.revision
}
/// Keep at most eight metadata-only warnings using the same deterministic
/// priority as complete draft selection, irrespective of HashMap order.
pub(crate) fn select_lost_draft_notices(lost:&HashMap<u64,LostDraft>)->HashSet<u64>{
    let candidates=lost.iter().map(|(id,notice)|RetentionCandidate{
        callback:*id,
        last_focus:notice.last_focus,
        bytes:0,
        identity:&notice.identity,
    }).collect::<Vec<_>>();
    select_retained(&candidates)
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
/// Optional host-wide route/session authority. Without a root opt-in the
/// existing per-field scope contract is unchanged. A malformed root command
/// fails closed rather than retaining drafts under an ambiguous session.
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) enum ScopeGate{Legacy,Only(String),Reject}
impl ScopeGate{
    pub(crate) fn allows(&self,scope:&str)->bool{
        match self{
            Self::Legacy=>true,
            Self::Only(active)=>active==scope,
            Self::Reject=>false,
        }
    }
}
#[derive(Clone,Debug,PartialEq,Eq,Hash)]
pub(crate) struct ClaimKey {
    scope:String,
    key:String,
    form:Option<String>,
}
#[derive(Default)]
pub(crate) struct Catalog{
    pub(crate) active:HashMap<u64,Identity>,
    /// Even hidden/disabled/malformed fields are present and MUST NOT be
    /// silently treated as an unmounted route with retained private data.
    pub(crate) present:HashSet<u64>,
    pub(crate) claims:HashMap<ClaimKey,HashSet<Option<u64>>>,
    pub(crate) ambiguous_claims:bool,
    pub(crate) scope_gate:ScopeGate,
}
impl Default for ScopeGate{
    fn default()->Self{Self::Legacy}
}
impl Catalog {
    /// Any same-key claim by a different callback, even an ineligible,
    /// disabled or hidden TextInput, revokes the old cached editor.
    pub(crate) fn conflicts_with(&self,cb:u64,field:&Identity)->bool{
        self.ambiguous_claims || self.claims.iter().any(|(claim,owners)|
            claim.scope==field.scope && claim.key==field.key
            && (claim.form==field.form || claim.form.is_none()
                || field.form.is_none())
            && owners.iter().any(|owner|*owner!=Some(cb)))
    }
}
fn prop<'a>(n:&'a UiNode,k:&str)->Option<&'a str>{
    n.props.iter().find(|(key,_)|key==k).map(|(_,v)|v.as_str())
}
fn valid_key(key:&str)->bool {
    !key.is_empty() && key.len()<=128 && key.trim()==key &&
        !key.chars().any(|c|c.is_control()||
            matches!(c,'\u{202A}'..='\u{202E}'|'\u{2066}'..='\u{2069}'))
}
const IDENTITY_PROPS:[&str;11]=["retain_draft","draft_scope","draft_key",
    "on_change","value_revision","form","type","input_type","sensitive",
    "surrounding_text","read_only"];
fn duplicates_identity_props(node:&UiNode)->bool{
    IDENTITY_PROPS.iter().any(|key|
        node.props.iter().filter(|(k,_)|k.as_str()==*key).take(2).count()>1)
}
/// Single authoritative root scope, optional. The host rotates this value
/// during logout/context replacement even when all TextInputs are unmounted.
pub(crate) fn root_scope_gate(root:&UiNode)->ScopeGate{
    if root.kind!="App"{return ScopeGate::Legacy;}
    let mut values=root.props.iter().filter(|(key,_)|key=="draft_active_scope");
    match (values.next(),values.next()){
        (None,_)=>ScopeGate::Legacy,
        (Some((_,scope)),None) if valid_key(scope)=>ScopeGate::Only(scope.as_str().to_owned()),
        _=>ScopeGate::Reject,
    }
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
    if node.kind!="TextInput" || duplicates_identity_props(node)
        || crate::input_policy::edit_mode(node)==crate::input_policy::EditMode::Invalid
        || prop(node,"retain_draft")!=Some("route"){return None;}
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
        callbacks:&mut HashMap<u64,usize>,
        result:&mut Catalog){
        if node.kind=="TextInput"{
            let id=callback(node);
            if let Some(id)=id{
                result.present.insert(id);
                *callbacks.entry(id).or_default()+=1;
            }
            if duplicates_identity_props(node){
                // A duplicate prop can present two contradictory ownership
                // claims. Never pick the first token and keep private data.
                result.ambiguous_claims=true;
            } else if let (Some(scope),Some(key))=
                (prop(node,"draft_scope"),prop(node,"draft_key")){
                if valid_key(scope) && valid_key(key){
                    let form=match prop(node,"form"){
                        Some(value) if valid_key(value)=>Some(value.to_owned()),
                        None=>None,
                        Some(_)=>{
                            result.ambiguous_claims=true;
                            None
                        }
                    };
                    result.claims.entry(ClaimKey{
                        scope:scope.to_owned(),key:key.to_owned(),form,
                    }).or_default().insert(id);
                }
            }
            // A key claimed by a non-retainable or sensitive widget must
            // still invalidate older cached drafts from another callback.
            if let Some(identity)=identity(node){
                *counts.entry((identity.scope.clone(),
                    identity.key.clone(),identity.form.clone())).or_default()+=1;
                if let Some(id)=id{
                    by_id.entry(id).and_modify(|(_,n)|*n+=1)
                        .or_insert((identity,1));
                }
            }
        }
        for child in &node.children{scan(child,by_id,counts,callbacks,result);}
    }
    result.scope_gate=root_scope_gate(root);
    scan(root,&mut by_id,&mut key_counts,&mut callback_counts,&mut result);
    for (id,(value,multiplicity)) in by_id{
        if allowed.contains(&id) && result.scope_gate.allows(&value.scope)
            && !result.ambiguous_claims && multiplicity==1
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
        for &(k,v) in fields{out.set_prop(k,v);}
        out.children=children;out
    }
    fn field(fields:&[(&str,&str)])->UiNode{
        let mut node=n("TextInput",&[
            ("on_change","cb:10"),("x","20"),("y","20"),
            ("retain_draft","route"),("draft_scope","window-session-1"),
            ("draft_key","registration.email"),
            ("value_revision","1"),("form","registration"),
        ],vec![]);
        for &(k,v) in fields{node.set_prop(k,v);}
        node
    }

    fn sampled_identity(key:&str)->Identity{
        Identity{
            scope:"window-session-1".into(),
            key:key.into(),
            form:Some("checkout".into()),
            type_kind:"text".into(),
            revision:1,
        }
    }
    #[test]
    fn discarded_draft_notice_is_bound_to_exact_host_revision_and_identity(){
        let original=sampled_identity("name");
        let notice=LostDraft{identity:original.clone(),last_focus:7};
        assert!(lost_draft_matches(&notice,&original));
        let mut different=original.clone();
        different.revision+=1;
        assert!(!lost_draft_matches(&notice,&different),
            "a newer authoritative host revision supersedes the warning");
        different=original.clone();different.key="other".into();
        assert!(!lost_draft_matches(&notice,&different));
        different=original.clone();different.scope="other-user".into();
        assert!(!lost_draft_matches(&notice,&different));
        different=original.clone();different.type_kind="email".into();
        assert!(!lost_draft_matches(&notice,&different));
    }
    #[test]
    fn discarded_draft_notices_are_metadata_only_and_bounded_deterministically(){
        let mut warnings=HashMap::new();
        for i in 0..20{
            warnings.insert(i+10,LostDraft{
                identity:sampled_identity(&format!("f.{i:02}")),
                last_focus:i,
            });
        }
        let selected=select_lost_draft_notices(&warnings);
        assert_eq!(selected.len(),MAX_DRAFTS);
        for i in 0..20{
            assert_eq!(selected.contains(&(i+10)),i>=12,
                "newest eight only; no dependency on randomized map iteration");
        }
        let mut reverse=HashMap::new();
        for i in (0..20).rev(){
            reverse.insert(i+10,LostDraft{
                identity:sampled_identity(&format!("f.{i:02}")),last_focus:i,
            });
        }
        assert_eq!(selected,select_lost_draft_notices(&reverse));
    }
    #[test]
    fn deterministic_ninth_field_eviction_keeps_last_eight_focused(){
        let identities:Vec<_>=(0..9).map(|i|sampled_identity(&format!("field.{i}"))).collect();
        let list:Vec<_>=identities.iter().enumerate().map(|(i,identity)|
            RetentionCandidate{callback:100+i as u64,last_focus:i as u64+1,
                bytes:10,identity}
        ).collect();
        let chosen=select_retained(&list);
        assert_eq!(chosen.len(),MAX_DRAFTS);
        assert!(!chosen.contains(&100),"least recently focused field is evicted");
        for i in 101..109{assert!(chosen.contains(&i));}
        for shift in 0..list.len(){
            let mut permuted=list.iter().collect::<Vec<_>>();
            permuted.rotate_left(shift);
            let copied=permuted.iter().map(|c|RetentionCandidate{
                callback:c.callback,last_focus:c.last_focus,
                bytes:c.bytes,identity:c.identity,
            }).collect::<Vec<_>>();
            assert_eq!(select_retained(&copied),chosen,
                "iteration order cannot change selected fields");
        }
    }
    #[test]
    fn retained_bytes_budget_is_ranked_and_skips_oversized_candidates(){
        let identities:Vec<_>=(0..10).map(|i|sampled_identity(&format!("key.{i}"))).collect();
        let list:Vec<_>=identities.iter().enumerate().map(|(i,identity)|{
            RetentionCandidate{callback:i as u64+1,
                last_focus:(100-i) as u64,
                bytes:match i{
                    0=>MAX_ENTRY_BYTES+1, // reject despite highest priority
                    1..=4=>32*1024,
                    5=>1024, // cannot fit after four 32KiB choices
                    _=>0,
                },
                identity}
        }).collect();
        let chosen=select_retained(&list);
        assert!(!chosen.contains(&1),"oversized entry must not be truncated");
        for id in 2..=5{assert!(chosen.contains(&id));}
        assert!(!chosen.contains(&6),
            "cannot overrun the 128KiB aggregate even for a valid entry");
        assert_eq!(chosen.len(),8,
            "zero-byte fields may fill remaining entry slots after byte budget");
        let mut reverse=list.into_iter().rev().collect::<Vec<_>>();
        assert_eq!(select_retained(&reverse),chosen);
        reverse.reverse();
        assert_eq!(select_retained(&reverse),chosen);
    }
    #[test]
    fn stable_tie_break_uses_logical_key_and_callback_not_insertion_order(){
        let identities:Vec<_>=(0..10).rev()
            .map(|i|sampled_identity(&format!("key.{i:02}"))).collect();
        let list=identities.iter().enumerate().map(|(i,identity)|
            RetentionCandidate{callback:(i+10) as u64,last_focus:7,
                bytes:12,identity}
        ).collect::<Vec<_>>();
        let chosen=select_retained(&list);
        assert_eq!(chosen.len(),8);
        for (i,identity) in identities.iter().enumerate(){
            assert_eq!(chosen.contains(&((i+10) as u64)),
                identity.key.as_str()<"key.08",
                "tie-breaking must prefer first eight keys lexically");
        }
        let reverse=list.into_iter().rev().collect::<Vec<_>>();
        assert_eq!(select_retained(&reverse),chosen);
    }
    #[test]
    fn byte_budget_prefers_recent_editor_then_fits_older_small_session(){
        let ids=(0..4).map(|i|sampled_identity(&format!("x{i}"))).collect::<Vec<_>>();
        let choices=[
            (10,4,80*1024),
            (11,3,32*1024),
            (12,2,32*1024),
            (13,1,16*1024),
        ];
        let list=choices.iter().enumerate().map(|(i,(cb,focus,bytes))|
            RetentionCandidate{callback:*cb,last_focus:*focus,bytes:*bytes,
                identity:&ids[i]}
        ).collect::<Vec<_>>();
        let chosen=select_retained(&list);
        assert!(!chosen.contains(&10),
            "single entry above 32KiB must fail closed");
        assert!(chosen.contains(&11));
        assert!(chosen.contains(&12));
        assert!(chosen.contains(&13));
        assert_eq!(select_retained(&list.iter().rev().map(|c|RetentionCandidate{
            callback:c.callback,last_focus:c.last_focus,
            bytes:c.bytes,identity:c.identity,
        }).collect::<Vec<_>>()),chosen);
    }
    #[test]
    fn read_only_retains_only_with_unambiguous_valid_host_policy(){
        let mut input=field(&[]);
        input.set_prop("read_only","true");
        assert!(identity(&input).is_some(),
            "read_only does not mean disabled or secret");
        input.set_prop("read_only","false");
        assert!(identity(&input).is_some());
        input.set_prop("read_only","unknown");
        assert!(identity(&input).is_none(),
            "ambiguous edit policy cannot qualify private drafts");
        input.set_prop("read_only","true");
        input.props.push(("read_only".into(),"false".into()));
        assert!(identity(&input).is_none(),
            "duplicate edit policy disqualifies route draft retention");
    }
    #[test]
    fn root_scope_rotation_is_explicit_and_strictly_single_valued(){
        let mut root=n("App",&[],vec![field(&[])]);
        assert_eq!(root_scope_gate(&root),ScopeGate::Legacy);
        root.set_prop("draft_active_scope","window-session-1");
        assert_eq!(root_scope_gate(&root),
            ScopeGate::Only("window-session-1".to_owned()));
        let p=layout::arrange(&root,Bounds::new(0.,0.,600.,400.));
        assert_eq!(catalog(&root,&layout::focus_order(&root,&p)).active.len(),1);
        root.set_prop("draft_active_scope","window-session-2");
        let c=catalog(&root,&layout::focus_order(&root,&p));
        assert!(c.active.is_empty(),"new session must not retain prior user's field");
        root.set_prop("draft_active_scope","bad\nscope");
        assert_eq!(root_scope_gate(&root),ScopeGate::Reject);
        root.set_prop("draft_active_scope","window-session-1");
        root.props.push(("draft_active_scope".into(),"window-session-2".into()));
        assert_eq!(root_scope_gate(&root),ScopeGate::Reject);
        assert!(catalog(&root,&layout::focus_order(&root,&p)).active.is_empty(),
            "duplicate host-wide scope must never silently choose one");
    }
    #[test]
    fn noneligible_foreign_claim_blocks_cached_draft_even_when_hidden(){
        let original=field(&[]);
        let saved=identity(&original).unwrap();
        let mut intruder=field(&[]);
        intruder.set_prop("on_change","cb:99");
        intruder.set_prop("disabled","true");
        let mut root=n("App",&[],vec![intruder.clone()]);
        let plan=layout::arrange(&root,Bounds::new(0.,0.,600.,400.));
        let c=catalog(&root,&layout::focus_order(&root,&plan));
        assert!(c.active.is_empty());
        assert!(c.conflicts_with(10,&saved),
            "disabled foreign callback still claims the private identity");
        intruder.set_prop("x","9000");
        root.children[0]=intruder;
        let p=layout::arrange(&root,Bounds::new(0.,0.,600.,400.));
        assert!(catalog(&root,&layout::focus_order(&root,&p))
            .conflicts_with(10,&saved),"hidden foreign owner also revokes");
        root.children[0].set_prop("on_change","malformed");
        let p=layout::arrange(&root,Bounds::new(0.,0.,600.,400.));
        assert!(catalog(&root,&layout::focus_order(&root,&p))
            .conflicts_with(10,&saved),"unparseable foreign identity revokes");
    }
    #[test]
    fn sensitive_or_opted_out_foreign_field_cannot_keep_original_cache(){
        let saved=identity(&field(&[])).unwrap();
        for (key,value) in [("type","password"),("retain_draft","false"),
            ("sensitive","true"),("surrounding_text","none")] {
            let mut foreign=field(&[]);
            foreign.set_prop("on_change","cb:22");
            foreign.set_prop(key,value);
            let root=n("App",&[],vec![foreign]);
            let p=layout::arrange(&root,Bounds::new(0.,0.,600.,400.));
            assert!(catalog(&root,&layout::focus_order(&root,&p))
                .conflicts_with(10,&saved),"{key} should revoke foreign cache");
        }
    }
    #[test]
    fn duplicated_or_ambiguous_critical_props_never_select_first_identity(){
        let original=field(&[]);
        let saved=identity(&original).unwrap();
        for name in IDENTITY_PROPS{
            let mut node=original.clone();
            // Optional identity props may be absent in the base fixture.
            // Install a valid first occurrence before adding a duplicate;
            // a lone newly-added prop is not an ambiguous duplicate.
            let first=prop(&node,name).unwrap_or(match name{
                "type"|"input_type"=>"text",
                "sensitive"=>"false",
                "surrounding_text"=>"full",
                _=>"value",
            }).to_owned();
            node.set_prop(name,&first);
            node.props.push((name.to_owned(),first));
            assert!(identity(&node).is_none(),"duplicate {name} cannot qualify");
            let root=n("App",&[],vec![node]);
            let p=layout::arrange(&root,Bounds::new(0.,0.,600.,400.));
            let c=catalog(&root,&layout::focus_order(&root,&p));
            assert!(c.ambiguous_claims);
            assert!(c.conflicts_with(10,&saved));
        }
    }
    #[test]
    fn unscoped_foreign_claim_cannot_evade_a_scoped_cached_field(){
        let saved=identity(&field(&[])).unwrap();
        let mut unscoped=field(&[]);
        unscoped.set_prop("on_change","cb:88");
        unscoped.props.retain(|(name,_)|name!="form");
        let root=n("App",&[],vec![unscoped]);
        let plan=layout::arrange(&root,Bounds::new(0.,0.,600.,400.));
        let c=catalog(&root,&layout::focus_order(&root,&plan));
        assert!(c.conflicts_with(10,&saved),
            "missing form namespace is ambiguous, not an escape from ownership");
    }
    #[test]
    fn independent_forms_can_share_a_human_key_but_not_a_claim_owner(){
        let saved=identity(&field(&[])).unwrap();
        let mut other=field(&[]);
        other.set_prop("form","different");
        other.set_prop("on_change","cb:90");
        let root=n("App",&[],vec![other]);
        let p=layout::arrange(&root,Bounds::new(0.,0.,600.,400.));
        let c=catalog(&root,&layout::focus_order(&root,&p));
        assert!(!c.conflicts_with(10,&saved),
            "a different valid form namespace is a distinct identity");
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
