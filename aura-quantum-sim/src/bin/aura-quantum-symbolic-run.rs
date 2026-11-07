//! Offline CPU-only exact small symbolic Clifford+T phase verifier.
//! One bounded JSON stdin request -> complete stdout result or error stderr.
use aura_quantum_sim::symbolic_json::{
    execute_symbolic_json,SYMBOLIC_ERROR_SCHEMA,MAX_SYMBOLIC_INPUT_BYTES,
};
use serde_json::json;
use std::io::{self,Read};

fn fail(code:&str,message:&str)->! {
    eprintln!("{}",json!({
        "schema":SYMBOLIC_ERROR_SCHEMA,"status":"error",
        "code":code,"message":message,
        "evidence_class":"simulation","qpu_hardware_jobs":0
    }));
    std::process::exit(2);
}
fn main() {
    if std::env::args_os().len()!=1 {
        fail("usage","read one bounded symbolic JSON document on stdin; no flags");
    }
    let mut input=Vec::new();
    if let Err(e)=io::stdin().lock()
        .take((MAX_SYMBOLIC_INPUT_BYTES+1) as u64)
        .read_to_end(&mut input) {
        fail("io_error",&e.to_string());
    }
    match execute_symbolic_json(&input) {
        Ok(v)=>println!("{v}"),
        Err(e)=>fail(e.code(),&e.to_string()),
    }
}
