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
    /// A terminal host claim, not proof that a remote task was aborted.
    /// The renderer only acknowledges a revision it requested itself.
    Cancelled{revision:u64,notice:Option<&'a str>},
    Invalid,
}
impl SubmitState<'_>{
    pub(crate) fn actionable(self)->bool{
        matches!(self,Self::Ready|Self::Error(_))
    }
}
fn safe_status_text(message:&str,max_bytes:usize)->bool {
    !message.trim().is_empty() && message.trim()==message
        && message.len()<=max_bytes
        && !message.chars().any(|c|c.is_control()
            || matches!(c,'\u{202A}'..='\u{202E}'
                |'\u{2066}'..='\u{2069}'))
}
/// Form status and remote error are a single atomic host-authored snapshot.
/// Do not display arbitrary control/bidi escape strings from remote services.
pub(crate) fn submit_state(node:&UiNode)->SubmitState<'_>{
    if node.kind!="Button" || prop(node,"form_submit").is_none(){
        return SubmitState::Ready;
    }
    let error=prop(node,"form_error");
    let acknowledge=prop(node,"form_cancel_ack_revision");
    let notice=prop(node,"form_cancel_notice");
    if prop(node,"form_status")!=Some("cancelled")
        && (acknowledge.is_some() || notice.is_some()){
        return SubmitState::Invalid;
    }
    match prop(node,"form_status"){
        None|Some("ready") if error.is_none()=>SubmitState::Ready,
        Some("busy") if error.is_none()=>SubmitState::Busy,
        Some("error")=>{
            match error{
                Some(message) if safe_status_text(message,180) =>
                    SubmitState::Error(message),
                _=>SubmitState::Invalid,
            }
        },
        Some("cancelled") if error.is_none()=>{
            match (request_revision(node),acknowledge){
                (Some(revision),Some(ack)) if ack==revision.to_string().as_str()
                    && notice.is_none_or(|message|safe_status_text(message,120))=>
                    SubmitState::Cancelled{revision,notice},
                _=>SubmitState::Invalid,
            }
        },
        _=>SubmitState::Invalid,
    }
}

/// A cancellation is only a callback request to the host, never a network
/// abort performed by Lumina. The host supplies a strictly increasing request
/// revision on the one busy submitter in the named form.
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) enum CancelDecision {
    Dispatch { form:String, revision:u64 },
    Reject,
}
#[derive(Default)]
pub(crate) struct CancelPlan {
    pub(crate) decisions:HashMap<u64,CancelDecision>,
    pub(crate) blocked:HashSet<u64>,
    /// A host request was already sent for the current (or older) epoch.
    pub(crate) requested:HashSet<u64>,
    /// The host has presented a terminal status for the same generation
    /// for which this native window actually dispatched a cancel request.
    pub(crate) acknowledged:HashSet<u64>,
}
fn request_revision(node:&UiNode)->Option<u64>{
    let raw=prop(node,"form_request_revision")?;
    if raw.is_empty() || raw.len()>20 || !raw.bytes().all(|c|c.is_ascii_digit()){
        return None;
    }
    let value=raw.parse::<u64>().ok().filter(|v|*v>0)?;
    (value.to_string()==raw).then_some(value)
}
/// Single-pass read-only form cancel resolver. If a cancel callback collides
/// with any visible Button callback, block ALL uses of that identity. An
/// earlier cancel for the same or newer request revision must never repeat.
/// Retaining the maximum seen revision also prevents ABA after ready/error.
pub(crate) fn cancel_plan(root:&UiNode,plan:&LayoutNode,
    last_cancelled:&HashMap<String,u64>)->CancelPlan {
    #[derive(Clone)]
    struct Submit {revision:Option<u64>}
    let mut submitters:HashMap<String,Vec<Submit>>=HashMap::new();
    let mut callbacks:HashMap<u64,usize>=HashMap::new();
    let mut cancellers:Vec<(Option<u64>,Option<String>,bool)>=Vec::new();
    let mut acknowledgements:Vec<(u64,String,u64)>=Vec::new();
    fn scan(node:&UiNode,plan:&LayoutNode,
        submits:&mut HashMap<String,Vec<Submit>>,
        callbacks:&mut HashMap<u64,usize>,
        cancels:&mut Vec<(Option<u64>,Option<String>,bool)>,
        acks:&mut Vec<(u64,String,u64)>){
        if plan.bounds.intersect(plan.clip).is_empty(){return;}
        if node.kind=="Button"{
            let id=callback(node,"on_click");
            if let Some(id)=id{*callbacks.entry(id).or_default()+=1;}
            if let Some(form)=prop(node,"form_submit"){
                if let (Some(id),SubmitState::Cancelled{revision,..})=
                    (id,submit_state(node)){
                    acks.push((id,form.to_owned(),revision));
                }
                let revision=(matches!(submit_state(node),SubmitState::Busy)
                    && prop(node,"disabled")!=Some("true")
                    && matches!(prop(node,"control"),None|Some("button")))
                    .then(||request_revision(node)).flatten();
                submits.entry(form.to_owned()).or_default().push(Submit{revision});
            }
            if let Some(form)=prop(node,"form_cancel"){
                let valid=valid_name(form)
                    && prop(node,"form_submit").is_none()
                    && prop(node,"form_status").is_none()
                    && prop(node,"form_error").is_none()
                    && prop(node,"form_request_revision").is_none()
                    && prop(node,"form_cancel_ack_revision").is_none()
                    && prop(node,"form_cancel_notice").is_none()
                    && matches!(prop(node,"control"),None|Some("button"))
                    && prop(node,"disabled")!=Some("true");
                cancels.push((id,Some(form.to_owned()),valid));
            }
        }
        for (child,prepared) in node.children.iter().zip(&plan.children){
            if let Some(prepared)=prepared{
                scan(child,prepared,submits,callbacks,cancels,acks);
            }
        }
    }
    scan(root,plan,&mut submitters,&mut callbacks,&mut cancellers,
        &mut acknowledgements);
    let mut result=CancelPlan::default();
    // A terminal label is only shown when the host-authored status matches
    // an actual cancellation requested in this same window. Malformed,
    // ambiguous or stale generations remain nonactionable but unverified.
    for (id,form,revision) in acknowledgements {
        if valid_name(&form)
            && submitters.get(&form).is_some_and(|group|group.len()==1)
            && callbacks.get(&id)==Some(&1)
            && last_cancelled.get(&form)==Some(&revision) {
            result.acknowledged.insert(id);
        }
    }
    for (id,form,valid) in cancellers {
        let Some(id)=id else{continue;};
        let mut requested=false;
        let selected=form.as_deref()
            .filter(|f|valid && callbacks.get(&id)==Some(&1))
            .and_then(|f|{
                let items=submitters.get(f)?;
                if items.len()!=1{return None;}
                let revision=items[0].revision?;
                if last_cancelled.get(f).is_some_and(|previous|revision<=*previous){
                    requested=true;
                    return None;
                }
                if last_cancelled.len()>=32 && !last_cancelled.contains_key(f){
                    return None;
                }
                Some(CancelDecision::Dispatch{
                    form:f.to_owned(),revision,
                })
            }).unwrap_or(CancelDecision::Reject);
        if selected==CancelDecision::Reject{
            result.blocked.insert(id);
        }
        if requested{result.requested.insert(id);}
        result.decisions.insert(id,selected);
    }
    result
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

/// Strictly opt-in: Enter in this field may invoke its form's existing
/// guarded Submit Button callback instead of its per-field on_submit.
/// Conflicting or duplicate Host identities fail closed; no new Nexus event.
pub(crate) fn enter_submits_form(node:&UiNode)->Result<bool,()> {
    if node.kind!="TextInput"{return Err(());}
    fn unique<'a>(node:&'a UiNode,key:&str)->Result<Option<&'a str>,()>{
        let mut values=node.props.iter().filter(|(k,_)|k==key);
        let first=values.next().map(|(_,v)|v.as_str());
        if values.next().is_some(){return Err(());}
        Ok(first)
    }
    match unique(node,"submit_form_on_enter")? {
        None|Some("false")=>Ok(false),
        Some("true")=>{
            let form=unique(node,"form")?.ok_or(())?;
            if !valid_name(form) ||
                unique(node,"on_change")?.is_none() ||
                callback(node,"on_change").is_none() ||
                unique(node,"on_submit")?.is_some() {return Err(());}
            Ok(true)
        },
        _=>Err(()),
    }
}

/// Reuse exactly the current guarded Submit Button, if unambiguous and
/// actionable. Do not synthesize a new callback, bypass busy/disabled, or
/// accept two same-form submitters just because the first is clickable.
pub(crate) fn enter_submit_button(root:&UiNode,plan:&LayoutNode,form:&str)->Option<u64>{
    if !valid_name(form){return None;}
    fn visit(node:&UiNode,plan:&LayoutNode,form:&str,
        count:&mut usize,found:&mut Option<u64>,ambiguous:&mut bool){
        if plan.bounds.intersect(plan.clip).is_empty(){return;}
        if node.kind=="Button" &&
            node.props.iter().any(|(k,v)|k=="form_submit" && v==form) {
            *count+=1;
            let matches=node.props.iter().filter(|(k,_)|k=="form_submit").count();
            if matches!=1{*ambiguous=true;}
            *found=callback(node,"on_click");
        }
        for (child,layout) in node.children.iter().zip(&plan.children){
            if let Some(layout)=layout{
                visit(child,layout,form,count,found,ambiguous);
            }
        }
    }
    let (mut count,mut found,mut ambiguous)=(0,None,false);
    visit(root,plan,form,&mut count,&mut found,&mut ambiguous);
    if count!=1 || ambiguous{return None;}
    let id=found?;
    match submit_action(root,plan,id){
        SubmitAction::Validate(name) if name==form=>Some(id),
        _=>None,
    }
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
/// Opt-in validation displayed after the USER moves focus away from a field.
/// Revisions are mandatory: a new Host authority must never inherit a stale
/// touched/error marker from another server-owned field instance.
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct BlurBinding{
    pub(crate) form:String,
    pub(crate) revision:u64,
}
fn blur_prop<'a>(node:&'a UiNode,name:&str)->Result<Option<&'a str>,()>{
    let mut values=node.props.iter().filter(|(key,_)|key==name);
    let found=values.next().map(|(_,value)|value.as_str());
    if values.next().is_some(){return Err(());}
    Ok(found)
}
pub(crate) fn blur_binding(node:&UiNode)->Result<Option<BlurBinding>,()>{
    if node.kind!="TextInput"{return Err(());}
    match blur_prop(node,"validate_on_blur")?{
        None|Some("false")=>Ok(None),
        Some("true")=>{
            let form=blur_prop(node,"form")?.filter(|x|valid_name(x)).ok_or(())?;
            // A valid callback is required; runtime independently verifies
            // uniqueness, clipping and current focus eligibility.
            blur_prop(node,"on_change")?.and_then(|x|x.strip_prefix("cb:"))
                .and_then(|x|x.parse::<u64>().ok()).ok_or(())?;
            let rev=blur_prop(node,"value_revision")?.and_then(|v|{
                if v.is_empty()||!v.bytes().all(|b|b.is_ascii_digit()){return None;}
                v.parse::<u64>().ok()
            }).ok_or(())?;
            Ok(Some(BlurBinding{form:form.to_owned(),revision:rev}))
        },
        Some(_)=>Err(()),
    }
}
/// Only the committed local EditSession value is supplied. No projected
/// IBus preedit, private text or Host echo is persisted as warning metadata.
pub(crate) fn blur_error(node:&UiNode,committed:&str)->Option<FieldError>{
    match rule(node,committed){
        Ok(value)=>value,
        Err(())=>Some(FieldError::InvalidRule),
    }
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
    // A broken opt-in cannot silently allow the form to proceed.
    blur_binding(node)?;
    enter_submits_form(node)?;
    crate::input_policy::clipboard_access(node)?;
    let required=match prop(node,"required"){
        None|Some("false")=>false,
        Some("true")=>true,
        _=>return Err(()),
    };
    let min=parse_limit(prop(node,"min_graphemes"))?;
    // The same strict Host-authored cap used by native editing. Duplicate
    // limits are ambiguous, not a first-value-wins validation bypass.
    let max=crate::input_policy::grapheme_limit(node)?;
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
                && crate::input_policy::edit_mode(node)
                    !=crate::input_policy::EditMode::Invalid
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
    fn enter_form_opt_in_requires_exact_unique_host_contract(){
        let base=[("form","registration"),("on_change","cb:31"),
            ("submit_form_on_enter","true")];
        let mut field=n("TextInput",&base,vec![]);
        assert_eq!(enter_submits_form(&field),Ok(true));
        field.set_prop("submit_form_on_enter","false");
        assert_eq!(enter_submits_form(&field),Ok(false));
        field.props.retain(|(k,_)|k!="submit_form_on_enter");
        assert_eq!(enter_submits_form(&field),Ok(false));
        for bad in ["True","yes","1"," true","","false "]{
            let mut candidate=n("TextInput",&base,vec![]);
            candidate.set_prop("submit_form_on_enter",bad);
            assert_eq!(enter_submits_form(&candidate),Err(()));
        }
        for duplicate in ["submit_form_on_enter","form","on_change"]{
            let mut candidate=n("TextInput",&base,vec![]);
            let value=prop(&candidate,duplicate).unwrap().to_owned();
            candidate.props.push((duplicate.to_owned(),value));
            assert_eq!(enter_submits_form(&candidate),Err(()));
        }
        let conflict=n("TextInput",&[("form","registration"),
            ("on_change","cb:31"),("on_submit","cb:32"),
            ("submit_form_on_enter","true")],vec![]);
        assert_eq!(enter_submits_form(&conflict),Err(()));
        assert_eq!(rule(&conflict,"valid"),Err(()));
    }
    #[test]
    fn enter_form_target_requires_one_live_actionable_submitter(){
        let field=n("TextInput",&[("form","registration"),
            ("on_change","cb:31"),("submit_form_on_enter","true"),
            ("x","20"),("y","20"),("width","160"),("height","40")],vec![]);
        let button=n("Button",&[("form_submit","registration"),
            ("on_click","cb:42"),("x","20"),("y","90"),
            ("width","160"),("height","40")],vec![]);
        let mut root=n("App",&[],vec![field,button]);
        assert_eq!(enter_submit_button(&root,&plan(&root),"registration"),Some(42));
        assert_eq!(enter_submit_button(&root,&plan(&root),"other"),None);
        root.children[1].set_prop("form_status","busy");
        assert_eq!(enter_submit_button(&root,&plan(&root),"registration"),None);
        root.children[1].set_prop("form_status","error");
        root.children[1].set_prop("form_error","Remote failure");
        assert_eq!(enter_submit_button(&root,&plan(&root),"registration"),Some(42));
        root.children[1].set_prop("form_status","ready");
        root.children[1].props.retain(|(k,_)|k!="form_error");
        root.children.push(n("Button",&[("form_submit","registration"),
            ("on_click","cb:43"),("x","200"),("y","90"),
            ("width","160"),("height","40")],vec![]));
        assert_eq!(enter_submit_button(&root,&plan(&root),"registration"),None);
        root.children.pop();
        root.children[1].props.push(("form_submit".into(),"registration".into()));
        assert_eq!(enter_submit_button(&root,&plan(&root),"registration"),None);
    }
    #[test]
    fn explicit_blur_policy_requires_unique_versioned_form_and_callback(){
        let mut field=n("TextInput",&[("form","signup"),("on_change","cb:101"),
            ("value_revision","8"),("validate_on_blur","true")],vec![]);
        assert_eq!(blur_binding(&field),Ok(Some(BlurBinding{
            form:"signup".to_owned(),revision:8
        })));
        field.set_prop("validate_on_blur","false");
        assert_eq!(blur_binding(&field),Ok(None));
        field.props.retain(|(k,_)|k!="validate_on_blur");
        assert_eq!(blur_binding(&field),Ok(None));
        for value in ["True","yes",""," true","false ","1"]{
            field.set_prop("validate_on_blur",value);
            assert_eq!(blur_binding(&field),Err(()));
        }
        field.set_prop("validate_on_blur","true");
        for (key,value) in [("value_revision","x"),("form"," bad"),
            ("on_change","invalid")] {
            let mut copy=field.clone();
            copy.set_prop(key,value);
            assert_eq!(blur_binding(&copy),Err(()),"{key} must fail closed");
        }
        for key in ["validate_on_blur","value_revision","form","on_change"]{
            let mut copy=field.clone();
            let first=prop(&copy,key).unwrap().to_owned();
            copy.props.push((key.to_owned(),first));
            assert_eq!(blur_binding(&copy),Err(()),"duplicate {key} rejected");
        }
    }
    #[test]
    fn blur_validation_uses_unicode_committed_value_and_not_projected_preedit(){
        let input=n("TextInput",&[("form","signup"),("on_change","cb:77"),
            ("value_revision","10"),("validate_on_blur","true"),
            ("required","true"),("min_graphemes","2"),("max_graphemes","3")],vec![]);
        assert_eq!(blur_error(&input,""),Some(FieldError::Required));
        assert_eq!(blur_error(&input,"👩‍💻"),Some(FieldError::TooShort));
        assert_eq!(blur_error(&input,"👩‍💻🇮🇱"),None);
        assert_eq!(blur_error(&input,"👩‍💻🇮🇱e\u{301}"),None);
        assert_eq!(blur_error(&input,"👩‍💻🇮🇱e\u{301}a"),Some(FieldError::TooLong));
        let mut broken=input.clone();
        broken.set_prop("validate_on_blur","invalid");
        assert_eq!(blur_error(&broken,"👩‍💻🇮🇱"),Some(FieldError::InvalidRule));
    }
    #[test]
    fn malformed_validate_on_blur_blocks_form_submit_not_only_hint(){
        let mut root=n("App",&[],vec![n("TextInput",&[
            ("form","signup"),("on_change","cb:71"),("value","abc"),
            ("value_revision","4"),("validate_on_blur","True")],vec![])]);
        let p=plan(&root);
        let mut values=HashMap::new();values.insert(71,"abc".to_owned());
        assert!(!assess(&root,&p,"signup",&values,&eligible(&root,&p)).valid());
        root.children[0].set_prop("validate_on_blur","true");
        assert!(assess(&root,&p,"signup",&values,&eligible(&root,&p)).valid());
    }
    #[test]
    fn live_edit_limit_and_form_validation_share_one_fail_closed_parser(){
        let mut root=n("App",&[],vec![n("TextInput",&[
            ("x","20"),("y","20"),("width","180"),("height","44"),
            ("form","reg"),("on_change","cb:10"),("value","👩‍💻🇮🇱"),
            ("max_graphemes","2")],vec![])]);
        let mut values=HashMap::new();
        values.insert(10,"👩‍💻🇮🇱".to_owned());
        let p=plan(&root);
        assert!(assess(&root,&p,"reg",&values,&eligible(&root,&p)).valid());
        values.insert(10,"👩‍💻🇮🇱e\u{301}".to_owned());
        assert!(!assess(&root,&p,"reg",&values,&eligible(&root,&p)).valid(),
            "form still rejects an over-limit value authored externally");
        root.children[0].set_prop("max_graphemes","4097");
        assert!(!assess(&root,&p,"reg",&values,&eligible(&root,&p)).valid());
        root.children[0].set_prop("max_graphemes","2");
        root.children[0].props.push(("max_graphemes".into(),"3".into()));
        values.insert(10,"a".to_owned());
        assert!(!assess(&root,&p,"reg",&values,&eligible(&root,&p)).valid(),
            "duplicate limit fails closed even with short valid text");
    }
    #[test]
    fn readonly_field_remains_valid_for_required_form_submission(){
        let mut tree=n("App",&[],vec![
            n("TextInput",&[("x","20"),("y","20"),("width","240"),
                ("height","46"),("form","checkout"),("on_change","cb:10"),
                ("value","Host data"),("required","true"),
                ("read_only","true")],vec![]),
        ]);
        let p=plan(&tree);
        let expected=eligible(&tree,&p);
        let mut values=HashMap::new();values.insert(10,"Host data".to_owned());
        assert!(assess(&tree,&p,"checkout",&values,&expected).valid(),
            "read_only is focusable and immutable, not an unavailable field");
        tree.children[0].set_prop("value"," ");
        values.insert(10," ".into());
        let assessed=assess(&tree,&plan(&tree),"checkout",&values,&expected);
        assert!(!assessed.valid(),"Host read_only value still obeys validation");
    }
    #[test]
    fn malformed_readonly_field_fails_closed_for_form_submission(){
        let mut tree=n("App",&[],vec![
            n("TextInput",&[("x","20"),("y","20"),("width","240"),
                ("height","46"),("form","checkout"),("on_change","cb:10"),
                ("value","Host data"),("required","true"),
                ("read_only","maybe")],vec![]),
        ]);
        let mut values=HashMap::new();values.insert(10,"Host data".to_owned());
        let p=plan(&tree);
        assert!(!assess(&tree,&p,"checkout",&values,&eligible(&tree,&p)).valid(),
            "unknown policy must not submit invalid form");
        tree.children[0].set_prop("read_only","false");
        assert!(assess(&tree,&p,"checkout",&values,&eligible(&tree,&p)).valid());
        tree.children[0].props.push(("read_only".into(),"true".into()));
        assert!(!assess(&tree,&p,"checkout",&values,&eligible(&tree,&p)).valid(),
            "ambiguous duplicate policy must fail closed");
    }

    #[test]
    fn cancelled_submit_does_not_reenable_click_and_cancel_props_fail_closed(){
        let mut root=n("App",&[],vec![
            n("Button",&[("form_submit","order"),("on_click","cb:30"),
                ("form_status","cancelled"),("form_request_revision","5"),
                ("form_cancel_ack_revision","5")],vec![]),
            n("Button",&[("form_cancel","order"),("on_click","cb:31")],vec![]),
        ]);
        let p=plan(&root);
        assert_eq!(submit_action(&root,&p,30),SubmitAction::Reject);
        assert!(cancel_plan(&root,&p,&HashMap::new()).acknowledged.is_empty());
        let mut hist=HashMap::new();hist.insert("order".into(),5);
        assert!(cancel_plan(&root,&p,&hist).acknowledged.contains(&30));
        assert!(cancel_plan(&root,&p,&hist).blocked.contains(&31));
        root.children[1].set_prop("form_cancel_notice","Malformed duplicate");
        assert!(cancel_plan(&root,&p,&hist).blocked.contains(&31));
        root.children[1].props.retain(|(k,_)|k!="form_cancel_notice");
        root.children[1].set_prop("form_cancel_ack_revision","5");
        assert!(cancel_plan(&root,&p,&hist).blocked.contains(&31));
        root.children[1].props.retain(|(k,_)|k!="form_cancel_ack_revision");
        root.children[0].set_prop("form_status","busy");
        root.children[0].props.retain(|(k,_)|k!="form_cancel_ack_revision");
        assert!(cancel_plan(&root,&p,&hist).blocked.contains(&31),
            "old already cancelled busy revision must remain blocked");
        root.children[0].set_prop("form_request_revision","6");
        assert!(matches!(cancel_plan(&root,&p,&hist).decisions.get(&31),
            Some(CancelDecision::Dispatch{revision:6,..})));
    }
    #[test]
    fn cancellation_terminal_requires_exact_request_and_explicit_host_ack(){
        let mut root=n("App",&[],vec![
            n("Button",&[("form_submit","signup"),("on_click","cb:11"),
                ("form_status","busy"),("form_request_revision","8")],vec![]),
            n("Button",&[("form_cancel","signup"),("on_click","cb:12")],vec![]),
        ]);
        let mut hist=HashMap::new();hist.insert("signup".into(),8);
        let p=plan(&root);
        root.children[0].set_prop("form_status","cancelled");
        assert_eq!(submit_state(&root.children[0]),SubmitState::Invalid,
            "status alone is NOT a sufficient terminal acknowledgement");
        assert!(!cancel_plan(&root,&p,&hist).acknowledged.contains(&11));
        root.children[0].set_prop("form_cancel_ack_revision","8");
        assert_eq!(submit_state(&root.children[0]),
            SubmitState::Cancelled{revision:8,notice:None});
        assert!(cancel_plan(&root,&p,&hist).acknowledged.contains(&11));
        assert_eq!(submit_action(&root,&p,11),SubmitAction::Reject,
            "cancelled submit is always nonactionable");
        assert!(cancel_plan(&root,&p,&hist).blocked.contains(&12),
            "cancel button cannot send a new cancellation after completion");
        root.children[0].set_prop("form_cancel_notice","Cancelled by service");
        assert_eq!(submit_state(&root.children[0]),
            SubmitState::Cancelled{revision:8,notice:Some("Cancelled by service")});
        root.children[0].set_prop("form_cancel_ack_revision","7");
        assert_eq!(submit_state(&root.children[0]),SubmitState::Invalid);
        root.children[0].set_prop("form_cancel_ack_revision","08");
        assert_eq!(submit_state(&root.children[0]),SubmitState::Invalid,
            "noncanonical ack must not be coerced");
    }

    #[test]
    fn acknowledgement_is_bounded_bidi_safe_and_not_reused_after_ready(){
        let mut submit=n("Button",&[("form_submit","f"),("form_status","cancelled"),
            ("form_request_revision","2"),("form_cancel_ack_revision","2"),
            ("on_click","cb:1")],vec![]);
        assert!(matches!(submit_state(&submit),SubmitState::Cancelled{revision:2,..}));
        for message in ["","\n"," leading","trailing ","bad\u{202E}rtl","bad\u{2067}rtl"]{
            submit.set_prop("form_cancel_notice",message);
            assert_eq!(submit_state(&submit),SubmitState::Invalid,
                "invalid notice content must never render an acknowledgement");
        }
        submit.set_prop("form_cancel_notice",&"a".repeat(121));
        assert_eq!(submit_state(&submit),SubmitState::Invalid);
        submit.set_prop("form_cancel_notice","בוטל לבקשתך");
        assert!(matches!(submit_state(&submit),SubmitState::Cancelled{notice:Some(_),..}));
        submit.set_prop("form_status","ready");
        assert_eq!(submit_state(&submit),SubmitState::Invalid,
            "stale cancellation metadata in ready must fail closed");
        submit.props.retain(|(k,_)|k!="form_cancel_notice"
            && k!="form_cancel_ack_revision");
        assert_eq!(submit_state(&submit),SubmitState::Ready);
        submit.set_prop("form_status","busy");
        submit.set_prop("form_cancel_ack_revision","2");
        assert_eq!(submit_state(&submit),SubmitState::Invalid,
            "stale acknowledgement in a new request must not leak");
    }

    #[test]
    fn late_ack_of_older_request_is_not_presented_as_current_completion(){
        let mut root=n("App",&[],vec![n("Button",&[
            ("form_submit","f"),("on_click","cb:10"),("form_status","cancelled"),
            ("form_request_revision","9"),("form_cancel_ack_revision","9")
        ],vec![])]);
        let p=plan(&root);
        let mut history=HashMap::new();
        assert!(cancel_plan(&root,&p,&history).acknowledged.is_empty(),
            "native window cannot attest a cancellation it never requested");
        history.insert("f".into(),10);
        assert!(!cancel_plan(&root,&p,&history).acknowledged.contains(&10),
            "stale host result for request 9 must not acknowledge newer 10");
        history.insert("f".into(),9);
        assert!(cancel_plan(&root,&p,&history).acknowledged.contains(&10));
        root.children.push(n("Button",&[
            ("form_submit","f"),("on_click","cb:11"),("form_status","cancelled"),
            ("form_request_revision","9"),("form_cancel_ack_revision","9")
        ],vec![]));
        assert!(cancel_plan(&root,&plan(&root),&history).acknowledged.is_empty(),
            "two submitters with the same form name are ambiguous");
    }

    #[test]
    fn unmount_and_remount_does_not_rearm_stale_cancel_or_rewrite_history(){
        let mut root=n("App",&[],vec![
            n("Button",&[("form_submit","f"),("form_status","busy"),
                ("form_request_revision","8"),("on_click","cb:10")],vec![]),
            n("Button",&[("form_cancel","f"),("on_click","cb:11")],vec![]),
        ]);
        let mut history=HashMap::new();history.insert("f".to_owned(),8);
        let first=cancel_plan(&root,&plan(&root),&history);
        assert!(first.blocked.contains(&11));
        let hidden=n("App",&[],vec![n("Text",&[("text","Other screen")],vec![])]);
        assert!(cancel_plan(&hidden,&plan(&hidden),&history).acknowledged.is_empty());
        root.children[0].set_prop("form_status","cancelled");
        root.children[0].set_prop("form_cancel_ack_revision","8");
        let returned=cancel_plan(&root,&plan(&root),&history);
        assert!(returned.acknowledged.contains(&10));
        assert!(returned.blocked.contains(&11));
        root.children[0].set_prop("form_status","busy");
        root.children[0].props.retain(|(k,_)|k!="form_cancel_ack_revision");
        assert!(cancel_plan(&root,&plan(&root),&history).blocked.contains(&11),
            "remount cannot reset cancellation deduplication with old epoch");
        root.children[0].set_prop("form_request_revision","9");
        assert_eq!(cancel_plan(&root,&plan(&root),&history).decisions.get(&11),
            Some(&CancelDecision::Dispatch{form:"f".into(),revision:9}));
    }

    #[test]
    fn form_cancel_emits_once_per_host_generation_and_never_replays_stale_epoch(){
        let mut root=n("App",&[],vec![
            n("Button",&[("x","20"),("y","20"),("form_submit","signup"),
                ("form_status","busy"),("form_request_revision","14"),
                ("on_click","cb:100")],vec![]),
            n("Button",&[("x","20"),("y","90"),("form_cancel","signup"),
                ("on_click","cb:101")],vec![]),
        ]);
        let p=plan(&root);
        let mut history=HashMap::new();
        let initial=cancel_plan(&root,&p,&history);
        assert_eq!(initial.decisions.get(&101),Some(&CancelDecision::Dispatch{
            form:"signup".to_string(),revision:14,
        }));
        assert!(initial.blocked.is_empty());
        // A real click is the only writer of this state. Validation is pure.
        assert!(history.is_empty());
        history.insert("signup".to_string(),14);
        let repeating=cancel_plan(&root,&p,&history);
        assert!(repeating.blocked.contains(&101));
        assert!(repeating.requested.contains(&101));
        assert_eq!(repeating.decisions.get(&101),Some(&CancelDecision::Reject));
        root.children[0].set_prop("form_status","ready");
        let ready=cancel_plan(&root,&p,&history);
        assert!(ready.blocked.contains(&101),"cancel cannot target a completed request");
        assert!(ready.requested.is_empty(),"ready is completed, not awaiting cancellation");
        root.children[0].set_prop("form_status","busy");
        let aba=cancel_plan(&root,&p,&history);
        assert!(aba.blocked.contains(&101),"host may not recycle a cancelled revision");
        root.children[0].set_prop("form_request_revision","15");
        assert_eq!(cancel_plan(&root,&p,&history).decisions.get(&101),
            Some(&CancelDecision::Dispatch{form:"signup".to_string(),revision:15}));
    }
    #[test]
    fn cancellation_rejects_unknown_form_status_revision_and_callback_alias(){
        let mut root=n("App",&[],vec![
            n("Button",&[("x","20"),("y","20"),("form_submit","signup"),
                ("form_status","busy"),("form_request_revision","1"),
                ("on_click","cb:100")],vec![]),
            n("Button",&[("x","20"),("y","90"),("form_cancel","signup"),
                ("on_click","cb:101")],vec![]),
            n("Button",&[("x","20"),("y","160"),("on_click","cb:102")],vec![]),
        ]);
        let p=plan(&root);
        for bad in ["0","01","-1","1.0","999999999999999999999999"]{
            root.children[0].set_prop("form_request_revision",bad);
            assert!(cancel_plan(&root,&p,&HashMap::new()).blocked.contains(&101),
                "bad request generation must fail closed: {bad}");
        }
        root.children[0].set_prop("form_request_revision","2");
        root.children[1].set_prop("on_click","cb:102");
        let collision=cancel_plan(&root,&p,&HashMap::new());
        assert!(collision.blocked.contains(&102),
            "shared cancel and ordinary Button identity is ambiguous");
        root.children[1].set_prop("on_click","cb:101");
        root.children[1].set_prop("form_cancel"," other");
        assert!(cancel_plan(&root,&p,&HashMap::new()).blocked.contains(&101));
        root.children[1].set_prop("form_cancel","signup");
        root.children[1].set_prop("form_submit","signup");
        assert!(cancel_plan(&root,&p,&HashMap::new()).blocked.contains(&101),
            "a single Button cannot be both submitter and canceller");
    }
    #[test]
    fn cancellation_rejects_multiple_submitters_and_foreign_busy_forms(){
        let mut root=n("App",&[],vec![
            n("Button",&[("x","20"),("y","20"),("form_submit","one"),
                ("form_status","busy"),("form_request_revision","2"),
                ("on_click","cb:10")],vec![]),
            n("Button",&[("x","20"),("y","90"),("form_submit","two"),
                ("form_status","busy"),("form_request_revision","3"),
                ("on_click","cb:20")],vec![]),
            n("Button",&[("x","20"),("y","160"),("form_cancel","one"),
                ("on_click","cb:11")],vec![]),
            n("Button",&[("x","20"),("y","230"),("form_cancel","two"),
                ("on_click","cb:21")],vec![]),
        ]);
        let p=plan(&root);
        let good=cancel_plan(&root,&p,&HashMap::new());
        assert!(!good.blocked.contains(&11));
        assert!(!good.blocked.contains(&21));
        root.children[0].set_prop("form_status","error");
        root.children[0].set_prop("form_error","Request finished");
        let separate=cancel_plan(&root,&p,&HashMap::new());
        assert!(separate.blocked.contains(&11));
        assert!(!separate.blocked.contains(&21));
        root.children.push(n("Button",&[("x","300"),("y","20"),
            ("form_submit","two"),("form_status","busy"),
            ("form_request_revision","3"),("on_click","cb:22")],vec![]));
        let p=plan(&root);
        let double=cancel_plan(&root,&p,&HashMap::new());
        assert!(double.blocked.contains(&21),
            "two submitters for same form are unresolvable request identities");
    }
    #[test]
    fn cancellation_history_has_bounded_memory_and_does_not_mutate_ui_tree(){
        let root=n("App",&[],vec![
            n("Button",&[("x","20"),("y","20"),("form_submit","signup"),
                ("form_status","busy"),("form_request_revision","9"),
                ("on_click","cb:90")],vec![]),
            n("Button",&[("x","20"),("y","90"),("form_cancel","signup"),
                ("on_click","cb:91")],vec![]),
        ]);
        let before=root.clone();
        let p=plan(&root);
        let mut history:HashMap<String,u64>=(0..32).map(|i|
            (format!("older-{i}"),1)).collect();
        let rejected=cancel_plan(&root,&p,&history);
        assert!(rejected.blocked.contains(&91));
        history.insert("signup".to_string(),2);
        assert!(!cancel_plan(&root,&p,&history).blocked.contains(&91),
            "an existing tracked form may advance even when cache is otherwise full");
        assert_eq!(root.children[0].props,before.children[0].props);
        assert_eq!(root.children[1].props,before.children[1].props);
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
