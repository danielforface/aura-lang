//! C-015 high-shot CPU-only histogram/Counts CLI, no QPU or live provider.
//! One bounded request on stdin. Success JSON exclusively on stdout.
//! Failure -> structured JSON stderr and exit=2 (never partial counts).
use aura_quantum_sim::factorized_counts_json::{
    execute_factorized_counts_json,FACTOR_COUNTS_ERROR_SCHEMA,
};
use aura_quantum_sim::factorized_json::MAX_REQUEST_BYTES;
use serde_json::json;
use std::io::{self,Read};

fn fail(code:&str,message:&str)->! {
    eprintln!("{}",json!({
        "schema":FACTOR_COUNTS_ERROR_SCHEMA,
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
        fail("usage","bounded factorized Counts JSON via stdin only; no flags");
    }
    let mut raw=Vec::new();
    if let Err(e)=io::stdin().lock()
        .take((MAX_REQUEST_BYTES+1) as u64)
        .read_to_end(&mut raw)
    {
        fail("io_error",&e.to_string());
    }
    match execute_factorized_counts_json(&raw) {
        Ok(response)=>println!("{response}"),
        Err(e)=>fail(e.code(),&e.to_string()),
    }
}
