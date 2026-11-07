//! Issue #51: real Aura source -> Parser -> sema -> IR -> C backend boundary.
//! Source-valid Record programs must fail C emission explicitly until layouts
//! and field operations are actually implemented. This is not native support.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{validate_module, InstKind};

#[test]
fn source_valid_record_read_emits_concrete_typed_scalar_c_layout() {
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
    let artifact = emit_module(&module, None)
        .expect("qualified local scalar Record now emits real named C struct fields");
    assert!(artifact.module_c.contains("aura_record_"));
    assert!(artifact.module_c.contains("aura_f_"));

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
