//! Strict offline CPU Pauli Hamiltonian second-moment/variance CLI.
use aura_quantum_sim::moments_json::{execute_moments_json,MOMENTS_ERROR_SCHEMA};
use aura_quantum_sim::factorized_json::MAX_REQUEST_BYTES;
use serde_json::json;
use std::io::{self,Read};
fn fail(code:&str,message:&str)->! {
    eprintln!("{}",json!({
        "schema":MOMENTS_ERROR_SCHEMA,
        "status":"error",
        "code":code,
        "message":message,
        "evidence_class":"simulation",
        "qpu_hardware_jobs":0
    }));
    std::process::exit(2)
}
fn main(){
    if std::env::args_os().len()!=1 {
        fail("usage","one offline moments/1 request via stdin; no flags");
    }
    let mut raw=Vec::new();
    if let Err(e)=io::stdin().lock()
        .take((MAX_REQUEST_BYTES+1) as u64)
        .read_to_end(&mut raw)
    {
        fail("io_error",&e.to_string());
    }
    match execute_moments_json(&raw) {
        Ok(result)=>println!("{result}"),
        Err(err)=>fail(err.code(),&err.to_string()),
    }
}
