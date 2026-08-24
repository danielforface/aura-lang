use aura_parse::parse_source;

#[test]
fn chained_comparisons_are_rejected() {
    let src = "val x = 0\nval y = 1\nval z = 2\nval a = x < y < z\n";
    let err = parse_source(src).expect_err("expected parse error");
    let msg = err.to_string();
    assert!(
        msg.contains("chained comparisons"),
        "unexpected error message: {msg}"
    );
}

#[test]
fn match_statement_parses() {
    let src = "match 1:\n    1:\n        val a = 0\n    _:\n        val a = 1\n";
    parse_source(src).expect("match should parse");
}

#[test]
fn record_enum_and_where_parse() {
    let src = r#"
trait Numeric

type Box<T: Numeric> = T

type Point = record { x: u32 = 0, y: u32 = 0 }
type OptionU32 = enum { Some(value: u32), None() }

cell main() ->:
    val p: Point = Point { x: 10 }
    val _px: u32 = p.x
    val n: Int where n >= 0 = 10
    val m: Int where m >= 0 = n + 2
    match OptionU32::Some(1):
        OptionU32::Some(v):
            val _k = v
        _:
            val _k = 0
"#;
    parse_source(src).expect("new 0.3 syntax should parse");
}

#[test]
fn extern_cell_supports_arrow_and_colon() {
    let src1 = "trusted extern cell println(text: String) -> Unit\n";
    parse_source(src1).expect("extern cell with arrow should parse");

    let src2 = "extern cell native_read(fd: u32): u32\n";
    parse_source(src2).expect("extern cell with colon should parse");
}
