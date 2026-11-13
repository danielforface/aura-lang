//! C-025 real offline 2x2 PPT entanglement analysis, no hardware/QPU.
use aura_quantum_sim::noisy_entanglement_json::{
    execute_noisy_pair_entanglement_json,ENTANGLEMENT_ERROR_SCHEMA,
};
use aura_quantum_sim::noisy_json::MAX_NOISY_REQUEST_BYTES;
use serde_json::json;
use std::io::{self,Read};
fn fail(code:&str,message:&str)->!{
    eprintln!("{}",json!({
        "schema":ENTANGLEMENT_ERROR_SCHEMA,"status":"error",
        "code":code,"message":message,
        "evidence_class":"simulation","qpu_hardware_jobs":0
    }));
    std::process::exit(2)
}
fn main(){
    if std::env::args_os().len()!=1{
        fail("usage","one strict offline noisy-pair-entanglement/1 request");
    }
    let mut bytes=Vec::new();
    if let Err(e)=io::stdin().lock()
        .take((MAX_NOISY_REQUEST_BYTES+1) as u64)
        .read_to_end(&mut bytes){
        fail("io_error",&e.to_string());
    }
    match execute_noisy_pair_entanglement_json(&bytes){
        Ok(result)=>println!("{result}"),
        Err(e)=>fail(e.code(),&e.to_string()),
    }
}
