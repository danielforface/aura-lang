//! Native opt-in controlled form submission (no Core ABI or Nexus schema change).
//! A Button(form_submit: "...") emits its normal on_click only when all
//! associated TextInput(form: "...") values satisfy bounded local rules.
//! The host remains authoritative over persisted values and revisions.
use std::collections::{HashMap,HashSet};
use aura_nexus::UiNode;
use unicode_segmentation::UnicodeSegmentation;
use crate::layout::LayoutNode;

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
                    && matches!(prop(node,"control"),None|Some("button")){
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
}
impl FieldError {
    pub(crate) fn label(self)->&'static str{
        match self{
            Self::Required=>"Required",
            Self::TooShort=>"Too short",
            Self::TooLong=>"Too long",
            Self::InvalidRule=>"Invalid rule",
            Self::Unavailable=>"Unavailable",
        }
    }
}
#[derive(Clone,Debug,Default,PartialEq,Eq)]
pub(crate) struct Assessment {
    pub(crate) errors:Vec<(u64,FieldError)>,
    pub(crate) unavailable:bool,
    pub(crate) count:usize,
}
impl Assessment{
    pub(crate) fn valid(&self)->bool{
        self.count>0 && !self.unavailable && self.errors.is_empty()
    }
    pub(crate) fn first_focusable_error(&self,eligible:&HashSet<u64>)->Option<u64>{
        self.errors.iter().find_map(|(id,_)|eligible.contains(id).then_some(*id))
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
