//! Q03-001: actual public Aura no-Z3 command path, not just emit_module().
#![cfg(not(feature = "z3"))]
use std::{fs, path::{Path, PathBuf}, process::{Command, Output}, sync::atomic::{AtomicU64, Ordering}, time::{SystemTime, UNIX_EPOCH}};
static NEXT_CLI_SCRATCH: AtomicU64 = AtomicU64::new(0);
struct SourceFile { root: PathBuf, file: PathBuf }
impl SourceFile {
    fn new(text: &str) -> Self {
        let tick = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let sequence = NEXT_CLI_SCRATCH.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("aura-record-cli-{}-{tick}-{sequence}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let file = root.join("main.aura");
        fs::write(&file, text).unwrap();
        Self { root, file }
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_aura"))
            .arg(args[0]).arg(&self.file).args(&args[1..])
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .output().expect("run real Aura no-Z3 CLI binary")
    }
}
impl Drop for SourceFile {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); }
}
#[test]
fn genuine_local_scalar_record_passes_check_and_c_artifact_build_without_z3() {
    let fixture = SourceFile::new(r#"
type Point = record {
    x: u32,
    y: u32,
}
cell main() ->:
    val p: Point = Point { y: 13, x: 7 }
    yield p.x
"#);
    let check = fixture.run(&["check"]);
    assert!(check.status.success(),
        "source record check failed; stdout={} stderr={}",
        String::from_utf8_lossy(&check.stdout), String::from_utf8_lossy(&check.stderr));
    assert!(String::from_utf8_lossy(&check.stdout).contains("formal verification NOT RUN"),
        "no Z3 check must not assert proof");
    let build = fixture.run(&["build", "--mode", "hybrid", "--backend", "c", "--profile", "dev"]);
    assert!(build.status.success(),
        "source Record C build failed; stdout={} stderr={}",
        String::from_utf8_lossy(&build.stdout), String::from_utf8_lossy(&build.stderr));
}


#[test]
fn public_no_z3_aura_cli_accepts_record_field_assign_through_if_else_join() {
    let fixture = SourceFile::new(r#"
type Point = record { x: u32, y: u32 }
cell main() ->:
    val mut p: Point = Point { x: 7, y: 13 }
    if true:
        p.x = 19
    else:
        p.x = 23
    yield p.x
"#);
    let check = fixture.run(&["check"]);
    assert!(check.status.success(),
        "public Aura check must accept typed flat Record Phi source, stderr={}",
        String::from_utf8_lossy(&check.stderr));
    assert!(String::from_utf8_lossy(&check.stdout).contains("formal verification NOT RUN"),
        "no Z3 enabled: check cannot claim proof");
    let build = fixture.run(&["build", "--mode", "hybrid", "--backend", "c", "--profile", "dev"]);
    assert!(build.status.success(),
        "public Aura C build must preserve Record Phi, stdout={} stderr={}",
        String::from_utf8_lossy(&build.stdout), String::from_utf8_lossy(&build.stderr));
}


#[test]
fn public_no_z3_aura_cli_accepts_cross_cell_nominal_record_return_and_argument() {
    let fixture = SourceFile::new(r#"
type Point = record { x: u32, y: u32 }
cell make_point(x: u32, y: u32) ->:
    yield Point { y: y, x: x }
cell project_x(p: Point) ->:
    yield p.x
cell main() ->:
    val p: Point = make_point(7, 13)
    yield project_x(p)
"#);
    let check = fixture.run(&["check"]);
    assert!(check.status.success(),
        "public CLI check rejects Record parameter/return: stdout={} stderr={}",
        String::from_utf8_lossy(&check.stdout), String::from_utf8_lossy(&check.stderr));
    assert!(String::from_utf8_lossy(&check.stdout).contains("formal verification NOT RUN"),
        "no-Z3 public CLI must not claim formal proof of Record ABI");
    let build = fixture.run(&["build", "--mode", "hybrid", "--backend", "c", "--profile", "dev"]);
    assert!(build.status.success(),
        "public CLI C build rejects nominal Record function ABI: stdout={} stderr={}",
        String::from_utf8_lossy(&build.stdout), String::from_utf8_lossy(&build.stderr));
}


#[test]
fn public_no_z3_aura_named_record_function_abi_check_and_build() {
    let fixture = SourceFile::new(r#"
type Point = record { x: u32, y: u32 }
cell make_point(x: u32, y: u32) ->:
    yield Point { x: x, y: y }
cell sum_point(p: Point, increment: u32) ->:
    yield p.x + increment
cell main() ->:
    val p: Point = make_point(y: 13, x: 7)
    yield sum_point(increment: 5, p: p)
"#);
    let check = fixture.run(&["check"]);
    assert!(check.status.success(),
        "public Aura check rejects named Record ABI: stdout={} stderr={}",
        String::from_utf8_lossy(&check.stdout), String::from_utf8_lossy(&check.stderr));
    assert!(String::from_utf8_lossy(&check.stdout).contains("formal verification NOT RUN"),
        "a successful no-Z3 check must not claim formal verification");
    let build = fixture.run(&["build", "--mode", "hybrid", "--backend", "c", "--profile", "dev"]);
    assert!(build.status.success(),
        "public Aura named Record C build failed: stdout={} stderr={}",
        String::from_utf8_lossy(&build.stdout), String::from_utf8_lossy(&build.stderr));
}


#[test]
fn public_no_z3_aura_nested_nominal_record_function_check_and_c_build() {
    let fixture = SourceFile::new(r#"
type Point = record { x: u32, y: u32 }
type Outer = record { p: Point, tag: u32 }
cell make_outer() ->:
    yield Outer { p: Point { x: 7, y: 13 }, tag: 99 }
cell read_x(o: Outer) ->:
    yield o.p.x
cell main() ->:
    val mut o: Outer = make_outer()
    o.p = Point { x: 41, y: 43 }
    yield read_x(o)
"#);
    let check=fixture.run(&["check"]);
    assert!(check.status.success(),
        "public CLI nested nominal Record check fails: stdout={} stderr={}",
        String::from_utf8_lossy(&check.stdout), String::from_utf8_lossy(&check.stderr));
    assert!(String::from_utf8_lossy(&check.stdout).contains("formal verification NOT RUN"),
        "no-Z3 nested Record check must disclose no formal verification");
    let build=fixture.run(&["build","--mode","hybrid","--backend","c","--profile","dev"]);
    assert!(build.status.success(),
        "public CLI nested nominal Record native C build fails: stdout={} stderr={}",
        String::from_utf8_lossy(&build.stdout), String::from_utf8_lossy(&build.stderr));
}


#[test]
fn public_no_z3_aura_omitted_scalar_record_defaults_check_and_build() {
    let fixture=SourceFile::new(r#"
type Point = record { x: u32 = 7, y: u32 = 13 }
cell build_point() ->:
    yield Point { x: 19 }
cell main() ->:
    val p: Point = build_point()
    yield p.y
"#);
    let check=fixture.run(&["check"]);
    assert!(check.status.success(),
        "public Aura defaults check: stdout={} stderr={}",
        String::from_utf8_lossy(&check.stdout),String::from_utf8_lossy(&check.stderr));
    assert!(String::from_utf8_lossy(&check.stdout).contains("formal verification NOT RUN"),
        "no Z3 verification must not be represented as a formal proof");
    let build=fixture.run(&["build","--mode","hybrid","--backend","c","--profile","dev"]);
    assert!(build.status.success(),
        "public Aura default-valued Record C build: stdout={} stderr={}",
        String::from_utf8_lossy(&build.stdout),String::from_utf8_lossy(&build.stderr));
}


#[test]
fn public_no_z3_aura_check_rejects_invalid_unused_or_overridden_record_declarations() {
    for (source, expected) in [
        (
            "type Unused = record { x: u32 = true, y: u32 }\ncell main() ->:\n    yield 7\n",
            "type",
        ),
        (
            "type Point = record { x: u32 = 1 + 2, y: u32 }\ncell main() ->:\n    val p: Point = Point { x: 7, y: 13 }\n    yield p.x\n",
            "unqualified",
        ),
        (
            "type Point = record { x: u32 = 7, x: u32 = 13 }\ncell main() ->:\n    yield 7\n",
            "duplicate",
        ),
    ] {
        let fixture = SourceFile::new(source);
        let check = fixture.run(&["check"]);
        let output = format!("{}\n{}", String::from_utf8_lossy(&check.stdout),
            String::from_utf8_lossy(&check.stderr));
        assert!(!check.status.success(),
            "public Aura check must reject invalid Record declaration: {output}");
        assert!(output.contains(expected),
            "public Aura check must preserve semantic diagnostic {expected:?}: {output}");
    }
}


#[test]
fn public_no_z3_aura_nested_nominal_defaults_check_and_build() {
    let fixture=SourceFile::new(r#"
type Point = record { x: u32 = 7, y: u32 = 13 }
type Outer = record { p: Point = Point {}, tag: u32 = 99 }
cell make_outer() ->:
    yield Outer {}
cell main() ->:
    val o: Outer = make_outer()
    yield o.p.x
"#);
    let check=fixture.run(&["check"]);
    assert!(check.status.success(),
        "public Aura nested defaults check failed: stdout={} stderr={}",
        String::from_utf8_lossy(&check.stdout),String::from_utf8_lossy(&check.stderr));
    assert!(String::from_utf8_lossy(&check.stdout).contains("formal verification NOT RUN"),
        "no-Z3 check cannot claim formal verification");
    let build=fixture.run(&["build","--mode","hybrid","--backend","c","--profile","dev"]);
    assert!(build.status.success(),
        "public Aura nested default Record build failed: stdout={} stderr={}",
        String::from_utf8_lossy(&build.stdout),String::from_utf8_lossy(&build.stderr));
}


#[test]
fn public_no_z3_aura_nested_field_assignment_check_and_build() {
    let fixture=SourceFile::new(r#"
type Point = record { x: u32 = 7, y: u32 = 13 }
type Outer = record { p: Point = Point {}, tag: u32 = 99 }
cell main() ->:
    val mut o: Outer = Outer {}
    o.p.x = 19
    yield o.p.x
"#);
    let check=fixture.run(&["check"]);
    assert!(check.status.success(),
        "public nested assignment check failed: stdout={} stderr={}",
        String::from_utf8_lossy(&check.stdout),String::from_utf8_lossy(&check.stderr));
    assert!(String::from_utf8_lossy(&check.stdout).contains("formal verification NOT RUN"));
    let build=fixture.run(&["build","--mode","hybrid","--backend","c","--profile","dev"]);
    assert!(build.status.success(),
        "public nested assignment native build failed: stdout={} stderr={}",
        String::from_utf8_lossy(&build.stdout),String::from_utf8_lossy(&build.stderr));
}


#[test]
fn public_aura_check_rejects_record_integer_narrowing_before_native_build() {
    for (source,label) in [
        ("type Tiny = record { n: u8 = 300 }\ncell main() ->:\n    yield 7\n", "unused u8 default"),
        ("type Tiny = record { n: i8 = 200 }\ncell main() ->:\n    yield 7\n", "unused i8 default"),
        ("type Tiny = record { n: u8 }\ncell main() ->:\n    val t: Tiny = Tiny { n: 256 }\n    yield 7\n", "explicit u8 initializer"),
        ("type Tiny = record { n: u8 = 7 }\ncell main() ->:\n    val mut t: Tiny = Tiny {}\n    t.n = 300\n    yield t.n\n", "mutable u8 field assignment"),
    ] {
        let fixture=SourceFile::new(source);
        let check=fixture.run(&["check"]);
        let diagnostic=format!("{}\n{}",
            String::from_utf8_lossy(&check.stdout),
            String::from_utf8_lossy(&check.stderr));
        assert!(!check.status.success(),
            "{label}: public checker must reject narrowing before codegen: {diagnostic}");
        assert!(diagnostic.contains("out of range"),
            "{label}: preserve exact width diagnostic: {diagnostic}");
    }
}


#[test]
fn public_aura_check_rejects_duplicate_nominal_record_type_definitions() {
    let source=concat!(
        "type Point = record { x: u32 = 7 }\n",
        "type Point = record { x: u32 = 13 }\n",
        "cell main() ->:\n    yield 7\n"
    );
    let fixture=SourceFile::new(source);
    let checked=fixture.run(&["check"]);
    let output=format!("{}\n{}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr));
    assert!(!checked.status.success(),
        "public Aura check must reject duplicate Record declarations: {output}");
    assert!(output.contains("duplicate") && output.contains("Point"),
        "public diagnostic must explain nominal type collision: {output}");
}

#[test]
fn public_aura_check_rejects_type_namespace_collisions_even_if_unused() {
    for (label, declarations) in [
        ("record_then_enum", "type Collision = record { x: u32 = 7 }\ntype Collision = enum { First }\n"),
        ("enum_then_record", "type Collision = enum { First }\ntype Collision = record { x: u32 = 7 }\n"),
        ("record_then_alias", "type Collision = record { x: u32 = 7 }\ntype Collision = u32\n"),
        ("alias_then_record", "type Collision = u32\ntype Collision = record { x: u32 = 7 }\n"),
        ("enum_then_alias", "type Collision = enum { First }\ntype Collision = u32\n"),
        ("alias_then_enum", "type Collision = u32\ntype Collision = enum { First }\n"),
        ("enum_then_enum", "type Collision = enum { First }\ntype Collision = enum { Second }\n"),
        ("alias_then_alias", "type Collision = u32\ntype Collision = bool\n"),
    ] {
        let source = format!("{declarations}cell main() ->:\n    yield 7\n");
        let fixture = SourceFile::new(&source);
        let result = fixture.run(&["check"]);
        let diagnostic = format!("{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr));
        assert!(!result.status.success(),
            "{label}: public Aura check accepted ambiguous type namespace: {diagnostic}");
        assert!(diagnostic.contains("duplicate") && diagnostic.contains("Collision"),
            "{label}: expected duplicate type diagnosis, got {diagnostic}");
    }
}

#[test]
fn public_checker_refuses_proven_local_narrowing_and_unsigned_negative_record() {
    for (label, src) in [
        ("u8 from local range",
         "type Tiny = record { n: u8 }\ncell main() ->:\n    val n = 300\n    val t: Tiny = Tiny { n: n }\n    yield 7\n"),
        ("i8 from local range",
         "type Tiny = record { n: i8 }\ncell main() ->:\n    val n = 128\n    val t: Tiny = Tiny { n: n }\n    yield 7\n"),
        ("unsigned negative initializer",
         "type Tiny = record { n: u32 }\ncell main() ->:\n    val t: Tiny = Tiny { n: -1 }\n    yield 7\n"),
        ("unsigned negative mutation",
         "type Tiny = record { n: u32 = 7 }\ncell main() ->:\n    val mut t: Tiny = Tiny {}\n    t.n = -1\n    yield 7\n"),
    ] {
        let fixture=SourceFile::new(src);
        let result=fixture.run(&["check"]);
        let text=format!("{}\n{}",String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr));
        assert!(!result.status.success(),
            "{label}: actual public check accepted unsafe Record value: {text}");
        assert!(text.contains("out of range") || text.contains("negative"),
            "{label}: expected numeric Record diagnostic: {text}");
    }
}

#[test]
fn public_aura_check_refuses_typed_integer_local_range_laundering() {
    for (label, source) in [
        ("typed literal u8", "type Tiny = record { n: u8 }\ncell main() ->:\n    val n: u8 = 300\n    yield Tiny { n: n }\n"),
        ("typed source copy u8", "type Tiny = record { n: u8 }\ncell main() ->:\n    val raw = 300\n    val n: u8 = raw\n    yield Tiny { n: n }\n"),
        ("typed literal i8", "type Signed = record { n: i8 }\ncell main() ->:\n    val n: i8 = 128\n    yield Signed { n: n }\n"),
        ("mutable typed local", "type Tiny = record { n: u8 }\ncell main() ->:\n    val mut n: u8 = 7\n    n = 300\n    yield Tiny { n: n }\n"),
    ] {
        let fixture=SourceFile::new(source);
        let res=fixture.run(&["check"]);
        let diagnostic=format!("{}\n{}",String::from_utf8_lossy(&res.stdout),
            String::from_utf8_lossy(&res.stderr));
        assert!(!res.status.success(),
            "{label}: real CLI accepted unsafe typed local narrowing: {diagnostic}");
        assert!(diagnostic.contains("out of range"),
            "{label}: expected numeric range failure: {diagnostic}");
    }
}

#[test]
fn public_cli_accepts_valid_typed_integer_local_record_check_and_c_build() {
    let source=concat!(
        "type Tiny = record { n: u8 }\n",
        "cell main() ->:\n",
        "    val raw = 255\n",
        "    val n: u8 = raw\n",
        "    val t: Tiny = Tiny { n: n }\n",
        "    yield t.n\n"
    );
    let fixture=SourceFile::new(source);
    for args in [
        &["check"][..],
        &["build","--mode","hybrid","--backend","c","--profile","dev"][..]
    ] {
        let result=fixture.run(args);
        let output=format!("{}\n{}",String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr));
        assert!(result.status.success(),
            "public CLI {:?} must preserve valid typed Record scalar value: {output}",args);
    }
}
