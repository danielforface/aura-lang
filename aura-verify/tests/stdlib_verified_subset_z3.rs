#![cfg(feature = "z3")]

use aura_nexus::NexusContext;
use aura_verify::{verify_program_z3_profile, SmtProfile, Z3Prover};

#[test]
fn verified_stdlib_subset_passes_z3_fast_profile() {
    // Compile-time include to avoid any encoding/FS surprises.
    let src = include_str!("../../sdk/std/verified_core.aura");
    let program = aura_parse::parse_source(src).unwrap_or_else(|e| panic!("parse verified stdlib subset failed: {e:?}"));

    let mut prover = Z3Prover::new();
    let mut nexus = NexusContext::default();

    verify_program_z3_profile(&program, &mut prover, &(), &mut nexus, SmtProfile::Fast)
        .expect("expected verified stdlib subset to pass");
}
