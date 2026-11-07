//! Measured Q28 C-004 scaling, not a performance guarantee.
//! Each process emits one CSV record; for each width, run a separate process.
//! Timed window includes state allocation, pure gates, state validation.
//! VmHWM is a process-wide OS high-water mark, NOT an allocation profiler.
use aura_quantum_sim::{Gate, Statevector};
use std::error::Error;
use std::time::Instant;

fn peak_rss_kib() -> Option<usize> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find(|line| line.starts_with("VmHWM:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|amount| amount.parse::<usize>().ok())
}

fn run_once(qubits: usize, gate_count: usize) -> Result<f64, Box<dyn Error>> {
    let start = Instant::now();
    // 64 MiB logical state budget. Quotas and OS RSS are still distinct.
    let mut state = Statevector::zero(qubits, 64 * 1024 * 1024)?;
    for step in 0..gate_count {
        let q = step % qubits;
        let operation = match step % 5 {
            0 => Gate::H(q),
            1 => Gate::T(q),
            2 if qubits > 1 => Gate::Cnot {
                control: q,
                target: (q + 1) % qubits,
            },
            3 => Gate::S(q),
            _ => Gate::X(q),
        };
        state.apply(operation)?;
    }
    state.validate()?;
    std::hint::black_box(state.norm_squared()?);
    Ok(start.elapsed().as_secs_f64() * 1e6)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut qubits = None;
    let mut gates = 64_usize;
    let mut reps = 9_usize;
    let mut input = std::env::args().skip(1);
    while let Some(flag) = input.next() {
        let value = input.next().ok_or("expected a value after each flag")?;
        match flag.as_str() {
            "--qubits" => qubits = Some(value.parse::<usize>()?),
            "--gates" => gates = value.parse::<usize>()?,
            "--reps" => reps = value.parse::<usize>()?,
            _ => return Err(format!("unknown benchmark flag: {flag}").into()),
        }
    }
    let n = qubits.ok_or("--qubits is mandatory")?;
    if !(1..=18).contains(&n) || gates == 0 || !(1..=100).contains(&reps) {
        return Err("limits: n=1..18, gates>0, reps=1..100".into());
    }

    // One unreported warmup per benchmark process.
    run_once(n, gates)?;
    let mut times = Vec::with_capacity(reps);
    for _ in 0..reps {
        times.push(run_once(n, gates)?);
    }
    times.sort_by(|a, b| a.total_cmp(b));
    let median = if reps % 2 == 1 {
        times[reps / 2]
    } else {
        (times[reps / 2 - 1] + times[reps / 2]) * 0.5
    };
    let p95_index = ((0.95 * reps as f64).ceil() as usize).saturating_sub(1);
    let state_bytes = (1usize << n) * std::mem::size_of::<aura_quantum_sim::Complex>();
    let rss = peak_rss_kib().map_or_else(|| "NA".to_owned(), |value| value.to_string());
    println!("qubits,gates,reps,state_bytes,min_us,median_us,p95_us,max_us,peak_process_rss_kib");
    println!(
        "{n},{gates},{reps},{state_bytes},{:.3},{median:.3},{:.3},{:.3},{rss}",
        times[0], times[p95_index], times[reps - 1]
    );
    Ok(())
}
