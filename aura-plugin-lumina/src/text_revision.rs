//! Opt-in controlled TextInput value authority protocol.
//!
//! value_revision (monotonic u64): ONLY a strictly higher revision performs
//! an external authoritative replacement. An unmodified revision plus value
//! equal to a locally emitted text is an observational echo, NOT an ACK ID:
//! UiTextInputEvent does not carry unique sequence IDs. Unknown values at
//! the same revision fail closed, never erase a newer draft.
//! Legacy (no revision prop) preserves previous changed-value authority.
//! No IME/BiDi/accessibility/platform certification implied.

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum Decision { KeepDraft, Rebase }
#[derive(Clone,Debug)]
struct Emission { local_seq:u64, value:String }
#[derive(Clone,Debug)]
pub(crate) struct RevisionTracker {
    version:Option<u64>,
    last_seen:String,
    emissions:Vec<Emission>,
    next_seq:u64,
    acknowledged_seq:u64,
    bytes:usize,
}
const MAX_ECHOES:usize=32;
const MAX_ECHO_BYTES:usize=128*1024;
impl RevisionTracker {
    pub(crate) fn new(value:&str,revision:Option<u64>)->Self {
        Self {
            version:revision,last_seen:value.to_owned(),
            emissions:Vec::new(),next_seq:0,acknowledged_seq:0,bytes:0,
        }
    }
    pub(crate) fn record_local_edit(&mut self, value:&str) {
        if self.version.is_none() { return; }
        self.next_seq=self.next_seq.saturating_add(1);
        if value.len()>MAX_ECHO_BYTES { return; } // oversize remains safe to ignore
        self.bytes=self.bytes.saturating_add(value.len());
        self.emissions.push(Emission{local_seq:self.next_seq,value:value.to_owned()});
        while self.emissions.len()>MAX_ECHOES || self.bytes>MAX_ECHO_BYTES {
            let first=self.emissions.remove(0);
            self.bytes=self.bytes.saturating_sub(first.value.len());
        }
    }
    pub(crate) fn observe(&mut self, value:&str,revision:Option<u64>)->Decision {
        match (self.version,revision) {
            (Some(old),Some(next)) if next<old =>
                Decision::KeepDraft, // stale version, never decrease revision
            (Some(old),Some(next)) if next>old => {
                self.rebase(value,revision);
                Decision::Rebase
            }
            (Some(_),Some(_)) => {
                if self.last_seen==value { return Decision::KeepDraft; }
                // Multiple local emissions may have identical values after
                // undo/redo. Prefer the newest matching emission.
                if let Some(found)=self.emissions.iter().rev()
                    .find(|x|x.value==value).map(|x|x.local_seq) {
                    if found>self.acknowledged_seq {
                        self.acknowledged_seq=found;
                        self.last_seen=value.to_owned();
                    }
                    // Earlier out-of-order echoes are not authoritative.
                }
                // Unrecognized same-revision values cannot become authority.
                Decision::KeepDraft
            }
            (None,None) => {
                if self.last_seen==value {Decision::KeepDraft}
                else {
                    self.rebase(value,None);
                    Decision::Rebase
                }
            }
            _ => {
                // Mode transition is explicit; do not reinterpret a previous
                // legacy value as an unacknowledged versioned local echo.
                self.rebase(value,revision);
                Decision::Rebase
            }
        }
    }
    fn rebase(&mut self,value:&str,rev:Option<u64>) {
        self.version=rev;
        self.last_seen=value.to_owned();
        self.emissions.clear();
        self.next_seq=0;
        self.acknowledged_seq=0;
        self.bytes=0;
    }
    pub(crate) fn version(&self)->Option<u64>{self.version}
    pub(crate) fn echoed(&self)->&str{&self.last_seen}
    #[cfg(test)]
    fn emission_count(&self)->usize{self.emissions.len()}
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn legacy_changed_prop_remains_authority(){
    let mut r=RevisionTracker::new("A",None);
    assert_eq!(r.observe("A",None),Decision::KeepDraft);
    assert_eq!(r.observe("REMOTE",None),Decision::Rebase);
    assert_eq!(r.echoed(),"REMOTE");
 }
 #[test] fn delayed_echo_cannot_replace_newer_draft(){
    let mut r=RevisionTracker::new("A",Some(7));
    r.record_local_edit("Aq");
    r.record_local_edit("Aqe");
    assert_eq!(r.observe("Aq",Some(7)),Decision::KeepDraft);
    assert_eq!(r.observe("Aqe",Some(7)),Decision::KeepDraft);
    assert_eq!(r.echoed(),"Aqe");
    assert_eq!(r.observe("Aq",Some(7)),Decision::KeepDraft);
    assert_eq!(r.echoed(),"Aqe");
 }
 #[test] fn unknown_same_revision_is_never_authoritative(){
    let mut r=RevisionTracker::new("A",Some(2));
    r.record_local_edit("Ab");
    assert_eq!(r.observe("INJECTED",Some(2)),Decision::KeepDraft);
    assert_eq!(r.echoed(),"A");
 }
 #[test] fn strictly_increasing_revision_overrides_and_erases_pending(){
    let mut r=RevisionTracker::new("A",Some(10));
    r.record_local_edit("Aq");
    assert_eq!(r.observe("SERVER",Some(11)),Decision::Rebase);
    assert_eq!(r.echoed(),"SERVER");
    assert_eq!(r.emission_count(),0);
    assert_eq!(r.observe("Aq",Some(10)),Decision::KeepDraft);
    assert_eq!(r.version(),Some(11));
 }
 #[test] fn mode_transition_explicit_both_directions(){
    let mut r=RevisionTracker::new("A",None);
    assert_eq!(r.observe("B",Some(1)),Decision::Rebase);
    r.record_local_edit("Bc");
    assert_eq!(r.observe("C",None),Decision::Rebase);
    assert_eq!(r.emission_count(),0);
 }
 #[test] fn bounded_pending_echoes_and_oversized_values_fail_closed(){
    let mut r=RevisionTracker::new("",Some(1));
    for n in 0..100 {r.record_local_edit(&format!("value-{n}"));}
    assert_eq!(r.emission_count(),32);
    r.record_local_edit(&"x".repeat(128*1024+1));
    assert_eq!(r.emission_count(),32);
    assert_eq!(r.observe("value-0",Some(1)),Decision::KeepDraft);
    assert_eq!(r.echoed(),"");
 }
 #[test] fn duplicate_text_emission_prefers_latest_receipt(){
    let mut r=RevisionTracker::new("A",Some(1));
    r.record_local_edit("B");r.record_local_edit("A");r.record_local_edit("B");
    assert_eq!(r.observe("B",Some(1)),Decision::KeepDraft);
    assert_eq!(r.echoed(),"B");
    assert_eq!(r.observe("A",Some(1)),Decision::KeepDraft);
    assert_eq!(r.echoed(),"B");
 }
 #[test] fn overflow_sequence_saturates_without_panicking(){
    let mut r=RevisionTracker::new("",Some(u64::MAX));
    r.next_seq=u64::MAX;
    r.record_local_edit("a");
    assert_eq!(r.observe("REMOTE",Some(u64::MAX)),Decision::KeepDraft);
    assert_eq!(r.observe("REMOTE",Some(0)),Decision::KeepDraft);
 }
}
