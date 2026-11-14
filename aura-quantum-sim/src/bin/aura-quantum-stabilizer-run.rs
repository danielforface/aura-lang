//! One stdin JSON document -> one complete stdout JSON result (CPU-only).
use aura_quantum_sim::stabilizer_json::{
    execute_stabilizer_json,ERROR_SCHEMA,MAX_INPUT_BYTES,
};
use serde_json::json;
use std::io::{self,Read};

fn fail(code:&str,message:&str)->! {
    eprintln!("{}",json!({
        "schema":ERROR_SCHEMA,"status":"error",
        "code":code,"message":message,
        "evidence_class":"simulation","qpu_hardware_jobs":0
    }));
    std::process::exit(2);
}
fn main() {
    if std::env::args_os().len()!=1 {
        fail("usage","read one bounded JSON document from stdin; no CLI flags");
    }
    let mut raw=Vec::new();
    if let Err(err)=io::stdin().lock()
        .take((MAX_INPUT_BYTES+1) as u64)
        .read_to_end(&mut raw) {
        fail("io_error",&err.to_string());
    }
    match execute_stabilizer_json(&raw) {
        Ok(result)=>println!("{result}"),
        Err(error)=>fail(error.code(),&error.to_string()),
    }
}
