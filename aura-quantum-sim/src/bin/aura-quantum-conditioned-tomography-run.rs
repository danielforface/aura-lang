//! C-021 conditional final-register tomography; CPU-only, no seed/QPU.
use aura_quantum_sim::conditioned_tomography_json::{
    execute_conditioned_tomography_json,CONDITIONED_TOMOGRAPHY_ERROR_SCHEMA,
};
use aura_quantum_sim::factorized_json::MAX_REQUEST_BYTES;
use serde_json::json;
use std::io::{self,Read};
fn fail(code:&str,message:&str)->!{
    eprintln!("{}",json!({
        "schema":CONDITIONED_TOMOGRAPHY_ERROR_SCHEMA,
        "status":"error","code":code,"message":message,
        "evidence_class":"simulation","qpu_hardware_jobs":0
    }));
    std::process::exit(2)
}
fn main(){
    if std::env::args_os().len()!=1{
        fail("usage","one offline conditioned-tomography/1 request; no flags");
    }
    let mut input=Vec::new();
    if let Err(e)=io::stdin().lock()
        .take((MAX_REQUEST_BYTES+1) as u64)
        .read_to_end(&mut input){
        fail("io_error",&e.to_string());
    }
    match execute_conditioned_tomography_json(&input){
        Ok(out)=>println!("{out}"),
        Err(e)=>fail(e.code(),&e.to_string()),
    }
}
