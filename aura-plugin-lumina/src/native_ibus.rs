//! Opt-in Linux IBus InputContext transport. A real /usr/bin/python3 GI
//! helper forwards ONLY IBus daemon/engine-origin preedit/commit signals.
//! No FFI/unsafe, no Cargo.lock changes, no OS input claims outside IBus.
//! Bounded per-frame reads/writes, focused widget and focus-ACK fencing.
use crate::CompositionEvent;
use std::io::{BufRead,BufReader,Write};
use std::process::{Child,Command,Stdio};
use std::sync::mpsc::{self,Receiver,SyncSender};
use std::thread;
use unicode_segmentation::UnicodeSegmentation;

const MAX_OUTPUT_LINE:usize=4096;
const MAX_SIGNAL_TEXT:usize=1024;
const MAX_DRAIN:usize=64;
#[derive(Clone,Debug,PartialEq,Eq)]
enum Reply {Ready,Focus(bool),Preedit(String,usize),Commit(String),Hide,Fail}
fn from_hex(hex:&str)->Option<String>{
    if hex.len()>MAX_SIGNAL_TEXT*2 || hex.len()%2!=0{return None;}
    let mut bytes=Vec::with_capacity(hex.len()/2);
    for chunk in hex.as_bytes().chunks_exact(2){
        let hi=(chunk[0] as char).to_digit(16)? as u8;
        let lo=(chunk[1] as char).to_digit(16)? as u8;
        bytes.push((hi<<4)|lo);
    }
    let s=String::from_utf8(bytes).ok()?;
    if s.chars().any(|ch|ch.is_control()||
        matches!(ch,'\u{202A}'..='\u{202E}'|'\u{2066}'..='\u{2069}')){
        return None;
    }
    Some(s)
}
fn parse_reply(line:&str)->Reply{
    if line.len()>MAX_OUTPUT_LINE{return Reply::Fail;}
    let fields:Vec<_>=line.trim_end_matches(['\n','\r']).split('\t').collect();
    match fields.as_slice(){
        ["READY"]=>Reply::Ready,
        ["A","1"]=>Reply::Focus(true),
        ["A","0"]=>Reply::Focus(false),
        ["H"]=>Reply::Hide,
        ["C",hex]=>from_hex(hex).filter(|v|!v.is_empty())
            .map(Reply::Commit).unwrap_or(Reply::Fail),
        ["P",hex,cursor]=>{
            let Some(text)=from_hex(hex) else{return Reply::Fail;};
            let Some(caret)=cursor.parse::<usize>().ok() else{return Reply::Fail;};
            if caret>text.chars().count(){return Reply::Fail;}
            Reply::Preedit(text,caret)
        }
        _=>Reply::Fail
    }
}
fn scalar_to_grapheme(text:&str,scalar:usize)->Option<usize>{
    if scalar>text.chars().count(){return None;}
    let byte=text.char_indices().nth(scalar).map(|(i,_)|i).unwrap_or(text.len());
    // UAX29 snap forward for a scalar cursor inside a combining cluster.
    let mut n=0;
    for (start,cluster) in text.grapheme_indices(true){
        if byte<=start{return Some(n);}
        if byte<start+cluster.len(){return Some(n+1);}
        n+=1;
    }
    Some(n)
}

pub(crate) struct NativeIbus {
    child: Child,
    tx: SyncSender<String>,
    rx: Receiver<Reply>,
    ready: bool,
    focus_ack: bool,
    focus: Option<u64>,
    composing: bool,
    next_sequence: u64,
}
impl NativeIbus {
    pub(crate) fn spawn(script:&str)->Option<Self>{
        if !std::path::Path::new(script).is_file() || script.len()>1024 {return None;}
        let mut child=Command::new("/usr/bin/python3")
            .arg("-u").arg(script)
            .stdin(Stdio::piped()).stdout(Stdio::piped())
            .stderr(Stdio::inherit()).spawn().ok()?;
        let mut stdin=child.stdin.take()?;
        let stdout=child.stdout.take()?;
        let (tx,commands)=mpsc::sync_channel::<String>(32);
        let (events,rx)=mpsc::sync_channel::<Reply>(64);
        thread::spawn(move ||{
            while let Ok(command)=commands.recv(){
                if stdin.write_all(command.as_bytes()).is_err() ||
                    stdin.flush().is_err(){break;}
            }
        });
        thread::spawn(move ||{
            let mut reader=BufReader::new(stdout);
            loop{
                let mut line=Vec::new();
                // Limit even malformed/compromised helper stdout to 4KiB.
                // Do not allocate until unbounded newline.
                let mut complete=false;
                for _ in 0..MAX_OUTPUT_LINE {
                    let mut chunk=[0u8;1];
                    use std::io::Read;
                    if reader.read_exact(&mut chunk).is_err(){break;}
                    if chunk[0]==b'\n'{complete=true;break;}
                    line.push(chunk[0]);
                }
                if !complete{let _=events.try_send(Reply::Fail);break;}
                let response=std::str::from_utf8(&line).map(parse_reply)
                    .unwrap_or(Reply::Fail);
                if events.try_send(response).is_err(){break;}
            }
        });
        Some(Self {child,tx,rx,ready:false,focus_ack:false,focus:None,
            composing:false,next_sequence:0})
    }
    pub(crate) fn active(&self,cb:Option<u64>)->bool {
        self.ready && self.focus_ack && self.focus.is_some() && self.focus==cb
    }
    pub(crate) fn focus(&mut self,cb:Option<u64>){
        if self.focus==cb{return;}
        // Any old messages from the previous widget are now invalid; all
        // subsequent messages must await a real helper FocusIn ACK.
        while self.rx.try_recv().is_ok(){}
        self.focus=cb;
        self.focus_ack=false;
        self.composing=false;
        let cmd=if cb.is_some(){"F\t1\n"}else{"F\t0\n"};
        if self.tx.try_send(cmd.to_owned()).is_err(){self.ready=false;}
    }
    pub(crate) fn key(&mut self,raw:u32)->bool{
        if !self.active(self.focus){return false;}
        // This first slice supports only physical US-ASCII A-Z, space and
        // Return for the Hangul two-set engine. No keymap/shift/release,
        // candidate navigation or surrounding text claim.
        let keysym=match raw {
            65..=90=>raw+32,
            32=>32,
            257=>0xff0d,
            _=>return false,
        };
        if self.tx.try_send(format!("K\t{keysym}\t0\t0\n")).is_err(){
            self.ready=false;return false;
        }
        true
    }
    fn serial(&mut self)->u64{
        self.next_sequence=self.next_sequence.saturating_add(1);
        self.next_sequence
    }
    pub(crate) fn drain(&mut self)->Vec<CompositionEvent>{
        let mut out=Vec::new();
        for _ in 0..MAX_DRAIN{
            let Ok(reply)=self.rx.try_recv() else{break;};
            match reply{
                Reply::Ready=>{self.ready=true;}
                Reply::Focus(state)=>{self.focus_ack=state&&self.focus.is_some();}
                Reply::Fail=>{self.ready=false;self.focus_ack=false;self.composing=false;}
                _ if !self.active(self.focus)=>{}
                Reply::Preedit(text,cursor)=>{
                    let Some(cb)=self.focus else{continue;};
                    let Some(caret)=scalar_to_grapheme(&text,cursor) else{continue;};
                    if !self.composing {
                        let sequence=self.serial();
                        out.push(CompositionEvent::Start{callback_id:cb,sequence});
                        self.composing=true;
                    }
                    let sequence=self.serial();
                    out.push(CompositionEvent::Update{callback_id:cb,sequence,text,caret});
                }
                Reply::Commit(text)=>{
                    let Some(cb)=self.focus else{continue;};
                    if !self.composing {
                        let sequence=self.serial();
                        out.push(CompositionEvent::Start{callback_id:cb,sequence});
                    }
                    let sequence=self.serial();
                    out.push(CompositionEvent::Commit{callback_id:cb,sequence,text});
                    self.composing=false;
                }
                Reply::Hide=>{
                    if let Some(cb)=self.focus.filter(|_|self.composing){
                        let sequence=self.serial();
                        out.push(CompositionEvent::Cancel{callback_id:cb,sequence});
                        self.composing=false;
                    }
                }
            }
        }
        out
    }
}
impl Drop for NativeIbus {
    fn drop(&mut self){
        let _=self.tx.try_send("Q\n".to_owned());
        let _=self.child.kill();
        let _=self.child.wait();
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn strict_hex_serialization_and_unicode_scalar_to_grapheme(){
        assert_eq!(parse_reply("READY"),Reply::Ready);
        assert_eq!(parse_reply("P\t65cc81\t2"),Reply::Preedit("e\u{301}".into(),2));
        assert_eq!(scalar_to_grapheme("e\u{301}",1),Some(1));
        assert_eq!(scalar_to_grapheme("e\u{301}",2),Some(1));
        assert_eq!(parse_reply("C\td795d5dc"),Reply::Commit("שלום".into()));
        assert_eq!(parse_reply("P\tff\t1"),Reply::Fail);
        assert_eq!(parse_reply("P\t65cc81\t3"),Reply::Fail);
        assert_eq!(parse_reply("K\t97\t0\t0"),Reply::Fail);
        assert_eq!(parse_reply(&("C\t".to_owned()+&"61".repeat(1025))),Reply::Fail);
    }
}
