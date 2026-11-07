//! Real caller using one CPU-only automatic circuit-entry point.
use aura_quantum_sim::dispatch_json::execute_dispatch_json;

fn main()->Result<(),Box<dyn std::error::Error>> {
    let cases=[
        ("clifford",include_bytes!("../fixtures/dispatch_clifford_v1.json").as_slice(),
                    "clifford_stabilizer"),
        ("nonclifford",include_bytes!("../fixtures/dispatch_nonclifford_v1.json").as_slice(),
                       "pure_statevector"),
        ("noise",include_bytes!("../fixtures/dispatch_noise_v1.json").as_slice(),
                "noisy_density"),
        ("exact",include_bytes!("../fixtures/dispatch_exact_v1.json").as_slice(),
                "exact_symbolic"),
    ];
    for (name,raw,expected) in cases {
        let encoded=execute_dispatch_json(raw)?;
        let result:serde_json::Value=serde_json::from_str(&encoded)?;
        if result["selected_backend"]!=expected ||
            result["qpu_hardware_jobs"]!=0 ||
            result["engine_result"]["status"]!="simulated" {
            return Err(format!("{name}: unsafe backend selection").into());
        }
        println!("Q28_DISPATCH_DELIVERY_PASS case={name} backend={expected} inner_schema={}",
                result["engine_result"]["schema"]);
    }
    Ok(())
}
