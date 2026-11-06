//! Standalone stdin-JSON to stdout-JSON CPU-only simulator.
use aura_quantum_sim::json_protocol::{
    execute_json_bytes, ERROR_SCHEMA, MAX_REQUEST_BYTES,
};
use serde_json::json;
use std::io::{self, Read};

fn failure(code: &str, message: &str) -> ! {
    // A stable machine-readable error on stderr; stdout remains empty.
    eprintln!(
        "{}",
        json!({
            "schema": ERROR_SCHEMA,
            "status": "error",
            "code": code,
            "message": message,
            "qpu_hardware_jobs": 0,
        })
    );
    std::process::exit(2);
}

fn main() {
    if std::env::args_os().len() != 1 {
        failure("usage", "read exactly one JSON request from stdin; no CLI flags");
    }
    let mut data = Vec::new();
    let mut stream = io::stdin().lock().take((MAX_REQUEST_BYTES + 1) as u64);
    if let Err(error) = stream.read_to_end(&mut data) {
        failure("io_error", &error.to_string());
    }
    match execute_json_bytes(&data) {
        Ok(response) => println!("{response}"),
        Err(error) => failure(error.code(), &error.to_string()),
    }
}
