//! CPU-only C-020 exact ideal channel reduced density tomography CLI.
use aura_quantum_sim::channel_tomography_json::{
    execute_channel_tomography_json,TOMOGRAPHY_ERROR_SCHEMA,
};
use aura_quantum_sim::factorized_json::MAX_REQUEST_BYTES;
use serde_json::json;
use std::io::{self,Read};
fn fail(code:&str,message:&str)->!{
    eprintln!("{}",json!({
        "schema":TOMOGRAPHY_ERROR_SCHEMA,"status":"error",
        "code":code,"message":message,
        "evidence_class":"simulation","qpu_hardware_jobs":0
    }));
    std::process::exit(2)
}
fn main(){
    if std::env::args_os().len()!=1{
        fail("usage","offline channel-tomography/1 request via stdin; no flags");
    }
    let mut input=Vec::new();
    if let Err(e)=io::stdin().lock()
        .take((MAX_REQUEST_BYTES+1) as u64)
        .read_to_end(&mut input){
        fail("io_error",&e.to_string());
    }
    match execute_channel_tomography_json(&input){
        Ok(out)=>println!("{out}"),
        Err(e)=>fail(e.code(),&e.to_string()),
    }
}
