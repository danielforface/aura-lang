#![forbid(unsafe_code)]

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Unknown,
    Unit,
    Bool,
    U32,
    String,
    Style,
    Model,
    Tensor {
        elem: Box<Type>,
        shape: Option<Vec<u64>>,
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
            Type::U32 => "u32".to_string(),
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
}

impl Type {
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
