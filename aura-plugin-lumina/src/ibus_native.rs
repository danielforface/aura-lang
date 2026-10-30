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
pub(crate) struct NativeIbus{
    child:Child,
    input:ChildStdin,
    output:Receiver<String>,
    focus:Option<u64>,
    serial:u64,
    composing:bool,
    disconnected:bool,
    /// Number of actual engine-produced nonempty preedit/commit callbacks.
    pub(crate) engine_events:usize,
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
            composing:false,disconnected:false,engine_events:0})
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
    fn await_ack(&mut self)->bool{
        for _ in 0..MAX_LINES_PER_KEY {
            let Some(line)=self.next() else{
                self.disconnected=true;return false;
            };
            if line=="A"{return true;}
            // Some IBus engines emit hide-preedit during FocusOut/FocusIn.
            // Such events belong to the OLD focus generation and must not
            // be delivered to the newly focused native TextInput.
            if line=="H"||line.starts_with("P\t")||line.starts_with("C\t"){
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
        self.focus=None;
        if let Some(id)=target{
            if !self.send("F1"){return false;}
            if !self.await_ack(){return false;}
            self.focus=Some(id);
        }
        true
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
        if !key.valid() || !self.sync_focus(target){return None;}
        let cb=target?;
        if !self.send(&format!("K\t{}\t{}\t{}\t0",
            key.keysym,key.modifiers,u8::from(key.released))){return None;}
        let mut lines=Vec::new();
        let mut handled=None;
        for _ in 0..MAX_LINES_PER_KEY{
            let Some(line)=self.next()else{
                self.disconnected=true;return Some((true,Vec::new()));
            };
            if let Some(bit)=line.strip_prefix("R\t"){
                handled=match bit{"1"=>Some(true),"0"=>Some(false),_=>None};
                break;
            }
            if line=="H"||line.starts_with("P\t")||line.starts_with("C\t"){
                lines.push(line);
            }else{self.disconnected=true;return Some((true,Vec::new()));}
        }
        let Some(mut consumed)=handled else{
            self.disconnected=true;return Some((true,Vec::new()));
        };
        let committed=lines.iter().any(|s|s.starts_with("C\t"));
        let mut out=Vec::new();
        for l in lines{
            if l=="H"{
                if !committed&&self.composing {
                    let seq=self.new_serial()?;
                    self.add(CompositionEvent::Cancel{callback_id:cb,sequence:seq},&mut out);
                    self.composing=false;consumed=true;
                }
            }else if let Some(hex)=l.strip_prefix("C\t"){
                let Some(text)=decode_hex(hex)else{self.disconnected=true;return Some((true,Vec::new()));};
                if !self.composing{
                    let seq=self.new_serial()?;
                    self.add(CompositionEvent::Start{callback_id:cb,sequence:seq},&mut out);
                }
                let seq=self.new_serial()?;
                self.add(CompositionEvent::Commit{callback_id:cb,sequence:seq,text},&mut out);
                self.engine_events=self.engine_events.saturating_add(1);
                self.composing=false;consumed=true;
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
}
