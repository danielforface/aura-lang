//! C-024 strict offline Kraus/dynamic postselection: no seed, no QPU.
use aura_quantum_sim::noisy_conditioned_json::{
    execute_noisy_conditioned_json,NOISY_CONDITIONED_ERROR_SCHEMA,
};
use aura_quantum_sim::noisy_json::MAX_NOISY_REQUEST_BYTES;
use serde_json::json;
use std::io::{self,Read};
fn reject(code:&str,message:&str)->!{
    eprintln!("{}",json!({
        "schema":NOISY_CONDITIONED_ERROR_SCHEMA,
        "status":"error","code":code,"message":message,
        "evidence_class":"simulation","qpu_hardware_jobs":0
    }));
    std::process::exit(2)
}
fn main(){
    if std::env::args_os().len()!=1{
        reject("usage","one offline noisy-conditioned/1 JSON request; no flags");
    }
    let mut raw=Vec::new();
    if let Err(e)=io::stdin().lock()
        .take((MAX_NOISY_REQUEST_BYTES+1) as u64).read_to_end(&mut raw){
        reject("io_error",&e.to_string());
    }
    match execute_noisy_conditioned_json(&raw){
        Ok(s)=>println!("{s}"),
        Err(e)=>reject(e.code(),&e.to_string()),
    }
}
