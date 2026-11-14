//! Separate bounded noisy-density JSON command line. The existing
//! "aura-quantum-run" pure circuit v1 remains fully backward compatible.
use aura_quantum_sim::noisy_json::{
    execute_noisy_json_bytes, NOISY_ERROR_SCHEMA, MAX_NOISY_REQUEST_BYTES,
};
use serde_json::json;
use std::io::{self, Read};

fn reject(code:&str, message:&str) -> ! {
    eprintln!("{}", json!({
        "schema": NOISY_ERROR_SCHEMA,
        "status": "error",
        "code": code,
        "message": message,
        "evidence_class": "simulation",
        "qpu_hardware_jobs": 0
    }));
    std::process::exit(2);
}

fn main() {
    if std::env::args_os().len()!=1 {
        reject("usage","read exactly one bounded JSON document on stdin");
    }
    let mut data=Vec::new();
    if let Err(e)=io::stdin().lock()
        .take((MAX_NOISY_REQUEST_BYTES+1) as u64)
        .read_to_end(&mut data) {
        reject("io_error",&e.to_string());
    }
    match execute_noisy_json_bytes(&data) {
        Ok(response)=>println!("{response}"),
        Err(err)=>reject(err.code(),&err.to_string()),
    }
}
