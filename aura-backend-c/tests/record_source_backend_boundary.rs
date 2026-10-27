//! Issue #51: real Aura source -> Parser -> sema -> IR -> C backend boundary.
//! Source-valid Record programs must fail C emission explicitly until layouts
//! and field operations are actually implemented. This is not native support.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{validate_module, InstKind};

#[test]
fn source_valid_record_read_is_never_a_successful_c_artifact_without_layouts() {
    let source = r#"
type Point = record {
    x: u32,
    y: u32,
}

cell read_x() ->:
    val point: Point = Point { x: 7, y: 11 }
    yield point.x
"#;
    let ast = aura_parse::parse_source(source).expect("valid Aura record syntax");
    let mut checker = Checker::new();
    checker.check_program(&ast).expect("Record program semantically valid");
    let module = lower_program(&ast).expect("real Aura Record lowered into IR");
    validate_module(&module).expect("Record IR must be SSA-valid");
    let function = module.functions.get("read_x").expect("record function");
    assert!(function.blocks.iter().flat_map(|b| b.insts.iter()).any(|i|
        matches!(&i.kind, InstKind::ConstructRecord { .. })),
        "real compiler must retain Record constructor instruction");
    assert!(function.blocks.iter().flat_map(|b| b.insts.iter()).any(|i|
        matches!(&i.kind, InstKind::GetField { .. })),
        "real compiler must retain field access instruction");
    match emit_module(&module, None) {
        Ok(artifact) => panic!(
            "real source Record unexpectedly emitted successful C without typed layout: {}",
            artifact.module_c
        ),
        Err(error) => {
            let msg = error.message.to_ascii_lowercase();
            assert!(msg.contains("record") && msg.contains("unqualified"),
                "C backend must reject Record for missing layout, not unrelated error: {error}");
        }
    }
}

#[test]
fn source_valid_plain_u32_control_still_emits_c() {
    let ast = aura_parse::parse_source("cell scalar() ->:\n    yield 17\n")
        .expect("valid scalar Aura source");
    let module = lower_program(&ast).expect("scalar Aura lowers");
    validate_module(&module).expect("valid scalar IR");
    let artifact = emit_module(&module, None).expect("scalar C backend remains available");
    assert!(artifact.module_c.contains("scalar"));
}
