//! C-015 high-shot histogram only: exact seed parity, no retained trace,
//! strict full-circuit preflight and result/resource admission limits.
use aura_quantum_sim::circuit::{Circuit,Instruction};
use aura_quantum_sim::factorized::{
    FactorCircuit,FactorError,MAX_FACTOR_COUNT_SHOTS,MAX_FACTOR_COUNT_BINS,

};
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
use std::collections::BTreeMap;

fn histogram(records:&[aura_quantum_sim::circuit::ShotRecord])
    ->BTreeMap<String,usize>
{
    let mut out=BTreeMap::new();
    for r in records {
        let key=r.bits.iter().rev().map(|b|match b {
            Some(true)=>'1',Some(false)=>'0',None=>'?'
        }).collect::<String>();
        *out.entry(key).or_insert(0)+=1;
    }
    out
}
fn hybrid()->FactorCircuit {
    let mut p=FactorCircuit::new(4,3);
    for op in [
        Instruction::Gate(Gate::H(0)),
        Instruction::Gate(Gate::T(1)),
        Instruction::Gate(Gate::Cnot{control:0,target:1}),
        Instruction::Measure{qubit:0,into_bit:0},
        Instruction::If{bit:0,equals:true,gate:Gate::X(1)},
        Instruction::Gate(Gate::H(2)),
        Instruction::Reset{qubit:2},
        Instruction::Gate(Gate::Cz{control:1,target:3}),
        Instruction::Measure{qubit:1,into_bit:2},
    ] {p.push(op);}
    p
}
#[test]
fn zero_to_1024_shots_match_legacy_full_trace_histograms_for_many_seeds() {
    let p=hybrid();
    for seed in [0u64,1,0x5eed,20261108,u64::MAX] {
        for shots in [1,2,7,64,512,1024] {
            let old=p.execute_shots(shots,seed,DEFAULT_MEMORY_BUDGET).unwrap();
            let new=p.execute_counts(shots,seed,DEFAULT_MEMORY_BUDGET).unwrap();
            assert_eq!(new.histogram,histogram(&old.records));
            assert_eq!(new.metadata.seed,old.metadata.seed);
            assert_eq!(new.metadata.shots,old.metadata.shots);
            assert_eq!(new.projective_wire_splits,old.projective_wire_splits);
            assert_eq!(new.projective_split_deferred,old.projective_split_deferred);
            assert_eq!(new.product_preserving_two_wire_gates,
                       old.product_preserving_two_wire_gates);
            assert_eq!(new.retained_shot_records,0);
            assert!(new.histogram.values().sum::<usize>()==shots);
        }
    }
}
#[test]
fn real_65536_shot_measurement_has_no_per_shot_records_or_traces() {
    let mut p=FactorCircuit::new(1,1);
    p.push(Instruction::Gate(Gate::H(0)));
    p.push(Instruction::Measure{qubit:0,into_bit:0});
    let counts=p.execute_counts(MAX_FACTOR_COUNT_SHOTS,20261108,DEFAULT_MEMORY_BUDGET)
        .unwrap();
    assert_eq!(counts.metadata.shots,65536);
    assert_eq!(counts.retained_shot_records,0);
    assert_eq!(counts.max_histogram_bins,MAX_FACTOR_COUNT_BINS);
    assert_eq!(counts.histogram.len(),2);
    assert_eq!(counts.histogram.values().sum::<usize>(),65536);
    let ones=counts.histogram["1"];
    assert!(ones.abs_diff(32768)<800,"ones={ones}");
    assert_eq!(counts.logical_histogram_reserved_bytes,2*(128+1));
    assert_eq!(counts.metadata.qpu_hardware_jobs,0);
    assert!(counts.peak_logical_state_bytes<=128);
}
#[test]
fn no_classical_measurements_preserve_existing_empty_string_histogram_key() {
    let p=FactorCircuit::new(128,0);
    let a=p.execute_counts(4096,1709,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(a.histogram.len(),1);
    assert_eq!(a.histogram[""],4096);
    assert_eq!(a.projective_wire_splits,0);
    assert_eq!(a.product_preserving_two_wire_gates,0);
    assert_eq!(a.peak_logical_state_bytes,4096);
}
#[test]
fn unwritten_register_bits_have_ordered_question_mark_histogram_encoding() {
    let mut p=FactorCircuit::new(1,3);
    p.push(Instruction::Gate(Gate::H(0)));
    p.push(Instruction::Measure{qubit:0,into_bit:1});
    let counts=p.execute_counts(256,98765,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(counts.histogram.len(),2);
    assert!(counts.histogram.contains_key("?0?"));
    assert!(counts.histogram.contains_key("?1?"));
    assert_eq!(counts.histogram.values().sum::<usize>(),256);
}
#[test]
fn count_mode_rejects_zero_shots_excess_shots_and_excess_cpu_work() {
    let p=FactorCircuit::new(2,0);
    assert_eq!(p.execute_counts(0,0,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        FactorError::ZeroShots);
    assert_eq!(p.execute_counts(MAX_FACTOR_COUNT_SHOTS+1,0,DEFAULT_MEMORY_BUDGET)
        .unwrap_err(),FactorError::TooManyShots);
    let mut slow=FactorCircuit::new(1,0);
    for _ in 0..5 {slow.push(Instruction::Gate(Gate::T(0)));}
    assert_eq!(slow.execute_counts(MAX_FACTOR_COUNT_SHOTS,0,DEFAULT_MEMORY_BUDGET)
        .unwrap_err(),FactorError::WorkLimit);
}
#[test]
fn excessive_distinct_outcomes_fail_closed_above_4096_bins() {
    let mut p=FactorCircuit::new(13,13);
    for q in 0..13 {
        p.push(Instruction::Gate(Gate::H(q)));
        p.push(Instruction::Measure{qubit:q,into_bit:q});
    }
    let err=p.execute_counts(8000,20261108,DEFAULT_MEMORY_BUDGET).unwrap_err();
    assert_eq!(err,FactorError::HistogramBinLimit{
        max_bins:MAX_FACTOR_COUNT_BINS,
    });
}
#[test]
fn invalid_inactive_conditionals_are_preflight_rejected_before_sampling() {
    let mut p=FactorCircuit::new(2,1);
    p.push(Instruction::Measure{qubit:0,into_bit:0});
    p.push(Instruction::If{bit:0,equals:true,
        gate:Gate::Cnot{control:1,target:1}});
    assert_eq!(p.execute_counts(4096,7,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        FactorError::DuplicateQubit(1));
    let mut uninit=FactorCircuit::new(2,1);
    uninit.push(Instruction::If{bit:0,equals:true,gate:Gate::X(1)});
    assert_eq!(uninit.execute_counts(512,9,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        FactorError::ReadBeforeWrite{bit:0,instruction:0});
}
#[test]
fn counts_memory_budget_accounts_for_histogram_without_relaxing_join_guard() {
    let p=FactorCircuit::new(2,0);
    assert!(matches!(p.execute_counts(1,0,128),
        Err(FactorError::BudgetExceeded{..})));
    let mut controlled=FactorCircuit::new(2,1);
    controlled.push(Instruction::Gate(Gate::H(0)));
    controlled.push(Instruction::Gate(Gate::Cnot{control:0,target:1}));
    controlled.push(Instruction::Measure{qubit:0,into_bit:0});
    assert!(matches!(controlled.execute_counts(1,1,160),
        Err(FactorError::BudgetExceeded{..})));
}
#[test]
fn full_bin_metadata_comes_from_actual_gate_and_projective_calls() {
    let mut p=FactorCircuit::new(3,1);
    p.push(Instruction::Gate(Gate::H(0)));
    p.push(Instruction::Gate(Gate::Cnot{control:0,target:1}));
    p.push(Instruction::Measure{qubit:0,into_bit:0});
    p.push(Instruction::If{bit:0,equals:true,gate:Gate::X(0)});
    p.push(Instruction::Gate(Gate::Cnot{control:0,target:2}));
    let counts=p.execute_counts(64,13,DEFAULT_MEMORY_BUDGET).unwrap();
    let old=p.execute_shots(64,13,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(counts.histogram,histogram(&old.records));
    assert_eq!(counts.projective_wire_splits,64);
    assert_eq!(counts.product_preserving_two_wire_gates,64);
    assert_eq!(counts.projective_split_deferred,0);
}
#[test]
fn baseline_dense_simulation_seed_matches_count_mode_for_bell_measurements() {
    let mut factor=FactorCircuit::new(2,1);
    let mut dense=Circuit::new(2,1);
    for op in [
        Instruction::Gate(Gate::H(0)),
        Instruction::Gate(Gate::Cnot{control:0,target:1}),
        Instruction::Measure{qubit:0,into_bit:0},
    ] {factor.push(op.clone());dense.push(op);}
    let counts=factor.execute_counts(256,4242,DEFAULT_MEMORY_BUDGET).unwrap();
    let reference=dense.execute_shots(256,4242,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(counts.histogram,histogram(&reference.shots));
}
