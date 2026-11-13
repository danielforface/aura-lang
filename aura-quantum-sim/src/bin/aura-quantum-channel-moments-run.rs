//! Offline exact ideal-instrument channel moments (no QPU or random shots).
use aura_quantum_sim::channel_moments_json::{
    execute_channel_moments_json,CHANNEL_ERROR_SCHEMA,
};
use aura_quantum_sim::factorized_json::MAX_REQUEST_BYTES;
use serde_json::json;
use std::io::{self,Read};
fn fail(code:&str,message:&str)->!{
    eprintln!("{}",json!({
        "schema":CHANNEL_ERROR_SCHEMA,"status":"error","code":code,
        "message":message,"evidence_class":"simulation","qpu_hardware_jobs":0
    }));
    std::process::exit(2)
}
fn main(){
    if std::env::args_os().len()!=1{
        fail("usage","offline channel-moments/1 request via stdin; no flags");
    }
    let mut raw=Vec::new();
    if let Err(e)=io::stdin().lock()
        .take((MAX_REQUEST_BYTES+1) as u64)
        .read_to_end(&mut raw){
        fail("io_error",&e.to_string());
    }
    match execute_channel_moments_json(&raw){
        Ok(value)=>println!("{value}"),
        Err(e)=>fail(e.code(),&e.to_string()),
    }
}
