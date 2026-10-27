//! Format/parse round-trip must preserve FieldAssign as a distinct AST node.
use aura_ast::{Stmt, ExprKind};

#[test]
fn mutable_record_field_assignment_roundtrip_preserves_target_and_rhs() {
    let source = r#"
type Point = record {
    x: u32,
    y: u32,
}
cell update() ->:
    val mut p: Point = Point { x: 7, y: 13 }
    p.x = 19
    yield p.x
"#;
    let parsed = aura_parse::parse_source(source).expect("source parses");
    let formatted = aura_parse::format_program(&parsed);
    assert!(formatted.contains("p.x = 19"),
        "formatter may not erase or misprint a Record field assignment: {formatted}");
    let reparsed = aura_parse::parse_source(&formatted).expect("formatted source reparses");
    let cell = reparsed.stmts.iter().find_map(|stmt|
        if let Stmt::CellDef(cell) = stmt { Some(cell) } else { None }
    ).expect("entry cell");
    let fields: Vec<_> = cell.body.stmts.iter().filter_map(|stmt|
        if let Stmt::FieldAssign(a) = stmt { Some(a) } else { None }
    ).collect();
    assert_eq!(fields.len(), 1, "must preserve distinct FieldAssign AST");
    assert_eq!(fields[0].target.node, "p");
    assert_eq!(fields[0].field.node, "x");
    assert!(matches!(fields[0].expr.kind, ExprKind::IntLit(19)));
}

#[test]
fn deeper_chained_field_assignment_is_not_silently_accepted_as_scalar_binding() {
    let source = r#"
type Point = record { x: u32, y: u32 }
cell reject() ->:
    val mut p: Point = Point { x: 7, y: 13 }
    p.x.y = 19
    yield p.x
"#;
    assert!(aura_parse::parse_source(source).is_err(),
        "only single-level Record field assign is qualified; chained fields need explicit AST/typing");
}
