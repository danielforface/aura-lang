//! C-026 full mixed-state Pauli Hamiltonian mean/variance, no QPU.
use aura_quantum_sim::noisy_hamiltonian_json::{
    execute_noisy_hamiltonian_json,NOISY_HAMILTONIAN_ERROR_SCHEMA,
};
use aura_quantum_sim::noisy_json::MAX_NOISY_REQUEST_BYTES;
use serde_json::json;
use std::io::{self,Read};
fn fail(code:&str,message:&str)->!{
    eprintln!("{}",json!({
        "schema":NOISY_HAMILTONIAN_ERROR_SCHEMA,"status":"error",
        "code":code,"message":message,
        "evidence_class":"simulation","qpu_hardware_jobs":0
    }));
    std::process::exit(2)
}
fn main(){
    if std::env::args_os().len()!=1{
        fail("usage","single strict offline noisy-hamiltonian/1 JSON, no flags");
    }
    let mut raw=Vec::new();
    if let Err(e)=io::stdin().lock()
        .take((MAX_NOISY_REQUEST_BYTES+1) as u64).read_to_end(&mut raw){
        fail("io_error",&e.to_string());
    }
    match execute_noisy_hamiltonian_json(&raw){
        Ok(s)=>println!("{s}"),
        Err(e)=>fail(e.code(),&e.to_string()),
    }
}
