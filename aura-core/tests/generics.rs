use aura_core::Checker;

#[test]
fn generic_record_infers_and_allows_annotation() {
    let src = "type Box<T> = record { x: T }\n\nval b: Box<u32> = Box { x: 1 }\nval y: u32 = b.x\n";
    let program = aura_parse::parse_source(src).expect("parse");
    Checker::new().check_program(&program).expect("sema");
}

#[test]
fn generic_enum_ctor_and_match_binder_types() {
    let src = "type Option<T> = enum { Some(x: T), None }\n\nval o: Option<u32> = Option::Some(1)\n\nmatch o:\n    Option::Some(x):\n        val y: u32 = x\n    _:\n        val z: u32 = 0\n";
    let program = aura_parse::parse_source(src).expect("parse");
    Checker::new().check_program(&program).expect("sema");
}
