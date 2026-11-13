#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap};

use aura_ir::{
    BinOp, BlockId, DebugSource, FunctionIR, InstKind, ModuleIR, RValue, Terminator, Type, UnaryOp,
    ValueId,
};
use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("C backend error: {message}")]
#[diagnostic(code(aura::backend_c))]
#[allow(unused_assignments)]
pub struct CBackendError {
    pub message: String,
}

pub struct CArtifacts {
    pub runtime_h: String,
    pub module_c: String,
}

/// Produce C only after checking the source-declared ABI against independently
/// re-lowered, trusted Aura source. Unlike the IR-only API, this detects a
/// forged ModuleIR whose refined metadata and RangeCheck are both rewritten.
/// The supplied AST must be the original trusted source, not parsed from a
/// mutable IR payload. This is a *source-refinement ABI* check, not an
/// authenticated signature or proof of arbitrary IR instruction provenance.
pub fn emit_module_with_source_contract(
    original_source: &aura_ast::Program,
    module: &ModuleIR,
    debug: Option<&DebugSource>,
) -> Result<CArtifacts, CBackendError> {
    let trusted = aura_core::lower_program(original_source).map_err(|e| CBackendError {
        message: format!("trusted source requalification failed: {e}"),
    })?;
    if module.refined_return_bounds != trusted.refined_return_bounds {
        return Err(CBackendError {
            message: format!("refined source return contract mismatch: supplied={:?}; trusted={:?}",
                module.refined_return_bounds, trusted.refined_return_bounds),
        });
    }
    // No missing, added or modified per-parameter source interval may pass.
    // All supplied ModuleIR metadata is untrusted until this comparison.
    if module.refined_param_bounds != trusted.refined_param_bounds {
        return Err(CBackendError {
            message: format!(
                "refined source contract mismatch: supplied={:?}; trusted={:?}",
                module.refined_param_bounds, trusted.refined_param_bounds
            ),
        });
    }
    // Prevent a nominally matching source interval from being paired with
    // an unrelated or rewritten exported signature.
    if module.functions.len() != trusted.functions.len() ||
        module.functions.keys().ne(trusted.functions.keys()) ||
        module.record_layouts != trusted.record_layouts ||
        module.externs != trusted.externs
    {
        return Err(CBackendError {
            message: "trusted source contract mismatch: function, extern or Record ABI layout".into(),
        });
    }
    for (name, supplied) in &module.functions {
        let expected = trusted.functions.get(name).expect("checked function set");
        if supplied.ret != expected.ret ||
            supplied.params.len() != expected.params.len() ||
            supplied.params.iter().zip(expected.params.iter())
                .any(|(a,b)| a.name != b.name || a.ty != b.ty)
        {
            return Err(CBackendError {
                message: format!("trusted source function ABI contract mismatch for '{name}'"),
            });
        }
    }
    emit_module(module, debug)
}

/// Emit an already-trusted IR module. This lower-level API has no original
/// source anchor and CANNOT authenticate jointly rewritten refinement metadata
/// plus guards. Public C/WASM builds use emit_module_with_source_contract.
pub fn emit_module(module: &ModuleIR, debug: Option<&DebugSource>) -> Result<CArtifacts, CBackendError> {
    // Reject malformed SSA *before* optimization. A CFG simplifier may remove
    // stale Phi inputs as an optimization of a VALID graph; it must never
    // launder a forged incoming predecessor in an already-invalid module.
    aura_ir::validate_module(module).map_err(|e| CBackendError {
        message: format!("IR validation failed before C optimization: {e}"),
    })?;

    let mut module = module.clone();

    // The current IR keeps string interpolation as a ConstString template, so
    // `{ident}` references are not explicit SSA dependency edges yet. Running
    // DCE on such a function can remove bindings that the C emitter still needs
    // to reproduce AVM interpolation semantics. Keep those functions intact
    // until interpolation dependencies become first-class IR; optimize all
    // other functions normally.
    for function in module.functions.values_mut() {
        if !function_has_string_interpolation(function) {
            aura_ir::optimize_function(function);
        }
    }

    if let Err(e) = aura_ir::validate_module(&module) {
        return Err(CBackendError {
            message: format!("IR validation failed before C emission: {e}"),
        });
    }

    let runtime_h = emit_runtime_h();
    let module_c = emit_module_c(&module, debug)?;
    Ok(CArtifacts { runtime_h, module_c })
}

fn emit_runtime_h() -> String {
    // A tiny runtime that is:
    // - portable (falls back when C11 threads are unavailable)
    // - runnable (stub IO + compute)
    let mut out = String::new();
    out.push_str("#pragma once\n");
    out.push_str("#include <stdint.h>\n");
    out.push_str("#include <stdbool.h>\n");
    out.push_str("#include <stddef.h>\n");
    out.push_str("#include <inttypes.h>\n");
    out.push_str("#include <stdio.h>\n");
    out.push_str("#include <stdlib.h>\n\n");

    out.push_str("// ---- Standard 128-bit Types ----\n");
    out.push_str("#if defined(__SIZEOF_INT128__)\n");
    out.push_str("typedef unsigned __int128 aura_u128_t;\n");
    out.push_str("typedef __int128 aura_i128_t;\n");
    out.push_str("#else\n");
    out.push_str("typedef struct { uint64_t lo; uint64_t hi; } aura_u128_t;\n");
    out.push_str("typedef struct { uint64_t lo; int64_t hi; } aura_i128_t;\n");
    out.push_str("#endif\n\n");

    // From aura-stdlib (linked by the CLI when running the C backend).
    out.push_str("void aura_io_println(const char* s);\n\n");

    // The qualified native String ABI is a non-null NUL-terminated UTF-8
    // pointer. Pointer identity is NOT Aura value equality.
    out.push_str(r#"
static inline bool aura_string_content_equal(const char* left, const char* right) {
  if (!left || !right) {
    fputs("Aura String comparison rejected: null native String\n", stderr);
    fflush(stderr);
    abort();
  }
  const unsigned char* a = (const unsigned char*)left;
  const unsigned char* b = (const unsigned char*)right;
  while (*a && *b) {
    if (*a != *b) return false;
    ++a;
    ++b;
  }
  return *a == *b;
}

"#);


    out.push_str("// ---- Aura runtime (prototype) ----\n\n");
    out.push_str("typedef struct Tensor { uint32_t id; } Tensor;\n\n");

    out.push_str("static inline void AURA_RANGE_CHECK_U32(uint32_t v, uint32_t lo, uint32_t hi, const char* what) {\n");
    out.push_str("  if (v < lo || v > hi) {\n");
    out.push_str("    fprintf(stderr, \"Aura range check failed for %s: %u not in [%u..%u]\\n\", what, v, lo, hi);\n");
    out.push_str("    // Fail-fast for prototype runtime\n");
    out.push_str("    fflush(stderr);\n");
    out.push_str("    abort();\n");
    out.push_str("  }\n");
    out.push_str("}\n\n");
    // A wider entry guard must not truncate 64-bit arguments into U32 before
    // checking the declared interval (especially for an external C caller).
    out.push_str("static inline void AURA_RANGE_CHECK_U64(uint64_t v, uint64_t lo, uint64_t hi, const char* what) {\n");
    out.push_str("  if (v < lo || v > hi) {\n");
    out.push_str("    fprintf(stderr, \"Aura range check failed for %s: %llu not in [%llu..%llu]\\n\", what, (unsigned long long)v, (unsigned long long)lo, (unsigned long long)hi);\n");
    out.push_str("    fflush(stderr);\n");
    out.push_str("    abort();\n");
    out.push_str("  }\n");
    out.push_str("}\n\n");

    // Unsigned binary64 division is a supported typed-Record numeric ABI.
    // Unlike U32, checking a divisor by narrowing to uint32_t would reject
    // valid high-bit inputs or accidentally divide by zero. Always retain
    // the full uint64_t width and reject zero before the C division.
    out.push_str(r#"
static inline uint64_t aura_u64_div_checked(uint64_t numerator, uint64_t divisor) {
  if (divisor == UINT64_C(0)) {
    fputs("Aura u64 division rejected: zero divisor\n", stderr);
    fflush(stderr);
    abort();
  }
  return numerator / divisor;
}

"#);

    // Native signed i64 division is qualified at the already-typed IR/Record
    // ABI boundary. Unlike Add/Sub/Mul, signed division cannot be reduced
    // modulo 2^64: both division by zero and INT64_MIN / -1 are UB in C.
    // The IR Oracle refuses a zero divisor and Rust rejects the overflow.
    // Reject both at runtime before any C division (including under -O2).
    out.push_str(r#"
static inline int64_t aura_i64_div_checked(int64_t numerator, int64_t divisor) {
  if (divisor == 0 || (numerator == INT64_MIN && divisor == -1)) {
    fprintf(stderr, "Aura i64 division rejected: zero divisor or signed overflow\n");
    fflush(stderr);
    abort();
  }
  return numerator / divisor;
}

"#);

    // The IR Oracle's signed i64 arithmetic is modular, but native C
    // signed-overflow is undefined behavior (including negating INT*_MIN).
    // Convert signed operands to unsigned first; unsigned Add/Sub/Mul
    // has defined modulo-2^N semantics. Convert the final bit pattern
    // back using only representable signed casts and signed subtraction,
    // so neither Clang -O2 nor UBSan depends on implementation-defined
    // out-of-range unsigned->signed conversions.
    out.push_str(r#"
static inline int32_t aura_i32_from_wrapped_bits(uint32_t bits) {
  if (bits <= (uint32_t)INT32_MAX) return (int32_t)bits;
  return -1 - (int32_t)(UINT32_MAX - bits);
}
static inline int64_t aura_i64_from_wrapped_bits(uint64_t bits) {
  if (bits <= (uint64_t)INT64_MAX) return (int64_t)bits;
  return -1 - (int64_t)(UINT64_MAX - bits);
}
"#);

    // Aura char is a Unicode scalar, not a byte. The public C char ABI
    // stores it in uint32_t; printf's %c alone would truncate non-ASCII,
    // and %s alone cannot preserve U+0000. Encode one lead byte with %c
    // plus an optional null-terminated UTF-8 tail with %s. C99/C11 block
    // compound literals provide invocation-lifetime storage for each tail.
    out.push_str(r#"
// Converts only Unicode scalar values. Invalid foreign C callers fail
// closed rather than printing ill-formed UTF-8 or normalizing surrogates.
static inline uint32_t aura_unicode_scalar_checked(uint32_t scalar) {
  if (scalar > 0x10FFFFu || (scalar >= 0xD800u && scalar <= 0xDFFFu)) {
    fprintf(stderr, "invalid Aura char Unicode scalar U+%08X\n", (unsigned)scalar);
    fflush(stderr);
    abort();
  }
  return scalar;
}

static inline int aura_unicode_char_lead(uint32_t scalar) {
  scalar = aura_unicode_scalar_checked(scalar);
  if (scalar <= 0x7Fu) return (int)scalar;
  if (scalar <= 0x7FFu) return (int)(0xC0u | (scalar >> 6));
  if (scalar <= 0xFFFFu) return (int)(0xE0u | (scalar >> 12));
  return (int)(0xF0u | (scalar >> 18));
}

static inline const char* aura_unicode_char_tail(uint32_t scalar, char tail[4]) {
  scalar = aura_unicode_scalar_checked(scalar);
  if (scalar <= 0x7Fu) {
    tail[0] = '\0';
  } else if (scalar <= 0x7FFu) {
    tail[0] = (char)(0x80u | (scalar & 0x3Fu));
    tail[1] = '\0';
  } else if (scalar <= 0xFFFFu) {
    tail[0] = (char)(0x80u | ((scalar >> 6) & 0x3Fu));
    tail[1] = (char)(0x80u | (scalar & 0x3Fu));
    tail[2] = '\0';
  } else {
    tail[0] = (char)(0x80u | ((scalar >> 12) & 0x3Fu));
    tail[1] = (char)(0x80u | ((scalar >> 6) & 0x3Fu));
    tail[2] = (char)(0x80u | (scalar & 0x3Fu));
    tail[3] = '\0';
  }
  return tail;
}

"#);


    // Reconstruct an exact IEEE-754 binary64 from the source IR's bit
    // pattern. A decimal C literal is not a lossless code-generation ABI
    // for signed zero, NaN payloads, or the subnormal edge under -Werror
    // (-Wliteral-range). Use the ordinary C aliasing guarantee for
    // unsigned char to copy every byte, without platform headers or
    // implementation-specific builtins.
    out.push_str(r#"
static inline double aura_f64_from_bits(uint64_t bits) {
  _Static_assert(sizeof(double) == sizeof(uint64_t),
                 "Aura f64 requires a 64-bit native double");
#if defined(__DBL_MANT_DIG__) && defined(__DBL_MAX_EXP__)
  _Static_assert(__DBL_MANT_DIG__ == 53 && __DBL_MAX_EXP__ == 1024,
                 "Aura f64 requires IEEE-754 binary64");
#endif
  double value;
  unsigned char* dst = (unsigned char*)&value;
  const unsigned char* src = (const unsigned char*)&bits;
  for (size_t i = 0; i < sizeof(value); ++i) dst[i] = src[i];
  return value;
}

"#);

    // f64 interpolation must agree with AVM's Rust Display, not printf("%f")
    // (six decimals, loss of precision and massive-number truncation).
    // The shortest round-trippable decimal is expanded to fixed notation,
    // matching Rust's plain-decimal Display for finite f64. Each printf
    // argument receives its own temporary buffer (no global static slot).
    // GNU11 compiler builtins avoid importing an entire math.h identifier
    // namespace into user Cell exports.
    out.push_str(r#"
static inline const char* aura_f64_display(double value, char scratch[400]) {
  if (__builtin_isnan(value)) return "NaN";
  if (__builtin_isinf(value)) return __builtin_signbit(value) ? "-inf" : "inf";
  if (value == 0.0) return __builtin_signbit(value) ? "-0" : "0";

  char generated[64] = {0};
  int roundtrip = 0;
  for (int precision = 1; precision <= 17; ++precision) {
    int size = snprintf(generated, sizeof(generated), "%.*g", precision, value);
    if (size < 0 || (size_t)size >= sizeof(generated)) abort();
    char* end = NULL;
    double parsed = strtod(generated, &end);
    if (end && *end == '\0' && parsed == value) {
      roundtrip = 1;
      break;
    }
  }
  if (!roundtrip) abort();

  const char* e = generated;
  while (*e && *e != 'e' && *e != 'E') ++e;
  if (*e == '\0') {
    int n = snprintf(scratch, 400, "%s", generated);
    if (n < 0 || n >= 400) abort();
    return scratch;
  }

  int exponent = (int)strtol(e + 1, NULL, 10);
  const char* src = generated;
  int negative = 0;
  if (*src == '-') { negative = 1; ++src; }
  char digits[32];
  int digit_count = 0;
  int decimal_offset = -1;
  while (src < e) {
    if (*src == '.') {
      decimal_offset = digit_count;
    } else if (*src >= '0' && *src <= '9') {
      if (digit_count >= (int)sizeof(digits)) abort();
      digits[digit_count++] = *src;
    } else {
      abort();
    }
    ++src;
  }
  if (decimal_offset < 0) decimal_offset = digit_count;
  int point = decimal_offset + exponent;
  size_t out = 0;
  if (negative) scratch[out++] = '-';
#define AURA_F64_PUT(byte) do { \
    if (out + 1 >= 400) abort(); \
    scratch[out++] = (byte); \
  } while (0)
  if (point <= 0) {
    AURA_F64_PUT('0');
    AURA_F64_PUT('.');
    for (int i = 0; i < -point; ++i) AURA_F64_PUT('0');
    for (int i = 0; i < digit_count; ++i) AURA_F64_PUT(digits[i]);
  } else if (point >= digit_count) {
    for (int i = 0; i < digit_count; ++i) AURA_F64_PUT(digits[i]);
    for (int i = digit_count; i < point; ++i) AURA_F64_PUT('0');
  } else {
    for (int i = 0; i < point; ++i) AURA_F64_PUT(digits[i]);
    AURA_F64_PUT('.');
    for (int i = point; i < digit_count; ++i) AURA_F64_PUT(digits[i]);
  }
  scratch[out] = '\0';
#undef AURA_F64_PUT
  return scratch;
}

"#);
    out.push_str("// Builtins used by main.aura\n");
    out.push_str("static inline Tensor io_load_tensor(const char* path) {\n");
    out.push_str("  (void)path;\n");
    out.push_str("  Tensor t; t.id = 42;\n");
    out.push_str("  return t;\n");
    out.push_str("}\n\n");

    out.push_str("static inline void io_display(Tensor t) {\n");
    out.push_str("  printf(\"Tensor{id=%u}\\n\", t.id);\n");
    out.push_str("}\n\n");

    out.push_str("static inline Tensor compute_gradient(Tensor data, uint32_t weight) {\n");
    out.push_str("  Tensor out;\n");
    out.push_str("  out.id = data.id + weight;\n");
    out.push_str("  return out;\n");
    out.push_str("}\n\n");

    // Async execution wrappers.
    out.push_str("// ~> async execution (prototype)\n");
    out.push_str("#if defined(__STDC_NO_THREADS__)\n");
    out.push_str("  #define aura_async_tensor2(fn, a0, a1) (fn((a0), (a1)))\n");
    out.push_str("#else\n");
    out.push_str("  #include <threads.h>\n");
    out.push_str("  typedef struct aura_task_tensor2 {\n");
    out.push_str("    Tensor (*fn)(Tensor, uint32_t);\n");
    out.push_str("    Tensor a0;\n");
    out.push_str("    uint32_t a1;\n");
    out.push_str("    Tensor out;\n");
    out.push_str("  } aura_task_tensor2;\n");
    out.push_str("  static int aura_task_tensor2_entry(void* p) {\n");
    out.push_str("    aura_task_tensor2* t = (aura_task_tensor2*)p;\n");
    out.push_str("    t->out = t->fn(t->a0, t->a1);\n");
    out.push_str("    return 0;\n");
    out.push_str("  }\n");
    out.push_str("  static inline Tensor aura_async_tensor2(Tensor (*fn)(Tensor, uint32_t), Tensor a0, uint32_t a1) {\n");
    out.push_str("    aura_task_tensor2 task;\n");
    out.push_str("    task.fn = fn; task.a0 = a0; task.a1 = a1;\n");
    out.push_str("    thrd_t thr;\n");
    out.push_str("    thrd_create(&thr, aura_task_tensor2_entry, &task);\n");
    out.push_str("    thrd_join(thr, 0);\n");
    out.push_str("    return task.out;\n");
    out.push_str("  }\n");
    out.push_str("#endif\n\n");

    out
}

// Encode user-facing names into collision-free, C-safe identifiers without
// changing Record field ordering or accidentally using C reserved keywords.
fn encoded_record_name(s: &str) -> String {
    s.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect::<String>()
}
fn record_type_symbol(s: &str) -> String {
    format!("aura_record_{}", encoded_record_name(s))
}
fn record_field_symbol(s: &str) -> String {
    format!("aura_f_{}", encoded_record_name(s))
}
fn record_field_ctype(ty: &Type) -> Option<CType> {
    match ty {
        Type::Bool => Some(CType::Bool),
        Type::Char => Some(CType::Char),
        Type::U8 => Some(CType::U8),
        Type::U16 => Some(CType::U16),
        Type::U32 => Some(CType::U32),
        Type::U64 => Some(CType::U64),
        Type::I8 => Some(CType::I8),
        Type::I16 => Some(CType::I16),
        Type::I32 => Some(CType::I32),
        Type::I64 => Some(CType::I64),
        Type::F32 => Some(CType::F32),
        Type::F64 => Some(CType::F64),
        // A concrete owned Aura String is represented at the existing
        // public C boundary as a non-owning const char*. By-value Records
        // preserve that pointer exactly, including String fields nested
        // inside other qualified Records. Any interpolated String still
        // requiring capture materialization must be rejected at the
        // ConstructRecord/SetField write boundary below.
        Type::String => Some(CType::CString),
        _ => None,
    }
}
// A Record field receives a scalar of its exact C type, or a narrower
// integer converted from a statically proven U32 constant. Unknown runtime
// values and computed expressions are never narrowed based on C's implicit
// conversion rules. This is a proof local to IR SSA constant/copy provenance.
fn qualified_record_scalar_value(
    expected: CType,
    actual: CType,
    value: ValueId,
    cname: &str,
    known_u32_constants: &HashMap<ValueId, u64>,
) -> Option<String> {
    if expected == actual {
        return Some(cname.to_string());
    }
    if actual != CType::U32 {
        return None;
    }
    let max = match expected {
        CType::U8 => u8::MAX as u64,
        CType::U16 => u16::MAX as u64,
        CType::U64 => u64::MAX,
        CType::I8 => i8::MAX as u64,
        CType::I16 => i16::MAX as u64,
        CType::I32 => i32::MAX as u64,
        CType::I64 => i64::MAX as u64,
        _ => return None,
    };
    match known_u32_constants.get(&value) {
        Some(n) if *n <= u32::MAX as u64 && *n <= max =>
            Some(format!("({})({})", map_ctype_decl(expected), cname)),
        _ => None,
    }
}
// Each qualified nominal field retains its own type identity; a nested
// Record is NEVER reduced to a scalar width or the generic Tensor ABI.
#[derive(Clone, Debug, PartialEq, Eq)]
enum RecordFieldCType {
    Scalar(CType),
    Nominal(String),
}
impl RecordFieldCType {
    fn declaration(&self) -> String {
        match self {
            Self::Scalar(ct) => map_ctype_decl(*ct).to_string(),
            Self::Nominal(name) => record_type_symbol(name),
        }
    }
}
fn qualified_record_fields(
    layouts: &BTreeMap<String, Vec<(String, Type)>>,
    record: &str,
) -> Result<Vec<(String, RecordFieldCType)>, CBackendError> {
    fn qualify(
        layouts: &BTreeMap<String, Vec<(String, Type)>>,
        record: &str,
        stack: &mut Vec<String>,
    ) -> Result<Vec<(String, RecordFieldCType)>, CBackendError> {
        if stack.iter().any(|entry| entry == record) {
            return Err(CBackendError {
                message: format!("unqualified C Record by-value layout cycle: {} -> {record}",
                    stack.join(" -> ")),
            });
        }
        let fields = layouts.get(record).ok_or_else(|| CBackendError {
            message: format!("unqualified C Record '{}' missing typed field layout", record),
        })?;
        if fields.is_empty() {
            return Err(CBackendError {
                message: format!("unqualified C Record '{}' empty layout not supported", record),
            });
        }
        stack.push(record.to_string());
        let mut result: Vec<(String, RecordFieldCType)> = Vec::with_capacity(fields.len());
        for (name, ty) in fields {
            if result.iter().any(|(existing, _)| existing == name) {
                return Err(CBackendError { message:
                    format!("unqualified C Record '{}' duplicate layout field '{}'", record, name) });
            }
            let field_type = if let Some(ct) = record_field_ctype(ty) {
                RecordFieldCType::Scalar(ct)
            } else if let Type::Opaque(nested) = ty {
                if !layouts.contains_key(nested) {
                    return Err(CBackendError { message: format!(
                        "unqualified C Record '{}' field '{}' type {:?} unsupported",
                        record, name, ty,
                    ) });
                }
                qualify(layouts, nested, stack)?;
                RecordFieldCType::Nominal(nested.clone())
            } else if let Type::Record { name: nested, fields: actual_fields } = ty {
                if layouts.get(nested) != Some(actual_fields) {
                    return Err(CBackendError { message: format!(
                        "unqualified C Record '{}' field '{}' nested layout mismatch '{}'",
                        record, name, nested,
                    ) });
                }
                qualify(layouts, nested, stack)?;
                RecordFieldCType::Nominal(nested.clone())
            } else {
                return Err(CBackendError { message: format!(
                    "unqualified C Record '{}' field '{}' type {:?} unsupported",
                    record, name, ty,
                ) });
            };
            result.push((name.clone(), field_type));
        }
        stack.pop();
        Ok(result)
    }
    qualify(layouts, record, &mut Vec::new())
}

// Semantic record returns can recursively carry expanded Type::Record
// fields while declaration layouts contain Type::Opaque nominal references.
fn matching_record_layout_type(
    declared: &Type, actual: &Type,
    layouts: &BTreeMap<String, Vec<(String, Type)>>,
) -> bool {
    if declared == actual { return true; }
    match (declared, actual) {
        (Type::Opaque(expected), Type::Record { name, fields }) if expected == name => {
            layouts.get(name).is_some_and(|layout|
                layout.len() == fields.len() &&
                layout.iter().zip(fields).all(|((a, at), (b, bt))|
                    a == b && matching_record_layout_type(at, bt, layouts)))
        }
        _ => false,
    }
}
fn emit_qualified_record_layout(
    out: &mut String,
    layouts: &BTreeMap<String, Vec<(String, Type)>>,
    name: &str,
    emitted: &mut BTreeSet<String>,
) -> Result<(), CBackendError> {
    if emitted.contains(name) { return Ok(()); }
    let fields = qualified_record_fields(layouts, name)?;
    // A by-value nested C struct must be complete before the parent typedef.
    for (_, ty) in &fields {
        if let RecordFieldCType::Nominal(child) = ty {
            emit_qualified_record_layout(out, layouts, child, emitted)?;
        }
    }
    out.push_str("typedef struct {\n");
    for (field, ty) in &fields {
        out.push_str(&format!("  {} {};\n",
            ty.declaration(), record_field_symbol(field)));
    }
    out.push_str(&format!("}} {};\n\n", record_type_symbol(name)));
    emitted.insert(name.to_string());
    Ok(())
}

fn qualified_record_abi_type(
    ty: &Type,
    layouts: &BTreeMap<String, Vec<(String, Type)>>,
) -> Result<Option<String>, CBackendError> {
    match ty {
        Type::Record { name, fields } => {
            let layout = layouts.get(name).ok_or_else(|| CBackendError {
                message: format!("unqualified Record function ABI missing layout for '{name}'"),
            })?;
            qualified_record_fields(layouts, name)?;
            if layout.len() != fields.len() ||
               !layout.iter().zip(fields).all(|((a, at), (b, bt))|
                   a == b && matching_record_layout_type(at, bt, layouts)) {
                return Err(CBackendError {
                    message: format!("unqualified Record function ABI layout mismatch for '{name}'"),
                });
            }
            Ok(Some(name.clone()))
        }
        Type::Opaque(name) if layouts.contains_key(name) => {
            qualified_record_fields(layouts, name)?;
            Ok(Some(name.clone()))
        }
        _ => Ok(None),
    }
}

// The backend has several distinct C namespaces: exported Cells, formal
// parameters, SSA temporaries, runtime symbols and nominal Record typedefs.
// Map by source symbol identity, never by lossy punctuation substitution.
// Existing C-safe public Cell names remain unchanged for ABI compatibility.
struct CNameMap {
    exports: BTreeMap<String, String>,
    formals: BTreeMap<String, Vec<String>>,
    // Immutable snapshot of names pre-occupied by the generated C runtime
    // and included headers, before local Cell symbols join the namespace.
    // A foreign ABI symbol must never be silently rewritten or re-bound.
    reserved_extern_symbols: BTreeSet<String>,
}

fn c_safe_export_ident(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else { return false };
    // All leading underscores are reserved for external identifiers in C.
    if !first.is_ascii_alphabetic()
        || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return false;
    }
    !matches!(name,
        "auto" | "break" | "case" | "char" | "const" | "continue" |
        "default" | "do" | "double" | "else" | "enum" | "extern" |
        "float" | "for" | "goto" | "if" | "inline" | "int" | "long" |
        "register" | "restrict" | "return" | "short" | "signed" |
        "sizeof" | "static" | "struct" | "switch" | "typedef" |
        "union" | "unsigned" | "void" | "volatile" | "while" |
        // gnu11 extensions and macro names introduced by included headers.
        "asm" | "typeof" | "bool" | "true" | "false" | "NULL" |
        "alignof" | "alignas" | "static_assert" | "thread_local")
}

impl CNameMap {
    fn new(module: &ModuleIR) -> Self {
        // Reserved by emitted scaffolding, runtime ABI or included C headers.
        let mut used: BTreeSet<String> = [
            "main", "aura_main", "aura_io_println", "aura_async_tensor2",
            "aura_task_tensor2", "aura_task_tensor2_entry",
            "aura_u128_t", "aura_i128_t", "Tensor",
            "uint8_t", "uint16_t", "uint32_t", "uint64_t",
            "int8_t", "int16_t", "int32_t", "int64_t",
            "size_t", "ptrdiff_t", "intptr_t", "uintptr_t",
            "io_load_tensor", "io_display", "compute_gradient",
            "printf", "fprintf", "puts", "malloc", "calloc", "realloc",
            "free", "abort", "exit", "fwrite",
            "AURA_RANGE_CHECK_U32", "AURA_RANGE_CHECK_U64",
            "aura_unicode_scalar_checked", "aura_unicode_char_lead",
            "aura_unicode_char_tail", "aura_f64_display",
            "aura_f64_from_bits",
            "aura_i32_from_wrapped_bits", "aura_i64_from_wrapped_bits",
            "aura_i64_div_checked", "aura_u64_div_checked", "aura_string_content_equal",
            // Names and object-like macros from the C11 headers that
            // emit_runtime_h() actually includes. These occupy the same C
            // ordinary-identifier namespace as user Cell exports, or are
            // preprocessor macros that would rewrite those exports.
            // Keep this policy explicit: accidental overlap with libc is
            // not a public Aura ABI contract and must not be emitted raw.
            //
            // <stdio.h>: public functions, stream objects/macros, typedefs
            // and constant-like macros.
            "FILE", "fpos_t", "stdin", "stdout", "stderr", "BUFSIZ",
            "EOF", "FILENAME_MAX", "FOPEN_MAX", "L_tmpnam",
            "SEEK_CUR", "SEEK_END", "SEEK_SET", "TMP_MAX",
            "_IOFBF", "_IOLBF", "_IONBF",
            "clearerr", "fclose", "feof", "ferror", "fflush",
            "fgetc", "fgetpos", "fgets", "fopen", "freopen",
            "fputc", "fputs", "fread", "fscanf", "fseek",
            "fsetpos", "ftell", "getc", "getchar", "perror",
            "putc", "putchar", "remove", "rename", "rewind",
            "scanf", "setbuf", "setvbuf", "snprintf", "sprintf",
            "sscanf", "tmpfile", "tmpnam", "ungetc", "vfprintf",
            "vfscanf", "vprintf", "vscanf", "vsnprintf",
            "vsprintf", "vsscanf",
            // <stdlib.h>: public functions, typedefs and macros.
            "EXIT_SUCCESS", "EXIT_FAILURE", "RAND_MAX", "MB_CUR_MAX",
            "div_t", "ldiv_t", "lldiv_t", "atof", "atoi",
            "atol", "atoll", "strtod", "strtof", "strtold",
            "strtol", "strtoll", "strtoul", "strtoull", "rand",
            "srand", "aligned_alloc", "atexit", "at_quick_exit",
            "quick_exit", "_Exit", "getenv", "system", "bsearch",
            "qsort", "abs", "labs", "llabs", "div", "ldiv",
            "lldiv", "mblen", "mbtowc", "wctomb", "mbstowcs",
            "wcstombs",
            // <threads.h>: C11 threading names from the runtime's
            // conditional asynchronous code path.
            "thrd_t", "thrd_create", "thrd_current", "thrd_detach",
            "thrd_equal", "thrd_exit", "thrd_join", "thrd_sleep",
            "thrd_yield", "thrd_success", "thrd_timedout",
            "thrd_busy", "thrd_error", "mtx_t", "mtx_init",
            "mtx_destroy", "mtx_lock", "mtx_trylock",
            "mtx_timedlock", "mtx_unlock", "mtx_plain",
            "mtx_recursive", "mtx_timed", "cnd_t", "cnd_init",
            "cnd_destroy", "cnd_signal", "cnd_broadcast",
            "cnd_wait", "cnd_timedwait", "tss_t", "tss_create",
            "tss_delete", "tss_get", "tss_set", "tss_dtor_t",
            "TSS_DTOR_ITERATIONS", "once_flag", "ONCE_FLAG_INIT",
            "call_once",
            // <stddef.h>/<stdint.h>/<inttypes.h> common exposed types
            // and C library declarations; additional header macro
            // families remain outside this explicitly tested subset.
            "wchar_t", "max_align_t", "intmax_t", "uintmax_t",
            "int_fast8_t", "int_fast16_t", "int_fast32_t",
            "int_fast64_t", "uint_fast8_t", "uint_fast16_t",
            "uint_fast32_t", "uint_fast64_t", "int_least8_t",
            "int_least16_t", "int_least32_t", "int_least64_t",
            "uint_least8_t", "uint_least16_t", "uint_least32_t",
            "uint_least64_t", "imaxabs", "imaxdiv", "strtoimax",
            "strtoumax", "wcstoimax", "wcstoumax", "imaxdiv_t",
            "INT8_MAX", "INT16_MAX", "INT32_MAX", "INT64_MAX",
            "INT64_MIN",
            "UINT8_MAX", "UINT16_MAX", "UINT32_MAX", "UINT64_MAX",
        ].into_iter().map(str::to_owned).collect();
        let reserved_extern_symbols = used.clone();
        for record in module.record_layouts.keys() {
            used.insert(record_type_symbol(record));
        }

        let mut exports = BTreeMap::new();
        if module.functions.contains_key("main") {
            exports.insert("main".into(), "aura_main".into());
        }
        // Keep pre-existing, valid external ABI symbols first. Every other
        // Cell (including C keywords and colliding dotted names) is assigned
        // an injective hex-encoded C symbol from the remaining namespace.
        for source in module.functions.keys().filter(|s| s.as_str() != "main") {
            if c_safe_export_ident(source) && used.insert(source.clone()) {
                exports.insert(source.clone(), source.clone());
            }
        }
        for source in module.functions.keys() {
            if exports.contains_key(source) { continue; }
            let stem = format!("aura_cell_{}", encoded_record_name(source));
            let mut candidate = stem.clone();
            let mut suffix = 0usize;
            while !used.insert(candidate.clone()) {
                suffix += 1;
                candidate = format!("{stem}_{suffix}");
            }
            exports.insert(source.clone(), candidate);
        }

        let mut formals = BTreeMap::new();
        for (source, function) in &module.functions {
            let mut occupied = used.clone();
            // Retain every safe, existing formal spelling, not just the
            // public function ABI. This preserves established generated-C
            // consumers and diagnostics (e.g. `return input;`).
            // Reserve all safe names *before* allocating replacements, so a
            // mangled earlier argument cannot capture a later named formal.
            let mut args = vec![None; function.params.len()];
            for (index, param) in function.params.iter().enumerate() {
                let ssa_collision = param.name.strip_prefix('v')
                    .is_some_and(|digits| !digits.is_empty()
                        && digits.bytes().all(|b| b.is_ascii_digit()));
                if c_safe_export_ident(&param.name) && !ssa_collision
                    && occupied.insert(param.name.clone())
                {
                    args[index] = Some(param.name.clone());
                }
            }
            for (index, name) in args.iter_mut().enumerate() {
                if name.is_some() { continue; }
                let stem = format!("aura_param_{index}");
                let mut candidate = stem.clone();
                let mut suffix = 0usize;
                while !occupied.insert(candidate.clone()) {
                    suffix += 1;
                    candidate = format!("{stem}_{suffix}");
                }
                *name = Some(candidate);
            }
            formals.insert(source.clone(), args.into_iter().map(|n|
                n.expect("each formal receives a unique C name")).collect());
        }
        Self { exports, formals, reserved_extern_symbols }
    }

    fn symbol(&self, source: &str) -> &str {
        self.exports.get(source).map(String::as_str)
            .expect("emitted user Cell must have C mapping")
    }

    fn formal(&self, source: &str, index: usize) -> &str {
        self.formals.get(source).and_then(|v| v.get(index))
            .map(String::as_str)
            .expect("emitted formal must have C mapping")
    }

    fn callee(&self, source: &str) -> String {
        self.exports.get(source).cloned().unwrap_or_else(|| map_callee(source))
    }
}

fn emit_function_signature(
    out: &mut String,
    f: &FunctionIR,
    layouts: &BTreeMap<String, Vec<(String, Type)>>,
    name: &str,
    param_names: &[String],
) -> Result<(), CBackendError> {
    let returns_value = f.blocks.iter().any(|b| matches!(b.term, Terminator::Return(Some(_))));
    if returns_value {
        if let Some(record) = qualified_record_abi_type(&f.ret, layouts)? {
            out.push_str(&record_type_symbol(&record));
        } else {
            out.push_str(map_ctype_decl(function_return_ctype(f)));
        }
    } else {
        out.push_str("void");
    }
    out.push(' ');
    out.push_str(name);
    out.push('(');
    if f.params.is_empty() {
        out.push_str("void");
    } else {
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            if let Some(record) = qualified_record_abi_type(&p.ty, layouts)? {
                out.push_str(&record_type_symbol(&record));
            } else {
                out.push_str(map_type(&p.ty));
            }
            out.push(' ');
            out.push_str(&param_names[i]);
        }
    }
    out.push(')');
    Ok(())
}

// The emitted runtime always includes <stdint.h> and <inttypes.h>.
// Reject their documented object-like limit constants and PRI*/SCN*
// format macro families as *extern C function* names. These macros occupy
// the preprocessor namespace, so a type-correct source extern can still
// expand to a literal at the C declaration/call site. Keep this explicit,
// deterministic and independent of whichever host compiler runs the emitter.
fn is_reserved_runtime_header_macro(name: &str) -> bool {
    if matches!(name,
        "SIZE_MAX" | "PTRDIFF_MIN" | "PTRDIFF_MAX"
        | "SIG_ATOMIC_MIN" | "SIG_ATOMIC_MAX"
        | "WCHAR_MIN" | "WCHAR_MAX" | "WINT_MIN" | "WINT_MAX"
        | "INTPTR_MIN" | "INTPTR_MAX" | "UINTPTR_MAX"
        | "INTMAX_MIN" | "INTMAX_MAX" | "UINTMAX_MAX"
    ) {
        return true;
    }
    const FAMILIES: &[&str] = &[
        "PRId", "PRIi", "PRIo", "PRIu", "PRIx", "PRIX",
        "SCNd", "SCNi", "SCNo", "SCNu", "SCNx",
    ];
    const SUFFIXES: &[&str] = &[
        "8", "16", "32", "64",
        "LEAST8", "LEAST16", "LEAST32", "LEAST64",
        "FAST8", "FAST16", "FAST32", "FAST64",
        "MAX", "PTR",
    ];
    FAMILIES.iter().any(|family| {
        name.strip_prefix(family)
            .is_some_and(|suffix| SUFFIXES.contains(&suffix))
    })
}

// An Aura extern keeps the target's existing native-linker name. Unlike
// locally defined Cells it cannot be hex-renamed without silently changing
// the external library ABI. The legacy map_callee() flattens qualified names,
// so distinct source symbols can coincide in the C linker namespace.
//
// Validate the entire declared extern namespace before emitting *any* C,
// including for unused declarations. Otherwise optimization, call ordering,
// or whether a caller is present could change the meaning of a public ABI.
fn validate_extern_c_symbol_namespace(
    module: &ModuleIR,
    c_names: &CNameMap,
) -> Result<(), CBackendError> {
    let mut extern_by_c_symbol: BTreeMap<String, String> = BTreeMap::new();
    for source_extern in module.externs.keys() {
        let c_symbol = map_callee(source_extern);
        // An Aura-valid source name need not be a valid C identifier.
        // Local Cells can be hex-mangled, but an extern MUST preserve the
        // exact native-linker contract: never silently rename it, and never
        // allow C keywords, non-ASCII or implementation-reserved spellings
        // through into an invalid generated prototype/call expression.
        if !c_safe_export_ident(&c_symbol) {
            return Err(CBackendError { message: format!(
                "extern C identifier invalid: external '{}' maps to '{}', which cannot be emitted as a portable C function identifier",
                source_extern, c_symbol
            ) });
        }
        // Unlike locally defined Cells, foreign symbols cannot be renamed
        // to avoid libc, macro and Aura-runtime declarations. Reject even
        // unused colliding declarations: a later Call or optimizer pass must
        // not change the validity of a linker ABI.
        if is_reserved_runtime_header_macro(&c_symbol) {
            return Err(CBackendError { message: format!(
                "extern C symbol collision: external '{}' targets reserved runtime/header macro '{}'; refusing ambiguous native ABI",
                source_extern, c_symbol
            ) });
        }
        if c_names.reserved_extern_symbols.contains(&c_symbol) {
            return Err(CBackendError { message: format!(
                "extern C symbol collision: external '{}' targets reserved runtime/header identifier '{}'; refusing ambiguous native ABI",
                source_extern, c_symbol
            ) });
        }
        for (source_cell, exported_c_symbol) in &c_names.exports {
            if exported_c_symbol == &c_symbol {
                return Err(CBackendError { message: format!(
                    "extern C symbol collision: external '{}' and local Cell '{}' both map to native '{}'; refusing ambiguous ABI",
                    source_extern, source_cell, c_symbol
                ) });
            }
        }
        if let Some(previous_extern) =
            extern_by_c_symbol.insert(c_symbol.clone(), source_extern.clone())
        {
            return Err(CBackendError { message: format!(
                "extern C symbol collision: external '{}' and external '{}' both map to native '{}'; refusing ambiguous ABI",
                previous_extern, source_extern, c_symbol
            ) });
        }
    }
    Ok(())
}

// Only C-compatible primitive signatures with an independently unambiguous
// native representation are admitted for an actually invoked extern Cell.
// Unused declarations remain inert; "Tensor" is never an acceptable silent
// fallback for an unknown external C signature.
fn qualified_extern_scalar_ctype(
    ty: &Type,
    allow_unit_return: bool,
    callee: &str,
) -> Result<CType, CBackendError> {
    let ct = match ty {
        Type::Unit if allow_unit_return => CType::Void,
        Type::Bool => CType::Bool,
        Type::U8 => CType::U8,
        Type::U16 => CType::U16,
        Type::U32 => CType::U32,
        Type::U64 => CType::U64,
        Type::I8 => CType::I8,
        Type::I16 => CType::I16,
        Type::I32 => CType::I32,
        Type::I64 => CType::I64,
        _ => return Err(CBackendError { message: format!(
            "unqualified extern C scalar ABI for '{}' type {:?}; nominal, Tensor, pointers, strings, floats, wide/platform integers and unit parameters need separate qualification",
            callee, ty
        ) }),
    };
    Ok(ct)
}

fn emit_module_c(module: &ModuleIR, debug: Option<&DebugSource>) -> Result<String, CBackendError> {
    let mut out = String::new();
    let c_names = CNameMap::new(module);
    validate_extern_c_symbol_namespace(module, &c_names)?;
    out.push_str("// Proof-carrying: IR validated by aura_ir::validate_module\n");
    out.push_str("#include \"aura_runtime.h\"\n\n");

    // Emit only qualified nominal layouts, in dependency order. Unused
    // unsupported declarations remain inert until they are actually used.
    let mut emitted_records = BTreeSet::new();
    for name in module.record_layouts.keys() {
        if qualified_record_fields(&module.record_layouts, name).is_ok() {
            emit_qualified_record_layout(&mut out, &module.record_layouts,
                name, &mut emitted_records)?;
        }
    }

    // External C function declarations are source-signature-qualified.
    // Emit a prototype only for an actual call, prior to all user Cell bodies,
    // so Clang cannot infer implicit 'int' or fabricate a Tensor result.
    // Never manufacture prototype/ABI support for unused unsupported externs.
    let mut called_externs: BTreeSet<&str> = BTreeSet::new();
    for function in module.functions.values() {
        for block in &function.blocks {
            for inst in &block.insts {
                match &inst.kind {
                    InstKind::Call { callee, .. } if module.externs.contains_key(callee) => {
                        called_externs.insert(callee.as_str());
                    }
                    InstKind::ComputeKernel { callee, .. }
                        if module.externs.contains_key(callee) =>
                    {
                        return Err(CBackendError { message: format!(
                            "unqualified extern C async kernel ABI for '{}'", callee
                        ) });
                    }
                    _ => {}
                }
            }
        }
    }
    for source in &called_externs {
        let signature = &module.externs[*source];
        if signature.call_conv != aura_ir::CallConv::C {
            return Err(CBackendError { message: format!(
                "unqualified extern C calling convention for '{}': {:?}",
                source, signature.call_conv
            ) });
        }
        let ret = qualified_extern_scalar_ctype(&signature.ret, true, source)?;
        let params = if signature.params.is_empty() {
            "void".to_string()
        } else {
            signature.params.iter().map(|ty|
                qualified_extern_scalar_ctype(ty, false, source)
                    .map(map_ctype_decl).map(str::to_owned)
            ).collect::<Result<Vec<_>, _>>()?.join(", ")
        };
        out.push_str(&format!("extern {} {}({});\n",
            map_ctype_decl(ret), map_callee(source), params));
    }
    if !called_externs.is_empty() { out.push('\n'); }

    // Declare all functions before emitting bodies: calls may target a later
    // source cell, including a function with a nominal struct return.
    for (name, f) in &module.functions {
        emit_function_signature(&mut out, f, &module.record_layouts,
            c_names.symbol(name), &c_names.formals[name])?;
        out.push_str(";\n");
    }
    out.push('\n');

    // Determine function return types by terminator shape (prototype).
    let mut ret_map: HashMap<String, CType> = HashMap::new();
    for (name, f) in &module.functions {
        ret_map.insert(name.clone(), function_return_ctype(f));
    }
    // External call results have the *declared native scalar ABI*, never the
    // old builtin Tensor default. This ret_map is keyed by Aura identity.
    for source in &called_externs {
        let sig = &module.externs[*source];
        ret_map.insert((*source).to_string(),
            qualified_extern_scalar_ctype(&sig.ret, true, source)?);
    }

    // Emit non-entry functions first.
    for (name, f) in &module.functions {
        if name == "main" {
            continue;
        }
        emit_function(&mut out, debug, f, &ret_map, &module.record_layouts, &module.functions, &module.externs,
            &module.refined_param_bounds, &module.refined_return_bounds, &c_names, name)?;
        out.push('\n');
    }

    // Emit Aura entry block as `void aura_main(void)` then C main.
    if let Some(main_f) = module.functions.get("main") {
        let mut f = main_f.clone();
        f.name = "aura_main".to_string();
        emit_function(&mut out, debug, &f, &ret_map, &module.record_layouts, &module.functions, &module.externs,
            &module.refined_param_bounds, &module.refined_return_bounds, &c_names, "main")?;
        out.push_str("\nint main(void) {\n  aura_main();\n  return 0;\n}\n");
    }

    Ok(out)
}

fn c_escape_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out
}

fn function_has_string_interpolation(function: &FunctionIR) -> bool {
    function.blocks.iter().any(|block| {
        block.insts.iter().any(|inst| {
            matches!(
                &inst.kind,
                InstKind::BindStrand {
                    expr: RValue::ConstString(template),
                    ..
                } if template.contains('{') && template.contains('}')
            )
        })
    })
}

fn emit_line_directive(out: &mut String, debug: Option<&DebugSource>, span: aura_ast::Span) {
    let Some(dbg) = debug else { return };
    let lc = dbg.line_col(span);
    let file = c_escape_string_literal(&dbg.file_name);
    out.push_str(&format!("#line {} \"{}\"\n", lc.line, file));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CType {
    Void,
    Bool,
    Char,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    F32,
    F64,
    CString,
    Tensor,
}

fn function_return_ctype(f: &FunctionIR) -> CType {
    for b in &f.blocks {
        if let Terminator::Return(Some(_)) = &b.term {
            return map_type_to_ctype(&f.ret);
        }
    }
    CType::Void
}

fn emit_function(
    out: &mut String,
    debug: Option<&DebugSource>,
    f: &FunctionIR,
    ret_map: &HashMap<String, CType>,
    record_layouts: &BTreeMap<String, Vec<(String, Type)>>,
    functions: &BTreeMap<String, FunctionIR>,
    externs: &BTreeMap<String, aura_ir::ExternFnSig>,
    source_refined_param_bounds: &BTreeMap<String, Vec<Option<(u64, u64)>>>,
    source_refined_return_bounds: &BTreeMap<String, (u64, u64)>,
    c_names: &CNameMap,
    source_name: &str,
) -> Result<(), CBackendError> {
    emit_function_signature(out, f, record_layouts,
        c_names.symbol(source_name), &c_names.formals[source_name])?;
    out.push_str(" {\n");
    if function_has_string_interpolation(f) {
        // Nominal and scalar parameters may be used only as captured
        // template identifiers; strict C -Wunused-parameter must not fail.
        for index in 0..f.params.len() {
            out.push_str(&format!("  (void)({});\n", c_names.formal(source_name, index)));
        }
    }

    // The *callee* is also a public C ABI trust boundary. Aura call sites
    // have verified RangeCheck instructions, but an external C caller has
    // none. Validate the Checker-resolved source contract upon every entry,
    // before executing even the first basic block. Do not infer any contract
    // from the integer ABI width alone.
    if let Some(bounds) = source_refined_param_bounds.get(source_name) {
        if bounds.len() != f.params.len() {
            return Err(CBackendError { message: format!(
                "refined ABI entry '{}' metadata arity mismatch: {} vs {}",
                f.name, bounds.len(), f.params.len()
            ) });
        }
        for (idx, (param, interval)) in f.params.iter().zip(bounds.iter()).enumerate() {
            let Some((lo, hi)) = interval else { continue };
            let cty = map_type_to_ctype(&param.ty);
            let (max, wide) = match cty {
                CType::U8 => (u8::MAX as u64, false),
                CType::U16 => (u16::MAX as u64, false),
                CType::U32 => (u32::MAX as u64, false),
                CType::I8 => (i8::MAX as u64, false),
                CType::I16 => (i16::MAX as u64, false),
                CType::I32 => (i32::MAX as u64, false),
                CType::U64 | CType::USize => (u64::MAX, true),
                CType::I64 | CType::ISize => (i64::MAX as u64, true),
                _ => return Err(CBackendError { message: format!(
                    "unqualified refined ABI entry '{}' parameter {} type {:?}",
                    f.name, idx, cty
                ) }),
            };
            if lo > hi || *hi > max {
                return Err(CBackendError { message: format!(
                    "refined ABI entry '{}' parameter {} range [{}..{}] exceeds {:?}",
                    f.name, idx, lo, hi, cty
                ) });
            }
            let arg = c_names.formal(source_name, idx);
            let display = escape_c_string(&format!("{}.{}", f.name, param.name));
            if wide {
                out.push_str(&format!(
                    "  AURA_RANGE_CHECK_U64((uint64_t)({arg}), {lo}ULL, {hi}ULL, \"{display}\");\n"
                ));
            } else {
                out.push_str(&format!(
                    "  AURA_RANGE_CHECK_U32((uint32_t)({arg}), {lo}u, {hi}u, \"{display}\");\n"
                ));
            }
        }
    }

    // Value table: ValueId -> (ctype, generated C name)
    let mut values: HashMap<ValueId, (CType, String)> = HashMap::new();
    // Nominal Record SSA lives in a separate typed namespace. Never pass
    // a Record as a Tensor/Scalar to unqualified operations or ABI paths.
    let mut records: HashMap<ValueId, (String, String)> = HashMap::new();

    // Preserve source-level names for AVM-compatible `{ident}` interpolation.
    let mut named_values: HashMap<String, (CType, String)> = HashMap::new();

    // AVM interpolates nominal Record values as `<RecordName>`, rather than
    // exposing fields or preserving the `{ident}` placeholder. Track source
    // bindings separately from scalar printf operands; no C struct ABI cast.
    let mut named_record_types: HashMap<String, String> = HashMap::new();

    // Preserve the frozen source-value bindings through local aliases.
    let mut string_literals: HashMap<ValueId, StringSnapshot> = HashMap::new();
    let mut named_string_snapshots: HashMap<String, StringSnapshot> = HashMap::new();

    for (index, p) in f.params.iter().enumerate() {
        if let Some(record) = qualified_record_abi_type(&p.ty, record_layouts)? {
            named_record_types.insert(p.name.clone(), record.clone());
            records.insert(p.value, (record, c_names.formal(source_name, index).to_string()));
        } else {
            let binding = (map_type_to_ctype(&p.ty), c_names.formal(source_name, index).to_string());
            named_values.insert(p.name.clone(), binding.clone());
            values.insert(p.value, binding);
        }
    }

    // Static SSA type prepass for supported value-producing instructions.
    // Phi incoming definitions can be in any predecessor block; the normal
    // code emitter has not yet visited those blocks when it allocates Phi
    // storage. Infer types from the same typed operands used by codegen,
    // iterating to a fixed point for chains and other already-resolved Phi.
    //
    // This is intentionally NOT a default U32, a full IR type checker, or
    // inference for opaque/composite instructions: unresolved Phi inputs
    // remain rejected below, never treated as a successful C emission.
    let mut known_ssa_types: HashMap<ValueId, CType> = values
        .iter()
        .map(|(value, (ty, _))| (*value, *ty))
        .collect();
    let max_rounds = f.blocks.iter().map(|b| b.insts.len()).sum::<usize>() + 1;
    for _ in 0..max_rounds {
        let mut progressed = false;
        for b in &f.blocks {
            for inst in &b.insts {
                let Some(dest) = inst.dest else { continue };
                let inferred = match &inst.kind {
                    InstKind::BindStrand { expr, .. } => match expr {
                        RValue::ConstU32(_) => Some(CType::U32),
                        RValue::ConstI64(_) => Some(CType::I64),
                        RValue::ConstF64(_) => Some(CType::F64),
                        RValue::ConstChar(_) => Some(CType::Char),
                        RValue::ConstBool(_) => Some(CType::Bool),
                        RValue::ConstString(_) => Some(CType::CString),
                        RValue::Local(source) => known_ssa_types.get(source).copied(),
                    },
                    InstKind::Unary { op, operand } => known_ssa_types
                        .get(operand)
                        .copied()
                        .and_then(|ty| match op {
                            UnaryOp::Neg if matches!(
                                ty, CType::I32 | CType::I64 | CType::U32 | CType::U64
                                    | CType::F32 | CType::F64
                            ) => Some(ty),
                            UnaryOp::Not if ty == CType::Bool => Some(CType::Bool),
                            _ => None,
                        }),
                    InstKind::Binary { op, left, right } => {
                        match (known_ssa_types.get(left), known_ssa_types.get(right)) {
                            (Some(&lt), Some(&rt)) => emit_binop(*op, lt, rt, "lhs", "rhs")
                                .ok()
                                .map(|(ty, _)| ty),
                            _ => None,
                        }
                    }
                    InstKind::Phi { incomings } if !incomings.is_empty() => {
                        // Loop-carried SSA forms a legitimate type-inference
                        // cycle: header Phi -> body Binary -> backedge Phi.
                        // Seed the candidate from a *known* incoming (usually
                        // the typed preheader). This is only a provisional
                        // type used to unblock the fixed point; the mandatory
                        // Phi declaration pass below checks EVERY incoming
                        // before C is emitted, refusing mismatched widths.
                        // Never default an entirely unknown Phi to U32.
                        let known = incomings.iter()
                            .filter_map(|(_, value)| known_ssa_types.get(value).copied())
                            .collect::<Vec<_>>();
                        known.first().copied().filter(|first|
                            known.iter().all(|candidate| candidate == first)
                        )
                    },
                    // A user-cell Call is a typed SSA producer. Its result
                    // may be an incoming value of a join-block Phi even when
                    // the Call appears in a successor block in serialized IR.
                    // Use the exact declared source-lowered return ABI; never
                    // infer scalar U32 by default, and keep nominal Records
                    // exclusively on the independent Record-type path.
                    InstKind::Call { callee, .. } => {
                        if let Some(called) = functions.get(callee) {
                            if qualified_record_abi_type(&called.ret, record_layouts)?.is_some() {
                                None
                            } else {
                                let ty = map_type_to_ctype(&called.ret);
                                if matches!(ty, CType::Void | CType::Tensor) {
                                    None
                                } else {
                                    Some(ty)
                                }
                            }
                        } else if let Some(external) = externs.get(callee) {
                            let ty = qualified_extern_scalar_ctype(&external.ret, true, callee)?;
                            if ty == CType::Void { None } else { Some(ty) }
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(ty) = inferred {
                    match known_ssa_types.get(&dest) {
                        Some(previous) if *previous != ty => return Err(CBackendError {
                            message: format!(
                                "conflicting inferred SSA types for v{}: {previous:?} vs {ty:?}",
                                dest.0
                            ),
                        }),
                        Some(_) => {}
                        None => {
                            known_ssa_types.insert(dest, ty);
                            progressed = true;
                        }
                    }
                }
            }
        }
        if !progressed { break; }
    }

    // Nominal Record SSA prepass. Unlike primitive CType inference, a Record
    // must carry its exact declared name across copies, functional SetField
    // updates, and join-block Phi nodes. Never infer it as Tensor from width
    // or initializer shape. Fixed-point inference handles predecessor blocks
    // appearing after a join in the serialized IR.
    let mut known_record_types: HashMap<ValueId, String> = records.iter().map(|(id, (name, _))| (*id, name.clone())).collect();
    for _ in 0..max_rounds {
        let mut progressed = false;
        for block in &f.blocks {
            for inst in &block.insts {
                let Some(dest) = inst.dest else { continue };
                let inferred = match &inst.kind {
                    InstKind::ConstructRecord { record_name, .. } => {
                        // Missing or unqualified layouts cannot become a
                        // nominal Record at Phi simply by supplying a name.
                        qualified_record_fields(record_layouts, record_name)?;
                        Some(record_name.clone())
                    }
                    InstKind::GetField { base, field_name } => {
                        // A child Record projected from a parent is a new
                        // nominal SSA producer too. The Phi type prepass runs
                        // BEFORE emitting individual GetField instructions,
                        // so it cannot rely on the runtime 'records' table.
                        if let Some(parent) = known_record_types.get(base) {
                            let fields = qualified_record_fields(record_layouts, parent)?;
                            match fields.iter().find(|(name, _)| name == field_name) {
                                Some((_, RecordFieldCType::Nominal(child))) => Some(child.clone()),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    }
                    InstKind::BindStrand { expr: RValue::Local(source), .. } =>
                        known_record_types.get(source).cloned(),
                    InstKind::SetField { base, .. } =>
                        known_record_types.get(base).cloned(),
                    // A Record returned by another function is a nominal SSA
                    // producer too. Without this edge, a later if/else Phi
                    // cannot resolve the type of values sourced from calls.
                    InstKind::Call { callee, .. } => {
                        if let Some(called) = functions.get(callee) {
                            qualified_record_abi_type(&called.ret, record_layouts)?
                        } else {
                            None
                        }
                    }
                    InstKind::Phi { incomings } if !incomings.is_empty() => {
                        // A loop-carried Record Phi can depend cyclically on
                        // another Phi. Seed from an independently typed
                        // preheader/formal, then require *every* incoming to
                        // have the same nominal type in the declaration pass.
                        let known = incomings.iter()
                            .filter_map(|(_, incoming)| known_record_types.get(incoming).cloned())
                            .collect::<Vec<_>>();
                        known.first().cloned().filter(|name|
                            known.iter().all(|candidate| candidate == name)
                        )
                    }
                    _ => None,
                };
                if let Some(record) = inferred {
                    if let Some(scalar) = known_ssa_types.get(&dest) {
                        return Err(CBackendError { message: format!(
                            "unqualified Record SSA v{} conflicts with scalar type {scalar:?}",
                            dest.0,
                        ) });
                    }
                    match known_record_types.get(&dest) {
                        Some(previous) if previous != &record =>
                            return Err(CBackendError { message: format!(
                                "unqualified Record SSA v{} mixes '{}' and '{}'",
                                dest.0, previous, record,
                            ) }),
                        Some(_) => {}
                        None => { known_record_types.insert(dest, record); progressed = true; }
                    }
                }
            }
        }
        if !progressed { break; }
    }

    // Qualified scalar Record narrowing from literal-derived locals needs
    // provenance beyond CType: a typed U32 alone can represent unsafe values.
    // Track ONLY SSA ConstU32 and pure Local copies, to a fixed point. No
    // generic arithmetic, call, Phi, or mutable parameter is a proof of range.
    let mut known_u32_constants: HashMap<ValueId, u64> = HashMap::new();
    for _ in 0..max_rounds {
        let mut progressed = false;
        for block in &f.blocks {
            for inst in &block.insts {
                let Some(dest) = inst.dest else { continue };
                let constant = match &inst.kind {
                    InstKind::BindStrand { expr: RValue::ConstU32(n), .. }
                        if *n <= u32::MAX as u64 => Some(*n),
                    InstKind::BindStrand { expr: RValue::Local(source), .. } =>
                        known_u32_constants.get(source).copied(),
                    _ => None,
                };
                if let Some(n) = constant {
                    if let Some(old) = known_u32_constants.insert(dest, n) {
                        if old != n {
                            return Err(CBackendError { message: format!(
                                "conflicting inferred U32 constant at SSA v{}", dest.0,
                            ) });
                        }
                    } else {
                        progressed = true;
                    }
                }
            }
        }
        if !progressed { break; }
    }

    // A Phi is SSA-level typed storage whose writes occur on its incoming
    // predecessor edges. Declare it before all block labels so every goto
    // and edge assignment references an in-scope C variable.
    //
    // A Phi is emitted only if all incoming SSA types were inferred
    // consistently; unresolved, cyclic or unmodeled producers fail closed.
    let mut phi_inj: HashMap<(BlockId, BlockId), Vec<(ValueId, ValueId)>> = HashMap::new();
    let mut phi_decls: Vec<(ValueId, CType)> = Vec::new();
    let mut record_phi_decls: Vec<(ValueId, String)> = Vec::new();
    for b in &f.blocks {
        for inst in &b.insts {
            if let InstKind::Phi { incomings } = &inst.kind {
                let dest = inst.dest.ok_or_else(|| CBackendError {
                    message: "Phi instruction is missing its SSA destination".to_string(),
                })?;
                if incomings.is_empty() {
                    return Err(CBackendError {
                        message: format!("Phi v{} has no predecessor values", dest.0),
                    });
                }
                // All incoming values must be the SAME nominal Record. A
                // scalar/Record mix or distinct Record names is a typed error,
                // never a Tensor fallback or raw struct bitcast.
                if incomings.iter().any(|(_, incoming)| known_record_types.contains_key(incoming)) {
                    let mut nominal: Option<&str> = None;
                    for (pred, incoming) in incomings {
                        let incoming_name = known_record_types.get(incoming).ok_or_else(|| {
                            CBackendError { message: format!(
                                "unqualified Record Phi v{} missing nominal type of incoming v{}",
                                dest.0, incoming.0,
                            ) }
                        })?;
                        if let Some(previous) = nominal {
                            if previous != incoming_name {
                                return Err(CBackendError { message: format!(
                                    "unqualified Record Phi v{} mixes nominal layouts '{}' and '{}'",
                                    dest.0, previous, incoming_name,
                                ) });
                            }
                        } else {
                            nominal = Some(incoming_name);
                        }
                        if known_ssa_types.contains_key(incoming) {
                            return Err(CBackendError { message: format!(
                                "unqualified Record Phi v{} incoming v{} is also scalar",
                                dest.0, incoming.0,
                            ) });
                        }
                        phi_inj.entry((*pred, b.id)).or_default().push((dest, *incoming));
                    }
                    let record_name = nominal.expect("nonempty Record Phi");
                    qualified_record_fields(record_layouts, record_name)?;
                    if values.contains_key(&dest) || records.contains_key(&dest) {
                        return Err(CBackendError { message: format!(
                            "unqualified Record Phi v{} duplicate SSA destination", dest.0,
                        ) });
                    }
                    records.insert(dest, (record_name.to_string(), format!("v{}", dest.0)));
                    record_phi_decls.push((dest, record_name.to_string()));
                    continue;
                }
                let mut unified: Option<CType> = None;
                for (pred, incoming) in incomings {
                    let incoming_type = known_ssa_types.get(incoming)
                        .copied()
                        .ok_or_else(|| CBackendError {
                            message: format!(
                                "unqualified Phi v{}: type of incoming v{} is not available",
                                dest.0, incoming.0
                            ),
                        })?;
                    if let Some(ty) = unified {
                        if ty != incoming_type {
                            return Err(CBackendError {
                                message: format!(
                                    "Phi v{} merges incompatible types {ty:?} and {incoming_type:?}",
                                    dest.0
                                ),
                            });
                        }
                    } else {
                        unified = Some(incoming_type);
                    }
                    phi_inj.entry((*pred, b.id)).or_default().push((dest, *incoming));
                }
                let ty = unified.expect("nonempty Phi");
                if ty == CType::Void || values.contains_key(&dest) {
                    return Err(CBackendError {
                        message: format!("Phi v{} has void or duplicate SSA destination", dest.0),
                    });
                }
                values.insert(dest, (ty, format!("v{}", dest.0)));
                phi_decls.push((dest, ty));
            }
        }
    }
    for (dest, ty) in &phi_decls {
        out.push_str(&format!("  {} v{};\n", map_ctype_decl(*ty), dest.0));
    }

    for (dest, record) in &record_phi_decls {
        out.push_str(&format!("  {} v{};\n", record_type_symbol(record), dest.0));
    }

    // Emit an entry jump (we use gotos for CFG).
    out.push_str("  goto ");
    out.push_str(&bb_label(f.entry));
    out.push_str(";\n");

    // Emit blocks
    for b in &f.blocks {
        // Only a guard executed in this same basic block before the call
        // authorizes an exact integer ABI narrowing. Never infer a proof
        // from a different predecessor, an SSA type, or a source-level name.
        let mut current_block_ranges: HashMap<ValueId, (u64, u64)> = HashMap::new();
        emit_line_directive(out, debug, b.span);
        out.push_str("\n");
        out.push_str(&bb_label(b.id));
        // C11 requires a label to precede a statement, not a declaration.
        // Record and scalar SSA blocks may begin with variable declarations.
        out.push_str(":\n  ;\n");

        for inst in &b.insts {
            emit_line_directive(out, debug, inst.span);
            match &inst.kind {
                InstKind::AllocCapability { .. } => {}

                InstKind::Phi { .. } => {
                    // Phi nodes are implemented by assignments in predecessors (see terminators).
                }

                InstKind::RangeCheckU32 { value, lo, hi } | InstKind::RangeCheck { value, lo, hi } => {
                    let (ct, name) = values.get(value).ok_or_else(|| CBackendError {
                        message: format!("range guard has no typed SSA operand v{}", value.0),
                    })?;
                    if lo > hi {
                        return Err(CBackendError {
                            message: format!("inverted range guard [{lo}..{hi}] for SSA v{}", value.0),
                        });
                    }
                    // A source-refined call on U64 must never use a U32
                    // helper: high bits of the operand AND bounds are part
                    // of the contract. Preserve the same-block guard proof
                    // for both 32-bit and 64-bit scalar call operands.
                    let wide = match ct {
                        CType::U8 | CType::U16 | CType::U32 |
                        CType::I8 | CType::I16 | CType::I32 => {
                            if *hi > u32::MAX as u64 {
                                return Err(CBackendError {
                                    message: format!("u32 range guard bound {hi} exceeds 32-bit ABI"),
                                });
                            }
                            false
                        }
                        CType::U64 | CType::USize | CType::I64 | CType::ISize => true,
                        other => return Err(CBackendError {
                            message: format!("unqualified range guard operand {:?} for SSA v{}", other, value.0),
                        }),
                    };
                    current_block_ranges.insert(*value, (*lo, *hi));
                    if wide {
                        out.push_str(&format!(
                            "  AURA_RANGE_CHECK_U64((uint64_t)({name}), {lo}ULL, {hi}ULL, \"{}\");\n",
                            escape_c_string(name)
                        ));
                    } else {
                        out.push_str(&format!(
                            "  AURA_RANGE_CHECK_U32((uint32_t)({name}), {lo}u, {hi}u, \"{}\");\n",
                            escape_c_string(name)
                        ));
                    }
                }

                InstKind::ConstructRecord { record_name, fields } => {
                    let Some(dest) = inst.dest else {
                        return Err(CBackendError { message:
                            "unqualified C Record emission without SSA destination".into() });
                    };
                    let layout = qualified_record_fields(record_layouts, record_name)?;
                    if fields.len() != layout.len() {
                        return Err(CBackendError { message: format!(
                            "unqualified C Record '{}' field count mismatch: got {}, expected {}",
                            record_name, fields.len(), layout.len()
                        ) });
                    }
                    let mut initializers = Vec::new();
                    for (field, expected) in &layout {
                        let mut matching = fields.iter().filter(|(given, _)| given == field);
                        let Some((_, value)) = matching.next() else {
                            return Err(CBackendError { message: format!(
                                "unqualified C Record '{}' missing field '{}'", record_name, field
                            ) });
                        };
                        if matching.next().is_some() {
                            return Err(CBackendError { message: format!(
                                "unqualified C Record '{}' duplicate field '{}'", record_name, field
                            ) });
                        }
                        let cname = match expected {
                            RecordFieldCType::Scalar(CType::CString)
                                if string_literals.get(value)
                                    .is_some_and(StringSnapshot::has_substitutions) =>
                            {
                                // Local String snapshots are not materialized
                                // in this C ABI. A Record holding the raw
                                // template would print a different value from
                                // AVM and could escape through a return/Cell.
                                return Err(CBackendError { message: format!(
                                    "unqualified interpolated String Record field '{}' in '{}': captured String cannot escape as raw C pointer",
                                    field, record_name
                                ) });
                            }
                            RecordFieldCType::Scalar(ty) => match values.get(value) {
                                Some((actual, cname)) => qualified_record_scalar_value(
                                    *ty, *actual, *value, cname, &known_u32_constants,
                                ).ok_or_else(|| CBackendError { message: format!(
                                    "unqualified C Record '{}' field '{}' type mismatch {:?} != {:?} (no proven in-range constant)",
                                    record_name, field, actual, ty,
                                ) })?,
                                None => return Err(CBackendError { message: format!(
                                    "unqualified C Record '{}' field '{}' unemitted SSA v{} or nominal Record supplied to scalar field",
                                    record_name, field, value.0,
                                ) }),
                            },
                            RecordFieldCType::Nominal(name) => match records.get(value) {
                                Some((actual, cname)) if actual == name => cname.clone(),
                                Some((actual, _)) => return Err(CBackendError { message: format!(
                                    "unqualified C Record '{}' field '{}' nominal mismatch '{}' != '{}'",
                                    record_name, field, actual, name,
                                ) }),
                                None => return Err(CBackendError { message: format!(
                                    "unqualified C Record '{}' field '{}' requires nominal '{}' at SSA v{}",
                                    record_name, field, name, value.0,
                                ) }),
                            },
                        };
                        initializers.push(format!(".{} = {}", record_field_symbol(field), cname));
                    }
                    let cname = format!("v{}", dest.0);
                    out.push_str(&format!(
                        "  {} {} = {{ {} }};\n",
                        record_type_symbol(record_name), cname, initializers.join(", ")
                    ));
                    records.insert(dest, (record_name.clone(), cname));
                }
                InstKind::GetField { base, field_name } => {
                    let Some(dest) = inst.dest else {
                        return Err(CBackendError { message:
                            "unqualified C Record GetField without SSA destination".into() });
                    };
                    let (record, cname) = records.get(base).ok_or_else(|| CBackendError {
                        message: format!("unqualified C Record GetField from unavailable v{}", base.0),
                    })?;
                    let layout = qualified_record_fields(record_layouts, record)?;
                    let (_, ty) = layout.iter().find(|(name, _)| name == field_name)
                        .ok_or_else(|| CBackendError { message: format!(
                            "unqualified C Record '{}' unknown field '{}'", record, field_name
                        ) })?;
                    let dest_name = format!("v{}", dest.0);
                    out.push_str(&format!("  {} {} = {}.{};\n",
                        ty.declaration(), dest_name, cname, record_field_symbol(field_name)));
                    match ty {
                        RecordFieldCType::Scalar(ct) => {
                            values.insert(dest, (*ct, dest_name));
                        }
                        RecordFieldCType::Nominal(name) => {
                            records.insert(dest, (name.clone(), dest_name));
                        }
                    }
                }
                InstKind::SetField { base, field_name, value } => {
                    // This IR instruction is a functional update: the Oracle
                    // creates a NEW Record value only when dest is Some.
                    // Never overwrite the base SSA value (or aliases of it).
                    let Some(dest) = inst.dest else {
                        return Err(CBackendError {
                            message: format!(
                                "unqualified C Record SetField without result SSA at source span {:?}; refusing divergent in-place mutation",
                                inst.span,
                            ),
                        });
                    };
                    if records.contains_key(&dest) || values.contains_key(&dest) {
                        return Err(CBackendError { message: format!(
                            "unqualified C Record SetField duplicate destination v{}", dest.0,
                        ) });
                    }
                    let (record, original_name) = records.get(base).cloned().ok_or_else(|| {
                        CBackendError { message: format!(
                            "unqualified C Record SetField from unavailable base SSA v{}", base.0,
                        ) }
                    })?;
                    let layout = qualified_record_fields(record_layouts, &record)?;
                    let (_, expected) = layout.iter().find(|(field, _)| field == field_name)
                        .ok_or_else(|| CBackendError { message: format!(
                            "unqualified C Record '{}' unknown SetField '{}'", record, field_name,
                        ) })?;
                    let rhs = match expected {
                        RecordFieldCType::Scalar(CType::CString)
                            if string_literals.get(value)
                                .is_some_and(StringSnapshot::has_substitutions) =>
                        {
                            return Err(CBackendError { message: format!(
                                "unqualified interpolated String Record SetField '{}.{}': captured String cannot escape as raw C pointer",
                                record, field_name
                            ) });
                        }
                        RecordFieldCType::Scalar(ty) => match values.get(value) {
                            Some((actual, name)) => qualified_record_scalar_value(
                                *ty, *actual, *value, name, &known_u32_constants,
                            ).ok_or_else(|| CBackendError { message: format!(
                                "unqualified C Record '{}' SetField '{}' type mismatch {:?} != {:?} (no proven in-range constant)",
                                record, field_name, actual, ty,
                            ) })?,
                            None => return Err(CBackendError { message: format!(
                                "unqualified C Record '{}' SetField '{}' unavailable scalar SSA v{}",
                                record, field_name, value.0,
                            ) }),
                        },
                        RecordFieldCType::Nominal(expected_name) => match records.get(value) {
                            Some((actual, name)) if actual == expected_name => name.clone(),
                            Some((actual, _)) => return Err(CBackendError { message: format!(
                                "unqualified C Record '{}' SetField '{}' nominal mismatch '{}' != '{}'",
                                record, field_name, actual, expected_name,
                            ) }),
                            None => return Err(CBackendError { message: format!(
                                "unqualified C Record '{}' SetField '{}' missing nominal '{}' at SSA v{}",
                                record, field_name, expected_name, value.0,
                            ) }),
                        },
                    };
                    let updated_name = format!("v{}", dest.0);
                    out.push_str(&format!(
                        "  {} {} = {};\n",
                        record_type_symbol(&record), updated_name, original_name,
                    ));
                    out.push_str(&format!(
                        "  {}.{} = {};\n",
                        updated_name, record_field_symbol(field_name), rhs,
                    ));
                    records.insert(dest, (record, updated_name));
                }

                InstKind::ConstructEnumVariant { .. }
                | InstKind::GetEnumTag { .. }
                | InstKind::GetEnumPayload { .. } => {
                    // Tagged Enum layout, payload widths and discriminant
                    // projection have not been qualified in this C backend.
                    // Never silently emit a valid-looking C artifact whose
                    // live Enum value was discarded.
                    return Err(CBackendError {
                        message: format!(
                            "unqualified C Enum emission: unsupported instruction {:?} at source span {:?}; typed tag and payload representation required",
                            &inst.kind, inst.span,
                        ),
                    });
                }

                InstKind::BindStrand { name, expr } => {
                    // Aura's String contains arbitrary Unicode, including
                    // U+0000. The currently qualified native String ABI is
                    // a NUL-terminated const char*: a literal with embedded
                    // NUL would be silently truncated by libc, Record fields
                    // and String Cells. Reject it before emitting C until
                    // an explicit pointer+length String representation is
                    // qualified across consumers.
                    if let RValue::ConstString(contents) = expr {
                        if contents.contains('\0') {
                            return Err(CBackendError { message: format!(
                                "unqualified native String embedded NUL in Cell '{}' binding '{}': const char* ABI cannot represent length",
                                f.name, name,
                            ) });
                        }
                    }
                    if let Some(dest) = inst.dest {
                        // A Record local must preserve its nominal C struct
                        // identity; do not cast/copy it through Tensor ABI.
                        if let RValue::Local(source) = expr {
                            if let Some((record, from)) = records.get(source).cloned() {
                                let cname = format!("v{}", dest.0);
                                out.push_str(&format!("  {} {} = {};\n",
                                    record_type_symbol(&record), cname, from));
                                named_values.remove(name);
                                named_record_types.insert(name.clone(), record.clone());
                                named_string_snapshots.remove(name);
                                records.insert(dest, (record, cname));
                                if function_has_string_interpolation(f) {
                                    out.push_str(&format!("  (void)v{};\n", dest.0));
                                }
                                continue;
                            }
                        }
                        // RValue::Local is an alias, so its C type must be the
                        // source value's type. The old prototype hard-coded
                        // Tensor here, which is incorrect for u32/bool/string.
                        // Capture before binding the new source name, so
                        // self-referential and forward names remain unknown.
                        let captured = match expr {
                            RValue::ConstString(t) => Some(capture_string_snapshot(
                                t, &named_values, &named_record_types, &named_string_snapshots
                            )),
                            RValue::Local(src) => string_literals.get(src).cloned(),
                            _ => None,
                        };
                        let (ct, decl) = match expr {
                            RValue::Local(source) => {
                                let (source_ct, source_name) = values
                                    .get(source)
                                    .cloned()
                                    .unwrap_or((CType::Tensor, format!("v{}", source.0)));
                                (
                                    source_ct,
                                    // CString maps to "const char*"; an extra
                                    // leading const creates invalid strict C:
                                    // "const const char*". This Local alias
                                    // copies the pointer value, not the data.
                                    format!(
                                        "{}{} v{} = {};",
                                        if source_ct == CType::CString { "" } else { "const " },
                                        map_ctype_decl(source_ct),
                                        dest.0,
                                        source_name
                                    ),
                                )
                            }
                            _ => emit_rvalue_decl(dest, name, expr),
                        };

                        out.push_str("  ");
                        out.push_str(&decl);
                        out.push('\n');

                        let binding = (ct, format!("v{}", dest.0));
                        values.insert(dest, binding.clone());
                        named_record_types.remove(name);
                        named_values.insert(name.clone(), binding);

                        if let Some(snapshot) = captured {
                            string_literals.insert(dest, snapshot.clone());
                            named_string_snapshots.insert(name.clone(), snapshot);
                        } else {
                            string_literals.remove(&dest);
                            named_string_snapshots.remove(name);
                        }
                        if function_has_string_interpolation(f) {
                            // Optimizer roots the textual interpolation
                            // dependencies; consume each retained C binding
                            // even when used only through a source template.
                            out.push_str(&format!("  (void)v{};\n", dest.0));
                        }
                    }
                }

                InstKind::Unary { op, operand } => {
                    let Some(dest) = inst.dest else { continue };
                    let (operand_type, operand_name) = values
                        .get(operand)
                        .cloned()
                        .ok_or_else(|| CBackendError {
                            message: format!("untyped operand v{} in unary instruction", operand.0),
                        })?;
                    let (ct, expr_s) = match op {
                        // The same signed modular interpretation as the IR
                        // Oracle, without invoking undefined signed C negation.
                        UnaryOp::Neg if operand_type == CType::I32 => (
                            operand_type,
                            format!("aura_i32_from_wrapped_bits(0u - (uint32_t)({operand_name}))"),
                        ),
                        UnaryOp::Neg if operand_type == CType::I64 => (
                            operand_type,
                            format!("aura_i64_from_wrapped_bits(0ULL - (uint64_t)({operand_name}))"),
                        ),
                        UnaryOp::Neg if matches!(
                            operand_type,
                            CType::I32 | CType::I64 | CType::U32 | CType::U64
                                | CType::F32 | CType::F64
                        ) => (operand_type, format!("-({operand_name})")),
                        UnaryOp::Not if operand_type == CType::Bool => {
                            (CType::Bool, format!("!({operand_name})"))
                        }
                        _ => return Err(CBackendError {
                            message: format!(
                                "C emission does not qualify unary {op:?} on {operand_type:?}"
                            ),
                        }),
                    };
                    out.push_str("  ");
                    out.push_str(map_ctype_decl(ct));
                    out.push(' ');
                    out.push_str(&format!("v{} = {};\n", dest.0, expr_s));
                    values.insert(dest, (ct, format!("v{}", dest.0)));
                }

                InstKind::Binary { op, left, right } => {
                    let Some(dest) = inst.dest else { continue };
                    let (left_type, ln) = values
                        .get(left)
                        .cloned()
                        .ok_or_else(|| CBackendError {
                            message: format!("untyped left operand v{} in binary instruction", left.0),
                        })?;
                    let (right_type, rn) = values
                        .get(right)
                        .cloned()
                        .ok_or_else(|| CBackendError {
                            message: format!("untyped right operand v{} in binary instruction", right.0),
                        })?;
                    // A deferred interpolation String is still a template in
                    // this native ABI, not the value already computed by AVM.
                    if matches!(*op, BinOp::Eq | BinOp::Ne)
                        && left_type == CType::CString && right_type == CType::CString
                        && (string_literals.get(left).is_some_and(StringSnapshot::has_substitutions)
                            || string_literals.get(right).is_some_and(StringSnapshot::has_substitutions))
                    {
                        return Err(CBackendError {
                            message: "unqualified interpolated String equality: value not materialized in C ABI".into(),
                        });
                    }
                    let (ct, expr_s) = emit_binop(*op, left_type, right_type, &ln, &rn)?;
                    out.push_str("  ");
                    out.push_str(map_ctype_decl(ct));
                    out.push(' ');
                    out.push_str(&format!("v{} = {};\n", dest.0, expr_s));
                    values.insert(dest, (ct, format!("v{}", dest.0)));
                }

                InstKind::Call { callee, args } => {
                    let mut call_values = values.clone();
                    if let Some(target) = functions.get(callee) {
                        if target.params.len() != args.len() {
                            return Err(CBackendError { message: format!(
                                "Record ABI call '{}' has wrong arity: expected {}, got {}",
                                callee, target.params.len(), args.len(),
                            ) });
                        }
                        let source_bounds = source_refined_param_bounds.get(callee);
                        if let Some(bounds) = source_bounds {
                            if bounds.len() != target.params.len() {
                                return Err(CBackendError { message: format!(
                                    "refined ABI metadata arity mismatch for '{callee}': {} vs {}",
                                    bounds.len(), target.params.len()
                                ) });
                            }
                        }
                        for (i, (arg, expected)) in args.iter().zip(target.params.iter()).enumerate() {
                            let declared_range = source_bounds
                                .and_then(|b| b.get(i)).copied().flatten();
                            // IR source contract and runtime guard are separate
                            // authorities. For refined source parameters, the
                            // executable same-block interval must lie entirely
                            // within the original declared interval. A widened
                            // or missing guard cannot certify the call even if
                            // the constant fits the underlying primitive.
                            if let Some((decl_lo, decl_hi)) = declared_range {
                                let emitted_guard = current_block_ranges.get(arg).copied();
                                if decl_lo > decl_hi ||
                                    !emitted_guard.is_some_and(|(lo, hi)|
                                        lo <= hi && lo >= decl_lo && hi <= decl_hi)
                                {
                                    return Err(CBackendError { message: format!(
                                        "refined ABI call '{callee}' arg {i} range guard mismatch: source=[{decl_lo}..{decl_hi}], emitted={emitted_guard:?}"
                                    ) });
                                }
                            }
                            let expected_record = qualified_record_abi_type(&expected.ty, record_layouts)?;
                            match expected_record {
                                Some(name) => match records.get(arg) {
                                    Some((actual, _)) if actual == &name => {}
                                    _ => return Err(CBackendError { message: format!(
                                        "Record ABI call '{}' arg {} expects nominal '{}', got {:?}",
                                        callee, i, name, records.get(arg).map(|r| &r.0),
                                    ) }),
                                },
                                None => {
                                    if records.contains_key(arg) {
                                        return Err(CBackendError { message: format!(
                                            "Record ABI call '{}' arg {} passes Record to scalar parameter",
                                            callee, i,
                                        ) });
                                    }
                                    if let Some((actual, _)) = values.get(arg) {
                                        let required = map_type_to_ctype(&expected.ty);
                                        if *actual != required {
                                            // A U32 constant fitting U8 is NOT itself
                                            // proof of a stricter source refinement such
                                            // as Window=u8[10..20]. Only an executable
                                            // same-basic-block range guard can certify the
                                            // narrowed call. This also rejects IR with a
                                            // stripped proof, even if an SSA constant is
                                            // numerically representable as U8.
                                            let original = values.get(arg)
                                                .expect("scalar operand checked").1.clone();
                                            let range = current_block_ranges.get(arg);
                                            let guarded_cast = if *actual == CType::U32 && required == CType::U8 &&
                                                declared_range.is_some() &&
                                                range.is_some_and(|(lo, hi)| *lo <= *hi && *hi <= u8::MAX as u64)
                                            {
                                                Some(format!("((uint8_t)({original}))"))
                                            } else {
                                                None
                                            };
                                            if let Some(cast) = guarded_cast {
                                                call_values.insert(*arg, (required, cast));
                                            } else {
                                                return Err(CBackendError { message: format!(
                                                    "Record ABI call '{}' arg {} scalar type mismatch {:?} != {:?}; same-block-guards={:?}",
                                                    callee, i, actual, required, current_block_ranges,
                                                ) });
                                            }
                                        }
                                    } else {
                                        return Err(CBackendError { message: format!(
                                            "Record ABI call '{}' arg {} has unemitted scalar v{}",
                                            callee, i, arg.0,
                                        ) });
                                    }
                                }
                            }
                        }
                        if let Some(record) = qualified_record_abi_type(&target.ret, record_layouts)? {
                            let arg_names = args.iter().map(|a| {
                                records.get(a).map(|(_, n)| n.clone())
                                    .or_else(|| call_values.get(a).map(|(_, n)| n.clone()))
                                    .expect("all Record ABI call args checked")
                            }).collect::<Vec<_>>();
                            if let Some(dest) = inst.dest {
                                if records.contains_key(&dest) || values.contains_key(&dest) {
                                    return Err(CBackendError { message: format!(
                                        "Record ABI call '{}' duplicates destination v{}", callee, dest.0,
                                    ) });
                                }
                                let local = format!("v{}", dest.0);
                                out.push_str(&format!("  {} {} = {}({});\n",
                                    record_type_symbol(&record), local, c_names.symbol(callee), arg_names.join(", ")));
                                records.insert(dest, (record, local));
                            } else {
                                out.push_str(&format!("  {}({});\n", c_names.symbol(callee), arg_names.join(", ")));
                            }
                            continue;
                        }
                    } else if let Some(external) = externs.get(callee) {
                        // Validate the live IR operands against the ABI
                        // declaration; the source Checker alone cannot prove
                        // that the mutable IR still has the same argument types.
                        if external.params.len() != args.len() {
                            return Err(CBackendError { message: format!(
                                "extern C ABI call '{}' arity mismatch: {} vs {}",
                                callee, args.len(), external.params.len()
                            ) });
                        }
                        for (i, (arg, declared)) in args.iter().zip(&external.params).enumerate() {
                            let expected = qualified_extern_scalar_ctype(declared, false, callee)?;
                            let actual = call_values.get(arg).map(|(ct, _)| *ct)
                                .ok_or_else(|| CBackendError { message: format!(
                                    "extern C ABI call '{}' argument {} has no scalar SSA value",
                                    callee, i
                                ) })?;
                            if actual != expected {
                                return Err(CBackendError { message: format!(
                                    "extern C ABI call '{}' argument {} type mismatch: {:?} != {:?}",
                                    callee, i, actual, expected
                                ) });
                            }
                        }
                        let ret = qualified_extern_scalar_ctype(&external.ret, true, callee)?;
                        if (ret == CType::Void) != inst.dest.is_none() {
                            return Err(CBackendError { message: format!(
                                "extern C ABI call '{}' return destination mismatch: {:?} vs {:?}",
                                callee, inst.dest, ret
                            ) });
                        }
                    } else {
                        if args.iter().any(|arg| records.contains_key(arg)) {
                            return Err(CBackendError { message: format!(
                                "unqualified C Record argument in external/builtin call '{}'", callee
                            ) });
                        }
                        // The Checker also registers AVM-only standard functions
                        // (e.g. io.read_text/io.read_line). Those have NO
                        // native C implementation. Previously the generic
                        // callee mapper emitted io_read_text(...) and guessed a
                        // Tensor result, producing a successful Aura artifact
                        // that failed only when compiled by Clang. Require an
                        // exact source-name match against the builtins that
                        // this emitted C runtime actually implements.
                        // Never trust a dotted->underscore collision or a
                        // forged IR Call as proof that a runtime symbol exists.
                        if !matches!(callee.as_str(),
                            "io.println" | "io.load_tensor" |
                            "io.display" | "compute_gradient"
                        ) {
                            return Err(CBackendError { message: format!(
                                "unqualified native C Call '{}': no declared Aura Cell, trusted extern or implemented native builtin",
                                callee
                            ) });
                        }
                    }
                    // A raw const char* is not the evaluated AVM String.
                    // Other Cells and foreign calls require an owned String
                    // representation, so reject rather than pass raw braces.
                    if c_names.callee(callee) != "aura_io_println" &&
                        args.iter().any(|arg| string_literals.get(arg)
                            .is_some_and(StringSnapshot::has_substitutions))
                    {
                        return Err(CBackendError { message: format!(
                            "unqualified interpolated String escape in Call '{}': native C string ABI cannot preserve captured values",
                            callee
                        ) });
                    }
                    emit_call(
                        out, inst.dest, callee, args, &mut call_values, &records, ret_map,
                        &string_literals, c_names,
                    );
                    if let Some(dest) = inst.dest {
                        if let Some(binding) = call_values.get(&dest).cloned() {
                            values.insert(dest, binding);
                        }
                    }
                }
                InstKind::ComputeKernel { callee, args } => {
                    if args.iter().any(|arg| records.contains_key(arg)) {
                        return Err(CBackendError { message: format!(
                            "unqualified C Record argument in kernel '{}'", callee
                        ) });
                    }
                    emit_kernel(out, inst.dest, callee, args, &mut values, c_names);
                }
            }
        }

        // Terminator
        match &b.term {
            Terminator::Return(Some(v)) => {
                if string_literals.get(v).is_some_and(StringSnapshot::has_substitutions) {
                    return Err(CBackendError { message: format!(
                        "unqualified interpolated String escape in Return of '{}': native C pointer cannot represent its captured source value",
                        f.name
                    ) });
                }
                let expected_record = qualified_record_abi_type(&f.ret, record_layouts)?;
                let name = if let Some(expected) = expected_record {
                    let (actual, c_name) = records.get(v).ok_or_else(|| CBackendError {
                        message: format!("Record ABI return in '{}' is not emitted Record v{}", f.name, v.0),
                    })?;
                    if actual != &expected {
                        return Err(CBackendError { message: format!(
                            "Record ABI return in '{}' expects nominal '{}', got '{}'", f.name, expected, actual,
                        ) });
                    }
                    c_name
                } else {
                    if records.contains_key(v) {
                        return Err(CBackendError { message: format!(
                            "Record ABI return in '{}' passes Record to non-Record result", f.name,
                        ) });
                    }
                    let (actual, c_name) = values.get(v).ok_or_else(|| CBackendError {
                        message: format!(
                            "unemitted C return SSA v{} in function '{}' block bb{} at source span {:?}; refusing fabricated return",
                            v.0, f.name, b.id.0, b.span,
                        ),
                    })?;
                    // EV-197/198: unrefined integer SSA must preserve
                    // its exact public C return ABI width AND signedness.
                    // C's implicit conversions are not source/IR proof:
                    // u16 -> u8 loses bits, u64 -> i64 may reinterpret the
                    // high bit, and u8 -> u16 may conceal a forged Return.
                    // The sole unrefined cross-width exception is a known,
                    // locally derived nonnegative U32 literal whose complete
                    // value fits the destination ABI; never treat a parameter,
                    // Call, Binary or unproven Phi as constant provenance.
                    // Source-authenticated refined Returns keep their earlier
                    // independent guard-before-cast pathway below.
                    let required = map_type_to_ctype(&f.ret);
                    let return_key = source_name;
                    let integer_abi = |ty: CType| matches!(ty,
                        CType::U8 | CType::U16 | CType::U32 | CType::U64 |
                        CType::I8 | CType::I16 | CType::I32 | CType::I64);
                    let literal_fits = if *actual == CType::U32 {
                        let max = match required {
                            CType::U8 => Some(u8::MAX as u64),
                            CType::U16 => Some(u16::MAX as u64),
                            CType::U32 => Some(u32::MAX as u64),
                            CType::U64 => Some(u64::MAX),
                            CType::I8 => Some(i8::MAX as u64),
                            CType::I16 => Some(i16::MAX as u64),
                            CType::I32 => Some(i32::MAX as u64),
                            CType::I64 => Some(i64::MAX as u64),
                            _ => None,
                        };
                        max.is_some_and(|max| known_u32_constants.get(v)
                            .is_some_and(|n| *n <= max))
                    } else {
                        false
                    };
                    if !source_refined_return_bounds.contains_key(return_key)
                        && integer_abi(required) && integer_abi(*actual)
                        && *actual != required && !literal_fits
                    {
                        return Err(CBackendError {
                            message: format!(
                                "C return scalar type mismatch in '{}' block bb{}: SSA v{} is {:?}, declared ABI {:?}; no proven in-range literal or refined return boundary",
                                f.name, b.id.0, v.0, actual, required
                            ),
                        });
                    }
                    c_name
                };
                let mut return_expression = name.to_string();
                // A source-qualified refined return must be tested against
                // the unconverted SSA value. Checking after the C return
                // cast would permit U32(277) -> U8(21) truncation.
                if let Some((lo, hi)) = source_refined_return_bounds.get(
                    source_name
                ) {
                    let (actual, value_name) = values.get(v).ok_or_else(|| CBackendError {
                        message: format!("refined return '{}' has no scalar SSA v{}", f.name, v.0),
                    })?;
                    let (max, wide_return) = match map_type_to_ctype(&f.ret) {
                        CType::U8 => (u8::MAX as u64, false),
                        CType::U16 => (u16::MAX as u64, false),
                        CType::U32 => (u32::MAX as u64, false),
                        CType::U64 | CType::USize => (u64::MAX, true),
                        CType::I8 => (i8::MAX as u64, false),
                        CType::I16 => (i16::MAX as u64, false),
                        CType::I32 => (i32::MAX as u64, false),
                        CType::I64 | CType::ISize => (i64::MAX as u64, true),
                        other => return Err(CBackendError {
                            message: format!("unqualified refined return '{}' ABI {:?}", f.name, other),
                        }),
                    };
                    if lo > hi || *hi > max {
                        return Err(CBackendError {
                            message: format!("refined return '{}' range [{lo}..{hi}] exceeds ABI", f.name),
                        });
                    }
                    let wide_value = matches!(actual, CType::U64 | CType::USize | CType::I64 | CType::ISize);
                    if !matches!(actual, CType::U8 | CType::U16 | CType::U32 |
                        CType::U64 | CType::USize | CType::I8 | CType::I16 |
                        CType::I32 | CType::I64 | CType::ISize)
                    {
                        return Err(CBackendError {
                            message: format!("unqualified refined return '{}' scalar {:?}", f.name, actual),
                        });
                    }
                    // Emit a *deliberate* cast only after the preceding
                    // runtime guard. Without the cast, Clang -Werror correctly
                    // rejects a constant U32(277) -> U8 conversion at compile
                    // time before we can exercise Aura's own range boundary.
                    if *actual != map_type_to_ctype(&f.ret) {
                        let target = map_ctype_decl(map_type_to_ctype(&f.ret));
                        return_expression = format!("(({target})({value_name}))");
                    }
                    let label = escape_c_string(&format!("{}.return", f.name));
                    if wide_value || wide_return {
                        out.push_str(&format!(
                            "  AURA_RANGE_CHECK_U64((uint64_t)({value_name}), {lo}ULL, {hi}ULL, \"{label}\");\n"
                        ));
                    } else {
                        out.push_str(&format!(
                            "  AURA_RANGE_CHECK_U32((uint32_t)({value_name}), {lo}u, {hi}u, \"{label}\");\n"
                        ));
                    }
                }
                out.push_str("  return ");
                out.push_str(&return_expression);
                out.push_str(";\n");
            }
            Terminator::Return(None) => {
                out.push_str("  return;\n");
            }
            Terminator::Br(t) => {
                emit_phi_assigns(out, &phi_inj, &values, &records, b.id, *t)?;
                out.push_str("  goto ");
                out.push_str(&bb_label(*t));
                out.push_str(";\n");
            }
            Terminator::CondBr {
                cond,
                then_bb,
                else_bb,
            } => {
                let cn = values
                    .get(cond)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_else(|| format!("v{}", cond.0));
                out.push_str("  if (");
                out.push_str(&cn);
                out.push_str(") {\n");
                emit_phi_assigns(out, &phi_inj, &values, &records, b.id, *then_bb)?;
                out.push_str("    goto ");
                out.push_str(&bb_label(*then_bb));
                out.push_str(";\n");
                out.push_str("  } else {\n");
                emit_phi_assigns(out, &phi_inj, &values, &records, b.id, *else_bb)?;
                out.push_str("    goto ");
                out.push_str(&bb_label(*else_bb));
                out.push_str(";\n");
                out.push_str("  }\n");
            }
            Terminator::Switch {
                scrut,
                default_bb,
                cases,
            } => {
                let sn = values
                    .get(scrut)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_else(|| format!("v{}", scrut.0));

                out.push_str("  switch (");
                out.push_str(&sn);
                out.push_str(") {\n");

                for (k, bb2) in cases {
                    out.push_str(&format!("    case {}u: ", k));
                    emit_phi_assigns(out, &phi_inj, &values, &records, b.id, *bb2)?;
                    out.push_str("goto ");
                    out.push_str(&bb_label(*bb2));
                    out.push_str(";\n");
                }

                out.push_str("    default: ");
                emit_phi_assigns(out, &phi_inj, &values, &records, b.id, *default_bb)?;
                out.push_str("goto ");
                out.push_str(&bb_label(*default_bb));
                out.push_str(";\n");

                out.push_str("  }\n");
            }
        }
    }

    out.push_str("}\n");
    Ok(())
}

fn emit_rvalue_decl(dest: ValueId, _name: &str, rv: &RValue) -> (CType, String) {
    let var = format!("v{}", dest.0);
    match rv {
        RValue::ConstU32(n) => (
            CType::U32,
            format!("const uint32_t {var} = {n}u;"),
        ),
        RValue::ConstI64(n) => (
            CType::I64,
            // Spelling -9223372036854775808LL makes the positive token
            // exceed signed long long before unary minus is applied;
            // strict Clang rejects it. INT64_MIN is representable and
            // defined by the already-included <stdint.h>.
            if *n == i64::MIN {
                format!("const int64_t {var} = INT64_MIN;")
            } else {
                format!("const int64_t {var} = {n}LL;")
            },
        ),
        RValue::ConstF64(f) => (
            CType::F64,
            // Preserve the actual SSA f64 bit pattern, including -0,
            // subnormal minima, infinities and quiet NaN payloads.
            // Decimals can be rejected, rounded or canonicalized by C.
            format!(
                "const double {var} = aura_f64_from_bits(UINT64_C(0x{:016x}));",
                f.to_bits()
            ),
        ),
        RValue::ConstChar(c) => (
            CType::Char,
            format!("const uint32_t {var} = {}u;", *c as u32),
        ),
        RValue::ConstBool(b) => (
            CType::Bool,
            format!("const bool {var} = {};", if *b { "true" } else { "false" }),
        ),
        RValue::ConstString(s) => (
            CType::CString,
            format!("const char* {var} = \"{}\";", escape_c_string(s)),
        ),
        RValue::Local(v) => (
            CType::Tensor,
            format!("/* alias */ Tensor {var} = v{};", v.0),
        ),
    }
}

fn push_c_printf_literal(out: &mut String, text: &str) {
    // Preserve exactly the same UTF-8 and control-byte semantics as a
    // normal C String literal. Only '%' is special for printf: a literal
    // percent must never become a format specifier. The octal sequences
    // emitted by escape_c_string() are fixed width, so a subsequent
    // decimal or hexadecimal source character cannot extend an escape.
    out.push_str(&escape_c_string(text).replace('%', "%%"));
}

#[derive(Clone, Debug)]
enum CapturedStringSlot {
    Scalar(CType, String),
    NominalRecord(String),
    EarlierString(Box<StringSnapshot>),
}

// Source String literals freeze their interpolation environment at creation,
// as the AVM does. No public IR or C ABI representation is changed.
#[derive(Clone, Debug)]
struct StringSnapshot {
    template: String,
    slots: HashMap<String, CapturedStringSlot>,
}
impl StringSnapshot {
    fn has_substitutions(&self) -> bool { !self.slots.is_empty() }
}

fn capture_string_snapshot(
    template: &str,
    named_values: &HashMap<String, (CType, String)>,
    named_record_types: &HashMap<String, String>,
    earlier_strings: &HashMap<String, StringSnapshot>,
) -> StringSnapshot {
    let mut slots = HashMap::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else { break };
        let key = &after[..close];
        if !slots.contains_key(key) {
            let slot = if let Some(earlier) = earlier_strings.get(key) {
                Some(CapturedStringSlot::EarlierString(Box::new(earlier.clone())))
            } else if let Some((ctype, c_name)) = named_values.get(key) {
                Some(CapturedStringSlot::Scalar(*ctype, c_name.clone()))
            } else {
                named_record_types.get(key).cloned().map(CapturedStringSlot::NominalRecord)
            };
            if let Some(slot) = slot { slots.insert(key.to_string(), slot); }
        }
        rest = &after[close + 1..];
    }
    StringSnapshot { template: template.to_string(), slots }
}

fn render_string_snapshot(
    snapshot: &StringSnapshot,
    format_string: &mut String,
    arguments: &mut Vec<String>,
) -> bool {
    let template = &snapshot.template;
    let mut cursor = 0usize;
    let mut replaced_any = false;
    while cursor < template.len() {
        let Some(open_rel) = template[cursor..].find('{') else {
            push_c_printf_literal(format_string, &template[cursor..]);
            break;
        };
        let open = cursor + open_rel;
        push_c_printf_literal(format_string, &template[cursor..open]);
        let after_open = open + 1;
        let Some(close_rel) = template[after_open..].find('}') else {
            push_c_printf_literal(format_string, &template[open..]);
            break;
        };
        let close = after_open + close_rel;
        let key = &template[after_open..close];
        match snapshot.slots.get(key) {
            Some(CapturedStringSlot::NominalRecord(record)) => {
                push_c_printf_literal(format_string, &format!("<{record}>"));
                replaced_any = true;
            }
            Some(CapturedStringSlot::EarlierString(earlier)) => {
                render_string_snapshot(earlier, format_string, arguments);
                // Even a plain earlier string was evaluated as a value,
                // rather than being reparsed in the later environment.
                replaced_any = true;
            }
            Some(CapturedStringSlot::Scalar(ct, c_name)) => {
                // A Char can encode to one-to-four UTF-8 bytes, including
                // the NUL scalar. Emit two real variadic arguments rather
                // than hiding a comma inside one argument string.
                if *ct == CType::Char {
                    format_string.push_str("%c%s");
                    arguments.push(format!(
                        "aura_unicode_char_lead((uint32_t)({c_name}))"
                    ));
                    arguments.push(format!(
                        "aura_unicode_char_tail((uint32_t)({c_name}), (char[4]){{0}})"
                    ));
                    replaced_any = true;
                    cursor = close + 1;
                    continue;
                }
                let replacement = match *ct {
                    CType::U8 | CType::U16 | CType::U32 =>
                        Some(("%u", format!("(unsigned int)({c_name})"))),
                    CType::U64 =>
                        Some(("%llu", format!("(unsigned long long)({c_name})"))),
                    CType::I8 | CType::I16 | CType::I32 =>
                        Some(("%d", format!("(int)({c_name})"))),
                    CType::I64 => Some(("%lld", format!("(long long)({c_name})"))),
                    // An f32 is stored exactly as a C float in qualified
                    // nominal Record fields. Widen to double *after* the
                    // float rounding, then print its exact numeric value by
                    // the same native formatter used for the AVM's f64
                    // storage. Do not silently leave a valid {f32} slot
                    // unexpanded or let printf's default %f truncate it.
                    CType::F32 | CType::F64 => Some(("%s", format!(
                        "aura_f64_display((double)({c_name}), (char[400]){{0}})"
                    ))),
                    CType::Bool => Some(("%s", format!("(({c_name}) ? \"true\" : \"false\")"))),
                    CType::CString => Some(("%s", format!("({c_name})"))),
                    _ => None,
                };
                if let Some((specifier, argument)) = replacement {
                    format_string.push_str(specifier);
                    arguments.push(argument);
                    replaced_any = true;
                } else {
                    push_c_printf_literal(format_string, &template[open..close + 1]);
                }
            }
            None => {
                // An unresolved name stays unresolved, even when a
                // declaration of that spelling appears before io.println.
                push_c_printf_literal(format_string, &template[open..close + 1]);
            }
        }
        cursor = close + 1;
    }
    replaced_any
}

fn emit_interpolated_println(out: &mut String, snapshot: &StringSnapshot) -> bool {
    let mut format_string = String::with_capacity(snapshot.template.len() + 8);
    let mut arguments = Vec::new();
    if !render_string_snapshot(snapshot, &mut format_string, &mut arguments) {
        return false;
    }
    out.push_str("  printf(\"");
    out.push_str(&format_string);
    out.push_str("\\n\"");
    for argument in arguments {
        out.push_str(", ");
        out.push_str(&argument);
    }
    out.push_str(");\n");
    true
}

fn emit_call(
    out: &mut String,
    dest: Option<ValueId>,
    callee: &str,
    args: &[ValueId],
    values: &mut HashMap<ValueId, (CType, String)>,
    records: &HashMap<ValueId, (String, String)>,
    ret_map: &HashMap<String, CType>,
    string_literals: &HashMap<ValueId, StringSnapshot>,
    c_names: &CNameMap,
) {
    let c_fn = c_names.callee(callee);

    if c_fn == "aura_io_println" && args.len() == 1 {
        if let Some(snapshot) = string_literals.get(&args[0]) {
            if emit_interpolated_println(out, snapshot) {
                // The template SSA is deliberately retained because the
                // interpolation dependency is not first-class in IR/DCE.
                // This printf substitutes the template; its original C
                // const-string binding must still count as used under
                // strict -Wall -Werror, without discarding other consumers.
                if let Some((_, source_name)) = values.get(&args[0]) {
                    out.push_str(&format!("  (void)({source_name});\n"));
                }
                return;
            }
        }
    }

    let ret = ret_map.get(callee).copied().unwrap_or_else(|| builtin_return_ctype(&c_fn));

    if let Some(d) = dest {
        if ret == CType::Void {
            out.push_str("  ");
        } else {
            let decl = map_ctype_decl(ret);
            out.push_str("  ");
            out.push_str(decl);
            out.push(' ');
            out.push_str(&format!("v{} = ", d.0));
        }

        out.push_str(&c_fn);
        out.push('(');
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            // Record formal parameters use their C identifier (e.g. p),
            // not a fabricated SSA temporary. Preserve the nominal C ABI.
            let emitted = values.get(a).map(|(_, n)| n.clone())
                .or_else(|| records.get(a).map(|(_, n)| n.clone()))
                .unwrap_or_else(|| format!("v{}", a.0));
            out.push_str(&emitted);
        }
        out.push_str(");\n");

        if ret != CType::Void {
            values.insert(d, (ret, format!("v{}", d.0)));
            // A Call is side-effecting and may be intentionally retained by
            // DCE after its SSA result becomes dead. Keep the call and its
            // SSA name for any live consumers, but mark the scalar result as
            // consumed in portable C so strict -Wall -Werror does not reject
            // source-valid optimized Aura with an otherwise unused result.
            // The cast is side-effect-free and optimized away.
            out.push_str(&format!("  (void)v{};\n", d.0));
        }
    } else {
        out.push_str("  ");
        out.push_str(&c_fn);
        out.push('(');
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            // Record formal parameters use their C identifier (e.g. p),
            // not a fabricated SSA temporary. Preserve the nominal C ABI.
            let emitted = values.get(a).map(|(_, n)| n.clone())
                .or_else(|| records.get(a).map(|(_, n)| n.clone()))
                .unwrap_or_else(|| format!("v{}", a.0));
            out.push_str(&emitted);
        }
        out.push_str(");\n");
    }
}

fn emit_kernel(
    out: &mut String,
    dest: Option<ValueId>,
    callee: &str,
    args: &[ValueId],
    values: &mut HashMap<ValueId, (CType, String)>,
    c_names: &CNameMap,
) {
    // Prototype supports the common case: Tensor f(Tensor, u32)
    // args[0] is Tensor, args[1] is u32.
    let c_fn = c_names.callee(callee);

    if let Some(d) = dest {
        out.push_str("  Tensor ");
        out.push_str(&format!("v{} = ", d.0));
        if args.len() == 2 {
            out.push_str("aura_async_tensor2(");
            out.push_str(&c_fn);
            out.push_str(", ");
            out.push_str(&values.get(&args[0]).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("v{}", args[0].0)));
            out.push_str(", ");
            out.push_str(&values.get(&args[1]).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("v{}", args[1].0)));
            out.push_str(")");
        } else {
            // Fallback: run synchronously
            out.push_str(&c_fn);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&values.get(a).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("v{}", a.0)));
            }
            out.push(')');
        }
        out.push_str(";\n");
        values.insert(d, (CType::Tensor, format!("v{}", d.0)));
    }
}

fn map_type(ty: &Type) -> &'static str {
    match ty {
        Type::Unit => "void",
        Type::Bool => "bool",
        Type::Char => "uint32_t",
        Type::U8 => "uint8_t",
        Type::U16 => "uint16_t",
        Type::U32 => "uint32_t",
        Type::U64 => "uint64_t",
        Type::U128 => "aura_u128_t",
        Type::USize => "size_t",
        Type::I8 => "int8_t",
        Type::I16 => "int16_t",
        Type::I32 => "int32_t",
        Type::I64 => "int64_t",
        Type::I128 => "aura_i128_t",
        Type::ISize => "ptrdiff_t",
        Type::F32 => "float",
        Type::F64 => "double",
        Type::Str | Type::String => "const char*",
        Type::Tensor => "Tensor",
        Type::Ptr(_) => "void*",
        Type::Record { .. } | Type::Enum { .. } | Type::Opaque(_) => "Tensor",
    }
}

fn map_type_to_ctype(ty: &Type) -> CType {
    match ty {
        Type::Unit => CType::Void,
        Type::Bool => CType::Bool,
        Type::Char => CType::Char,
        Type::U8 => CType::U8,
        Type::U16 => CType::U16,
        Type::U32 => CType::U32,
        Type::U64 => CType::U64,
        Type::U128 => CType::U128,
        Type::USize => CType::USize,
        Type::I8 => CType::I8,
        Type::I16 => CType::I16,
        Type::I32 => CType::I32,
        Type::I64 => CType::I64,
        Type::I128 => CType::I128,
        Type::ISize => CType::ISize,
        Type::F32 => CType::F32,
        Type::F64 => CType::F64,
        Type::Str | Type::String => CType::CString,
        Type::Tensor | Type::Ptr(_) | Type::Record { .. } | Type::Enum { .. } | Type::Opaque(_) => CType::Tensor,
    }
}

fn bb_label(id: BlockId) -> String {
    format!("bb{}", id.0)
}

fn map_ctype_decl(ct: CType) -> &'static str {
    match ct {
        CType::Void => "void",
        CType::Bool => "bool",
        CType::Char => "uint32_t",
        CType::U8 => "uint8_t",
        CType::U16 => "uint16_t",
        CType::U32 => "uint32_t",
        CType::U64 => "uint64_t",
        CType::U128 => "aura_u128_t",
        CType::USize => "size_t",
        CType::I8 => "int8_t",
        CType::I16 => "int16_t",
        CType::I32 => "int32_t",
        CType::I64 => "int64_t",
        CType::I128 => "aura_i128_t",
        CType::ISize => "ptrdiff_t",
        CType::F32 => "float",
        CType::F64 => "double",
        CType::CString => "const char*",
        CType::Tensor => "Tensor",
    }
}

fn emit_binop(
    op: BinOp, lhs_type: CType, rhs_type: CType, l: &str, r: &str,
) -> Result<(CType, String), CBackendError> {
    use BinOp::*;
    // Strings must compare by UTF-8 value, not C pointer identity. The
    // existing ABI does not qualify pointer ordering or mixed String types.
    if lhs_type == CType::CString || rhs_type == CType::CString {
        if lhs_type != CType::CString || rhs_type != CType::CString
            || !matches!(op, Eq | Ne)
        {
            return Err(CBackendError {
                message: format!("unqualified C String binary operation: {op:?} {lhs_type:?} vs {rhs_type:?}"),
            });
        }
        let equal = format!("aura_string_content_equal({l}, {r})");
        return Ok((CType::Bool, if matches!(op, Ne) {
            format!("!({equal})")
        } else { equal }));
    }
    // The IR presently carries no explicit per-instruction result type.
    // Do not use C's implicit mixed-type conversions or fabricate U32:
    // the supported arithmetic subset requires matching numeric operands.
    let arithmetic = matches!(op, Add | Sub | Mul);
    if arithmetic {
        if lhs_type != rhs_type || !matches!(
            lhs_type,
            CType::U32 | CType::U64 | CType::I32 | CType::I64
                | CType::F32 | CType::F64
        ) {
            return Err(CBackendError {
                message: format!(
                    "unqualified C numeric arithmetic: {op:?} {lhs_type:?} vs {rhs_type:?}"
                ),
            });
        }
        let symbol = match op { Add => "+", Sub => "-", Mul => "*", _ => unreachable!() };
        // Signed overflow in C is UB. Perform the exact-width modular
        // operation as an unsigned value, then decode the result without
        // implementation-defined out-of-range casts to a signed integer.
        if matches!(lhs_type, CType::I32 | CType::I64) {
            let (unsigned, wrapped) = if lhs_type == CType::I32 {
                ("uint32_t", "aura_i32_from_wrapped_bits")
            } else {
                ("uint64_t", "aura_i64_from_wrapped_bits")
            };
            return Ok((lhs_type, format!(
                "{wrapped}((({unsigned})({l})) {symbol} (({unsigned})({r})))"
            )));
        }
        return Ok((lhs_type, format!("({l}) {symbol} ({r})")));
    }

    // Fail closed on forged or otherwise unqualified typed IR. C silently
    // permits mixed signed/unsigned comparisons, truthiness in &&/||, and
    // some unsupported operand categories. The IR Oracle does not interpret
    // these implicit C conversions as Aura semantics. No AST/IR wire format
    // or valid same-type numeric/Boolean comparison ABI is changed.
    let qualified = match op {
        Eq | Ne => lhs_type == rhs_type && matches!(
            lhs_type, CType::Bool | CType::U8 | CType::U16 |
                CType::U32 | CType::U64 | CType::I8 | CType::I16 |
                CType::I32 | CType::I64 | CType::F32 | CType::F64
        ),
        Lt | Gt | Le | Ge => lhs_type == rhs_type && matches!(
            lhs_type, CType::U8 | CType::U16 | CType::U32 |
                CType::U64 | CType::I8 | CType::I16 | CType::I32 |
                CType::I64 | CType::F32 | CType::F64
        ),
        And | Or => lhs_type == CType::Bool && rhs_type == CType::Bool,
        _ => true, // Arithmetic and division have their own exact-type guards.
    };
    if !qualified {
        return Err(CBackendError {
            message: format!(
                "unqualified C comparison/logical operands: {op:?} {lhs_type:?} vs {rhs_type:?}"
            ),
        });
    }

    let pair = match op {
        // Preserve the original bounded U32 division rule ONLY for U32.
        // Non-U32 division needs a width/type-specific contract: never
        // cast its divisor to u32 as the original backend did.
        Div if lhs_type == CType::U32 && rhs_type == CType::U32 => (
            CType::U32,
            format!(
                "(AURA_RANGE_CHECK_U32((uint32_t)({r}), 1u, 0xFFFFFFFFu, \"divisor\"), ({l}) / ({r}))"
            ),
        ),
        // Full-width unsigned 64-bit division. The existing U64 Record/C
        // ABI and exact-width Add/Sub/Mul are already qualified; this branch
        // closes its missing Div operation without widening source Checker
        // inference or accepting mixed numeric types.
        Div if lhs_type == CType::U64 && rhs_type == CType::U64 => (
            CType::U64,
            format!("aura_u64_div_checked({l}, {r})"),
        ),
        // Typed i64 has a well-defined signed quotient only outside the
        // two C/Rust trap cases. This is NOT a source-level signed-return
        // inference change; all previously unsupported types still refuse.
        Div if lhs_type == CType::I64 && rhs_type == CType::I64 => (
            CType::I64,
            format!("aura_i64_div_checked({l}, {r})"),
        ),
        // IEEE-754 binary64 division defines the zero-divisor and overflow
        // outcomes (+/-Infinity, NaN, signed zero). Unlike integer division
        // these are valid results, not integer C UB or runtime traps. The
        // emitted runtime already asserts the native double/binary64 ABI.
        // Restrict to exactly F64/F64; source Float expression inference is
        // a separate cross-agent contract, not implicitly authorized here.
        Div if lhs_type == CType::F64 && rhs_type == CType::F64 => (
            CType::F64,
            format!("({l}) / ({r})"),
        ),
        Div => return Err(CBackendError {
            message: format!(
                "unqualified C numeric division: {lhs_type:?} vs {rhs_type:?}"
            ),
        }),
        Eq => (CType::Bool, format!("({l}) == ({r})")),
        Ne => (CType::Bool, format!("({l}) != ({r})")),
        Lt => (CType::Bool, format!("({l}) < ({r})")),
        Gt => (CType::Bool, format!("({l}) > ({r})")),
        Le => (CType::Bool, format!("({l}) <= ({r})")),
        Ge => (CType::Bool, format!("({l}) >= ({r})")),
        And => (CType::Bool, format!("({l}) && ({r})")),
        Or => (CType::Bool, format!("({l}) || ({r})")),
        _ => unreachable!("arithmetic handled above"),
    };
    Ok(pair)
}

fn emit_phi_assigns(
    out: &mut String,
    phi_inj: &HashMap<(BlockId, BlockId), Vec<(ValueId, ValueId)>>,
    values: &HashMap<ValueId, (CType, String)>,
    records: &HashMap<ValueId, (String, String)>,
    pred: BlockId,
    target: BlockId,
) -> Result<(), CBackendError> {
    if let Some(assigns) = phi_inj.get(&(pred, target)) {
        // Phi assignments on the same CFG edge are simultaneous. Staging
        // every source before writing any destination preserves loop swaps
        // and cyclic Record Phis; direct sequential C assignments do not.
        // Use a compound statement so declarations are also legal after a
        // C switch case label, without leaking their scope to other edges.
        let mut used_names: BTreeSet<String> = values.values()
            .map(|(_, name)| name.clone())
            .chain(records.values().map(|(_, name)| name.clone()))
            .collect();
        let mut staged = Vec::with_capacity(assigns.len());
        out.push_str("    {\n");
        for (dest, incoming) in assigns {
            if !values.contains_key(dest) && !records.contains_key(dest) {
                return Err(CBackendError { message: format!(
                    "unqualified Phi edge {:?}->{:?}: unknown destination v{}",
                    pred, target, dest.0
                ) });
            }
            let (decl, source) = if let Some((ty, name)) = values.get(incoming) {
                (map_ctype_decl(*ty).to_string(), name.as_str())
            } else if let Some((record, name)) = records.get(incoming) {
                (record_type_symbol(record), name.as_str())
            } else {
                return Err(CBackendError { message: format!(
                    "unqualified Phi edge {:?}->{:?}: unknown incoming v{}",
                    pred, target, incoming.0
                ) });
            };
            let mut temp = format!("aura_phi_snapshot_{}_{}_{}", pred.0, target.0, dest.0);
            while !used_names.insert(temp.clone()) {
                temp.push('_');
            }
            out.push_str(&format!("      {decl} {temp} = {source};\n"));
            staged.push((*dest, temp));
        }
        for (dest, temp) in staged {
            out.push_str(&format!("      v{} = {temp};\n", dest.0));
        }
        out.push_str("    }\n");
    }
    Ok(())
}

fn builtin_return_ctype(c_fn: &str) -> CType {
    match c_fn {
        "aura_io_println" => CType::Void,
        "io_load_tensor" => CType::Tensor,
        "io_display" => CType::Void,
        "compute_gradient" => CType::Tensor,
        _ => CType::Tensor,
    }
}

fn map_callee(name: &str) -> String {
    match name {
        // Match LLVM backend's builtin ABI mapping.
        "io.println" => "aura_io_println".to_string(),
        _ => {
            // Convert Aura module-style calls `io.load_tensor` -> `io_load_tensor`
            name.replace('.', "_")
        }
    }
}

fn escape_c_string(s: &str) -> String {
    // Emitted C must be strictly valid even when Aura source uses \n,
    // \r, \t, \u{...}, literal quotation marks and control characters.
    // Escape raw UTF-8 bytes (not Unicode code points) to make the
    // resulting C source encoding-independent and preserve exact bytes.
    // Exactly three octal digits also prevent C escape greediness.
    let mut escaped = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'\\' => escaped.push_str("\\\\"),
            b'"' => escaped.push_str("\\\""),
            b'\n' => escaped.push_str("\\n"),
            b'\r' => escaped.push_str("\\r"),
            b'\t' => escaped.push_str("\\t"),
            0x20..=0x7E => escaped.push(byte as char),
            _ => escaped.push_str(&format!("\\{:03o}", byte)),
        }
    }
    escaped
}

#[cfg(test)]
mod emit_tests {
    use super::*;
    use aura_ir::{BasicBlock, ExecutionHint, FunctionIR, Inst, InstKind, ModuleIR, Param, RValue, Terminator, Type, ValueId};
    use aura_ast::Span;
    use std::collections::BTreeMap;

    #[test]
    fn test_c_emission_primitives_and_typedefs() {
        let mut module = ModuleIR {
            functions: BTreeMap::new(),
            externs: BTreeMap::new(),
            record_layouts: BTreeMap::new(),
            refined_param_bounds: BTreeMap::new(),
            refined_return_bounds: BTreeMap::new(),
        };

        let dummy_span: Span = (0, 0).into();
        let func = FunctionIR {
            name: "test_fn".to_string(),
            span: dummy_span,
            params: vec![
                Param {
                    name: "x".to_string(),
                    ty: Type::U32,
                    span: dummy_span,
                    value: ValueId(0),
                },
                Param {
                    name: "y".to_string(),
                    ty: Type::I64,
                    span: dummy_span,
                    value: ValueId(1),
                },
            ],
            ret: Type::U32,
            entry: BlockId(0),
            blocks: vec![BasicBlock {
                id: BlockId(0),
                span: dummy_span,
                hint: ExecutionHint::Sequential,
                insts: vec![
                    Inst {
                        span: dummy_span,
                        dest: Some(ValueId(2)),
                        kind: InstKind::BindStrand {
                            name: "v2".to_string(),
                            expr: RValue::ConstU32(42),
                        },
                    },
                ],
                term: Terminator::Return(Some(ValueId(2))),
            }],
        };

        module.functions.insert("test_fn".to_string(), func);

        let artifacts = emit_module(&module, None).expect("emit module");
        assert!(artifacts.runtime_h.contains("aura_u128_t"));
        assert!(artifacts.runtime_h.contains("aura_i128_t"));
        assert!(artifacts.module_c.contains("uint32_t test_fn(uint32_t x, int64_t y)"));
    }
}
