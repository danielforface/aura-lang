//! Opt-in Linux libibus *native* engine protocol adapter. A separate local
//! helper, built from native/ibus_input_helper.c, owns a real IBusInputContext
//! on the user's session bus. It forwards actual daemon engine signals over
//! a framed ASCII/hex stdin/stdout pipe. NO unsafe Rust or Cargo deps.
//! Never confuse ordinary GLFW GetCharPressed with IME lifecycle events.
use crate::CompositionEvent;
use std::io::{BufRead,BufReader,Write};
use std::process::{Child,ChildStdin,Command,Stdio};
use std::sync::mpsc::{self,Receiver};
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;

const MAX_LINES_PER_KEY:usize=24;
const MAX_PROTOCOL_LINE:usize=4096;
const KEY_TIMEOUT_MS:u64=500;
const STARTUP_TIMEOUT_MS:u64=3500;
const MAX_TEXT_BYTES:usize=crate::text_input::MAX_PREEDIT_BYTES;

// Only modifier states sampled from Raylib are represented here.
// The hardware scancode and a complete XKB layout are NOT available.
pub(crate) const MOD_SHIFT:u32=1;
pub(crate) const MOD_CONTROL:u32=2;
pub(crate) const MOD_ALT:u32=4;
pub(crate) const MOD_SUPER:u32=8;
const VALID_MODIFIERS:u32=MOD_SHIFT|MOD_CONTROL|MOD_ALT|MOD_SUPER;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) struct NativeKeyEvent{
    pub(crate) keysym:u32,
    pub(crate) modifiers:u32,
    pub(crate) released:bool,
}
impl NativeKeyEvent{
    /// X11 Unicode keysym, not a fabricated physical hardware keycode.
    pub(crate) fn from_char(ch:char,modifiers:u32)->Option<Self>{
        if ch.is_control() || modifiers & !VALID_MODIFIERS != 0 {return None;}
        let scalar=ch as u32;
        let keysym=if scalar<=0xff{scalar}else{0x0100_0000|scalar};
        Some(Self{keysym,modifiers,released:false})
    }
    fn valid(self)->bool{
        self.modifiers & !VALID_MODIFIERS == 0 &&
        ((self.keysym>=0x20 && self.keysym<=0xff) ||
         (self.keysym>=0xff00 && self.keysym<=0xffff) ||
         (self.keysym>=0x0100_0100 && self.keysym<=0x0110_ffff))
    }
}
/// Engine-owned lookup table snapshot; never an authoritative EditSession edit.
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct IbusCandidatePage{
    pub(crate) callback_id:u64,
    pub(crate) cursor:usize,
    pub(crate) page_start:usize,
    pub(crate) total:usize,
    pub(crate) candidates:Vec<String>,
}
fn parse_lookup_frame(line:&str,callback_id:u64)->Option<Option<IbusCandidatePage>>{
    if line=="L\t0"{return Some(None);}
    let mut fields=line.split('\t');
    if fields.next()!=Some("L") || fields.next()!=Some("1"){return None;}
    let cursor=fields.next()?.parse::<usize>().ok()?;
    let page_start=fields.next()?.parse::<usize>().ok()?;
    let total=fields.next()?.parse::<usize>().ok()?;
    if total==0||total>100000||cursor>=total||page_start>cursor{return None;}
    let encoded:Vec<&str>=fields.collect();
    if encoded.is_empty()||encoded.len()>8||page_start.checked_add(encoded.len())?>total||
        cursor>=page_start.saturating_add(16){return None;}
    let mut candidates=Vec::with_capacity(encoded.len());
    for raw in encoded{
        if raw.len()>256 || raw.is_empty(){return None;}
        let v=decode_hex(raw)?;
        if v.is_empty() || v.chars().any(|c|c.is_control()){return None;}
        candidates.push(v);
    }
    Some(Some(IbusCandidatePage{callback_id,cursor,page_start,total,candidates}))
}
/// NOT a public synthetic CompositionEvent. Created only by the real
/// libibus helper after the same physical key emits one bounded D before C.
#[derive(Clone,Debug)]
pub(crate) struct NativeReplace{
    pub(crate) callback_id:u64,
    pub(crate) sequence:u64,
    pub(crate) authority_epoch:u64,
    pub(crate) expected:String,
    pub(crate) caret_scalar:u32,
    pub(crate) anchor_scalar:u32,
    pub(crate) offset:i32,
    pub(crate) count:u32,
    pub(crate) text:String,
}
pub(crate) struct NativeIbus{
    child:Child,
    input:ChildStdin,
    output:Receiver<String>,
    focus:Option<u64>,
    serial:u64,
    composing:bool,
    disconnected:bool,
    surrounding_enabled:bool,
    /// Number of actual engine-produced nonempty preedit/commit callbacks.
    pub(crate) engine_events:usize,
    pub(crate) key_acks:usize,
    pub(crate) release_acks:usize,
    pub(crate) shifted_acks:usize,
    pub(crate) candidate_events:usize,
    pub(crate) candidate_click_acks:usize,
    pub(crate) cursor_location_acks:usize,
    pub(crate) cursor_location:Option<(u32,u32,u32,u32)>,
    pub(crate) surrounding_text_acks:usize,
    pub(crate) surrounding_delete_requests:usize,
    /// Last real bounded D frame from this IBus engine focus generation.
    pub(crate) surrounding_last_delete:Option<(i32,u32)>,
    pub(crate) surrounding_requires:usize,
    pub(crate) surrounding_text:Option<(String,u32,u32)>,
    /// An acknowledged, focused screen-context's current host edit epoch.
    context_epoch:Option<u64>,
    authority_generation:Option<(u64,u64)>,
    native_replacements:Vec<NativeReplace>,
    pub(crate) native_replacement_pairs:usize,
    pub(crate) candidate_page:Option<IbusCandidatePage>,
    /// Bounded, content-free counts of actual frames ignored while awaiting
    /// a non-key ACK. Only compiled into diagnostic builds, never production.
    #[cfg(feature="lumina-diagnostics")]
    diagnostic_ack_advisory_deletes:usize,
    #[cfg(feature="lumina-diagnostics")]
    diagnostic_ack_discarded_commits:usize,
}
fn decode_hex(input:&str)->Option<String>{
    if input.len()%2!=0 || input.len()>MAX_TEXT_BYTES*2{return None;}
    let mut bytes=Vec::with_capacity(input.len()/2);
    for ch in input.as_bytes().chunks_exact(2) {
        let nibble=|v:u8|->Option<u8>{
            match v{b'0'..=b'9'=>Some(v-b'0'),b'a'..=b'f'=>Some(v-b'a'+10),_=>None}
        };
        bytes.push(nibble(ch[0])?*16+nibble(ch[1])?);
    }
    String::from_utf8(bytes).ok()
}
fn grapheme_from_scalar(text:&str,scalar:usize)->Option<usize>{
    let count=text.chars().count();
    if scalar>count{return None;}
    let byte=if scalar==count{text.len()}else{
        text.char_indices().nth(scalar)?.0
    };
    Some(text.grapheme_indices(true).take_while(|(at,_)|*at<byte).count())
}
fn parse_surrounding_delete_frame(line:&str)->Option<(i32,u32)>{
    let rest=line.strip_prefix("D\t")?;
    let mut fields=rest.split('\t');
    let (Some(offset),Some(count),None)=
        (fields.next(),fields.next(),fields.next())else{return None;};
    let (Ok(offset),Ok(count))=(offset.parse::<i32>(),count.parse::<u32>())else{
        return None;
    };
    ((-1024..=1024).contains(&offset)&&count>0&&count<=1024)
        .then_some((offset,count))
}
/// This guard never fabricates an engine transaction ID. It only narrows
/// the proven SAME ProcessKeyEvent signal order; asynchronous cross-key
/// deletes have no authority and remain unsupported.
fn same_key_replace_pair(lines:&[String])->Option<(i32,u32)>{
    let (mut delete,mut commit)=(None,None);
    for (i,line) in lines.iter().enumerate(){
        if line.starts_with("D\t"){
            let (offset,count)=parse_surrounding_delete_frame(line)?;
            if delete.replace((i,offset,count)).is_some(){return None;}
        }else if line.starts_with("C\t"){
            if commit.replace(i).is_some(){return None;}
        }else if line=="B" || line.starts_with("P\t"){
            return None;
        }
    }
    let (at,offset,count)=delete?;
    (at<commit?).then_some((offset,count))
}
/// The helper emits B immediately BEFORE calling the actual native
/// ProcessKeyEvent. Signals pumped from GLib before B are late/idle and
/// must never be attributed to this new physical key. B is a LOCAL protocol
/// fence, not a provenance ID invented for the external IME engine.
/// Signals that race AFTER B are not claimed to have universal provenance.
#[derive(Default)]
struct KeyDispatchFence { begun:bool }
impl KeyDispatchFence {
    /// Some(false) = discard stale pre-dispatch frame / consume B.
    /// Some(true) = frame belongs to the bounded dispatch observation window.
    /// None = malformed marker order, R before B or unsupported wire frame.
    fn admit(&mut self,line:&str)->Option<bool>{
        if !self.begun{
            if line=="B"{self.begun=true;return Some(false);}
            if line=="N"||line=="H"||line.starts_with("D\t")||
                line.starts_with("C\t")||line.starts_with("P\t")||
                line.starts_with("L\t"){
                return Some(false);
            }
            return None;
        }
        (line!="B").then_some(true)
    }
}
/// Fixed recovery budget per explicitly authorized host revision.
/// Never restart a failed native child in an unbounded automatic loop;
/// retries advance only on subsequent rendered UI frames.
#[derive(Clone,Debug,Default)]
pub(crate) struct IbusReconnectBudget{
    used:u8,
    countdown:u16,
    armed:bool,
    /// Previous trusted App ibus_retry_revision. A first observation is
    /// BASELINE ONLY, not permission to restart a missing helper.
    host_revision:Option<u64>,
}
impl IbusReconnectBudget{
    const MAX_ATTEMPTS:u8=3;
    const WAIT_FRAMES:[u16;3]=[24,72,144];
    /// Read-only per-authorized-epoch status; counts bounded attempts,
    /// never publishes input text, callback identifiers or native keys.
    /// On an explicit host rearm this count resets for the NEW finite epoch.
    pub(crate) fn diagnostic_status(&self)->(u8,bool){
        (self.used,self.used>=Self::MAX_ATTEMPTS)
    }
    /// Starts a fresh cooldown only after a real child was lost or an
    /// attempted connection failed. Does not retry unconfigured startup.
    pub(crate) fn arm(&mut self){
        if !self.armed && self.used<Self::MAX_ATTEMPTS{
            self.countdown=Self::WAIT_FRAMES[self.used as usize];
            self.armed=true;
        }
    }
    /// An explicit, strictly increasing host revision authorizes ONE fresh
    /// 3-attempt budget only if the helper is missing. No change, rollback,
    /// duplicate root props, invalid revision, or unobserved initial version
    /// may silently reset the breaker. This never changes native privacy.
    pub(crate) fn observe_host_retry_revision(
        &mut self,revision:u64,missing_helper:bool
    )->bool{
        match self.host_revision {
            None=>{
                self.host_revision=Some(revision);
                false
            }
            Some(last) if revision>last=>{
                self.host_revision=Some(revision);
                if !missing_helper{return false;}
                self.used=0;
                self.countdown=0;
                self.armed=false;
                self.arm();
                true
            }
            _=>false,
        }
    }
    /// At most one reconnect attempt in any frame; three attempts per
    /// host-authorized recovery epoch including its natural initial epoch.
    pub(crate) fn tick(&mut self)->bool{
        if !self.armed{return false;}
        if self.countdown>0{self.countdown-=1;return false;}
        self.armed=false;
        self.used=self.used.saturating_add(1);
        true
    }
}
impl NativeIbus{
    pub(crate) fn connect()->Option<Self>{
        let executable=std::env::var_os("LUMINA_IBUS_HELPER_BIN")?;
        let engine=std::env::var("LUMINA_IBUS_ENGINE").ok()?;
        if engine.len()>80||engine.is_empty()||
           !engine.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'_'|b'-'|b':'|b'.')){
            return None;
        }
        let mut child=Command::new(executable).arg(engine)
            .stdin(Stdio::piped()).stdout(Stdio::piped())
            .stderr(Stdio::inherit()).spawn().ok()?;
        let input=child.stdin.take()?;
        let stdout=child.stdout.take()?;
        let (tx,output)=mpsc::sync_channel(32);
        std::thread::spawn(move||{
            let mut reader=BufReader::new(stdout);
            loop {
                // BufRead::read_line can allocate an unbounded line *before*
                // the caller checks length. Read only bounded chunks instead.
                let mut raw=Vec::new();
                loop {
                    let Ok(available)=reader.fill_buf()else{return;};
                    if available.is_empty(){return;}
                    let end=available.iter().position(|v|*v==b'\n')
                        .map(|n|n+1).unwrap_or(available.len());
                    if raw.len().saturating_add(end)>MAX_PROTOCOL_LINE{return;}
                    let terminates=available[end-1]==b'\n';
                    raw.extend_from_slice(&available[..end]);
                    reader.consume(end);
                    if terminates{break;}
                }
                let Ok(line)=String::from_utf8(raw)else{return;};
                if tx.send(line.trim_end_matches(['\r','\n']).to_owned()).is_err(){return;}
            }
        });
        let ready=output.recv_timeout(Duration::from_millis(STARTUP_TIMEOUT_MS));
        if ready.as_deref()!=Ok("READY") {
            let _=child.kill();let _=child.wait();return None;
        }
        Some(Self{child,input,output,focus:None,serial:0,
            composing:false,disconnected:false,surrounding_enabled:false,engine_events:0,
            key_acks:0,release_acks:0,shifted_acks:0,
            candidate_events:0,candidate_click_acks:0,candidate_page:None,
            cursor_location_acks:0,cursor_location:None,
            surrounding_text_acks:0,surrounding_delete_requests:0,
            surrounding_last_delete:None,
            surrounding_requires:0,surrounding_text:None,
            context_epoch:None,authority_generation:None,
            native_replacements:Vec::new(),native_replacement_pairs:0,
            #[cfg(feature="lumina-diagnostics")]
            diagnostic_ack_advisory_deletes:0,
            #[cfg(feature="lumina-diagnostics")]
            diagnostic_ack_discarded_commits:0})
    }
    fn send(&mut self,command:&str)->bool {
        if self.disconnected {return false;}
        if self.input.write_all(command.as_bytes()).is_err() ||
            self.input.write_all(b"\n").is_err() ||
            self.input.flush().is_err(){
            self.disconnected=true;return false;
        }
        true
    }
    fn next(&mut self)->Option<String>{
        let line=self.output.recv_timeout(Duration::from_millis(KEY_TIMEOUT_MS)).ok()?;
        if line.len()>MAX_PROTOCOL_LINE{self.disconnected=true;return None;}
        Some(line)
    }
    fn observe_surrounding_signal(&mut self,line:&str)->bool{
        if line=="N"{
            self.surrounding_requires=self.surrounding_requires.saturating_add(1);
            return true;
        }
        if line=="D\t0\t0"{
            // Genuine upstream ibus-hangul sends an empty DeleteSurroundingText
            // before the first syllable when preedit-mode=none. Valid no-op:
            // do not disconnect, count it as a deletion, or touch EditSession.
            return true;
        }
        if line.starts_with("D\t"){
            let Some((offset,count))=parse_surrounding_delete_frame(line)
                else{return false;};
            // Engine-origin advisory only. Record bounded evidence and focus
            // epoch; NEVER apply deletion to the host-owned editor here.
            if self.focus.is_some() && self.surrounding_text.is_some(){
                self.surrounding_delete_requests=self.surrounding_delete_requests.saturating_add(1);
                self.surrounding_last_delete=Some((offset,count));
            }
            return true;
        }
        false
    }
    /// Neutral daemon-generation health check: pumps libibus real
    /// disconnected signal while idle. FATAL permanently poisons this child.
    pub(crate) fn probe_daemon_generation(&mut self)->bool{
        if self.disconnected || !self.send("T") || !self.await_ack(){
            self.disconnected=true;
            self.context_epoch=None;
            self.native_replacements.clear();
            return false;
        }
        true
    }
    /// Explicit privacy fence. The native helper starts with surrounding
    /// disabled. An ACKed switch invalidates all previous editor authority.
    pub(crate) fn set_surrounding_enabled(&mut self,enabled:bool)->bool{
        if self.disconnected{return false;}
        if self.surrounding_enabled==enabled{return true;}
        self.context_epoch=None;
        self.surrounding_text=None;
        self.surrounding_last_delete=None;
        self.native_replacements.clear();
        self.candidate_page=None;
        self.composing=false;
        self.authority_generation=None;
        if !self.send(if enabled{"V1"}else{"V0"}) || !self.await_ack(){
            self.disconnected=true;return false;
        }
        self.surrounding_enabled=enabled;
        true
    }
    /// UTF-8 content remains owned by host editor; IBus positions are
    /// Unicode scalar indices (not bytes or graphemes). Entire snapshot is
    /// bounded to 1024 bytes; oversize text is NEVER partially/truncated.
    pub(crate) fn sync_surrounding_text(
        &mut self,callback_id:u64,content:&str,caret:u32,anchor:u32
    )->bool{
        if self.disconnected || !self.surrounding_enabled ||
            self.focus!=Some(callback_id) || content.len()>1024 || content.contains('\0') ||
            content.chars().any(|c|c.is_control() ||
                matches!(c,'\u{202A}'..='\u{202E}'|'\u{2066}'..='\u{2069}')) {
            return false;
        }
        let chars=content.chars().count();
        if caret as usize>chars || anchor as usize>chars {return false;}
        if self.surrounding_text.as_ref().is_some_and(|t|
            t.0==content&&t.1==caret&&t.2==anchor){return true;}
        let mut hex=String::with_capacity(content.len()*2);
        for byte in content.as_bytes(){
            use std::fmt::Write as _;
            let _=write!(&mut hex,"{byte:02x}");
        }
        self.context_epoch=None;
        if !self.send(&format!("S\t{hex}\t{caret}\t{anchor}"))||
            !self.await_ack(){return false;}
        self.surrounding_text=Some((content.to_owned(),caret,anchor));
        self.surrounding_text_acks=self.surrounding_text_acks.saturating_add(1);
        true
    }
    #[cfg(feature="lumina-diagnostics")]
    pub(crate) fn diagnostic_ack_drop_counts(&self)->(usize,usize){
        (self.diagnostic_ack_advisory_deletes,
         self.diagnostic_ack_discarded_commits)
    }
    fn await_ack(&mut self)->bool{
        for _ in 0..MAX_LINES_PER_KEY {
            let Some(line)=self.next() else{
                self.disconnected=true;return false;
            };
            if line=="A"{return true;}
            // Observational counters ONLY; a late D stays advisory and a
            // late C remains untrusted. No user text, key or callback ID
            // enters these counters; ordinary production has no fields.
            #[cfg(feature="lumina-diagnostics")]
            {
                if line.starts_with("D\t"){
                    self.diagnostic_ack_advisory_deletes=
                        self.diagnostic_ack_advisory_deletes.saturating_add(1);
                }else if line.starts_with("C\t"){
                    self.diagnostic_ack_discarded_commits=
                        self.diagnostic_ack_discarded_commits.saturating_add(1);
                }
            }
            if self.observe_surrounding_signal(&line){continue;}
            // Some IBus engines emit hide-preedit during FocusOut/FocusIn.
            // Such events belong to the OLD focus generation and must not
            // be delivered to the newly focused native TextInput.
            if line=="H"||line.starts_with("P\t")||line.starts_with("C\t")||line.starts_with("L\t"){
                continue;
            }
            self.disconnected=true;return false;
        }
        self.disconnected=true;false
    }
    /// Focus is actually passed to libibus; suppress stale engine signals
    /// after blur. A newly focused session must own subsequent signals.
    pub(crate) fn sync_focus(&mut self,target:Option<u64>)->bool{
        if self.disconnected{return false;}
        if self.focus==target{return true;}
        if self.focus.is_some(){
            if !self.send("F0"){return false;}
            if !self.await_ack(){return false;}
        }
        self.composing=false;
        self.candidate_page=None;
        self.cursor_location=None;
        self.surrounding_text=None;
        self.context_epoch=None;
        self.authority_generation=None;
        self.native_replacements.clear();
        self.surrounding_last_delete=None;
        self.focus=None;
        if let Some(id)=target{
            if !self.send("F1"){return false;}
            if !self.await_ack(){return false;}
            self.focus=Some(id);
        }
        true
    }
    /// A newer host revision invalidates the input-method's own previous
    /// composition, even if new host text happens to be byte-for-byte equal.
    /// Local edits leave the engine running; only host rebase resets it.
    pub(crate) fn sync_authority_generation(
        &mut self,callback_id:u64,generation:u64
    )->bool{
        if self.disconnected||self.focus!=Some(callback_id){return false;}
        if self.authority_generation==Some((callback_id,generation)){return true;}
        if self.authority_generation.is_some() &&
            (!self.sync_focus(None)||!self.sync_focus(Some(callback_id))){
            return false;
        }
        self.authority_generation=Some((callback_id,generation));
        true
    }
    /// Epoch is obtained from the actual editor in the same focused frame,
    /// *after* the helper ACKed the matching immutable surrounding snapshot.
    pub(crate) fn mark_context_epoch(&mut self,callback_id:u64,epoch:u64)->bool{
        if self.disconnected||!self.surrounding_enabled||
            self.focus!=Some(callback_id)||self.surrounding_text.is_none(){self.context_epoch=None;return false;}
        self.context_epoch=Some(epoch);
        true
    }
    pub(crate) fn take_native_replacements(&mut self)->Vec<NativeReplace>{
        std::mem::take(&mut self.native_replacements)
    }
    /// Diagnostic-only REAL child-process termination. No fake libibus
    /// signals, protocol frames or composition events are injected.
    #[cfg(feature="lumina-diagnostics")]
    pub(crate) fn terminate_helper_for_diagnostic(&mut self)->bool{
        self.child.kill().is_ok()
    }
    /// Preserve monotonically increasing native editor event sequences across
    /// real helper process generations; a restarted child must not reuse the
    /// old EditSession replay barrier even when bytes/caret match.
    pub(crate) fn last_serial(&self)->u64{self.serial}
    pub(crate) fn set_serial_floor(&mut self,serial:u64){
        self.serial=self.serial.max(serial);
    }
    pub(crate) fn is_disconnected(&mut self)->bool{
        self.disconnected || matches!(self.child.try_wait(),Ok(Some(_))|Err(_))
    }
    /// Native IBus expects SCREEN coords, never logical window-local coords.
    /// Reject invalid/out-of-screen rectangles before the strictly decoded
    /// helper command; cache an acknowledged location only within this focus.
    pub(crate) fn set_cursor_location(
        &mut self, callback_id:u64,rect:(u32,u32,u32,u32)
    )->bool {
        if self.disconnected||self.focus!=Some(callback_id){return false;}
        let (x,y,w,h)=rect;
        if x>32767||y>32767||w==0||h==0||w>128||h>128||
            x.checked_add(w).is_none_or(|right|right>32768)||
            y.checked_add(h).is_none_or(|bottom|bottom>32768) {
            return false;
        }
        if self.cursor_location==Some(rect){return true;}
        if !self.send(&format!("G\t{x}\t{y}\t{w}\t{h}")) || !self.await_ack(){
            return false;
        }
        self.cursor_location=Some(rect);
        self.cursor_location_acks=self.cursor_location_acks.saturating_add(1);
        true
    }
    pub(crate) fn has_composition(&self)->bool{self.composing}
    pub(crate) fn candidate_snapshot(&self)->Option<IbusCandidatePage>{self.candidate_page.clone()}
    /// A popup mouse click is translated into bounded *real* engine navigation
    /// and a physical-equivalent Space key. No synthetic CommitText/Undo event.
    /// libibus IBusInputContext exposes no direct candidate_clicked client API;
    /// mouse selection through the daemon's panel interface is not claimed.
    pub(crate) fn select_candidate_via_engine(
        &mut self,callback_id:u64,absolute:usize
    )->Option<Vec<CompositionEvent>> {
        let page=self.candidate_snapshot()?;
        if self.focus!=Some(callback_id)||page.callback_id!=callback_id||
            absolute<page.page_start||
            absolute>=page.page_start.checked_add(page.candidates.len())? {
            return None;
        }
        let expected=page.candidates.get(absolute-page.page_start)?.clone();
        let mut out=Vec::new();
        // Select at most eight visible rows; do not silently wander across
        // unbounded engine lookup pages or trust the screen alone.
        for _ in 0..8 {
            let current=self.candidate_snapshot()?;
            if current.callback_id!=callback_id{return None;}
            if current.cursor==absolute{break;}
            let keysym=if current.cursor<absolute{0xff54}else{0xff52};
            let (handled,events)=self.process_ibus_key_event(
                NativeKeyEvent{keysym,modifiers:0,released:false},Some(callback_id))?;
            out.extend(events);
            let after=self.candidate_snapshot()?;
            if !handled||after.cursor==current.cursor{return None;}
        }
        let selected=self.candidate_snapshot()?;
        if selected.cursor!=absolute ||
            selected.candidates.get(absolute.checked_sub(selected.page_start)?)!=Some(&expected){
            return None;
        }
        let (handled,events)=self.process_ibus_key_event(
            NativeKeyEvent{keysym:0x20,modifiers:0,released:false},Some(callback_id))?;
        let committed=events.iter().any(|e|matches!(e,CompositionEvent::Commit{..}));
        out.extend(events);
        if !handled || !committed || self.candidate_page.is_some(){return None;}
        self.candidate_click_acks=self.candidate_click_acks.saturating_add(1);
        Some(out)
    }
    fn add(&mut self,event:CompositionEvent,output:&mut Vec<CompositionEvent>){
        if output.len()<MAX_LINES_PER_KEY{output.push(event);}
    }
    fn new_serial(&mut self)->Option<u64>{
        self.serial=self.serial.checked_add(1)?;
        Some(self.serial)
    }
    /// Keysym, sampled modifier state and release bit are passed to libibus.
    /// Unknown physical scancode is explicitly zero, not guessed from ASCII.
    pub(crate) fn process_ibus_key_event(
        &mut self,key:NativeKeyEvent,target:Option<u64>
    )->Option<(bool,Vec<CompositionEvent>)>{
        if !self.surrounding_enabled || !key.valid() || !self.sync_focus(target){return None;}
        let cb=target?;
        let context_at_key=self.surrounding_text.clone();
        let epoch_at_key=self.context_epoch;
        if !self.send(&format!("K\t{}\t{}\t{}\t0",
            key.keysym,key.modifiers,u8::from(key.released))){return None;}
        let mut lines=Vec::new();
        let mut handled=None;
        let mut dispatch=KeyDispatchFence::default();
        for _ in 0..MAX_LINES_PER_KEY{
            let Some(line)=self.next()else{
                self.disconnected=true;return Some((true,Vec::new()));
            };
            let Some(in_dispatch)=dispatch.admit(&line)else{
                self.disconnected=true;return Some((true,Vec::new()));
            };
            if !in_dispatch{continue;}
            if let Some(bit)=line.strip_prefix("R\t"){
                handled=match bit{"1"=>Some(true),"0"=>Some(false),_=>None};
                if handled.is_some(){
                    self.key_acks=self.key_acks.saturating_add(1);
                    if key.released{self.release_acks=self.release_acks.saturating_add(1);}
                    if key.modifiers & MOD_SHIFT != 0{
                        self.shifted_acks=self.shifted_acks.saturating_add(1);
                    }
                }
                break;
            }
            if line.starts_with("D\t"){
                if !self.observe_surrounding_signal(&line){
                    self.disconnected=true;return Some((true,Vec::new()));
                }
                if line!="D\t0\t0"{lines.push(line);}
                continue;
            }
            if self.observe_surrounding_signal(&line){continue;}
            if line=="H"||line.starts_with("P\t")||line.starts_with("C\t")||line.starts_with("L\t"){
                lines.push(line);
            }else{self.disconnected=true;return Some((true,Vec::new()));}
        }
        let Some(mut consumed)=handled else{
            self.disconnected=true;return Some((true,Vec::new()));
        };
        let committed=lines.iter().any(|s|s.starts_with("C\t"));
        let delete_frames:Vec<(usize,i32,u32)>=lines.iter().enumerate()
            .filter_map(|(i,line)|parse_surrounding_delete_frame(line)
                .map(|(offset,count)|(i,offset,count))).collect();
        let commits:Vec<usize>=lines.iter().enumerate()
            .filter_map(|(i,line)|line.starts_with("C\t").then_some(i)).collect();
        // A key may produce at most ONE authoritative CommitText. Checking
        // the COMPLETE bounded wire batch *before* emitting ANY events is
        // required even when no DeleteSurroundingText was requested:
        // otherwise C+C would create two editor Undo/on_change mutations.
        // Engine-origin multi-commit semantics are not yet qualified, so
        // ambiguous keys fail closed rather than partially applying a prefix.
        if commits.len()>1 {
            self.disconnected=true;
            self.native_replacements.clear();
            self.candidate_page=None;
            self.composing=false;
            return Some((true,Vec::new()));
        }
        // Engine-proven D alone NEVER mutates the editor. Only one positive
        // D preceding one real C during this SAME ProcessKeyEvent can produce
        // an internal NativeReplace. Reject missing/stale contexts/epochs.
        let replacement_proof=if !self.composing && self.surrounding_enabled &&
            self.focus==Some(cb){
            same_key_replace_pair(&lines).and_then(|(offset,count)|
                context_at_key.zip(epoch_at_key).map(|(ctx,epoch)|
                    (ctx,epoch,offset,count)))
        }else{None};
        let mut out=Vec::new();
        for l in lines{
            if l.starts_with("D\t"){continue;}
            if l.starts_with("L\t"){
                match parse_lookup_frame(&l,cb){
                    Some(Some(page))=>{
                        self.candidate_events=self.candidate_events.saturating_add(1);
                        self.candidate_page=Some(page);
                        consumed=true;
                    },
                    Some(None)=>{self.candidate_page=None;},
                    None=>{
                        self.disconnected=true;
                        self.candidate_page=None;
                        return Some((true,Vec::new()));
                    },
                }
            }else if l=="H"{
                if !committed&&self.composing {
                    let seq=self.new_serial()?;
                    self.add(CompositionEvent::Cancel{callback_id:cb,sequence:seq},&mut out);
                    self.composing=false;consumed=true;
                }
            }else if let Some(hex)=l.strip_prefix("C\t"){
                let Some(text)=decode_hex(hex)else{self.disconnected=true;return Some((true,Vec::new()));};
                if !delete_frames.is_empty(){
                    if let Some(((expected,caret_scalar,anchor_scalar),epoch,offset,count))=
                        replacement_proof.as_ref(){
                        if !text.is_empty() && self.native_replacements.len()<MAX_LINES_PER_KEY{
                            let seq=self.new_serial()?;
                            self.native_replacements.push(NativeReplace{
                                callback_id:cb,sequence:seq,authority_epoch:*epoch,
                                expected:expected.clone(),caret_scalar:*caret_scalar,
                                anchor_scalar:*anchor_scalar,offset:*offset,count:*count,
                                text,
                            });
                            self.native_replacement_pairs=self.native_replacement_pairs
                                .saturating_add(1);
                            self.engine_events=self.engine_events.saturating_add(1);
                            consumed=true;
                            self.composing=false;
                            self.candidate_page=None;
                        }
                    }
                    // A D without an authenticated same-key C is advisory only;
                    // suppress any unpaired C rather than duplicate edit.
                    continue;
                }
                if !self.composing{
                    let seq=self.new_serial()?;
                    self.add(CompositionEvent::Start{callback_id:cb,sequence:seq},&mut out);
                }
                let seq=self.new_serial()?;
                self.add(CompositionEvent::Commit{callback_id:cb,sequence:seq,text},&mut out);
                self.engine_events=self.engine_events.saturating_add(1);
                self.composing=false;consumed=true;
                self.candidate_page=None;
            }else if let Some(rem)=l.strip_prefix("P\t"){
                let mut fields=rem.split('\t');
                let Some(hex)=fields.next()else{continue;};
                let Some(raw_caret)=fields.next()else{continue;};
                if fields.next().is_some(){self.disconnected=true;return Some((true,Vec::new()));}
                let Some(text)=decode_hex(hex)else{self.disconnected=true;return Some((true,Vec::new()));};
                let Some(scalar)=raw_caret.parse::<usize>().ok()else{continue;};
                let Some(caret)=grapheme_from_scalar(&text,scalar)else{continue;};
                if !self.composing{
                    let seq=self.new_serial()?;
                    self.add(CompositionEvent::Start{callback_id:cb,sequence:seq},&mut out);
                    self.composing=true;
                }
                let seq=self.new_serial()?;
                self.add(CompositionEvent::Update{
                    callback_id:cb,sequence:seq,text,caret},&mut out);
                self.engine_events=self.engine_events.saturating_add(1);
                consumed=true;
            }
        }
        Some((consumed,out))
    }
}
impl Drop for NativeIbus{
    fn drop(&mut self){
        let _=self.child.kill();
        let _=self.child.wait();
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    /// Phase37 real registered ibus-hangul: an artificial *helper-local*
    /// scheduling deferral lets actual upstream D/C arrive AFTER R.
    /// This travels through the ACTUAL NativeIbus Rust child transport and
    /// its real await_ack(), not synthetic C/D frames or fake event queues.
    /// A host-owned EditSession may only mutate for emitted trusted edits.
    /// This is NOT a test of the standard production 25ms helper schedule.
    #[cfg(all(feature="lumina-diagnostics",target_os="linux"))]
    #[test]
    fn p37_real_post_r_ibus_ack_fence_keeps_editsession_and_undo_authoritative(){
        use crate::text_input::{EditCommand,EditSession,SessionAction};
        assert_eq!(std::env::var("LUMINA_IBUS_ENGINE").unwrap(),"hangul");
        assert_eq!(std::env::var("LUMINA_IBUS_P37_RUST_ACK_FENCE").unwrap(),"1");
        let mut session=EditSession::new("BASE".to_owned());
        let cb=3701;
        let mut native=NativeIbus::connect().expect("real registered libibus child");
        assert!(native.set_surrounding_enabled(true),"native V1 acknowledged");
        assert!(native.sync_focus(Some(cb)),"real libibus F1 acknowledged");
        assert!(native.sync_surrounding_text(cb,&session.buffer,4,4),
            "authoritative host-owned S snapshot acknowledged");
        assert!(native.mark_context_epoch(cb,session.authority_epoch()));
        assert_eq!(native.diagnostic_ack_drop_counts(),(0,0));
        let snapshot=session.clone();
        let (handled,emitted)=native.process_ibus_key_event(
            NativeKeyEvent::from_char('r',0).expect("valid X11 keysym"),
            Some(cb)).expect("REAL Rust native key adapter");
        assert!(handled,"upstream actual Hangul key handled");
        assert!(emitted.is_empty(),"NO C before R under test-only deferred helper");
        assert!(native.take_native_replacements().is_empty(),
            "no invented D/C same-key replacement");
        assert_eq!(native.last_serial(),0);
        // Real helper I200 flushes genuine upstream callbacks after R.
        // Production Rust *await_ack* is exercised unchanged and must drop
        // unauthoritative C rather than turning it into a new editor commit.
        assert!(native.send("I\t200"));
        assert!(native.await_ack(),"actual I200 ACK and late D/C from libibus");
        assert_eq!(native.diagnostic_ack_drop_counts(),(1,1),
            "the real engine's delayed D(0,0) and C(ㄱ) must reach Rust await_ack");
        assert_eq!(native.engine_events,0,"late C cannot become a trusted event");
        assert_eq!(native.native_replacement_pairs,0,"late D/C not same-key proof");
        assert_eq!(native.last_serial(),0,"untrusted callback cannot consume sequence");
        assert_eq!(session,snapshot,"no EditSession mutation or Undo snapshot");
        assert!(!session.apply(SessionAction::Undo),
            "no phantom Undo from a real late engine C");
        assert_eq!(session.buffer,"BASE");
        eprintln!("LUMINA_IBUS_P37_REAL_LATE_FIRST rust_ack_D=1 rust_ack_C=1 emitted=0 native_pairs=0 editsession_unchanged=true undo_empty=true");

        // POSITIVE CONTROL: a HOST-OWNED local edit advances authoritative
        // revision; it does not pretend the dropped engine C was accepted.
        assert!(session.apply(SessionAction::Edit(
            EditCommand::Insert("ㄱ".to_owned()))));
        assert_eq!(session.buffer,"BASEㄱ");
        assert!(native.sync_surrounding_text(cb,&session.buffer,5,5),
            "new local host value, not replayed engine text, must be ACKed");
        assert!(native.mark_context_epoch(cb,session.authority_epoch()));
        let snapshot2=session.clone();
        let (handled2,events2)=native.process_ibus_key_event(
            NativeKeyEvent::from_char('k',0).expect("valid X11 keysym"),
            Some(cb)).expect("REAL registered second key");
        assert!(handled2);
        assert!(events2.is_empty(),"test-only helper again responds R before engine C");
        assert!(native.take_native_replacements().is_empty());
        assert!(native.send("I\t200"));
        assert!(native.await_ack());
        assert_eq!(native.diagnostic_ack_drop_counts(),(2,2),
            "second genuine delayed upstream D(-1,1)+C(가) must be ignored in Rust ACK");
        assert_eq!(native.surrounding_last_delete,Some((-1,1)),
            "genuine late D remains non-authoritative advisory evidence");
        assert_eq!(native.engine_events,0);
        assert_eq!(native.native_replacement_pairs,0);
        assert_eq!(native.last_serial(),0);
        assert_eq!(session,snapshot2,
            "genuine post-R replacement may not delete the host-owned suffix");
        assert!(session.apply(SessionAction::Undo),
            "only the explicit HOST-OWNED local edit has one Undo snapshot");
        assert_eq!(session.buffer,"BASE");
        assert!(!session.apply(SessionAction::Undo),
            "real late D/C cannot manufacture another Undo snapshot");
        assert!(session.apply(SessionAction::Redo));
        assert_eq!(session.buffer,"BASEㄱ");
        assert!(!session.apply(SessionAction::Redo));
        eprintln!("LUMINA_IBUS_P37_REAL_LATE_SECOND rust_ack_D=2 rust_ack_C=2 emitted=0 native_pairs=0 host_suffix_preserved=true single_host_undo_redo=true diagnostic_only=true");
    }
    #[test]fn manually_rearm_exhausted_budget_only_on_monotonic_host_revision(){
        let mut b=IbusReconnectBudget::default();
        assert!(!b.observe_host_retry_revision(0,true),
            "first host value must be baseline, not an implicit command");
        for index in 0..IbusReconnectBudget::MAX_ATTEMPTS {
            b.arm();
            for _ in 0..IbusReconnectBudget::WAIT_FRAMES[index as usize]{
                assert!(!b.tick());
            }
            assert!(b.tick());
        }
        b.arm();
        for _ in 0..200{assert!(!b.tick(),"no automatic fourth retry");}
        assert!(!b.observe_host_retry_revision(0,true),"stable version no reset");
        assert!(!b.observe_host_retry_revision(1,false),
            "revision update on a live helper is not a reconnect request");
        assert!(!b.observe_host_retry_revision(0,true),"rollback no reset");
        assert!(!b.observe_host_retry_revision(1,true),"replay no reset");
        assert!(b.observe_host_retry_revision(2,true),
            "new strictly higher host command may open one bounded epoch");
        for _ in 0..24 { assert!(!b.tick()); }
        assert!(b.tick(),"only the commanded bounded retry is ready");
        assert!(!b.tick(),"cannot repeat a permitted attempt");
        assert!(!b.observe_host_retry_revision(2,true),"same version cannot rearm");
    }
    #[test]fn real_helper_recovery_budget_is_bounded_without_clock_or_sleep(){
        let mut b=IbusReconnectBudget::default();
        assert!(!b.tick(),"no automatic unconfigured retry");
        for attempt in 0..IbusReconnectBudget::MAX_ATTEMPTS{
            b.arm();
            let wait=IbusReconnectBudget::WAIT_FRAMES[attempt as usize];
            for _ in 0..wait{assert!(!b.tick());}
            assert!(b.tick(),"exactly one retry after bounded frame delay");
            assert!(!b.tick(),"cannot reconnect twice from same timer");
        }
        b.arm();
        for _ in 0..200{assert!(!b.tick(),"lifetime attempt cap must hold");}
        assert_eq!(b.diagnostic_status(),(3,true));
    }
    /// Controlled PROTOCOL sequence, NOT a claimed genuine engine trace.
    /// A D/C from the helper's idle/pre-dispatch GLib pump must never be
    /// assigned to the *following* ProcessKeyEvent, even if an R arrives.
    /// Pure wire-protocol test: these frames are CONTROLLED INPUT, not
    /// evidence that an upstream IBus engine emitted delayed signals.
    #[test]fn key_dispatch_fence_drops_late_pre_key_frames_and_rejects_duplicate_begin(){
        let mut fence=KeyDispatchFence::default();
        for stale in ["D\t-1\t1","C\teab080","P\teab080\t1","L\t0","N","H"]{
            assert_eq!(fence.admit(stale),Some(false),"{stale:?} must be quarantined");
        }
        assert_eq!(fence.admit("B"),Some(false),"only helper starts a key cycle");
        assert_eq!(fence.admit("D\t-1\t1"),Some(true));
        assert_eq!(fence.admit("C\teab080"),Some(true));
        assert_eq!(fence.admit("R\t1"),Some(true));
        assert_eq!(fence.admit("B"),None,"second begin must be fail-closed");
        let mut fresh=KeyDispatchFence::default();
        assert_eq!(fresh.admit("R\t1"),None,"R cannot close a key never begun");
        assert_eq!(fresh.admit("BOGUS"),None);
    }
    #[test]fn pre_dispatch_delayed_pair_must_not_cross_key_boundary(){
        let stale=vec![
            "D\t-1\t1".to_string(),
            "C\teab080".to_string(),
            "B".to_string(),  // proposed helper-origin key-begin marker
            "R\t1".to_string(),
        ];
        assert_eq!(same_key_replace_pair(&stale),None,
            "a D/C pair before the new key-dispatch boundary is NOT same-key");
    }
    /// CONTROLLED helper-wire frames, not an upstream IBus engine trace.
    /// This exercises the full native key parser and emitted host events
    /// without injecting events into any real daemon or live window.
    fn process_controlled_post_begin_key(lines:&[&str])
        ->(Vec<CompositionEvent>,Vec<NativeReplace>,bool) {
        // This harmless local process is just a writable stdin endpoint for
        // the private NativeIbus transport. It emits NO stdout or IBus data.
        let mut child=Command::new("sh").arg("-c").arg("cat >/dev/null")
            .stdin(Stdio::piped()).stdout(Stdio::null())
            .stderr(Stdio::null()).spawn().expect("local test sink");
        let input=child.stdin.take().expect("test stdin");
        let (tx,output)=mpsc::sync_channel(32);
        for line in lines{
            tx.send((*line).to_owned()).expect("test frame queue");
        }
        drop(tx);
        let mut bus=NativeIbus{
            child,input,output,focus:Some(1201),serial:0,
            composing:false,disconnected:false,surrounding_enabled:true,
            engine_events:0,key_acks:0,release_acks:0,shifted_acks:0,
            candidate_events:0,candidate_click_acks:0,
            cursor_location_acks:0,cursor_location:None,
            surrounding_text_acks:0,surrounding_delete_requests:0,
            surrounding_last_delete:None,surrounding_requires:0,
            surrounding_text:None,context_epoch:None,
            authority_generation:None,native_replacements:Vec::new(),
            native_replacement_pairs:0,candidate_page:None,
            #[cfg(feature="lumina-diagnostics")]
            diagnostic_ack_advisory_deletes:0,
            #[cfg(feature="lumina-diagnostics")]
            diagnostic_ack_discarded_commits:0,
        };
        let (_,events)=bus.process_ibus_key_event(
            NativeKeyEvent::from_char('k',0).expect("valid key"),
            Some(1201),
        ).expect("controlled key result");
        let replacements=bus.take_native_replacements();
        let disconnected=bus.disconnected;
        (events,replacements,disconnected)
    }
    #[test]fn duplicate_post_begin_commit_must_never_emit_partial_authoritative_edits(){
        // Both CommitText frames are untrusted CONTROLLED protocol data.
        // A second C in one key must not yield two host edits/Undo records.
        let (events,replacements,_) = process_controlled_post_begin_key(
            &["B","C\teab080","C\teab098","R\t1"]);
        assert!(events.is_empty(),
            "duplicate key-local CommitText must produce ZERO authoritative events: {events:?}");
        assert!(replacements.is_empty(),"duplicate C cannot create NativeReplace");
    }
    #[test]fn single_post_begin_commit_keeps_existing_authoritative_path(){
        let (events,replacements,disconnected)=process_controlled_post_begin_key(
            &["B","C\teab080","R\t1"]);
        assert!(!disconnected,"valid one-commit cycle must remain usable");
        assert_eq!(events.iter().filter(|e|
            matches!(e,CompositionEvent::Commit{..})).count(),1);
        assert!(replacements.is_empty());
    }
    #[test]fn adversarial_same_key_order_has_no_delete_authority(){
        let parse=|seq:&[&str]|same_key_replace_pair(
            &seq.iter().map(|s|s.to_string()).collect::<Vec<_>>());
        assert_eq!(parse(&["D\t-1\t1","C\teab080"]),Some((-1,1)));
        for seq in [
            vec!["D\t-1\t1"],vec!["C\teab080"],
            vec!["C\teab080","D\t-1\t1"],
            vec!["D\t-1\t1","D\t-1\t1","C\teab080"],
            vec!["D\t-1\t1","C\teab080","C\teab080"],
            vec!["D\t0\t0","C\teab080"],
            vec!["D\t-1025\t1","C\teab080"],
            vec!["P\teab080\t1","D\t-1\t1","C\teab080"],
        ]{assert_eq!(parse(&seq),None,"ambiguous controlled sequence {seq:?}");}
    }
    #[test]fn native_surrounding_delete_signal_bounds_never_apply_authority(){
        assert_eq!(parse_surrounding_delete_frame("D\t-2\t2"),Some((-2,2)));
        assert_eq!(parse_surrounding_delete_frame("D\t1024\t1"),Some((1024,1)));
        assert_eq!(parse_surrounding_delete_frame("D\t-1025\t1"),None);
        assert_eq!(parse_surrounding_delete_frame("D\t0\t0"),None);
        assert_eq!(parse_surrounding_delete_frame("D\t0\t1025"),None);
        assert_eq!(parse_surrounding_delete_frame("D\t-1\t1\t2"),None);
        assert_eq!(parse_surrounding_delete_frame("D\t-1\tbogus"),None);
        assert_eq!(parse_surrounding_delete_frame("D\t2147483648\t1"),None);
        assert_eq!(parse_surrounding_delete_frame("C\t-1\t1"),None);
    }
    #[test]fn real_native_signal_hex_protocol_rejects_invalid_utf8_and_oversize(){
        assert_eq!(decode_hex("eab080"),Some("가".into()));
        assert_eq!(decode_hex(""),Some("".into()));
        assert_eq!(decode_hex("zz"),None);
        assert_eq!(decode_hex("ff"),None);
        assert_eq!(decode_hex(&"ab".repeat(MAX_TEXT_BYTES+1)),None);
        assert_eq!(grapheme_from_scalar("e\u{301}x",1),Some(1));
        assert_eq!(grapheme_from_scalar("가나",1),Some(1));
    }
    #[test]fn typed_keysym_preserves_shift_and_unicode_not_fake_ascii(){
        let k=NativeKeyEvent::from_char('가',MOD_SHIFT).unwrap();
        assert_eq!((k.keysym,k.modifiers,k.released),
            (0x0100_ac00,MOD_SHIFT,false));
        assert!(k.valid());
        assert_eq!(NativeKeyEvent::from_char('A',MOD_SHIFT).unwrap().keysym,65);
        assert!(NativeKeyEvent::from_char('\n',0).is_none());
        assert!(!NativeKeyEvent{keysym:65,modifiers:0x80,released:false}.valid());
        assert!(NativeKeyEvent{keysym:0xff52,modifiers:0,released:true}.valid());
    }
    #[test]fn cursor_location_geometry_fail_closed_and_is_focus_owned(){
        // Direct static parser tests belong to the C helper. Rust bounds
        // are deliberately much smaller than the IBus signed gint range.
        let acceptable=|x:u32,y:u32,w:u32,h:u32|{
            x<=32767&&y<=32767&&w>0&&h>0&&w<=128&&h<=128&&
            x.checked_add(w).is_some_and(|v|v<=32768)&&
            y.checked_add(h).is_some_and(|v|v<=32768)
        };
        assert!(acceptable(220,84,1,18));
        assert!(!acceptable(220,84,0,18));
        assert!(!acceptable(220,84,1,129));
        assert!(!acceptable(32768,84,1,18));
        assert!(!acceptable(32767,84,2,18));
    }
    #[test]fn candidate_frame_bounds_unicode_and_focus_ownership(){
        let page=parse_lookup_frame("L\t1\t2\t0\t4\te4bda0\te5a5bd",936).unwrap().unwrap();
        assert_eq!((page.callback_id,page.cursor,page.page_start,page.total),
            (936,2,0,4));
        assert_eq!(page.candidates,vec!["你","好"]);
        assert_eq!(parse_lookup_frame("L\t0",936),Some(None));
        assert_eq!(parse_lookup_frame("L\t1\t2\t0\t4\tff",936),None);
        assert_eq!(parse_lookup_frame("L\t1\t4\t0\t4\te4bda0",936),None);
        assert_eq!(parse_lookup_frame("L\t1\t0\t0\t1\t",936),None);
        assert_eq!(parse_lookup_frame("L\t1\t0\t0\t1\te4bda0\te5a5bd",936),None);
        assert_eq!(parse_lookup_frame(&format!("L\t1\t0\t0\t1\t{}",
            "e4bda0".repeat(60)),936),None);
    }
}
