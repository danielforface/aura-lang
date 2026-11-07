//! Q03-001 Record function ABI: source-level Record return, by-value parameter,
 //! cross-cell call, and native strict C11 O0/O2 executable cross-check.
 //! This test was introduced while the C backend explicitly rejected Record ABI.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{run_oracle_entry, validate_module, InstKind, OracleConfig, OracleValue, Type};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

const SOURCE: &str = r#"
type Point = record { x: u32, y: u32 }
cell make_point(x: u32, y: u32) ->:
    yield Point { y: y, x: x }
cell project_x(p: Point) ->:
    yield p.x
cell forward_point(p: Point) ->:
    yield project_x(p)
cell identity_point(p: Point) ->:
    yield p
cell branch_from_call(flag: bool) ->:
    val mut p: Point = make_point(7, 13)
    if flag:
        p.x = 19
    else:
        p.x = 23
    yield p.x
cell maybe_update_param(flag: bool, p: mut Point) ->:
    if flag:
        p.x = 19
    yield p.x
cell round_trip(x: u32, y: u32) ->:
    val p: Point = make_point(x, y)
    yield project_x(p)
cell named_make_point() ->:
    yield make_point(y: 13, x: 7)
cell named_record_call() ->:
    yield project_x(p: make_point(y: 13, x: 7))
cell sum_point(p: Point, increment: u32) ->:
    yield p.x + increment
cell named_record_and_scalar() ->:
    yield sum_point(increment: 5, p: make_point(y: 13, x: 7))
cell mixed_positional_named() ->:
    yield sum_point(make_point(y: 13, x: 7), increment: 5)
"#;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let ns = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("aura-record-abi-{}-{ns}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

#[test]
fn source_record_return_and_parameter_are_qualified_by_ir_oracle_and_native() {
    let ast = aura_parse::parse_source(SOURCE).expect("source Record functions parse");
    Checker::new().check_program(&ast).expect("source nominal Record call typechecks");
    let ir = lower_program(&ast).expect("source Record call lowers");
    validate_module(&ir).expect("Record function SSA structurally valid");
    for (x, y) in [(7,13), (19,23)] {
        let result = run_oracle_entry(&ir, "round_trip",
            &[OracleValue::U32(x), OracleValue::U32(y)],
            OracleConfig::default()).expect("Oracle executes Record by-value call");
        assert!(result.ok && result.stderr.is_empty());
        assert_eq!(result.return_value, Some(OracleValue::U32(x)));
        let mut fields = std::collections::HashMap::new();
        fields.insert("x".into(), OracleValue::U32(x));
        fields.insert("y".into(), OracleValue::U32(y));
        let point = OracleValue::Record { name: "Point".into(), fields };
        let forward = run_oracle_entry(&ir, "forward_point", &[point.clone()], OracleConfig::default())
            .expect("Oracle forwards nominal Record parameter by value");
        assert_eq!(forward.return_value, Some(OracleValue::U32(x)));
        let identity = run_oracle_entry(&ir, "identity_point", &[point.clone()], OracleConfig::default())
            .expect("Oracle returns nominal Record parameter by value");
        assert_eq!(identity.return_value, Some(point));
    }

    for (flag, expected) in [(true, 19), (false, 23)] {
        let value = run_oracle_entry(&ir, "branch_from_call",
            &[OracleValue::Bool(flag)], OracleConfig::default())
            .expect("Oracle must join Record returned from a function call");
        assert_eq!(value.return_value, Some(OracleValue::U32(expected)));
    }
    for (flag, expected) in [(true, 19), (false, 7)] {
        let mut fields = std::collections::HashMap::new();
        fields.insert("x".into(), OracleValue::U32(7));
        fields.insert("y".into(), OracleValue::U32(13));
        let point = OracleValue::Record { name: "Point".into(), fields };
        let value = run_oracle_entry(&ir, "maybe_update_param",
            &[OracleValue::Bool(flag), point], OracleConfig::default())
            .expect("Oracle must join a named Record parameter from the unchanged edge");
        assert_eq!(value.return_value, Some(OracleValue::U32(expected)));
    }
    for cell in ["named_make_point", "named_record_call"] {
        let result = run_oracle_entry(&ir, cell, &[], OracleConfig::default())
            .expect("named call must execute via IR Oracle");
        if cell == "named_make_point" {
            let mut expected_fields = std::collections::HashMap::new();
            expected_fields.insert("x".to_string(), OracleValue::U32(7));
            expected_fields.insert("y".to_string(), OracleValue::U32(13));
            assert_eq!(result.return_value, Some(OracleValue::Record {
                name: "Point".into(), fields: expected_fields,
            }), "named argument values must bind to their declared parameter names");
        } else {
            assert_eq!(result.return_value, Some(OracleValue::U32(7)),
                "nested named Record call must bind to declared parameter names");
        }
    }
    for cell in ["named_record_and_scalar", "mixed_positional_named"] {
        let result = run_oracle_entry(&ir, cell, &[], OracleConfig::default())
            .expect("Oracle must preserve named and positional C ABI slot order");
        assert_eq!(result.return_value, Some(OracleValue::U32(12)),
            "{cell}: Point x=7 plus increment 5");
    }
    let output = emit_module(&ir, None).expect("nominal Record function ABI should emit native C");
    assert!(output.module_c.contains("aura_record_506f696e74"),
        "qualified nominal Point type required, not Tensor fallback");
    let tmp = Scratch::new();
    fs::write(tmp.0.join("aura_runtime.h"), output.runtime_h).unwrap();
    fs::write(tmp.0.join("module.c"), &output.module_c).unwrap();
    // Independent C translation unit: exercise the real platform linker ABI
    // for by-value struct parameters and returns, not merely one-TU inlining.
    fs::write(tmp.0.join("driver.c"), concat!(
        "#include <stdint.h>\n#include <stdbool.h>\n",
        "typedef struct { uint32_t aura_f_78; uint32_t aura_f_79; } aura_record_506f696e74;\n",
        "aura_record_506f696e74 make_point(uint32_t, uint32_t);\n",
        "uint32_t project_x(aura_record_506f696e74);\n",
        "uint32_t forward_point(aura_record_506f696e74);\n",
        "aura_record_506f696e74 identity_point(aura_record_506f696e74);\n",
        "uint32_t branch_from_call(bool);\n",
        "uint32_t maybe_update_param(bool, aura_record_506f696e74);\n",
        "uint32_t round_trip(uint32_t, uint32_t);\n",
        "aura_record_506f696e74 named_make_point(void);\n",
        "uint32_t named_record_call(void);\n",
        "uint32_t sum_point(aura_record_506f696e74, uint32_t);\n",
        "uint32_t named_record_and_scalar(void);\n",
        "uint32_t mixed_positional_named(void);\n",
        "int main(void) {\n",
        "  aura_record_506f696e74 p = make_point(31u, 37u);\n",
        "  return (p.aura_f_78 == 31u && p.aura_f_79 == 37u",
        " && project_x(p) == 31u",
        " && forward_point(p) == 31u",
        " && identity_point(p).aura_f_78 == 31u",
        " && branch_from_call(true) == 19u",
        " && branch_from_call(false) == 23u",
        " && maybe_update_param(true,p) == 19u",
        " && maybe_update_param(false,p) == 31u",
        " && round_trip(7u, 13u) == 7u",
        " && round_trip(19u, 23u) == 19u",
        " && named_make_point().aura_f_78 == 7u",
        " && named_make_point().aura_f_79 == 13u",
        " && named_record_call() == 7u",
        " && named_record_and_scalar() == 12u",
        " && mixed_positional_named() == 12u) ? 0 : 1;\n",
        "}\n"
    )).unwrap();
    for opt in ["-O0", "-O2"] {
        let exe = tmp.0.join(format!("record_abi_{}", &opt[1..]));
        let build = Command::new("clang")
            .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
            .arg(&tmp.0).arg(tmp.0.join("module.c")).arg(tmp.0.join("driver.c"))
            .arg("-o").arg(&exe).output().expect("clang");
        assert!(build.status.success(), "{opt}: {}\n{}", String::from_utf8_lossy(&build.stderr), output.module_c);
        let run = Command::new(exe).output().expect("run native Record ABI");
        assert!(run.status.success(), "{opt}: {}\n{}", String::from_utf8_lossy(&run.stderr), output.module_c);
    }
}


fn twin_layout(ir: &mut aura_ir::ModuleIR) {
    ir.record_layouts.insert("Twin".into(), vec![
        ("x".into(), Type::U32), ("y".into(), Type::U32),
    ]);
}

#[test]
fn record_abi_refuses_nominally_distinct_but_structurally_identical_argument() {
    let ast = aura_parse::parse_source(SOURCE).unwrap();
    let mut ir = lower_program(&ast).unwrap();
    twin_layout(&mut ir);
    ir.functions.get_mut("project_x").unwrap().params[0].ty = Type::Opaque("Twin".into());
    validate_module(&ir).expect("structural IR valid; typing is C ABI responsibility");
    let err = emit_module(&ir, None).err().expect("nominal mismatch must reject C emission");
    assert!(err.message.contains("Record ABI call 'project_x'") &&
        err.message.contains("expects nominal 'Twin'"), "{err}");
}

#[test]
fn record_abi_refuses_scalar_argument_for_record_parameter() {
    let ast = aura_parse::parse_source(SOURCE).unwrap();
    let mut ir = lower_program(&ast).unwrap();
    let round = ir.functions.get_mut("round_trip").unwrap();
    let x_param = round.params[0].value;
    let call = round.blocks.iter_mut().flat_map(|b| b.insts.iter_mut())
        .find(|i| matches!(&i.kind, InstKind::Call { callee, .. } if callee == "project_x"))
        .expect("source cell contains call");
    let InstKind::Call { args, .. } = &mut call.kind else { unreachable!() };
    *args = vec![x_param];
    validate_module(&ir).expect("structural SSA valid before type rejection");
    let err = emit_module(&ir, None).err().expect("passing scalar as nominal Record must reject");
    assert!(err.message.contains("Record ABI call 'project_x'") &&
        err.message.contains("expects nominal 'Point'"), "{err}");
}

#[test]
fn record_abi_refuses_returning_the_wrong_nominal_record() {
    let ast = aura_parse::parse_source(SOURCE).unwrap();
    let mut ir = lower_program(&ast).unwrap();
    twin_layout(&mut ir);
    ir.functions.get_mut("make_point").unwrap().ret = Type::Opaque("Twin".into());
    validate_module(&ir).expect("structural SSA valid before type rejection");
    let err = emit_module(&ir, None).err().expect("returning Point as Twin must reject");
    assert!(err.message.contains("Record ABI return") &&
        err.message.contains("expects nominal 'Twin'"), "{err}");
}


#[test]
fn record_interpolation_preserves_avm_nominal_display_for_param_and_local() {
    // AVM interpolates a Record env binding as "<Point>". Native C previously
    // printed the literal "{p}" because only scalar names were available
    // to the interpolation emitter. Test the actual Aura source, nominal
    // function ABI and observable executable stdout, not just C text.
    let source = r#"
type Point = record { x: u32, y: u32 }
cell render_param(p: Point) ->:
    io.println("param={p}")
    yield p.x
cell render_local() ->:
    val p: Point = Point { x: 7, y: 13 }
    io.println("local={p}, missing={unbound}")
    yield render_param(p)
"#;
    let ast = aura_parse::parse_source(source).expect("Record print source parses");
    Checker::new().check_program(&ast).expect("source-qualified Record print checks");
    let ir = lower_program(&ast).expect("Record print lowers to real source IR");
    validate_module(&ir).expect("source Record interpolation IR validates");
    let output = emit_module(&ir, None).expect("Record interpolation emits native C");
    assert!(output.module_c.contains("local=<Point>"),
        "native C must interpolate Record local nominal identity: {}", output.module_c);
    assert!(output.module_c.contains("param=<Point>"),
        "native C must interpolate Record parameter nominal identity: {}", output.module_c);
    // SSA keeps the original ConstString template in generated C, even
    // when the executed printf interpolates it. The executable stdout
    // assertion below is the authoritative observable product contract.

    let tmp = Scratch::new();
    fs::write(tmp.0.join("aura_runtime.h"), &output.runtime_h).unwrap();
    fs::write(tmp.0.join("module.c"), &output.module_c).unwrap();
    fs::write(tmp.0.join("driver.c"), concat!(
        "#include <stdint.h>\n",
        "uint32_t render_local(void);\n",
        "int main(void) { return render_local() == 7u ? 0 : 1; }\n"
    )).unwrap();
    for opt in ["-O0", "-O2"] {
        let exe = tmp.0.join(format!("record_interpolation_{}", &opt[1..]));
        let cc = Command::new("clang")
            .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
            .arg(&tmp.0).arg(tmp.0.join("module.c"))
            .arg(tmp.0.join("driver.c")).arg("-o").arg(&exe)
            .output().expect("Clang available in Native CI");
        assert!(cc.status.success(), "{opt} C compilation failed: {}\n{}",
            String::from_utf8_lossy(&cc.stderr), output.module_c);
        let run = Command::new(&exe).output().expect("native Record executable runs");
        assert!(run.status.success(), "{opt} Record native call failed: {}",
            String::from_utf8_lossy(&run.stderr));
        assert_eq!(
            String::from_utf8_lossy(&run.stdout),
            "local=<Point>, missing={unbound}\nparam=<Point>\n",
            "{opt}: Native must match AVM nominal string interpolation");
    }
}


#[test]
fn record_and_mixed_width_scalar_interpolation_matches_avm_display() {
    // Product-level parity: AVM's interpolate_string renders every supported
    // integer as its own value. Previously Native left narrow/signed/u64
    // placeholders literally in the printed output even beside a Record.
    // Keep the independent C driver and the full Aura Parser→Checker→IR path.
    let source = r#"
type Point = record { x: u32, y: u32 }
cell render_mixed(p: Point, n8: u8, n16: u16, big: u64, m8: i8, m16: i16, m32: i32) ->:
    io.println("record={p}, u8={n8}, u16={n16}, u64={big}, i8={m8}, i16={m16}, i32={m32}, unknown={unbound}")
    yield p.x
"#;
    let ast = aura_parse::parse_source(source).expect("mixed Record/scalar source parses");
    Checker::new().check_program(&ast).expect("mixed Record/scalar print typechecks");
    let original = lower_program(&ast).expect("real source lowers to typed IR");
    validate_module(&original).expect("mixed Record/scalar IR validates");
    let mut optimized = original.clone();
    aura_ir::optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized IR validates");

    for (label, ir) in [("source", &original), ("optimized", &optimized)] {
        let output = emit_module(ir, None).expect("mixed-width native interpolation emits C");
        let tmp = Scratch::new();
        fs::write(tmp.0.join("aura_runtime.h"), &output.runtime_h).unwrap();
        fs::write(tmp.0.join("module.c"), &output.module_c).unwrap();
        fs::write(tmp.0.join("driver.c"), concat!(
            "#include <stdint.h>\n",
            "typedef struct { uint32_t aura_f_78; uint32_t aura_f_79; } aura_record_506f696e74;\n",
            "uint32_t render_mixed(aura_record_506f696e74, uint8_t, uint16_t, uint64_t, int8_t, int16_t, int32_t);\n",
            "int main(void) {\n",
            "  aura_record_506f696e74 p = {7u,13u};\n",
            "  return render_mixed(p,250u,65000u,4294967301ULL,-7,-2048,-100000)==7u ? 0 : 1;\n",
            "}\n"
        )).unwrap();
        for opt in ["-O0", "-O2"] {
            let exe = tmp.0.join(format!("mixed_integer_interpolation_{label}_{}", &opt[1..]));
            let built = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
                .arg(&tmp.0).arg(tmp.0.join("module.c"))
                .arg(tmp.0.join("driver.c")).arg("-o").arg(&exe)
                .output().expect("Clang installed in Native CI");
            assert!(built.status.success(),
                "{label}/{opt}: generated C does not compile: {}\n{}",
                String::from_utf8_lossy(&built.stderr), output.module_c);
            let ran = Command::new(&exe).output().expect("linked native program executes");
            assert!(ran.status.success(),
                "{label}/{opt}: wrong Record ABI return: {}",
                String::from_utf8_lossy(&ran.stderr));
            assert_eq!(
                String::from_utf8_lossy(&ran.stdout),
                "record=<Point>, u8=250, u16=65000, u64=4294967301, i8=-7, i16=-2048, i32=-100000, unknown={unbound}\n",
                "{label}/{opt}: native integer interpolation differs from AVM value display"
            );
        }
    }
}


#[test]
fn stored_interpolated_strings_freeze_source_bindings_and_nested_aliases() {
    // AVM evaluates StringLit on creation. The old C emitter waited until
    // println, so names declared later altered an already-created String,
    // and nested String aliases still contained raw source braces.
    let source = r#"
type Point = record { x: u32, y: u32 }
cell render_saved(p: Point, number: u32) ->:
    val first: String = "point={p}, value={number}, percent=100%"
    val alias: String = first
    val nested: String = "nested=[{alias}]"
    val missing: String = "later={future}"
    val future: u32 = 91
    io.println(first)
    io.println(alias)
    io.println(nested)
    io.println(missing)
    io.println("live={future}")
    yield p.x
cell render_record_only(p: Point) ->:
    io.println("record-only={p}")
    yield 0
"#;
    let ast = aura_parse::parse_source(source).expect("stored String source parses");
    Checker::new().check_program(&ast).expect("source with Record and String checks");
    let original = lower_program(&ast).expect("real source lowers");
    validate_module(&original).expect("source IR validates");
    let mut optimized = original.clone();
    aura_ir::optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized source IR validates");

    for (label, ir) in [("source", &original), ("optimized", &optimized)] {
        let output = emit_module(ir, None).expect("frozen String source emits Native C");
        let tmp = Scratch::new();
        fs::write(tmp.0.join("aura_runtime.h"), &output.runtime_h).unwrap();
        fs::write(tmp.0.join("module.c"), &output.module_c).unwrap();
        fs::write(tmp.0.join("driver.c"), concat!(
            "#include <stdint.h>\n",
            "typedef struct { uint32_t aura_f_78; uint32_t aura_f_79; } aura_record_506f696e74;\n",
            "uint32_t render_saved(aura_record_506f696e74, uint32_t);\n",
            "uint32_t render_record_only(aura_record_506f696e74);\n",
            "int main(void) { aura_record_506f696e74 p={7u,13u};\n",
            "return (render_saved(p,31u)==7u && render_record_only(p)==0u) ? 0 : 1; }\n"
        )).unwrap();
        // The public C runtime normally supplies aura_io_println.
        // A separate standalone native-ABI test must link that symbol
        // explicitly when a String has no resolved interpolation slots.
        // Keep the unresolved/forward-binding behavior under test rather
        // than deleting it just to make the generated C link.
        fs::write(tmp.0.join("stdlib.c"),
            "#include <stdio.h>\nvoid aura_io_println(const char* s) { puts(s); }\n"
        ).unwrap();
        for opt in ["-O0", "-O2"] {
            let exe = tmp.0.join(format!("frozen_string_{label}_{}", &opt[1..]));
            let cc = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
                .arg(&tmp.0).arg(tmp.0.join("module.c"))
                .arg(tmp.0.join("stdlib.c")).arg(tmp.0.join("driver.c"))
                .arg("-o").arg(&exe)
                .output().expect("Clang available in Native CI");
            assert!(cc.status.success(), "{label}/{opt}: {}\n{}",
                String::from_utf8_lossy(&cc.stderr), output.module_c);
            let run = Command::new(&exe).output().expect("linked Native C executes");
            assert!(run.status.success(), "{label}/{opt}: wrong return: {}",
                String::from_utf8_lossy(&run.stderr));
            assert_eq!(String::from_utf8_lossy(&run.stdout), concat!(
                "point=<Point>, value=31, percent=100%\n",
                "point=<Point>, value=31, percent=100%\n",
                "nested=[point=<Point>, value=31, percent=100%]\n",
                "later={future}\n",
                "live=91\n",
                "record-only=<Point>\n",
            ), "{label}/{opt}: snapshot must be evaluated before later declarations");
        }
    }
}

#[test]
fn interpolated_string_return_and_user_cell_escape_fail_closed() {
    // This native C ABI still passes raw const char* for String. Direct
    // println is qualified, but returning or passing a captured String to
    // another Cell would expose the raw unformatted literal. Do not claim
    // an interprocedural ownership/representation guarantee.
    for (case, source) in [
        ("Return", r#"
cell escape_saved(number: u32) ->:
    val saved: String = "value={number}"
    yield saved
"#),
        ("Call", r#"
cell accept_string(s: String) ->:
    io.println(s)
    yield 1
cell pass_saved(number: u32) ->:
    val saved: String = "value={number}"
    yield accept_string(saved)
"#),
    ] {
        let ast = aura_parse::parse_source(source).expect("escape source parses");
        Checker::new().check_program(&ast).expect("escape source passes Checker");
        let original = lower_program(&ast).expect("escape source lowers");
        validate_module(&original).expect("escape source IR structurally validates");
        let mut optimized = original.clone();
        aura_ir::optimize_module(&mut optimized);
        validate_module(&optimized).expect("optimized escape source IR validates");
        for (label, ir) in [("source", &original), ("optimized", &optimized)] {
            let err = emit_module(ir, None).err().expect(
                "raw C string pointer must not silently represent captured interpolation"
            );
            assert!(err.message.contains("unqualified interpolated String escape")
                && err.message.contains(case),
                "{case}/{label}: wrong unsupported ABI diagnostic: {err}");
        }
    }
}


#[test]
fn native_unicode_char_interpolation_uses_exact_utf8_and_rejects_invalid_scalars() {
    // Q03-001 real Aura source: the AVM displays its Char value as the
    // Unicode scalar, not as a truncated 8-bit printf character or its
    // decimal codepoint. Exercise NUL, percent, ASCII, all UTF-8 lengths,
    // surrogate edges and supplementary-plane limits under strict native C.
    let source = r#"
type Point = record { x: u32, y: u32 }
cell aura_unicode_char_lead(n: u32) ->:
    yield n
cell validate_reserved_name() ->:
    yield aura_unicode_char_lead(37)
cell render_unicode(p: Point, scalar: char, marker: char) ->:
    val stored: String = "Record={p}; scalar={scalar}; marker={marker}; repeated={scalar}"
    io.println(stored)
    yield p.x
"#;
    let ast = aura_parse::parse_source(source).expect("Unicode char source parses");
    Checker::new().check_program(&ast).expect("Unicode char and nominal Record typecheck");
    let original = lower_program(&ast).expect("real source lowers into typed SSA");
    validate_module(&original).expect("source IR validates");
    let mut optimized = original.clone();
    aura_ir::optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized SSA validates");

    // Include NUL byte (which printf %s alone cannot emit), percent (which
    // may not become a format directive), multi-byte and edge scalars.
    let valid = [
        0u32, 0x25, 0x7F, 0x80, 0x7FF, 0x800, 0x05D0,
        0xD7FF, 0xE000, 0xFFFF, 0x10000, 0x1F642, 0x10FFFF,
    ];
    let invalid = [0xD800u32, 0xDFFF, 0x110000, 0xFFFF_FFFF];

    for (label, ir) in [("source", &original), ("optimized", &optimized)] {
        let c = emit_module(ir, None).expect("Unicode char source emits native C");
        assert!(c.runtime_h.contains("aura_unicode_scalar_checked"),
            "{label}: Unicode scalar guard is missing");
        assert!(c.module_c.contains("aura_unicode_char_tail"),
            "{label}: native interpolation must encode UTF-8");
        // A user's Cell that happens to collide with a new runtime helper
        // must be C-mangled, while its original Aura Call still works.
        assert!(c.module_c.contains("aura_cell_617572615f756e69636f64655f636861725f6c656164"),
            "{label}: reserved helper export collides with user Cell");
        let tmp = Scratch::new();
        fs::write(tmp.0.join("aura_runtime.h"), &c.runtime_h).unwrap();
        fs::write(tmp.0.join("module.c"), &c.module_c).unwrap();
        fs::write(tmp.0.join("driver.c"), concat!(
            "#include <stdint.h>\n",
            "#include <stdlib.h>\n",
            "typedef struct { uint32_t aura_f_78; uint32_t aura_f_79; } aura_record_506f696e74;\n",
            "uint32_t render_unicode(aura_record_506f696e74, uint32_t, uint32_t);\n",
            "uint32_t validate_reserved_name(void);\n",
            "int main(int argc, char** argv) {\n",
            " if (argc != 2 || validate_reserved_name() != 37u) return 2;\n",
            " uint32_t scalar = (uint32_t)strtoul(argv[1], NULL, 0);\n",
            " aura_record_506f696e74 p = {7u,13u};\n",
            " return render_unicode(p, scalar, 0x25u) == 7u ? 0 : 3;\n",
            "}\n"
        )).unwrap();

        for opt in ["-O0", "-O2"] {
            let exe = tmp.0.join(format!("record_unicode_char_{label}_{}", &opt[1..]));
            let built = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
                .arg(&tmp.0).arg(tmp.0.join("module.c"))
                .arg(tmp.0.join("driver.c")).arg("-o").arg(&exe)
                .output().expect("Clang installed for Native CI");
            assert!(built.status.success(),
                "{label}/{opt}: C source or helper ABI failed: {}\n{}",
                String::from_utf8_lossy(&built.stderr), c.module_c);
            for scalar in valid {
                let run = Command::new(&exe).arg(format!("0x{scalar:X}"))
                    .output().expect("native Unicode executable runs");
                assert!(run.status.success(),
                    "{label}/{opt} U+{scalar:04X} failed: {}",
                    String::from_utf8_lossy(&run.stderr));
                let ch = char::from_u32(scalar).expect("test valid Unicode scalar");
                let want = format!(
                    "Record=<Point>; scalar={ch}; marker=%; repeated={ch}\n"
                );
                assert_eq!(run.stdout, want.as_bytes(),
                    "{label}/{opt}: native UTF-8 differs from char display U+{scalar:04X}");
            }
            for scalar in invalid {
                let run = Command::new(&exe).arg(format!("0x{scalar:X}"))
                    .output().expect("native invalid-char executable runs");
                assert!(!run.status.success(),
                    "{label}/{opt}: invalid scalar U+{scalar:08X} accepted");
                assert!(String::from_utf8_lossy(&run.stderr)
                    .contains("invalid Aura char Unicode scalar"),
                    "{label}/{opt}: invalid scalar did not fail closed: {}",
                    String::from_utf8_lossy(&run.stderr));
            }
        }
    }
}


#[test]
fn native_float_interpolation_matches_avm_ryu_like_decimal_display() {
    // AVM display_value(Float) uses Rust f64::to_string; the former C
    // printf("%f") changed 1.5 -> 1.500000, 0.1 -> 0.100000, and could
    // lose all nonzero digits of tiny values. Test actual Aura source,
    // nominal Record, two separately formatted f64 arguments and repeated
    // use of the same source binding against Rust's Display oracle.
    let source=r#"
type Point = record { x: u32, y: u32 }
cell aura_f64_display(n: u32) ->:
    yield n
cell render_float(p: Point, sample: f64, twin: f64) ->:
    val stored: String = "Record={p}; sample={sample}; twin={twin}; again={sample}; percent=100%"
    io.println(stored)
    yield aura_f64_display(p.x)
"#;
    let ast=aura_parse::parse_source(source).expect("Float+Record Aura source parses");
    Checker::new().check_program(&ast).expect("Float+Record source typechecks");
    let original=lower_program(&ast).expect("real source lowers to typed IR");
    validate_module(&original).expect("source SSA is valid");
    let mut optimized=original.clone();
    aura_ir::optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized SSA is valid");
    let cases: [f64; 23]=[
        0.0, -0.0, 0.1, -0.1, 1.5, -1.5, 1.0/3.0, 1e-40,
        -1e-40, 1e40, 1e-8, 1e20, 9007199254740992.0,
        std::f64::consts::PI, std::f64::consts::E,
        f64::MIN_POSITIVE, f64::from_bits(1), f64::from_bits(2),
        f64::MAX, f64::MIN, f64::EPSILON,
        f64::INFINITY, f64::NEG_INFINITY,
    ];
    // NaN has explicit normalized case too; Rust produces "NaN".
    let mut samples=cases.to_vec();
    samples.push(f64::NAN);
    for (label, ir) in [("source", &original), ("optimized", &optimized)] {
        let c=emit_module(ir, None).expect("Float source emits C");
        assert!(c.runtime_h.contains("aura_f64_display"),
            "{label}: no Float display implementation");
        assert!(c.module_c.contains("aura_cell_617572615f6636345f646973706c6179"),
            "{label}: runtime helper collided with user Cell export");
        let tmp=Scratch::new();
        fs::write(tmp.0.join("aura_runtime.h"),&c.runtime_h).unwrap();
        fs::write(tmp.0.join("module.c"),&c.module_c).unwrap();
        fs::write(tmp.0.join("driver.c"),concat!(
            "#include <stdint.h>\n",
            "#include <stdlib.h>\n",
            "typedef struct { uint32_t aura_f_78; uint32_t aura_f_79; } aura_record_506f696e74;\n",
            "uint32_t render_float(aura_record_506f696e74, double, double);\n",
            "int main(int argc, char** argv) {\n",
            " if (argc != 3) return 2;\n",
            " char *end = NULL;\n",
            " double a = strtod(argv[1], &end); if (*end) return 3;\n",
            " double b = strtod(argv[2], &end); if (*end) return 4;\n",
            " aura_record_506f696e74 p={7u,13u};\n",
            " return render_float(p,a,b)==7u ? 0 : 5;\n",
            "}\n"
        )).unwrap();
        for opt in ["-O0","-O2"] {
            let exe=tmp.0.join(format!("float_display_{label}_{}",&opt[1..]));
            let build=Command::new("clang")
                .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
                .arg(&tmp.0).arg(tmp.0.join("module.c"))
                .arg(tmp.0.join("driver.c")).arg("-o").arg(&exe)
                .output().expect("Clang available in CI");
            assert!(build.status.success(),"{label}/{opt} C build: {}\n{}",
                String::from_utf8_lossy(&build.stderr),c.module_c);
            for sample in &samples {
                let run=Command::new(&exe)
                    .arg(sample.to_string())
                    .arg("0.1")
                    .output().expect("Native C executable runs");
                assert!(run.status.success(),"{label}/{opt}: float {} native return: {}",
                    sample,String::from_utf8_lossy(&run.stderr));
                let expect=format!(
                    "Record=<Point>; sample={}; twin=0.1; again={}; percent=100%\n",
                    sample,sample
                );
                assert_eq!(run.stdout,expect.as_bytes(),
                    "{label}/{opt}: Native F64 String interpolation differs from AVM's f64::to_string for {}",
                    sample);
            }
        }
    }
}


#[test]
fn record_string_fields_support_construct_get_copy_call_and_return() {
    // Supported source contract: String is a valid Record field and can be
    // constructed, copied, projected and passed/returned via ordinary Cells
    // without changing the nominal Record layout or source type system.
    // Mutating a non-scalar field remains explicitly rejected by Checker.
    let source=r#"
type Tagged = record { title: String, code: u32 }
cell make_tag(title: String) ->:
    yield Tagged { title: title, code: 7 }
cell title_of(item: Tagged) ->:
    yield item.title
cell show_tag(item: Tagged) ->:
    io.println(item.title)
    yield item.code
cell demo(title: String) ->:
    val tagged: Tagged = make_tag(title)
    val alias: Tagged = tagged
    io.println(tagged.title)
    io.println(alias.title)
    io.println(title_of(alias))
    yield show_tag(tagged)
"#;
    let ast=aura_parse::parse_source(source).expect("Record String source parses");
    Checker::new().check_program(&ast).expect("Record String fields typecheck");
    let original=lower_program(&ast).expect("real source Record String lowers");
    validate_module(&original).expect("source Record String IR valid");
    let mut optimized=original.clone();
    aura_ir::optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized Record String IR valid");

    for (label,ir) in [("source",&original),("optimized",&optimized)] {
        let c=emit_module(ir,None).expect("String Record fields emit native C");
        assert!(c.module_c.contains("const char* aura_f_7469746c65;"),
            "{label}: Record title String must retain C pointer layout: {}",c.module_c);
        let tmp=Scratch::new();
        fs::write(tmp.0.join("aura_runtime.h"),&c.runtime_h).unwrap();
        fs::write(tmp.0.join("module.c"),&c.module_c).unwrap();
        fs::write(tmp.0.join("stdlib.c"),
            "#include <stdio.h>\nvoid aura_io_println(const char* text){puts(text);}\n").unwrap();
        fs::write(tmp.0.join("driver.c"),concat!(
            "#include <stdint.h>\n",
            "typedef struct { const char* aura_f_7469746c65; uint32_t aura_f_636f6465; } aura_record_546167676564;\n",
            "uint32_t demo(const char*);\n",
            "int main(void){return demo(\"origin\")==7u?0:1;}\n"
        )).unwrap();
        for opt in ["-O0","-O2"] {
            let exe=tmp.0.join(format!("record_string_{label}_{}",&opt[1..]));
            let build=Command::new("clang")
                .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
                .arg(&tmp.0).arg(tmp.0.join("module.c"))
                .arg(tmp.0.join("stdlib.c")).arg(tmp.0.join("driver.c"))
                .arg("-o").arg(&exe).output().expect("Clang in Native CI");
            assert!(build.status.success(),
                "{label}/{opt}: Record String source failed Clang: {}\n{}",
                String::from_utf8_lossy(&build.stderr),c.module_c);
            let run=Command::new(&exe).output().expect("native String Record executable");
            assert!(run.status.success(),"{label}/{opt}: Record String return/Cell ABI failure: {}",
                String::from_utf8_lossy(&run.stderr));
            assert_eq!(String::from_utf8_lossy(&run.stdout),
                "origin\norigin\norigin\norigin\n",
                "{label}/{opt}: source String Record construct/copy/get/call semantics changed");
        }
    }
}

#[test]
fn record_string_fields_reject_unevaluated_interpolation_and_unqualified_mutation() {
    // The Checker intentionally rejects assignment to non-scalar fields.
    // That existing source-level safety boundary is NOT widened in this
    // additive C backend qualification, pending cross-agent semantic review.
    let invalid_set=r#"
type Tagged = record { title: String, code: u32 }
cell invalid_set(previous: Tagged, n: u32) ->:
    val mut next: Tagged = previous
    next.title = "value={n}"
    yield next.code
"#;
    let ast=aura_parse::parse_source(invalid_set).expect("String field update parses");
    let err=Checker::new().check_program(&ast).err()
        .expect("non-scalar Record field assignment is intentionally unqualified");
    assert!(err.message.contains("unqualified non-scalar Record field assignment")
        && err.message.contains("next.title"),
        "source Checker must fail closed on unsupported String field update: {err:?}");

    // Constructing with an interpolated String is valid source, but C
    // currently represents the source snapshot as an *unevaluated* static
    // template until its qualified local io.println consumer. A Record can
    // outlive that local context: raw pointer/brace escape must fail closed.
    let source=r#"
type Tagged = record { title: String, code: u32 }
cell invalid_construct(n: u32) ->:
    val t: Tagged = Tagged { title: "value={n}", code: 7 }
    yield t.code
"#;
    let ast=aura_parse::parse_source(source).expect("String Record capture source parses");
    Checker::new().check_program(&ast)
        .expect("interpolated String is a valid source Record constructor operand");
    let original=lower_program(&ast).expect("source String capture lowers");
    validate_module(&original).expect("source capture IR structurally valid");
    let mut optimized=original.clone();
    aura_ir::optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized capture IR validates");
    for (label,ir) in [("source",&original),("optimized",&optimized)] {
        let err=emit_module(ir,None).err()
            .expect("unevaluated String snapshot must not enter native Record");
        assert!(err.message.contains("unqualified interpolated String Record")
            && err.message.contains("title"),
            "{label}: missing explicit captured String Record ABI diagnostic: {err}");
    }
}


#[test]
fn signed_record_arithmetic_wraps_consistently_with_ir_oracle_and_ubsan() {
    // AURA-Q03-001 / Q03-006: a real source Record projects signed
    // fields into SSA arithmetic. The IR Oracle has wrapping I64 Add/Sub/Mul
    // and Neg; signed C overflow is UB and can disagree under Clang -O2.
    // Exercise externally supplied boundary values (not constant folding),
    // original+optimized Aura IR, both C optimization levels and UBSan.
    let source = r#"
type Signed64 = record { value: i64 }
type Signed32 = record { value: i32 }
cell add64(p: Signed64, delta: i64) ->:
    yield p.value + delta
cell sub64(p: Signed64, delta: i64) ->:
    yield p.value - delta
cell mul64(p: Signed64, delta: i64) ->:
    yield p.value * delta
cell neg64(p: Signed64) ->:
    yield -p.value
cell add32(p: Signed32, delta: i32) ->:
    yield p.value + delta
cell sub32(p: Signed32, delta: i32) ->:
    yield p.value - delta
cell mul32(p: Signed32, delta: i32) ->:
    yield p.value * delta
cell neg32(p: Signed32) ->:
    yield -p.value
cell aura_i64_from_wrapped_bits(n: i64) ->:
    yield n
cell aura_i32_from_wrapped_bits(n: i32) ->:
    yield n
cell probe_reserved64(n: i64) ->:
    yield aura_i64_from_wrapped_bits(n)
cell probe_reserved32(n: i32) ->:
    yield aura_i32_from_wrapped_bits(n)
"#;
    let ast = aura_parse::parse_source(source).expect("signed Record Aura source parses");
    Checker::new().check_program(&ast).expect("signed Record source typechecks");
    let original = lower_program(&ast).expect("signed Record source lowers to real IR");
    validate_module(&original).expect("source signed Record SSA validates");
    let mut optimized = original.clone();
    aura_ir::optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized signed Record SSA validates");

    let pairs64: &[(i64, i64)] = &[
        (0, 0), (1, -1), (-1, -1), (i64::MAX, 1),
        (i64::MIN, -1), (i64::MIN, 1),
        (i64::MAX, i64::MAX), (i64::MIN, i64::MIN),
        (i64::MIN, 0), (i64::MAX, -1),
        (-4_294_967_303, 3_000_000_023),
        (123_456_789_012_345_678, -9_223_372_036_854_775),
    ];
    let pairs32: &[(i32, i32)] = &[
        (0, 0), (1, -1), (-1, -1), (i32::MAX, 1),
        (i32::MIN, -1), (i32::MIN, 1),
        (i32::MAX, i32::MAX), (i32::MIN, i32::MIN),
        (i32::MIN, 0), (i32::MAX, -1),
        (-2_000_000_000, 2_000_000_000),
    ];

    let check64 = |ir: &aura_ir::ModuleIR, op: &str, x: i64, y: i64, expected: i64| {
        let mut fields = std::collections::HashMap::new();
        fields.insert("value".to_string(), OracleValue::I64(x));
        let record = OracleValue::Record { name: "Signed64".into(), fields };
        let mut args = vec![record];
        if op != "neg64" { args.push(OracleValue::I64(y)); }
        let result = run_oracle_entry(ir, op, &args, OracleConfig::default())
            .expect("Oracle accepts real signed Record and computes arithmetic");
        assert!(result.ok, "{op} Oracle failed on {x}/{y}: {result:?}");
        assert_eq!(result.return_value, Some(OracleValue::I64(expected)),
            "{op}: signed Record IR Oracle wrapping behavior changed");
    };
    // Standalone driver deliberately receives runtime inputs from argv so
    // neither the C optimizer nor the Rust IR optimizer can fold the
    // overflowing arithmetic into an unobservable constant.
    let driver = r#"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef struct { int64_t aura_f_76616c7565; } aura_record_5369676e65643634;
typedef struct { int32_t aura_f_76616c7565; } aura_record_5369676e65643332;
int64_t add64(aura_record_5369676e65643634, int64_t);
int64_t sub64(aura_record_5369676e65643634, int64_t);
int64_t mul64(aura_record_5369676e65643634, int64_t);
int64_t neg64(aura_record_5369676e65643634);
int32_t add32(aura_record_5369676e65643332, int32_t);
int32_t sub32(aura_record_5369676e65643332, int32_t);
int32_t mul32(aura_record_5369676e65643332, int32_t);
int32_t neg32(aura_record_5369676e65643332);
int64_t probe_reserved64(int64_t);
int32_t probe_reserved32(int32_t);
int main(int argc, char** argv) {
  if (argc != 4) return 2;
  const char* op = argv[1];
  const int64_t x = (int64_t)strtoll(argv[2], NULL, 10);
  const int64_t y = (int64_t)strtoll(argv[3], NULL, 10);
  const aura_record_5369676e65643634 wide = { x };
  const aura_record_5369676e65643332 narrow = { (int32_t)x };
  long long result = 0;
  if (!strcmp(op, "add64")) result = (long long)add64(wide,y);
  else if (!strcmp(op, "sub64")) result = (long long)sub64(wide,y);
  else if (!strcmp(op, "mul64")) result = (long long)mul64(wide,y);
  else if (!strcmp(op, "neg64")) result = (long long)neg64(wide);
  else if (!strcmp(op, "add32")) result = (long long)add32(narrow,(int32_t)y);
  else if (!strcmp(op, "sub32")) result = (long long)sub32(narrow,(int32_t)y);
  else if (!strcmp(op, "mul32")) result = (long long)mul32(narrow,(int32_t)y);
  else if (!strcmp(op, "neg32")) result = (long long)neg32(narrow);
  else if (!strcmp(op, "probe_reserved64")) result = (long long)probe_reserved64(x);
  else if (!strcmp(op, "probe_reserved32")) result = (long long)probe_reserved32((int32_t)x);
  else return 3;
  printf("%lld\n", result);
  return 0;
}
"#;
    for (label, ir) in [("source", &original), ("optimized", &optimized)] {
        let c = emit_module(ir, None).expect("signed Record emits defined native C");
        assert!(c.runtime_h.contains("aura_i64_from_wrapped_bits")
            && c.runtime_h.contains("aura_i32_from_wrapped_bits"),
            "{label}: wrapped integer conversion helpers must be present");
        assert!(c.module_c.contains("aura_cell_617572615f6936345f66726f6d5f777261707065645f62697473"),
            "{label}: user Cell colliding with i64 runtime helper must be renamed");
        assert!(c.module_c.contains("aura_cell_617572615f6933325f66726f6d5f777261707065645f62697473"),
            "{label}: user Cell colliding with i32 runtime helper must be renamed");
        let tmp = Scratch::new();
        fs::write(tmp.0.join("aura_runtime.h"), &c.runtime_h).unwrap();
        fs::write(tmp.0.join("module.c"), &c.module_c).unwrap();
        fs::write(tmp.0.join("driver.c"), driver).unwrap();
        for opt in ["-O0", "-O2"] {
            let exe = tmp.0.join(format!("signed_record_{label}_{}", &opt[1..]));
            let build = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror",
                       "-fsanitize=undefined", "-fno-sanitize-recover=all", opt, "-I"])
                .arg(&tmp.0).arg(tmp.0.join("module.c"))
                .arg(tmp.0.join("driver.c")).arg("-o").arg(&exe)
                .output().expect("Clang UBSan is available in Native CI");
            assert!(build.status.success(), "{label}/{opt}: strict C/UBSan failed:\n{}\n{}",
                String::from_utf8_lossy(&build.stderr), c.module_c);
            let run = |op: &str, x: i64, y: i64, expected: i64| {
                let value = Command::new(&exe).arg(op).arg(x.to_string())
                    .arg(y.to_string()).output().expect("Native Clang executable runs");
                assert!(value.status.success(), "{label}/{opt} {op}({x},{y}): undefined signed arithmetic or nonzero result:\n{}",
                    String::from_utf8_lossy(&value.stderr));
                assert_eq!(String::from_utf8_lossy(&value.stdout),
                    format!("{expected}\n"), "{label}/{opt} {op}({x},{y}) differs from modular arithmetic");
            };
            for &(x,y) in pairs64 {
                for (op, expected) in [
                    ("add64", x.wrapping_add(y)),
                    ("sub64", x.wrapping_sub(y)),
                    ("mul64", x.wrapping_mul(y)),
                    ("neg64", x.wrapping_neg()),
                ] {
                    check64(ir, op, x, y, expected);
                    run(op,x,y,expected);
                }
            }
            for &(x,y) in pairs32 {
                for (op, expected) in [
                    ("add32", x.wrapping_add(y)),
                    ("sub32", x.wrapping_sub(y)),
                    ("mul32", x.wrapping_mul(y)),
                    ("neg32", x.wrapping_neg()),
                ] {
                    run(op,x as i64,y as i64,expected as i64);
                }
            }
            for &n in &[0i64, i64::MIN, i64::MAX] {
                run("probe_reserved64",n,0,n);
            }
            for &n in &[0i32, i32::MIN, i32::MAX] {
                run("probe_reserved32",n as i64,0,n as i64);
            }
        }
    }
}
