//! Native opt-in controlled form submission (no Core ABI or Nexus schema change).
//! A Button(form_submit: "...") emits its normal on_click only when all
//! associated TextInput(form: "...") values satisfy bounded local rules.
//! The host remains authoritative over persisted values and revisions.
use std::collections::{HashMap,HashSet};
use aura_nexus::UiNode;
use unicode_segmentation::UnicodeSegmentation;
use crate::layout::{self,LayoutNode,HitTarget};

fn prop<'a>(node:&'a UiNode,key:&str)->Option<&'a str>{
    node.props.iter().find(|(k,_)|k==key).map(|(_,v)|v.as_str())
}
fn callback(node:&UiNode,key:&str)->Option<u64>{
    prop(node,key)?.strip_prefix("cb:")?.parse::<u64>().ok()
}
pub(crate) fn valid_name(name:&str)->bool {
    !name.is_empty() && name.len()<=128 && name.trim()==name
        && !name.chars().any(char::is_control)
}

/// The *application* owns this state. A renderer may not infer network
/// completion, cancel a request, clear server errors or invent a callback.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum SubmitState<'a> {
    Ready,
    Busy,
    Error(&'a str),
    Invalid,
}
impl SubmitState<'_>{
    pub(crate) fn actionable(self)->bool{
        matches!(self,Self::Ready|Self::Error(_))
    }
}
/// Form status and remote error are a single atomic host-authored snapshot.
/// Do not display arbitrary control/bidi escape strings from remote services.
pub(crate) fn submit_state(node:&UiNode)->SubmitState<'_>{
    if node.kind!="Button" || prop(node,"form_submit").is_none(){
        return SubmitState::Ready;
    }
    let error=prop(node,"form_error");
    match prop(node,"form_status"){
        None|Some("ready") if error.is_none()=>SubmitState::Ready,
        Some("busy") if error.is_none()=>SubmitState::Busy,
        Some("error")=>{
            match error{
                Some(message) if !message.trim().is_empty()
                    && message.trim()==message && message.len()<=180
                    && !message.chars().any(|c|c.is_control()
                        || matches!(c,'\u{202A}'..='\u{202E}'
                            |'\u{2066}'..='\u{2069}')) => SubmitState::Error(message),
                _=>SubmitState::Invalid,
            }
        },
        _=>SubmitState::Invalid,
    }
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) enum SubmitAction {
    Normal,
    Validate(String),
    Reject,
}
/// Callbacks are ambiguous without stable widget identity. If multiple
/// visible Buttons share a callback ID and any is a form submitter, do not
/// attempt to infer which one was physically or keyboard activated.
pub(crate) fn submit_action(root:&UiNode,plan:&LayoutNode,clicked:u64)->SubmitAction {
    fn scan(node:&UiNode,plan:&LayoutNode,id:u64,count:&mut usize,
        action:&mut SubmitAction){
        if plan.bounds.intersect(plan.clip).is_empty(){return;}
        if node.kind=="Button" && callback(node,"on_click")==Some(id){
            *count+=1;
            if let Some(form)=prop(node,"form_submit"){
                if valid_name(form) && prop(node,"disabled")!=Some("true")
                    && matches!(prop(node,"control"),None|Some("button"))
                    && submit_state(node).actionable(){
                    *action=SubmitAction::Validate(form.to_owned());
                }else{*action=SubmitAction::Reject;}
            }
        }
        for (child,layout) in node.children.iter().zip(&plan.children){
            if let Some(layout)=layout{scan(child,layout,id,count,action);}
        }
    }
    let mut count=0;
    let mut action=SubmitAction::Normal;
    scan(root,plan,clicked,&mut count,&mut action);
    if count!=1 && action!=SubmitAction::Normal{SubmitAction::Reject}else{action}
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum FieldError {
    Required,
    TooShort,
    TooLong,
    InvalidRule,
    Unavailable,
    GroupRequired,
    GroupInvalid,
}
impl FieldError {
    pub(crate) fn label(self)->&'static str{
        match self{
            Self::Required=>"Required",
            Self::TooShort=>"Too short",
            Self::TooLong=>"Too long",
            Self::InvalidRule=>"Invalid rule",
            Self::Unavailable=>"Unavailable",
            Self::GroupRequired=>"Select an option",
            Self::GroupInvalid=>"Invalid group",
        }
    }
}
#[derive(Clone,Debug,Default,PartialEq,Eq)]
pub(crate) struct Assessment {
    pub(crate) errors:Vec<(u64,FieldError)>,
    pub(crate) radio_errors:Vec<(u64,FieldError)>,
    pub(crate) unavailable:bool,
    pub(crate) count:usize,
}
impl Assessment{
    pub(crate) fn valid(&self)->bool{
        self.count>0 && !self.unavailable && self.errors.is_empty()
            && self.radio_errors.is_empty()
    }
    pub(crate) fn first_focusable_error(&self,eligible:&HashSet<u64>)->Option<u64>{
        self.errors.iter().find_map(|(id,_)|eligible.contains(id).then_some(*id))
    }
    /// Both domains can use the same numeric callback ID. Search typed
    /// source-order Focus entries instead of conflating Button and TextInput.
    pub(crate) fn first_focus_target(&self,order:&[HitTarget])->Option<HitTarget>{
        order.iter().find(|item|match item{
            HitTarget::TextInput{on_change,..}=>self.errors.iter().any(|(id,_)|id==on_change),
            HitTarget::Button(id)=>self.radio_errors.iter().any(|(bad,_)|bad==id),
        }).cloned()
    }
}
#[derive(Default)]
pub(crate) struct RenderErrors{
    pub(crate) text:HashMap<u64,FieldError>,
    pub(crate) radio:HashMap<u64,FieldError>,
}
fn parse_limit(raw:Option<&str>)->Result<Option<usize>,()>{
    match raw{
        None=>Ok(None),
        Some(s) if !s.is_empty() && s.len()<=4 && s.bytes().all(|b|b.is_ascii_digit()) =>
            s.parse::<usize>().ok().filter(|n|*n<=4096).map(Some).ok_or(()),
        _=>Err(()),
    }
}
fn rule(node:&UiNode,value:&str)->Result<Option<FieldError>,()>{
    let required=match prop(node,"required"){
        None|Some("false")=>false,
        Some("true")=>true,
        _=>return Err(()),
    };
    let min=parse_limit(prop(node,"min_graphemes"))?;
    let max=parse_limit(prop(node,"max_graphemes"))?;
    if min.zip(max).is_some_and(|(lo,hi)|lo>hi){return Err(());}
    if required && value.trim().is_empty(){return Ok(Some(FieldError::Required));}
    let n=value.graphemes(true).count();
    if min.is_some_and(|lo|n<lo){return Ok(Some(FieldError::TooShort));}
    if max.is_some_and(|hi|n>hi){return Ok(Some(FieldError::TooLong));}
    Ok(None)
}
/// Form validation is evaluated against current committed EditSession
/// buffers, not stale host echo props, never a projected IBus preedit string.
/// Invalid/hidden/disabled/ambiguous fields block submission; no guessing
/// about unmounted or duplicate callback identities.
pub(crate) fn assess(root:&UiNode,plan:&LayoutNode,form:&str,
    values:&HashMap<u64,String>,eligible:&HashSet<u64>)->Assessment{
    if !valid_name(form){return Assessment{unavailable:true,..Default::default()};}
    fn walk(node:&UiNode,plan:Option<&LayoutNode>,form:&str,
        values:&HashMap<u64,String>,eligible:&HashSet<u64>,out:&mut Assessment){
        if node.kind=="TextInput" && prop(node,"form")==Some(form){
            out.count+=1;
            if out.count>64{out.unavailable=true;return;}
            let id=callback(node,"on_change");
            let physically_visible=plan.is_some_and(|p|!p.bounds.intersect(p.clip).is_empty());
            let usable=physically_visible && prop(node,"disabled")!=Some("true")
                && id.is_some_and(|cb|eligible.contains(&cb));
            if !usable {out.unavailable=true;}
            if let Some(cb)=id{
                let value=values.get(&cb).map(String::as_str)
                    .or_else(||prop(node,"value").or_else(||prop(node,"text"))).unwrap_or("");
                if !usable{
                    out.errors.push((cb,FieldError::Unavailable));
                }else{
                    match rule(node,value){
                        Ok(Some(reason))=>out.errors.push((cb,reason)),
                        Err(())=>out.errors.push((cb,FieldError::InvalidRule)),
                        Ok(None)=>{},
                    }
                }
            }
        }
        for (i,child) in node.children.iter().enumerate(){
            let next=plan.and_then(|p|p.children.get(i)).and_then(Option::as_ref);
            walk(child,next,form,values,eligible,out);
        }
    }
    let mut assessment=Assessment::default();
    walk(root,Some(plan),form,values,eligible,&mut assessment);
    // Validate form-owned Radio members against the complete visible-tree
    // group authority. A radio group cannot silently straddle two forms.
    #[derive(Default)]
    struct RadioGroup{
        ids:Vec<u64>,
        required:bool,
        selected:usize,
        invalid:bool,
    }
    let invalid_radio=layout::invalid_radio_groups(root,plan);
    let allowed:HashSet<u64>=layout::keyboard_focus_order(root,plan,&[])
        .into_iter().filter_map(|hit|match hit{
            HitTarget::Button(id)=>Some(id),
            _=>None,
        }).collect();
    fn collect_radios(node:&UiNode,plan:Option<&LayoutNode>,form:&str,
        groups:&mut HashMap<String,RadioGroup>,order:&mut Vec<String>,
        invalid:&HashSet<String>,allowed:&HashSet<u64>,out:&mut Assessment){
        let is_radio=node.kind=="Button" && prop(node,"control")==Some("radio");
        if is_radio && prop(node,"form")==Some(form){
            out.count+=1;
            if out.count>64{out.unavailable=true;return;}
            let id=callback(node,"on_click");
            let group=prop(node,"group").filter(|name|valid_name(name));
            let visible=plan.is_some_and(|p|!p.bounds.intersect(p.clip).is_empty());
            let valid=group.is_some()
                && matches!(layout::button_control(node),layout::ButtonControl::Radio(_))
                && matches!(prop(node,"required"),None|Some("true")|Some("false"))
                && visible && prop(node,"disabled")!=Some("true")
                && id.is_some_and(|cb|allowed.contains(&cb));
            if !valid{
                out.unavailable=true;
                if let Some(cb)=id{out.radio_errors.push((cb,FieldError::GroupInvalid));}
            }
            if let Some(group)=group{
                let group=group.to_owned();
                if !groups.contains_key(&group){order.push(group.clone());}
                let entry=groups.entry(group.clone()).or_default();
                if let Some(cb)=id{entry.ids.push(cb);}
                if prop(node,"required")==Some("true"){entry.required=true;}
                if matches!(layout::button_control(node),layout::ButtonControl::Radio(true)){
                    entry.selected+=1;
                }
                if !valid || invalid.contains(&group){entry.invalid=true;}
            }
        }
        for (i,child) in node.children.iter().enumerate(){
            let next=plan.and_then(|p|p.children.get(i)).and_then(Option::as_ref);
            collect_radios(child,next,form,groups,order,invalid,allowed,out);
        }
    }
    let mut groups=HashMap::new();
    let mut order=Vec::new();
    collect_radios(root,Some(plan),form,&mut groups,&mut order,
        &invalid_radio,&allowed,&mut assessment);
    // Detect a form group name reused by a radio option owned by a *different*
    // form (or unowned). Otherwise the visible group would validate partially.
    fn cross_form(node:&UiNode,groups:&mut HashMap<String,RadioGroup>,form:&str){
        if node.kind=="Button" && prop(node,"control")==Some("radio"){
            if let Some(group)=prop(node,"group"){
                if prop(node,"form")!=Some(form){
                    if let Some(entry)=groups.get_mut(group){entry.invalid=true;}
                }
            }
        }
        for child in &node.children{cross_form(child,groups,form);}
    }
    cross_form(root,&mut groups,form);
    for group in order{
        let Some(entry)=groups.get(&group) else{continue;};
        if entry.invalid || entry.selected>1{
            assessment.unavailable=true;
            assessment.radio_errors.extend(entry.ids.iter().copied()
                .map(|id|(id,FieldError::GroupInvalid)));
        }else if entry.required && entry.selected==0 {
            assessment.radio_errors.extend(entry.ids.iter().copied()
                .map(|id|(id,FieldError::GroupRequired)));
        }
    }
    assessment
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::layout::{arrange,focus_order,Bounds,HitTarget};
    fn n(kind:&str,props:&[(&str,&str)],children:Vec<UiNode>)->UiNode{
        let mut node=UiNode::new(kind);
        for (k,v) in props{node.set_prop(*k,*v);}
        node.children=children;node
    }
    fn plan(root:&UiNode)->LayoutNode{arrange(root,Bounds::new(0.0,0.0,600.0,400.0))}
    fn eligible(root:&UiNode,layout:&LayoutNode)->HashSet<u64>{
        focus_order(root,layout).into_iter().filter_map(|entry|match entry{
            HitTarget::TextInput{on_change,..}=>Some(on_change),
            _=>None,
        }).collect()
    }
    #[test]
    fn controlled_form_status_is_atomic_bounded_and_safe_to_retry(){
        let mut root=n("App",&[],vec![
            n("Button",&[("x","20"),("y","20"),("form_submit","checkout"),
                ("on_click","cb:90")],vec![]),
        ]);
        let p=plan(&root);
        let node=&root.children[0];
        assert_eq!(submit_state(node),SubmitState::Ready);
        assert_eq!(submit_action(&root,&p,90),SubmitAction::Validate("checkout".into()));
        root.children[0].set_prop("form_status","busy");
        assert_eq!(submit_state(&root.children[0]),SubmitState::Busy);
        assert_eq!(submit_action(&root,&p,90),SubmitAction::Reject);
        root.children[0].set_prop("form_status","error");
        root.children[0].set_prop("form_error","Service unavailable");
        assert_eq!(submit_state(&root.children[0]),SubmitState::Error("Service unavailable"));
        assert_eq!(submit_action(&root,&p,90),SubmitAction::Validate("checkout".into()),
            "server error permits explicit user retry but never auto-retries");
        root.children[0].set_prop("form_error","bad\ninjected");
        assert_eq!(submit_state(&root.children[0]),SubmitState::Invalid);
        root.children[0].set_prop("form_error","bad\u{202E}direction");
        assert_eq!(submit_state(&root.children[0]),SubmitState::Invalid);
        root.children[0].set_prop("form_error",&"x".repeat(181));
        assert_eq!(submit_state(&root.children[0]),SubmitState::Invalid);
        root.children[0].set_prop("form_error","תקלה זמנית");
        assert!(matches!(submit_state(&root.children[0]),SubmitState::Error(_)));
        root.children[0].set_prop("form_status","ready");
        assert_eq!(submit_state(&root.children[0]),SubmitState::Invalid,
            "a stale error attached to ready/busy is ambiguous and rejected");
        root.children[0].set_prop("form_status","BUSY");
        assert_eq!(submit_action(&root,&p,90),SubmitAction::Reject);
    }

    #[test]
    fn required_radio_group_is_host_owned_and_validates_once_selected(){
        let mut root=n("App",&[],vec![
            n("Button",&[("x","20"),("y","20"),("form","checkout"),
                ("control","radio"),("group","tier"),("checked","false"),
                ("required","true"),("on_click","cb:21")],vec![]),
            n("Button",&[("x","20"),("y","90"),("form","checkout"),
                ("control","radio"),("group","tier"),("checked","false"),
                ("on_click","cb:22")],vec![]),
            n("Button",&[("x","20"),("y","165"),("form","other"),
                ("control","radio"),("group","different"),("checked","false"),
                ("required","true"),("on_click","cb:31")],vec![]),
        ]);
        let p=plan(&root);
        let a=assess(&root,&p,"checkout",&HashMap::new(),&HashSet::new());
        assert_eq!(a.count,2);
        assert_eq!(a.radio_errors,vec![(21,FieldError::GroupRequired),
            (22,FieldError::GroupRequired)]);
        assert!(!a.valid());
        root.children[1].set_prop("checked","true");
        let valid=assess(&root,&p,"checkout",&HashMap::new(),&HashSet::new());
        assert!(valid.valid(),"host changed checked only, same group now valid: {valid:?}");
        assert!(!assess(&root,&p,"other",&HashMap::new(),&HashSet::new()).valid());
        root.children[0].set_prop("checked","true");
        let conflicted=assess(&root,&p,"checkout",&HashMap::new(),&HashSet::new());
        assert!(!conflicted.valid());
        assert!(conflicted.unavailable);
        assert!(conflicted.radio_errors.iter().any(|(id,_)|*id==21));
    }

    #[test]
    fn malformed_or_cross_form_radio_group_fails_closed(){
        let mut root=n("App",&[],vec![
            n("Button",&[("x","20"),("y","20"),("form","checkout"),
                ("control","radio"),("group","shared"),("checked","true"),
                ("required","true"),("on_click","cb:21")],vec![]),
            n("Button",&[("x","20"),("y","90"),("form","other"),
                ("control","radio"),("group","shared"),("checked","false"),
                ("on_click","cb:22")],vec![]),
        ]);
        let p=plan(&root);
        assert!(assess(&root,&p,"checkout",&HashMap::new(),&HashSet::new()).unavailable);
        root.children[1].set_prop("group","other");
        assert!(assess(&root,&p,"checkout",&HashMap::new(),&HashSet::new()).valid());
        root.children[0].set_prop("checked","yes");
        assert!(!assess(&root,&p,"checkout",&HashMap::new(),&HashSet::new()).valid());
        root.children[0].set_prop("checked","true");
        root.children[0].set_prop("required","YES");
        assert!(!assess(&root,&p,"checkout",&HashMap::new(),&HashSet::new()).valid());
        root.children[0].set_prop("required","true");
        root.children[0].set_prop("disabled","true");
        assert!(!assess(&root,&p,"checkout",&HashMap::new(),&HashSet::new()).valid());
    }

    #[test]
    fn mixed_form_errors_focus_by_typed_source_order_not_numeric_id(){
        let root=n("App",&[],vec![
            n("Button",&[("x","20"),("y","20"),("form","signup"),("control","radio"),
                ("group","kind"),("checked","false"),("required","true"),
                ("on_click","cb:7")],vec![]),
            n("TextInput",&[("x","20"),("y","90"),("form","signup"),
                ("required","true"),("on_change","cb:7"),("value","")],vec![]),
        ]);
        let p=plan(&root);
        let fields=eligible(&root,&p);
        let assessment=assess(&root,&p,"signup",&HashMap::new(),&fields);
        assert_eq!(assessment.errors,vec![(7,FieldError::Required)]);
        assert_eq!(assessment.radio_errors,vec![(7,FieldError::GroupRequired)]);
        let keyboard=layout::keyboard_focus_order(&root,&p,&layout::focus_order(&root,&p));
        assert_eq!(assessment.first_focus_target(&keyboard),Some(HitTarget::Button(7)),
            "earlier Radio must win even when the TextInput shares numeric callback");
        let mut root=root;
        root.children[0].set_prop("checked","true");
        let a=assess(&root,&p,"signup",&HashMap::new(),&fields);
        assert_eq!(a.first_focus_target(&keyboard),
            Some(HitTarget::TextInput{on_change:7,on_submit:None,value:String::new()}));
    }

    #[test]
    fn bounded_form_membership_rejects_unmounted_and_excessive_radio_controls(){
        let mut options=Vec::new();
        for i in 0..65 {
            options.push(n("Button",&[
                ("x","20"),("y","20"),("control","radio"),("form","many"),
                ("group","many"),("checked","false"),("on_click","cb:99")],vec![]));
            options.last_mut().unwrap().set_prop("on_click",format!("cb:{}",i+50));
        }
        let root=n("App",&[],options);
        let p=plan(&root);
        let a=assess(&root,&p,"many",&HashMap::new(),&HashSet::new());
        assert!(!a.valid());assert!(a.unavailable);
        assert!(a.count>64);
    }

    #[test]
    fn form_submit_button_requires_unique_callback_and_exact_group(){
        let root=n("App",&[],vec![
            n("Button",&[("on_click","cb:44"),("form_submit","account")],vec![]),
            n("Button",&[("on_click","cb:51")],vec![]),
        ]);
        assert_eq!(submit_action(&root,&plan(&root),44),SubmitAction::Validate("account".into()));
        assert_eq!(submit_action(&root,&plan(&root),51),SubmitAction::Normal);
        let mut root=root;
        root.children.push(n("Button",&[("on_click","cb:44")],vec![]));
        assert_eq!(submit_action(&root,&plan(&root),44),SubmitAction::Reject);
        root.children[2].set_prop("on_click","cb:99");
        root.children[0].set_prop("form_submit"," account");
        assert_eq!(submit_action(&root,&plan(&root),44),SubmitAction::Reject);
        root.children[0].set_prop("form_submit","account");
        root.children[0].set_prop("control","radio");
        assert_eq!(submit_action(&root,&plan(&root),44),SubmitAction::Reject);
    }
    #[test]
    fn required_fields_validate_current_session_values_and_first_source_error(){
        let root=n("App",&[],vec![
            n("TextInput",&[("x","20"),("y","20"),("form","account"),
                ("on_change","cb:1"),("required","true"),("value","")],vec![]),
            n("TextInput",&[("x","20"),("y","90"),("form","account"),
                ("on_change","cb:2"),("min_graphemes","2"),("value","x")],vec![]),
            n("TextInput",&[("x","20"),("y","160"),("form","other"),
                ("on_change","cb:3"),("required","true"),("value","")],vec![]),
        ]);
        let p=plan(&root);let ids=eligible(&root,&p);
        let bad=assess(&root,&p,"account",&HashMap::new(),&ids);
        assert_eq!(bad.count,2);
        assert_eq!(bad.errors,vec![(1,FieldError::Required),(2,FieldError::TooShort)]);
        assert_eq!(bad.first_focusable_error(&ids),Some(1));
        let mut value=HashMap::new();
        value.insert(1,"hello".to_owned());value.insert(2,"xy".to_owned());
        assert!(assess(&root,&p,"account",&value,&ids).valid());
        assert!(!assess(&root,&p,"other",&value,&ids).valid());
        assert!(!assess(&root,&p,"missing",&value,&ids).valid());
    }
    #[test]
    fn grapheme_constraints_count_clusters_not_bytes_or_codepoints(){
        let root=n("App",&[],vec![n("TextInput",&[
            ("form","f"),("on_change","cb:5"),("min_graphemes","2"),
            ("max_graphemes","3"),("value","👩‍💻")],vec![])]);
        let p=plan(&root);let ids=eligible(&root,&p);
        assert_eq!(assess(&root,&p,"f",&HashMap::new(),&ids).errors,
            vec![(5,FieldError::TooShort)]);
        let mut value=HashMap::new();
        value.insert(5,"👩‍💻🇮🇱".to_owned());
        assert!(assess(&root,&p,"f",&value,&ids).valid());
        value.insert(5,"👩‍💻🇮🇱aB".to_owned());
        assert_eq!(assess(&root,&p,"f",&value,&ids).errors,
            vec![(5,FieldError::TooLong)]);
    }
    #[test]
    fn malformed_constraints_reject_instead_of_treating_as_absent(){
        let mut root=n("App",&[],vec![n("TextInput",&[
            ("form","f"),("on_change","cb:5"),("required","TRUE"),("value","x")],vec![])]);
        let p=plan(&root);let ids=eligible(&root,&p);
        assert_eq!(assess(&root,&p,"f",&HashMap::new(),&ids).errors,
            vec![(5,FieldError::InvalidRule)]);
        root.children[0].set_prop("required","false");
        root.children[0].set_prop("min_graphemes","4097");
        assert_eq!(assess(&root,&p,"f",&HashMap::new(),&ids).errors,
            vec![(5,FieldError::InvalidRule)]);
        root.children[0].set_prop("min_graphemes","3");
        root.children[0].set_prop("max_graphemes","2");
        assert_eq!(assess(&root,&p,"f",&HashMap::new(),&ids).errors,
            vec![(5,FieldError::InvalidRule)]);
        root.children[0].set_prop("max_graphemes","4");
        assert_eq!(assess(&root,&p,"f",&HashMap::new(),&ids).errors,
            vec![(5,FieldError::TooShort)]);
    }
    #[test]
    fn disabled_offscreen_and_duplicate_identity_fail_closed(){
        let root=n("App",&[],vec![
            n("TextInput",&[("x","15"),("y","15"),("form","f"),
                ("on_change","cb:10"),("disabled","true")],vec![]),
            n("TextInput",&[("x","900"),("y","900"),("form","f"),
                ("on_change","cb:11"),("required","true")],vec![]),
            n("TextInput",&[("x","15"),("y","85"),("form","f"),
                ("on_change","cb:12")],vec![]),
            n("TextInput",&[("x","15"),("y","145"),("form","f"),
                ("on_change","cb:12")],vec![]),
        ]);
        let p=plan(&root);let ids=eligible(&root,&p);
        let a=assess(&root,&p,"f",&HashMap::new(),&ids);
        assert!(!a.valid());
        assert!(a.unavailable);
        assert!(a.errors.contains(&(10,FieldError::Unavailable)));
        assert!(a.errors.contains(&(11,FieldError::Unavailable)));
        assert!(a.errors.contains(&(12,FieldError::Unavailable)));
        assert_eq!(a.first_focusable_error(&ids),None);
    }
    #[test]
    fn validation_does_not_mutate_source_or_emit_callbacks(){
        let root=n("App",&[],vec![n("TextInput",&[
            ("form","f"),("on_change","cb:21"),("required","true"),
            ("value","host")],vec![])]);
        let p=plan(&root);let ids=eligible(&root,&p);
        let before=root.clone();
        let a=assess(&root,&p,"f",&HashMap::new(),&ids);
        assert!(a.valid());assert_eq!(root.kind,before.kind);
        assert_eq!(root.children[0].props,before.children[0].props);
        let mut values=HashMap::new();values.insert(21,String::new());
        assert!(!assess(&root,&p,"f",&values,&ids).valid(),
            "current empty EditSession draft, not stale host echo, must govern");
    }
}
