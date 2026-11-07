//! CPU-only offline dynamic backend dispatch, one bounded JSON stdin request.
//! Strict errors go to stderr/exit 2. No QPU, network or paid provider.
use aura_quantum_sim::dispatch_json::{
    execute_dispatch_json, ERROR_SCHEMA, MAX_INPUT_BYTES,
};
use serde_json::json;
use std::io::{self, Read};

fn fail(code:&str,detail:&str)->! {
    eprintln!("{}",json!({
        "schema":ERROR_SCHEMA,
        "status":"error",
        "code":code,
        "detail":detail,
        "evidence_class":"simulation",
        "qpu_hardware_jobs":0
    }));
    std::process::exit(2)
}
fn main() {
    if std::env::args_os().len()!=1 {
        fail("usage","one bounded JSON object from stdin, no command-line flags");
    }
    let mut data=Vec::new();
    if let Err(e)=io::stdin().lock()
        .take((MAX_INPUT_BYTES+1) as u64)
        .read_to_end(&mut data) {
        fail("io_error",&e.to_string());
    }
    match execute_dispatch_json(&data) {
        Ok(result)=>println!("{result}"),
        Err(e)=>fail(e.code(),&e.to_string()),
    }
}
