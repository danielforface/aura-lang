use aura_core::{lower_program, Checker};

#[test]
fn test_all_integer_types_sema_and_lowering() {
    let src = r#"
val a: u8 = 8
val b: u16 = 16
val c: u32 = 32
val d: u64 = 64
val e: u128 = 128
val f: usize = 100

val g: i8 = 8
val h: i16 = 16
val i: i32 = 32
val j: i64 = 64
val k: i128 = 128
val l: isize = 100
"#;
    let program = aura_parse::parse_source(src).expect("parse integer types");
    let mut checker = Checker::new();
    checker.check_program(&program).expect("sema integer types");

    let ir = lower_program(&program).expect("lower integer types");
    assert!(!ir.functions.is_empty());
}

#[test]
fn test_float_and_text_types_sema_and_lowering() {
    let src = r#"
val f1: f32 = 0
val f2: f64 = 0
val b: bool = true
val s: String = "aura 1.0"
"#;
    let program = aura_parse::parse_source(src).expect("parse float and text types");
    let mut checker = Checker::new();
    checker.check_program(&program).expect("sema float and text types");

    let ir = lower_program(&program).expect("lower float and text types");
    assert!(!ir.functions.is_empty());
}

#[test]
fn test_record_and_enum_types_sema_and_lowering() {
    let src = r#"
type Point = record {
    x: i64,
    y: i64,
}

type Status = enum {
    Active,
    Code(code: u32),
}

val p: Point = Point { x: 10, y: 20 }
val px: i64 = p.x
val s1: Status = Status::Active
val s2: Status = Status::Code(404)
"#;
    let program = aura_parse::parse_source(src).expect("parse record and enum types");
    let mut checker = Checker::new();
    checker.check_program(&program).expect("sema record and enum types");

    let ir = lower_program(&program).expect("lower record and enum types");
    assert!(!ir.functions.is_empty());
}
