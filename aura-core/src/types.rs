#![forbid(unsafe_code)]

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Unknown,
    Unit,
    Bool,
    Char,

    // Unsigned integer primitives
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,

    // Signed integer primitives
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,

    // Floating-point primitives
    F32,
    F64,

    // Text & String primitives
    Str,
    String,

    // Domain & Builtins
    Style,
    Model,
    Tensor {
        elem: Box<Type>,
        shape: Option<Vec<u64>>,
    },

    // Reference / Borrow (Option B+)
    Ref {
        mutable: bool,
        inner: Box<Type>,
        region: Option<String>,
    },

    // Composite Types
    Tuple(Vec<Type>),
    Record {
        name: String,
        fields: Vec<(String, Type)>,
    },
    Enum {
        name: String,
        variants: Vec<(String, Vec<Type>)>,
    },

    // Named alias (kept for diagnostics); typically resolved before checking.
    Named(String),

    // Nominal type with type arguments (e.g. `Option<u32>`).
    Applied {
        name: String,
        args: Vec<Type>,
    },

    ConstrainedRange {
        base: Box<Type>,
        lo: u64,
        hi: u64,
    },
}

impl Type {
    pub fn display(&self) -> String {
        match self {
            Type::Unknown => "<unknown>".to_string(),
            Type::Unit => "Unit".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Char => "char".to_string(),
            Type::U8 => "u8".to_string(),
            Type::U16 => "u16".to_string(),
            Type::U32 => "u32".to_string(),
            Type::U64 => "u64".to_string(),
            Type::U128 => "u128".to_string(),
            Type::USize => "usize".to_string(),
            Type::I8 => "i8".to_string(),
            Type::I16 => "i16".to_string(),
            Type::I32 => "i32".to_string(),
            Type::I64 => "i64".to_string(),
            Type::I128 => "i128".to_string(),
            Type::ISize => "isize".to_string(),
            Type::F32 => "f32".to_string(),
            Type::F64 => "f64".to_string(),
            Type::Str => "str".to_string(),
            Type::String => "String".to_string(),
            Type::Style => "Style".to_string(),
            Type::Model => "Model".to_string(),
            Type::Tensor { elem, shape } => {
                let elem_s = elem.display();
                if let Some(dims) = shape {
                    let dims_s = dims
                        .iter()
                        .map(|d| d.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("Tensor<{elem_s}, [{dims_s}]>")
                } else {
                    format!("Tensor<{elem_s}>")
                }
            }
            Type::Ref { mutable, inner, region } => {
                let mut_prefix = if *mutable { "mut " } else { "" };
                if let Some(r) = region {
                    format!("&'{r} {mut_prefix}{}", inner.display())
                } else {
                    format!("&{mut_prefix}{}", inner.display())
                }
            }
            Type::Tuple(elems) => {
                let elems_s = elems.iter().map(|e| e.display()).collect::<Vec<_>>().join(", ");
                format!("({elems_s})")
            }
            Type::Record { name, .. } => name.clone(),
            Type::Enum { name, .. } => name.clone(),
            Type::Named(n) => n.clone(),
            Type::Applied { name, args } => {
                if args.is_empty() {
                    name.clone()
                } else {
                    let args_s = args
                        .iter()
                        .map(|t| t.display())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{name}<{args_s}>")
                }
            }
            Type::ConstrainedRange { base, lo, hi } => {
                format!("{}[{}..{}]", base.display(), lo, hi)
            }
        }
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::U128 | Type::USize
                | Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::I128 | Type::ISize
        )
    }

    pub fn is_signed_integer(&self) -> bool {
        matches!(
            self,
            Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::I128 | Type::ISize
        )
    }

    pub fn is_unsigned_integer(&self) -> bool {
        matches!(
            self,
            Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::U128 | Type::USize
        )
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Type::F32 | Type::F64)
    }

    pub fn is_numeric(&self) -> bool {
        self.is_integer() || self.is_float()
    }

    pub fn tensor_unknown() -> Self {
        Type::Tensor {
            elem: Box::new(Type::Unknown),
            shape: None,
        }
    }
}

pub fn is_subset_range(a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64) -> bool {
    a_lo >= b_lo && a_hi <= b_hi
}
