//! Offline CPU-only disconnected factor-product simulator.
//! One versioned bounded JSON request from stdin, result to stdout.
//! Unknown flags, hardware submit and invalid input -> stderr JSON / exit 2.
use aura_quantum_sim::factorized_json::{
    execute_factorized_json,FACTOR_ERROR_SCHEMA,MAX_REQUEST_BYTES,
};
use serde_json::json;
use std::io::{self,Read};

fn fail(code:&str,message:&str)->! {
    eprintln!("{}",json!({
        "schema":FACTOR_ERROR_SCHEMA,
        "status":"error",
        "code":code,
        "message":message,
        "evidence_class":"simulation",
        "qpu_hardware_jobs":0
    }));
    std::process::exit(2)
}
fn main() {
    if std::env::args_os().len()!=1 {
        fail("usage","read one bounded factorized JSON document from stdin; no flags");
    }
    let mut input=Vec::new();
    if let Err(err)=io::stdin().lock()
        .take((MAX_REQUEST_BYTES+1) as u64)
        .read_to_end(&mut input) {
        fail("io_error",&err.to_string());
    }
    match execute_factorized_json(&input) {
        Ok(result)=>println!("{result}"),
        Err(err)=>fail(err.code(),&err.to_string()),
    }
}
