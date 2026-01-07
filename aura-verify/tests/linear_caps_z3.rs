#![cfg(feature = "z3")]

use aura_nexus::NexusContext;
use aura_verify::{verify_program_z3_profile, SmtProfile, Z3Prover};

#[test]
fn z3_use_after_consume_in_flow_has_related_consumed_span() {
    // This test intentionally bypasses `aura-core` sema and exercises the Z3 verifier directly.
    //
    // The goal is to ensure the verifier models linear capability liveness transitions and that
    // a use-after-consume failure includes an explainable "consumed here" related span.
    let src = r#"
cell id(x: u32) ->:
    yield x

cell main() ->:
    val a: u32 = 1
    val b: u32 = a -> id()
    val c: u32 = b -> id()
    val d: u32 = c -> id()

    # use-after-consume (a was consumed by the first flow)
    val bad: u32 = a + 1
"#;

    let program = aura_parse::parse_source(src).expect("parse");

    let mut prover = Z3Prover::new();
    let mut nexus = NexusContext::default();

    let err = verify_program_z3_profile(&program, &mut prover, &(), &mut nexus, SmtProfile::Fast)
        .expect_err("expected verifier to reject use-after-consume");

    assert!(
        err.message.contains("use-after-consume") || err.message.contains("not available"),
        "unexpected message: {}",
        err.message
    );

    let related = err
        .meta
        .as_ref()
        .expect("expected diagnostic metadata")
        .related
        .iter()
        .map(|ri| ri.message.as_str())
        .collect::<Vec<_>>();

    assert!(
        related.iter().any(|m| m.contains("consumed here")),
        "expected a related 'consumed here' note; got: {:?}",
        related
    );
}
