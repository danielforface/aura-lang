#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

use aura_ast::{
    AssignStmt, BinOp, Block, CallArg, CellDef, EnumDef, Expr, ExprKind, ExternCell, FlowBlock,
    Ident, IfStmt, MatchStmt, Pattern, Program, RecordDef, Span, Stmt, StrandDef, TraitDef,
    TypeArg, TypeRef, UnaryOp, WhileStmt,
};

use crate::error::SemanticError;
use crate::capability::CapabilityGraph;
use crate::types::{is_subset_range, Type};
use crate::verifier::{DummySolver, Verifier};

const U32_MAX: u64 = 0xFFFF_FFFF;

/// Ownership state tracking for linear type enforcement.
/// 
/// This enum tracks the lifecycle state of each variable to enforce
/// move semantics and prevent use-after-move errors.
///
/// Example progression:
/// ```
/// let model = ai.load();        // Owned
/// ai.infer(model, data);        // Consumed (moved into function)
/// ai.infer(model, data);        // ERROR: Consumed already
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]  // Borrowed and Returned variants used in future phases
enum OwnershipState {
    /// Value is owned and can be moved, borrowed, or used
    Owned,
    
    /// Value has been consumed via move or used in linear context.
    /// Subsequent uses are not permitted (unless type is Copy).
    Consumed,
    
    /// Value is borrowed (immutable reference exists).
    /// May be used for reads, but not moved.
    Borrowed,
    
    /// Value has been returned or transferred to caller.
    /// Ownership moved to calling scope.
    Returned,
}

fn base_type(ty: &Type) -> &Type {
    match ty {
        Type::ConstrainedRange { base, .. } => base_type(base),
        other => other,
    }
}

fn applied_name_and_args(ty: &Type) -> Option<(&str, &[Type])> {
    match ty {
        Type::Applied { name, args } => Some((name.as_str(), args.as_slice())),
        Type::Named(n) => Some((n.as_str(), &[])),
        _ => None,
    }
}

fn is_u32_like(ty: &Type) -> bool {
    matches!(base_type(ty), Type::U32)
}

fn u32_bounds(ty: &Type) -> Option<(u64, u64)> {
    match ty {
        Type::U32 => Some((0, U32_MAX)),
        Type::ConstrainedRange { base, lo, hi } if **base == Type::U32 => Some((*lo, *hi)),
        _ => None,
    }
}

fn mk_u32_range(lo: u64, hi: u64) -> Type {
    let lo = lo.min(U32_MAX);
    let hi = hi.min(U32_MAX);
    if lo == 0 && hi == U32_MAX {
        Type::U32
    } else {
        Type::ConstrainedRange {
            base: Box::new(Type::U32),
            lo,
            hi,
        }
    }
}

fn infer_u32_range_binop(op: &BinOp, lt: &Type, rt: &Type) -> Type {
    let Some((l_lo, l_hi)) = u32_bounds(lt) else {
        return Type::U32;
    };
    let Some((r_lo, r_hi)) = u32_bounds(rt) else {
        return Type::U32;
    };

    match op {
        BinOp::Add => mk_u32_range(l_lo.saturating_add(r_lo), l_hi.saturating_add(r_hi)),

        BinOp::Sub => {
            // Non-negative domain; conservative bounds.
            let lo = l_lo.saturating_sub(r_hi);
            let hi = l_hi.saturating_sub(r_lo);
            mk_u32_range(lo, hi)
        }

        BinOp::Mul => mk_u32_range(l_lo.saturating_mul(r_lo), l_hi.saturating_mul(r_hi)),

        BinOp::Div => {
            // If divisor can be 0, be conservative.
            if r_lo == 0 {
                return Type::U32;
            }
            let lo = l_lo / r_hi.max(1);
            let hi = l_hi / r_lo;
            mk_u32_range(lo, hi)
        }

        _ => Type::U32,
    }
}

#[derive(Clone, Debug)]
struct FnParam {
    name: String,
    ty: Type,
}

#[derive(Clone, Debug)]
struct FnSig {
    params: Vec<FnParam>,
    ret: Type,
}

#[derive(Clone, Debug)]
struct TypeAliasDef {
    params: Vec<TypeParamDef>,
    target: TypeRef,
}

#[derive(Clone, Debug)]
struct TypeParamDef {
    name: String,
    bound: Option<String>,
}

#[derive(Clone, Debug)]
enum AliasEntry {
    Mono(Type),
    Generic(TypeAliasDef),
}

pub struct Checker {
    type_aliases: HashMap<String, AliasEntry>,
    traits: HashSet<String>,
    record_defs: HashMap<String, RecordDef>,
    enum_defs: HashMap<String, EnumDef>,
    functions: HashMap<String, FnSig>,
    extern_cells: HashMap<String, bool>,
    // value scopes
    scopes: Vec<HashMap<String, Type>>,
    mut_scopes: Vec<HashSet<String>>,
    
    // Linear type enforcement: track ownership state of each variable
    ownership_states: Vec<HashMap<String, OwnershipState>>,

    // If true, accept assignments into constrained ranges without proving them here.
    // This is intended for the LLVM pipeline where `aura-verify` (Z3) is a hard gate.
    defer_range_proofs: bool,

    // Capability tracking (Phase 2)
    cap: CapabilityGraph,
    cap_next: u32,

    // Formal verification stub
    verifier: Verifier<DummySolver>,

    // Unsafe context depth for explicit FFI/trust boundaries.
    unsafe_depth: u32,
    // If non-empty, we're inside an async lambda; the value is the scope depth
    // at which the lambda started. Any mutable binding resolved from an outer
    // scope is an invalid capture.
    async_lambda_bases: Vec<usize>,
}

impl Checker {
    pub fn new() -> Self {
        let mut checker = Self {
            type_aliases: HashMap::new(),
            traits: HashSet::new(),
            record_defs: HashMap::new(),
            enum_defs: HashMap::new(),
            functions: HashMap::new(),
            extern_cells: HashMap::new(),
            scopes: vec![HashMap::new()],
            mut_scopes: vec![HashSet::new()],
            ownership_states: vec![HashMap::new()],
            defer_range_proofs: false,

            cap: CapabilityGraph::default(),
            cap_next: 0,
            verifier: Verifier::new(DummySolver),
            unsafe_depth: 0,
            async_lambda_bases: Vec::new(),
        };

        // Builtins (minimal; extend later)
        // --- io ---
        checker.functions.insert(
            "io.println".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "text".to_string(),
                    ty: Type::String,
                }],
                ret: Type::Unit,
            },
        );
        checker.functions.insert(
            "io.load_tensor".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "path".to_string(),
                    ty: Type::String,
                }],
                ret: Type::tensor_unknown(),
            },
        );
        checker.functions.insert(
            "io.display".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "t".to_string(),
                    ty: Type::tensor_unknown(),
                }],
                ret: Type::Unit,
            },
        );

        // AVM-only IO helpers (prototype)
        checker.functions.insert(
            "io.read_line".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "prompt".to_string(),
                    ty: Type::String,
                }],
                ret: Type::String,
            },
        );
        checker.functions.insert(
            "io.read_text".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "path".to_string(),
                    ty: Type::String,
                }],
                ret: Type::String,
            },
        );
        checker.functions.insert(
            "io.write_text".to_string(),
            FnSig {
                params: vec![
                    FnParam {
                        name: "path".to_string(),
                        ty: Type::String,
                    },
                    FnParam {
                        name: "text".to_string(),
                        ty: Type::String,
                    },
                ],
                ret: Type::Unit,
            },
        );

        // --- ui (prototype) ---
        checker.functions.insert(
            "ui.event_text".to_string(),
            FnSig {
                params: vec![],
                ret: Type::String,
            },
        );
        checker.functions.insert(
            "ui.get_text".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "key".to_string(),
                    ty: Type::String,
                }],
                ret: Type::String,
            },
        );
        checker.functions.insert(
            "ui.set_text".to_string(),
            FnSig {
                params: vec![
                    FnParam {
                        name: "key".to_string(),
                        ty: Type::String,
                    },
                    FnParam {
                        name: "value".to_string(),
                        ty: Type::String,
                    },
                ],
                ret: Type::Unit,
            },
        );

        // AVM-only persistence + app-state helpers (prototype)
        checker.functions.insert(
            "shop.path".to_string(),
            FnSig {
                params: vec![],
                ret: Type::String,
            },
        );
        checker.functions.insert(
            "shop.select".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "index".to_string(),
                    ty: Type::U32,
                }],
                ret: Type::Unit,
            },
        );
        checker.functions.insert(
            "shop.selection_status".to_string(),
            FnSig {
                params: vec![],
                ret: Type::String,
            },
        );
        checker.functions.insert(
            "shop.clear_selection".to_string(),
            FnSig {
                params: vec![],
                ret: Type::Unit,
            },
        );
        checker.functions.insert(
            "shop.has_selection".to_string(),
            FnSig {
                params: vec![],
                ret: Type::Bool,
            },
        );
        checker.functions.insert(
            "shop.selected_index".to_string(),
            FnSig {
                params: vec![],
                ret: Type::U32,
            },
        );
        checker.functions.insert(
            "shop.status".to_string(),
            FnSig {
                params: vec![],
                ret: Type::String,
            },
        );
        checker.functions.insert(
            "shop.is_pending".to_string(),
            FnSig {
                params: vec![],
                ret: Type::Bool,
            },
        );
        checker.functions.insert(
            "shop.cancel".to_string(),
            FnSig {
                params: vec![],
                ret: Type::Unit,
            },
        );
        checker.functions.insert(
            "shop.begin_add".to_string(),
            FnSig {
                params: vec![],
                ret: Type::Unit,
            },
        );
        checker.functions.insert(
            "shop.begin_edit".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "index".to_string(),
                    ty: Type::U32,
                }],
                ret: Type::Unit,
            },
        );
        checker.functions.insert(
            "shop.load".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "path".to_string(),
                    ty: Type::String,
                }],
                ret: Type::Unit,
            },
        );
        checker.functions.insert(
            "shop.save".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "path".to_string(),
                    ty: Type::String,
                }],
                ret: Type::Unit,
            },
        );
        checker.functions.insert(
            "shop.count".to_string(),
            FnSig {
                params: vec![],
                ret: Type::U32,
            },
        );
        checker.functions.insert(
            "shop.upsert".to_string(),
            FnSig {
                params: vec![
                    FnParam {
                        name: "name".to_string(),
                        ty: Type::String,
                    },
                    FnParam {
                        name: "qty".to_string(),
                        ty: Type::String,
                    },
                    FnParam {
                        name: "notes".to_string(),
                        ty: Type::String,
                    },
                ],
                ret: Type::Unit,
            },
        );
        checker.functions.insert(
            "shop.get_name".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "index".to_string(),
                    ty: Type::U32,
                }],
                ret: Type::String,
            },
        );
        checker.functions.insert(
            "shop.get_qty".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "index".to_string(),
                    ty: Type::U32,
                }],
                ret: Type::String,
            },
        );
        checker.functions.insert(
            "shop.get_notes".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "index".to_string(),
                    ty: Type::U32,
                }],
                ret: Type::String,
            },
        );
        checker.functions.insert(
            "shop.get_done".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "index".to_string(),
                    ty: Type::U32,
                }],
                ret: Type::Bool,
            },
        );
        checker.functions.insert(
            "shop.add".to_string(),
            FnSig {
                params: vec![
                    FnParam {
                        name: "name".to_string(),
                        ty: Type::String,
                    },
                    FnParam {
                        name: "qty".to_string(),
                        ty: Type::String,
                    },
                    FnParam {
                        name: "notes".to_string(),
                        ty: Type::String,
                    },
                ],
                ret: Type::Unit,
            },
        );
        checker.functions.insert(
            "shop.edit".to_string(),
            FnSig {
                params: vec![
                    FnParam {
                        name: "index".to_string(),
                        ty: Type::U32,
                    },
                    FnParam {
                        name: "name".to_string(),
                        ty: Type::String,
                    },
                    FnParam {
                        name: "qty".to_string(),
                        ty: Type::String,
                    },
                    FnParam {
                        name: "notes".to_string(),
                        ty: Type::String,
                    },
                ],
                ret: Type::Unit,
            },
        );
        checker.functions.insert(
            "shop.remove".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "index".to_string(),
                    ty: Type::U32,
                }],
                ret: Type::Unit,
            },
        );
        checker.functions.insert(
            "shop.toggle".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "index".to_string(),
                    ty: Type::U32,
                }],
                ret: Type::Unit,
            },
        );
        checker.functions.insert(
            "shop.clear".to_string(),
            FnSig {
                params: vec![],
                ret: Type::Unit,
            },
        );
        checker.functions.insert(
            "shop.clear_completed".to_string(),
            FnSig {
                params: vec![],
                ret: Type::Unit,
            },
        );

        // --- tensor ---
        checker.functions.insert(
            "tensor.new".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "len".to_string(),
                    ty: Type::U32,
                }],
                ret: Type::tensor_unknown(),
            },
        );
        checker.functions.insert(
            "tensor.len".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "t".to_string(),
                    ty: Type::tensor_unknown(),
                }],
                ret: Type::U32,
            },
        );
        checker.functions.insert(
            "tensor.get".to_string(),
            FnSig {
                params: vec![
                    FnParam {
                        name: "t".to_string(),
                        ty: Type::tensor_unknown(),
                    },
                    FnParam {
                        name: "idx".to_string(),
                        ty: Type::U32,
                    },
                ],
                ret: Type::U32,
            },
        );
        checker.functions.insert(
            "tensor.set".to_string(),
            FnSig {
                params: vec![
                    FnParam {
                        name: "t".to_string(),
                        ty: Type::tensor_unknown(),
                    },
                    FnParam {
                        name: "idx".to_string(),
                        ty: Type::U32,
                    },
                    FnParam {
                        name: "value".to_string(),
                        ty: Type::U32,
                    },
                ],
                ret: Type::Unit,
            },
        );

        // --- std::collections (prototype vector API; currently backed by tensor ops) ---
        checker.functions.insert(
            "collections.vector_new".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "len".to_string(),
                    ty: Type::U32,
                }],
                ret: Type::tensor_unknown(),
            },
        );
        checker.functions.insert(
            "collections.vector_len".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "v".to_string(),
                    ty: Type::tensor_unknown(),
                }],
                ret: Type::U32,
            },
        );
        checker.functions.insert(
            "collections.vector_get".to_string(),
            FnSig {
                params: vec![
                    FnParam {
                        name: "v".to_string(),
                        ty: Type::tensor_unknown(),
                    },
                    FnParam {
                        name: "idx".to_string(),
                        ty: Type::U32,
                    },
                ],
                ret: Type::U32,
            },
        );
        checker.functions.insert(
            "collections.vector_set".to_string(),
            FnSig {
                params: vec![
                    FnParam {
                        name: "v".to_string(),
                        ty: Type::tensor_unknown(),
                    },
                    FnParam {
                        name: "idx".to_string(),
                        ty: Type::U32,
                    },
                    FnParam {
                        name: "value".to_string(),
                        ty: Type::U32,
                    },
                ],
                ret: Type::Unit,
            },
        );

        // --- ai ---
        checker.functions.insert(
            "ai.load_model".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "path".to_string(),
                    ty: Type::String,
                }],
                ret: Type::Model,
            },
        );
        checker.functions.insert(
            "ai.infer".to_string(),
            FnSig {
                params: vec![
                    FnParam {
                        name: "model".to_string(),
                        ty: Type::Model,
                    },
                    FnParam {
                        name: "input".to_string(),
                        ty: Type::tensor_unknown(),
                    },
                ],
                ret: Type::tensor_unknown(),
            },
        );

        // --- demo compat ---
        checker.functions.insert(
            "compute_gradient".to_string(),
            FnSig {
                params: vec![FnParam {
                    name: "x".to_string(),
                    ty: Type::U32,
                }],
                ret: Type::tensor_unknown(),
            },
        );

        checker
    }

    pub(crate) fn is_void_function(&self, name: &str) -> bool {
        self.functions
            .get(name)
            .is_some_and(|sig| matches!(base_type(&sig.ret), Type::Unit))
    }

    pub(crate) fn function_ret_type(&self, name: &str) -> Option<&Type> {
        self.functions.get(name).map(|sig| &sig.ret)
    }

    pub fn function_param_names(&self, name: &str) -> Option<Vec<String>> {
        self.functions.get(name).map(|sig| {
            sig.params
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>()
        })
    }

    pub fn set_defer_range_proofs(&mut self, defer: bool) {
        self.defer_range_proofs = defer;
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), SemanticError> {
        // First pass: register type aliases and cell headers.
        for stmt in &program.stmts {
            match stmt {
                Stmt::Import(i) => {
                    self.handle_import(i)?;
                }
                Stmt::MacroDef(_) | Stmt::MacroCall(_) => {}
                Stmt::TraitDef(TraitDef { name, .. }) => {
                    self.traits.insert(name.node.clone());
                }
                Stmt::TypeAlias(ta) => {
                    if ta.params.is_empty() {
                        let ty = self.resolve_type_ref(&ta.target)?;
                        self.type_aliases
                            .insert(ta.name.node.clone(), AliasEntry::Mono(ty));
                    } else {
                        let def = TypeAliasDef {
                            params: ta
                                .params
                                .iter()
                                .map(|p| TypeParamDef {
                                    name: p.name.node.clone(),
                                    bound: p.bound.as_ref().map(|b| b.node.clone()),
                                })
                                .collect(),
                            target: ta.target.clone(),
                        };
                        self.type_aliases
                            .insert(ta.name.node.clone(), AliasEntry::Generic(def));
                    }
                }
                Stmt::RecordDef(r) => {
                    self.record_defs.insert(r.name.node.clone(), r.clone());
                    self.define_type_placeholder(&r.name)?;
                }
                Stmt::EnumDef(e) => {
                    self.enum_defs.insert(e.name.node.clone(), e.clone());
                    self.define_type_placeholder(&e.name)?;
                }
                Stmt::CellDef(cell) => {
                    let sig = self.signature_from_cell(cell)?;
                    self.functions.insert(cell.name.node.clone(), sig);
                }
                Stmt::ExternCell(ext) => {
                    let sig = self.signature_from_extern_cell(ext)?;
                    self.functions.insert(ext.name.node.clone(), sig);
                    self.extern_cells
                        .insert(ext.name.node.clone(), ext.trusted);
                }
                Stmt::UnsafeBlock(_) => {}
                _ => {}
            }
        }

        // Second pass: check bodies / top-level flow blocks.
        for stmt in &program.stmts {
            match stmt {
                Stmt::Import(i) => {
                    self.handle_import(i)?;
                }
                Stmt::MacroDef(_) | Stmt::MacroCall(_) => {}
                Stmt::TypeAlias(_) => {}
                Stmt::TraitDef(_) | Stmt::RecordDef(_) | Stmt::EnumDef(_) => {}
                Stmt::CellDef(cell) => {
                    self.check_cell(cell)?;
                }
                Stmt::ExternCell(_) => {}
                Stmt::UnsafeBlock(s) => {
                    self.unsafe_depth += 1;
                    let _ = self.check_block(&s.body)?;
                    self.unsafe_depth -= 1;
                }
                Stmt::Layout(lb) => {
                    self.check_layout_block(lb)?;
                }
                Stmt::Render(rb) => {
                    self.check_render_block(rb)?;
                }
                Stmt::Prop(p) => {
                    let _ = self.infer_expr(&p.expr)?;
                }
                Stmt::FlowBlock(fb) => {
                    self.check_flow_block(fb)?;
                }
                Stmt::StrandDef(sd) => {
                    self.check_strand(sd)?;
                }
                Stmt::Assign(a) => {
                    self.check_assign(a)?;
                }
                Stmt::If(i) => {
                    self.check_if(i)?;
                }
                Stmt::Match(m) => {
                    self.check_match(m)?;
                }
                Stmt::While(w) => {
                    self.check_while(w)?;
                }
                Stmt::Requires(r) => {
                    let ty = self.infer_expr(&r.expr)?;
                    if ty != Type::Bool {
                        return Err(SemanticError {
                            message: format!("requires expects bool, got {}", ty.display()),
                            span: r.span,
                        });
                    }
                }
                Stmt::Ensures(e) => {
                    let ty = self.infer_expr(&e.expr)?;
                    if ty != Type::Bool {
                        return Err(SemanticError {
                            message: format!("ensures expects bool, got {}", ty.display()),
                            span: e.span,
                        });
                    }
                }
                Stmt::Assert(a) => {
                    let ty = self.infer_expr(&a.expr)?;
                    if ty != Type::Bool {
                        return Err(SemanticError {
                            message: format!("assert expects bool, got {}", ty.display()),
                            span: a.span,
                        });
                    }
                }
                Stmt::Assume(a) => {
                    let ty = self.infer_expr(&a.expr)?;
                    if ty != Type::Bool {
                        return Err(SemanticError {
                            message: format!("assume expects bool, got {}", ty.display()),
                            span: a.span,
                        });
                    }
                }
                Stmt::ExprStmt(expr) => {
                    let _ = self.infer_expr(expr)?;
                }
            }
        }

        Ok(())
    }

    fn signature_from_cell(&mut self, cell: &CellDef) -> Result<FnSig, SemanticError> {
        let mut params = Vec::new();
        for p in &cell.params {
            params.push(FnParam {
                name: p.name.node.clone(),
                ty: self.resolve_type_ref(&p.ty)?,
            });
        }
        Ok(FnSig {
            params,
            ret: Type::Unknown,
        })
    }

    fn signature_from_extern_cell(&mut self, ext: &ExternCell) -> Result<FnSig, SemanticError> {
        let mut params = Vec::new();
        for p in &ext.params {
            params.push(FnParam {
                name: p.name.node.clone(),
                ty: self.resolve_type_ref(&p.ty)?,
            });
        }
        let ret = self.resolve_type_ref(&ext.ret)?;
        Ok(FnSig { params, ret })
    }

    fn check_cell(&mut self, cell: &CellDef) -> Result<(), SemanticError> {
        self.push_scope();
        for p in &cell.params {
            let ty = self.resolve_type_ref(&p.ty)?;
            self.define_val(&p.name, ty, p.mutable)?;
        }
        let ret_ty = self.check_block(&cell.body)?;
        self.pop_scope();

        // Update function return type.
        if let Some(sig) = self.functions.get_mut(&cell.name.node) {
            sig.ret = ret_ty;
        }

        Ok(())
    }

    fn check_flow_block(&mut self, fb: &FlowBlock) -> Result<(), SemanticError> {
        self.push_scope();
        let _ret = self.check_block(&fb.body)?;
        self.pop_scope();
        Ok(())
    }

    fn check_block(&mut self, block: &Block) -> Result<Type, SemanticError> {
        self.push_scope();
        for stmt in &block.stmts {
            match stmt {
                Stmt::Import(i) => self.handle_import(i)?,
                Stmt::MacroDef(m) => {
                    return Err(SemanticError {
                        message: "macros must be expanded before semantic analysis".to_string(),
                        span: m.span,
                    });
                }
                Stmt::MacroCall(m) => {
                    return Err(SemanticError {
                        message: "macros must be expanded before semantic analysis".to_string(),
                        span: m.span,
                    });
                }
                Stmt::StrandDef(sd) => self.check_strand(sd)?,
                Stmt::Layout(lb) => {
                    self.check_layout_block(lb)?;
                }
                Stmt::Render(rb) => {
                    self.check_render_block(rb)?;
                }
                Stmt::Prop(p) => {
                    let _ = self.infer_expr(&p.expr)?;
                }
                Stmt::Assign(a) => self.check_assign(a)?,
                Stmt::If(i) => self.check_if(i)?,
                Stmt::Match(m) => self.check_match(m)?,
                Stmt::While(w) => self.check_while(w)?,
                Stmt::Requires(r) => {
                    let ty = self.infer_expr(&r.expr)?;
                    if ty != Type::Bool {
                        return Err(SemanticError {
                            message: format!("requires expects bool, got {}", ty.display()),
                            span: r.span,
                        });
                    }
                }
                Stmt::Ensures(e) => {
                    let ty = self.infer_expr(&e.expr)?;
                    if ty != Type::Bool {
                        return Err(SemanticError {
                            message: format!("ensures expects bool, got {}", ty.display()),
                            span: e.span,
                        });
                    }
                }
                Stmt::Assert(a) => {
                    let ty = self.infer_expr(&a.expr)?;
                    if ty != Type::Bool {
                        return Err(SemanticError {
                            message: format!("assert expects bool, got {}", ty.display()),
                            span: a.span,
                        });
                    }
                }
                Stmt::Assume(a) => {
                    let ty = self.infer_expr(&a.expr)?;
                    if ty != Type::Bool {
                        return Err(SemanticError {
                            message: format!("assume expects bool, got {}", ty.display()),
                            span: a.span,
                        });
                    }
                }
                Stmt::ExprStmt(expr) => {
                    let _ = self.infer_expr(expr)?;
                }
                Stmt::TraitDef(_) | Stmt::RecordDef(_) | Stmt::EnumDef(_) => {
                    return Err(SemanticError {
                        message: "type/trait declarations are only allowed at top-level".to_string(),
                        span: block.span,
                    });
                }
                Stmt::TypeAlias(ta) => {
                    if ta.params.is_empty() {
                        let ty = self.resolve_type_ref(&ta.target)?;
                        self.type_aliases
                            .insert(ta.name.node.clone(), AliasEntry::Mono(ty));
                    } else {
                        let def = TypeAliasDef {
                            params: ta
                                .params
                                .iter()
                                .map(|p| TypeParamDef {
                                    name: p.name.node.clone(),
                                    bound: p.bound.as_ref().map(|b| b.node.clone()),
                                })
                                .collect(),
                            target: ta.target.clone(),
                        };
                        self.type_aliases
                            .insert(ta.name.node.clone(), AliasEntry::Generic(def));
                    }
                }
                Stmt::CellDef(_) | Stmt::FlowBlock(_) => {
                    return Err(SemanticError {
                        message: "nested cell/flow blocks are not supported (yet)".to_string(),
                        span: block.span,
                    });
                }
                Stmt::ExternCell(_) => {
                    return Err(SemanticError {
                        message: "extern cell declarations are only allowed at top-level".to_string(),
                        span: block.span,
                    });
                }
                Stmt::UnsafeBlock(s) => {
                    self.unsafe_depth += 1;
                    let _ = self.check_block(&s.body)?;
                    self.unsafe_depth -= 1;
                }
            }
        }

        let ret = match &block.yield_expr {
            Some(expr) => self.infer_expr(expr)?,
            None => Type::Unit,
        };
        self.pop_scope();
        Ok(ret)
    }

    fn check_layout_block(&mut self, lb: &aura_ast::LayoutBlock) -> Result<(), SemanticError> {
        let _ = self.check_block(&lb.body)?;
        Ok(())
    }

    fn check_render_block(&mut self, rb: &aura_ast::RenderBlock) -> Result<(), SemanticError> {
        let _ = self.check_block(&rb.body)?;
        Ok(())
    }

    fn handle_import(&mut self, import: &aura_ast::ImportStmt) -> Result<(), SemanticError> {
        // Prototype module handling:
        // - `import aura::io` registers `io` as a module name in the current scope.
        // - `import aura::tensor` registers `tensor` as a module name.
        // - We treat module names as non-assignable placeholders.
        let Some(last) = import.path.last() else {
            return Ok(());
        };
        self.define_module_placeholder(last)?;
        Ok(())
    }

    fn define_module_placeholder(&mut self, name: &Ident) -> Result<(), SemanticError> {
        let scope = self.scopes.last_mut().expect("scope stack");
        if let Some(existing) = scope.get(&name.node) {
            let expected = Type::Named(format!("<module:{}>", name.node));
            if existing == &expected {
                // Idempotent: re-importing the same module name is allowed.
                return Ok(());
            }
            return Err(SemanticError {
                message: format!(
                    "import would shadow existing binding '{}' ({})",
                    name.node,
                    existing.display()
                ),
                span: name.span,
            });
        }

        // Store as a named placeholder type. This lets `tensor::foo` and `io::bar` be parsed as
        // member expressions without failing identifier lookup.
        scope.insert(name.node.clone(), Type::Named(format!("<module:{}>", name.node)));

        // Capability root (so flow/cap rules can still treat it as alive when referenced).
        let cap_id = self.fresh_cap(name.span);
        self.cap.alloc_root(cap_id, &name.node);
        Ok(())
    }

    fn check_strand(&mut self, sd: &StrandDef) -> Result<(), SemanticError> {
        if let Some(existing) = self.lookup_val(&sd.name.node) {
            return Err(SemanticError {
                message: format!("val '{}' already defined as {}", sd.name.node, existing.display()),
                span: sd.name.span,
            });
        }

        let expr_ty = self.infer_expr(&sd.expr)?;

        let final_ty = if let Some(annot) = &sd.ty {
            let expected = self.resolve_type_ref(annot)?;

            // If there is a `where` clause, we treat it as a refinement for type tracking,
            // but we don't require proving it here (the verifier is the hard gate).
            // We still check base assignability.
            self.check_assignable(&base_type(&expected).clone(), &expr_ty, &sd.expr)?;

            if let Some(w) = &sd.where_clause {
                if let Some((lo, hi)) = refinement_u32_range_from_where(&sd.name.node, w) {
                    Type::ConstrainedRange {
                        base: Box::new(Type::U32),
                        lo,
                        hi,
                    }
                } else {
                    expected
                }
            } else {
                expected
            }
        } else {
            expr_ty
        };

        // Mutation aliasing rule (MVP): values that behave like unique resources cannot be
        // duplicated by binding from an identifier. Binding from a resource identifier moves it.
        if let ExprKind::Ident(src) = &sd.expr.kind {
            if self.is_non_copy_type(&final_ty) {
                self.consume_move_from_value(&src.node, src.span)?;
            }
        }
        self.define_val(&sd.name, final_ty, sd.mutable)
    }

    fn check_assign(&mut self, assign: &AssignStmt) -> Result<(), SemanticError> {
        let Some(target_ty) = self.lookup_val(&assign.target.node) else {
            return Err(SemanticError {
                message: format!("unknown identifier '{}'", assign.target.node),
                span: assign.target.span,
            });
        };

        let _ = self
            .cap
            .ensure_alive(&assign.target.node, assign.target.span)?;

        if !self.is_mutable(&assign.target.node) {
            return Err(SemanticError {
                message: format!("cannot assign to immutable val '{}'", assign.target.node),
                span: assign.target.span,
            });
        }

        let rhs_ty = self.infer_expr(&assign.expr)?;
        // Mutation aliasing rule (MVP): assigning from a resource identifier moves it.
        if let ExprKind::Ident(src) = &assign.expr.kind {
            if src.node != assign.target.node && self.is_non_copy_type(&rhs_ty) {
                self.consume_move_from_value(&src.node, src.span)?;
            }
        }
        self.check_assignable(&target_ty, &rhs_ty, &assign.expr)
    }

    fn check_if(&mut self, if_stmt: &IfStmt) -> Result<(), SemanticError> {
        let cond_ty = self.infer_expr(&if_stmt.cond)?;
        if cond_ty != Type::Bool {
            return Err(SemanticError {
                message: format!("if condition must be bool, got {}", cond_ty.display()),
                span: if_stmt.cond.span,
            });
        }
        let _ = self.check_block(&if_stmt.then_block)?;
        if let Some(else_block) = &if_stmt.else_block {
            let _ = self.check_block(else_block)?;
        }
        Ok(())
    }

    fn check_match(&mut self, m: &MatchStmt) -> Result<(), SemanticError> {
        if m.arms.is_empty() {
            return Err(SemanticError {
                message: "match must have at least one arm".to_string(),
                span: m.span,
            });
        }

        let scrut_ty = self.infer_expr(&m.scrutinee)?;

        // First-pass exhaustiveness: require a trailing wildcard arm.
        // Keep semantics obvious: if a wildcard is present, it must be last.
        let mut wildcard_idx: Option<usize> = None;
        for (i, arm) in m.arms.iter().enumerate() {
            if matches!(arm.pat, Pattern::Wildcard { .. }) {
                wildcard_idx = Some(i);
                break;
            }
        }

        match wildcard_idx {
            None => {
                return Err(SemanticError {
                    message: "non-exhaustive match; add a final '_' arm".to_string(),
                    span: m.span,
                });
            }
            Some(i) if i != m.arms.len() - 1 => {
                return Err(SemanticError {
                    message: "wildcard '_' arm must be last".to_string(),
                    span: m.arms[i].span,
                });
            }
            Some(_) => {}
        }

        let mut seen_ints: HashSet<u64> = HashSet::new();
        let mut seen_strings: HashSet<String> = HashSet::new();

        for arm in &m.arms {
            match &arm.pat {
                Pattern::Wildcard { .. } => {}
                Pattern::IntLit { span, value } => {
                    if !is_u32_like(&scrut_ty) {
                        return Err(SemanticError {
                            message: format!(
                                "int pattern is not compatible with scrutinee type {}",
                                scrut_ty.display()
                            ),
                            span: *span,
                        });
                    }
                    if !seen_ints.insert(*value) {
                        return Err(SemanticError {
                            message: format!("duplicate match arm for literal {value}"),
                            span: *span,
                        });
                    }
                }
                Pattern::StringLit { span, value } => {
                    if base_type(&scrut_ty) != &Type::String {
                        return Err(SemanticError {
                            message: format!(
                                "string pattern is not compatible with scrutinee type {}",
                                scrut_ty.display()
                            ),
                            span: *span,
                        });
                    }
                    if !seen_strings.insert(value.clone()) {
                        return Err(SemanticError {
                            message: "duplicate match arm for string literal".to_string(),
                            span: *span,
                        });
                    }
                }
                Pattern::Ctor {
                    span,
                    ty,
                    variant,
                    binders,
                } => {
                    let scrut_base = base_type(&scrut_ty);
                    let Some((scrut_name, scrut_args)) = applied_name_and_args(scrut_base) else {
                        return Err(SemanticError {
                            message: format!(
                                "ctor pattern is not compatible with scrutinee type {}",
                                scrut_ty.display()
                            ),
                            span: *span,
                        });
                    };

                    if scrut_name != ty.node.as_str() {
                        return Err(SemanticError {
                            message: format!(
                                "ctor pattern expects '{}', but scrutinee is '{}'",
                                ty.node, scrut_name
                            ),
                            span: *span,
                        });
                    }

                    // Grab variant field TypeRefs up-front so we can mutate `self` freely after.
                    let (expected_fields, def_params): (Vec<TypeRef>, Vec<aura_ast::TypeParam>) = {
                        let Some(def) = self.enum_defs.get(&ty.node) else {
                            return Err(SemanticError {
                                message: format!("unknown enum type '{}'", ty.node),
                                span: ty.span,
                            });
                        };

                        let Some(var_def) =
                            def.variants.iter().find(|v| v.name.node == variant.node)
                        else {
                            return Err(SemanticError {
                                message: format!(
                                    "unknown variant '{}' for enum '{}'",
                                    variant.node, ty.node
                                ),
                                span: variant.span,
                            });
                        };

                        (
                            var_def.fields.iter().map(|f| f.ty.clone()).collect(),
                            def.params.clone(),
                        )
                    };

                    if binders.len() != expected_fields.len() {
                        return Err(SemanticError {
                            message: format!(
                                "wrong number of binders for '{}::{}': expected {}, got {}",
                                ty.node,
                                variant.node,
                                expected_fields.len(),
                                binders.len()
                            ),
                            span: *span,
                        });
                    }

                    // Bind ctor fields for the arm body.
                    self.push_scope();
                    for (b, tr) in binders.iter().zip(expected_fields.iter()) {
                        let field_ty = if def_params.is_empty() {
                            self.resolve_type_ref(tr)?
                        } else {
                            let mut subst: HashMap<String, Type> = HashMap::new();
                            for (p, a) in def_params.iter().zip(scrut_args.iter()) {
                                subst.insert(p.name.node.clone(), a.clone());
                            }
                            self.resolve_type_ref_with_type_params(tr, &subst)?
                        };
                        self.define_val(b, field_ty, false)?;
                    }
                    let _ = self.check_block(&arm.body)?;
                    self.pop_scope();
                    continue;
                }
            }

            let _ = self.check_block(&arm.body)?;
        }

        Ok(())
    }

    fn check_while(&mut self, while_stmt: &WhileStmt) -> Result<(), SemanticError> {
        let cond_ty = self.infer_expr(&while_stmt.cond)?;
        if cond_ty != Type::Bool {
            return Err(SemanticError {
                message: format!("while condition must be bool, got {}", cond_ty.display()),
                span: while_stmt.cond.span,
            });
        }
        if let Some(inv) = &while_stmt.invariant {
            let inv_ty = self.infer_expr(inv)?;
            if inv_ty != Type::Bool {
                return Err(SemanticError {
                    message: format!("while invariant must be bool, got {}", inv_ty.display()),
                    span: inv.span,
                });
            }
        }
        if let Some(dec) = &while_stmt.decreases {
            let dec_ty = self.infer_expr(dec)?;
            if !is_u32_like(&dec_ty) {
                return Err(SemanticError {
                    message: format!("decreases expects integer, got {}", dec_ty.display()),
                    span: dec.span,
                });
            }
        }
        let _ = self.check_block(&while_stmt.body)?;
        Ok(())
    }

    fn check_assignable(&self, expected: &Type, actual: &Type, rhs: &Expr) -> Result<(), SemanticError> {
        match (expected, actual, &rhs.kind) {
            // Range proof via literal.
            (
                Type::ConstrainedRange { base, lo, hi },
                _,
                _,
            ) if **base == Type::U32 => {
                if self.defer_range_proofs {
                    // Only allow u32-like values; proof is deferred to Z3.
                    if is_u32_like(actual) {
                        Ok(())
                    } else {
                        Err(SemanticError {
                            message: format!(
                                "type mismatch: expected {}, got {}",
                                expected.display(),
                                actual.display()
                            ),
                            span: rhs.span,
                        })
                    }
                } else {
                    self.verifier.prove_u32_in_range(rhs, *lo, *hi)
                }
            }

            // Range proof via subset relationship.
            (
                Type::ConstrainedRange {
                    base: exp_base,
                    lo: exp_lo,
                    hi: exp_hi,
                },
                Type::ConstrainedRange {
                    base: act_base,
                    lo: act_lo,
                    hi: act_hi,
                },
                _,
            ) if **exp_base == **act_base && is_subset_range(*act_lo, *act_hi, *exp_lo, *exp_hi) => Ok(()),

            // Tensor assignability (prototype):
            // - Element types must match, except that `Unknown` is treated as a wildcard.
            // - `Tensor<T, [..]>` is assignable to `Tensor<T>` (forgetting shape).
            // - `Tensor<T>` is NOT assignable to `Tensor<T, [..]>` (cannot invent shape).
            (
                Type::Tensor {
                    elem: exp_elem,
                    shape: exp_shape,
                },
                Type::Tensor {
                    elem: act_elem,
                    shape: act_shape,
                },
                _,
            ) => {
                let elem_ok = **exp_elem == Type::Unknown
                    || **act_elem == Type::Unknown
                    || exp_elem == act_elem;

                let shape_ok = match (exp_shape, act_shape) {
                    (None, _) => true,
                    (Some(exp), Some(act)) => exp == act,
                    (Some(exp), None) => {
                        // Special-case: allow `tensor.new(<literal_len>)` to flow into a
                        // statically-shaped tensor when <literal_len> == product(shape).
                        // This keeps sema simple while letting Z3 reason about shapes.
                        let mut ok = false;
                        if let ExprKind::Call { callee, args, .. } = &rhs.kind {
                            let name = match &callee.kind {
                                ExprKind::Member { base, member }
                                    if member.node == "infer"
                                        && !matches!(&base.kind, ExprKind::Ident(id) if id.node == "ai") =>
                                {
                                    "ai.infer".to_string()
                                }
                                _ => expr_to_callee_name(callee),
                            };

                            // ai.infer returns an unshaped tensor in sema; the verifier refines it.
                            if name == "ai.infer" {
                                ok = true;
                            }

                            // tensor.new: allow when literal length matches product(shape).
                            if !ok && name == "tensor.new" && args.len() == 1 {
                                let n = match &args[0] {
                                    CallArg::Positional(e) => match e.kind {
                                        ExprKind::IntLit(n) => Some(n),
                                        _ => None,
                                    },
                                    CallArg::Named { value, .. } => match value.kind {
                                        ExprKind::IntLit(n) => Some(n),
                                        _ => None,
                                    },
                                };
                                if let Some(n) = n {
                                    let mut prod: u64 = 1;
                                    let mut overflow = false;
                                    for d in exp {
                                        match prod.checked_mul(*d) {
                                            Some(v) => prod = v,
                                            None => {
                                                overflow = true;
                                                break;
                                            }
                                        }
                                    }
                                    ok = !overflow && prod == n;
                                }
                            }
                        }
                        ok
                    }
                };

                if elem_ok && shape_ok {
                    Ok(())
                } else {
                    Err(SemanticError {
                        message: format!(
                            "type mismatch: expected {}, got {}",
                            expected.display(),
                            actual.display()
                        ),
                        span: rhs.span,
                    })
                }
            }

            // Nominal generic types: same constructor name and compatible args.
            (
                Type::Applied {
                    name: exp_name,
                    args: exp_args,
                },
                Type::Applied {
                    name: act_name,
                    args: act_args,
                },
                _,
            ) if exp_name == act_name && exp_args.len() == act_args.len() => {
                for (e, a) in exp_args.iter().zip(act_args.iter()) {
                    if *e == Type::Unknown || *a == Type::Unknown {
                        continue;
                    }
                    // Permit the same range-subtyping rule inside nominal args as at top-level.
                    let arg_ok = if e == a {
                        true
                    } else {
                        matches!((e, a), (Type::U32, Type::ConstrainedRange { base, .. }) if **base == Type::U32)
                    };

                    if !arg_ok {
                        return Err(SemanticError {
                            message: format!(
                                "type mismatch: expected {}, got {}",
                                expected.display(),
                                actual.display()
                            ),
                            span: rhs.span,
                        });
                    }
                }
                Ok(())
            }

            // Base equality (very minimal today).
            (a, b, _) if a == b => Ok(()),

            // Allow constrained-range values to be used where the base type is expected.
            (Type::U32, Type::ConstrainedRange { base, .. }, _) if **base == Type::U32 => Ok(()),

            // Assigning unconstrained u32 into constrained range requires proof.
            (Type::ConstrainedRange { .. }, Type::U32, _) if !self.defer_range_proofs => Err(SemanticError {
                message: "cannot prove range safety for non-literal assignment (SMT stub)".to_string(),
                span: rhs.span,
            }),

            _ => Err(SemanticError {
                message: format!(
                    "type mismatch: expected {}, got {}",
                    expected.display(),
                    actual.display()
                ),
                span: rhs.span,
            }),
        }
    }

    fn infer_expr(&mut self, expr: &Expr) -> Result<Type, SemanticError> {
        match &expr.kind {
            ExprKind::IntLit(n) => {
                if *n > U32_MAX {
                    return Err(SemanticError {
                        message: format!(
                            "integer literal out of range for u32 (max={})",
                            U32_MAX
                        ),
                        span: expr.span,
                    });
                }
                Ok(Type::ConstrainedRange {
                    base: Box::new(Type::U32),
                    lo: *n,
                    hi: *n,
                })
            }
            ExprKind::StringLit(_) => Ok(Type::String),
            ExprKind::StyleLit { fields } => {
                for (_k, v) in fields {
                    let t = self.infer_expr(v)?;
                    let ok = matches!(base_type(&t), Type::U32 | Type::Bool | Type::String | Type::Style);
                    if !ok {
                        return Err(SemanticError {
                            message: format!(
                                "style field values must be scalar (u32/bool/String/Style), got {}",
                                t.display()
                            ),
                            span: v.span,
                        });
                    }
                }
                Ok(Type::Style)
            }
            ExprKind::RecordLit { name, fields } => {
                let Some(def) = self.record_defs.get(&name.node).cloned() else {
                    return Err(SemanticError {
                        message: format!("unknown record type '{}'", name.node),
                        span: name.span,
                    });
                };

                let param_subst = if def.params.is_empty() {
                    HashMap::new()
                } else {
                    self.infer_type_args_from_record_literal(&def, fields)?
                };

                let mut expected: HashMap<String, (TypeRef, bool)> = HashMap::new();
                for f in &def.fields {
                    expected.insert(
                        f.name.node.clone(),
                        (f.ty.clone(), f.default.is_some()),
                    );
                }

                // Check provided fields.
                for (k, v) in fields {
                    let Some((fty_ref, _has_default)) = expected.get(&k.node) else {
                        return Err(SemanticError {
                            message: format!(
                                "unknown field '{}' for record '{}'",
                                k.node, name.node
                            ),
                            span: k.span,
                        });
                    };
                    let expected_ty = if def.params.is_empty() {
                        self.resolve_type_ref(fty_ref)?
                    } else {
                        self.resolve_type_ref_with_type_params(fty_ref, &param_subst)?
                    };
                    let actual_ty = self.infer_expr(v)?;

                    // If a record field is initialized from a resource identifier, move it.
                    // This prevents creating implicit aliases to mutable resources.
                    if let ExprKind::Ident(src) = &v.kind {
                        if self.is_non_copy_type(&actual_ty) {
                            self.consume_move_from_value(&src.node, src.span)?;
                        }
                    }
                    self.check_assignable(&expected_ty, &actual_ty, v)?;
                    expected.remove(&k.node);
                }

                // Remaining fields must have defaults.
                for (fname, (_ty_ref, has_default)) in expected {
                    if !has_default {
                        return Err(SemanticError {
                            message: format!(
                                "missing required field '{}' for record '{}'",
                                fname, name.node
                            ),
                            span: expr.span,
                        });
                    }
                }

                if def.params.is_empty() {
                    Ok(Type::Named(name.node.clone()))
                } else {
                    let args = def
                        .params
                        .iter()
                        .map(|p| param_subst.get(&p.name.node).cloned().unwrap_or(Type::Unknown))
                        .collect::<Vec<_>>();
                    Ok(Type::Applied {
                        name: name.node.clone(),
                        args,
                    })
                }
            }
            ExprKind::Ident(id) => {
                self.check_async_capture(&id.node, id.span)?;
                
                // Look up the type first
                let ty = self.lookup_val(&id.node).ok_or_else(|| SemanticError {
                    message: format!("unknown identifier '{}'", id.node),
                    span: id.span,
                })?;
                
                // Enforce linear type ownership constraints
                self.enforce_linear_use(&id.node, &ty, id.span)?;
                
                // Global liveness check for the Flow/linear capability model.
                let _ = self.cap.ensure_alive(&id.node, id.span)?;
                
                Ok(ty)
            }
            ExprKind::Unary { op, expr: inner } => {
                let t = self.infer_expr(inner)?;
                match op {
                    UnaryOp::Neg => {
                        if !is_u32_like(&t) {
                            return Err(SemanticError {
                                message: format!("unary '-' expects u32, got {}", t.display()),
                                span: inner.span,
                            });
                        }
                        Ok(Type::U32)
                    }
                    UnaryOp::Not => {
                        if t != Type::Bool {
                            return Err(SemanticError {
                                message: format!("unary '!' expects bool, got {}", t.display()),
                                span: inner.span,
                            });
                        }
                        Ok(Type::Bool)
                    }
                }
            }
            ExprKind::Binary { left, op, right } => {
                let lt = self.infer_expr(left)?;
                let rt = self.infer_expr(right)?;
                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                        if !is_u32_like(&lt) || !is_u32_like(&rt) {
                            return Err(SemanticError {
                                message: format!(
                                    "arithmetic op expects u32,u32; got {},{}",
                                    lt.display(),
                                    rt.display()
                                ),
                                span: expr.span,
                            });
                        }

                        // Range inference (prototype): keep u32 range information when possible.
                        let inferred = infer_u32_range_binop(op, &lt, &rt);
                        Ok(inferred)
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                        if !is_u32_like(&lt) || !is_u32_like(&rt) {
                            return Err(SemanticError {
                                message: format!(
                                    "comparison op expects u32,u32; got {},{}",
                                    lt.display(),
                                    rt.display()
                                ),
                                span: expr.span,
                            });
                        }
                        Ok(Type::Bool)
                    }
                    BinOp::And | BinOp::Or => {
                        if lt != Type::Bool || rt != Type::Bool {
                            return Err(SemanticError {
                                message: format!(
                                    "boolean op expects bool,bool; got {},{}",
                                    lt.display(),
                                    rt.display()
                                ),
                                span: expr.span,
                            });
                        }
                        Ok(Type::Bool)
                    }
                }
            }
            ExprKind::Member { base, member } => {
                let base_ty = self.infer_expr(base)?;

                if let Some((rec_name, args)) = applied_name_and_args(base_type(&base_ty)) {
                    if let Some(def) = self.record_defs.get(rec_name) {
                        let Some(field) = def.fields.iter().find(|f| f.name.node == member.node) else {
                            return Err(SemanticError {
                                message: format!(
                                    "unknown field '{}' for record '{}'",
                                    member.node, rec_name
                                ),
                                span: member.span,
                            });
                        };

                        let ty = if def.params.is_empty() {
                            self.resolve_type_ref(&field.ty)?
                        } else {
                            let mut subst: HashMap<String, Type> = HashMap::new();
                            for (p, a) in def.params.iter().zip(args.iter()) {
                                subst.insert(p.name.node.clone(), a.clone());
                            }
                            self.resolve_type_ref_with_type_params(&field.ty, &subst)?
                        };
                        return Ok(ty);
                    }
                }

                // Fallback: member names are resolved at call sites via full-name mapping.
                Ok(Type::Named(member.node.clone()))
            }
            ExprKind::Call { callee, args, trailing } => {
                // Enum constructor calls: `Type::Variant(...)`.
                if let ExprKind::Member { base, member } = &callee.kind {
                    if let ExprKind::Ident(ty_id) = &base.kind {
                        // Pull expected field types out of `self.enum_defs` first so we can
                        // call `infer_expr` without holding an immutable borrow.
                        let expected_field_tys: Option<Vec<TypeRef>> = self
                            .enum_defs
                            .get(&ty_id.node)
                            .map(|def| {
                                let Some(variant) = def
                                    .variants
                                    .iter()
                                    .find(|v| v.name.node == member.node)
                                else {
                                    return Err(SemanticError {
                                        message: format!(
                                            "unknown variant '{}' for enum '{}'",
                                            member.node, ty_id.node
                                        ),
                                        span: member.span,
                                    });
                                };

                                Ok(variant.fields.iter().map(|f| f.ty.clone()).collect())
                            })
                            .transpose()?;

                        if let Some(expected_field_tys) = expected_field_tys {
                            let def = self
                                .enum_defs
                                .get(&ty_id.node)
                                .cloned()
                                .expect("enum def exists");

                            // MVP: positional args only for constructors.
                            if args.len() != expected_field_tys.len() {
                                return Err(SemanticError {
                                    message: format!(
                                        "wrong number of constructor args for '{}::{}': expected {}, got {}",
                                        ty_id.node,
                                        member.node,
                                        expected_field_tys.len(),
                                        args.len()
                                    ),
                                    span: expr.span,
                                });
                            }

                            let param_subst = if def.params.is_empty() {
                                HashMap::new()
                            } else {
                                let arg_exprs: Vec<&Expr> =
                                    args.iter().map(call_arg_value).collect();
                                self.infer_type_args_from_enum_ctor(
                                    &def,
                                    &member.node,
                                    &arg_exprs,
                                )?
                            };

                            for (i, (expected_tr, a)) in
                                expected_field_tys.iter().zip(args.iter()).enumerate()
                            {
                                let arg_expr = call_arg_value(a);
                                let expected_ty = if def.params.is_empty() {
                                    self.resolve_type_ref(expected_tr)?
                                } else {
                                    self.resolve_type_ref_with_type_params(
                                        expected_tr,
                                        &param_subst,
                                    )?
                                };
                                let actual_ty = self.infer_expr(arg_expr)?;
                                self.check_assignable(&expected_ty, &actual_ty, arg_expr)
                                    .map_err(|mut e| {
                                        e.message = format!("arg {i}: {}", e.message);
                                        e
                                    })?;

                                // Linear capability rule: initializing a constructor field from a
                                // non-copy identifier consumes (moves) the source value.
                                //
                                // This mirrors record literal initialization and prevents implicit
                                // aliasing of unique resources.
                                if let ExprKind::Ident(src) = &arg_expr.kind {
                                    if self.is_non_copy_type(&actual_ty) {
                                        self.consume_move_from_value(&src.node, src.span)?;
                                    }
                                }
                            }

                            // Trailing blocks are not supported for constructors.
                            if trailing.is_some() {
                                return Err(SemanticError {
                                    message: "enum constructors do not accept trailing blocks".to_string(),
                                    span: expr.span,
                                });
                            }

                            if def.params.is_empty() {
                                return Ok(Type::Named(ty_id.node.clone()));
                            }

                            let args = def
                                .params
                                .iter()
                                .map(|p| {
                                    param_subst
                                        .get(&p.name.node)
                                        .cloned()
                                        .unwrap_or(Type::Unknown)
                                })
                                .collect::<Vec<_>>();

                            return Ok(Type::Applied {
                                name: ty_id.node.clone(),
                                args,
                            });
                        }
                    }
                }

                // Method-call lowering (prototype): treat tensor instance methods
                // `.len/.get/.set` as `tensor.len(t, ...)`.
                let (name, all_args): (String, Vec<&Expr>) = match &callee.kind {
                    ExprKind::Member { base, member }
                        if matches!(member.node.as_str(), "len" | "get" | "set")
                            && !matches!(&base.kind, ExprKind::Ident(id) if id.node == "tensor") =>
                    {
                        // Ensure receiver is typed.
                        let _recv_ty = self.infer_expr(base)?;
                        let mut v = Vec::with_capacity(args.len() + 1);
                        v.push(base.as_ref());
                        for a in args {
                            v.push(call_arg_value(a));
                        }
                        (format!("tensor.{}", member.node), v)
                    }
                    ExprKind::Member { base, member }
                        if member.node == "infer"
                            && !matches!(&base.kind, ExprKind::Ident(id) if id.node == "ai") =>
                    {
                        let _recv_ty = self.infer_expr(base)?;
                        let mut v = Vec::with_capacity(args.len() + 1);
                        v.push(base.as_ref());
                        for a in args {
                            v.push(call_arg_value(a));
                        }
                        ("ai.infer".to_string(), v)
                    }
                    _ => {
                        let name = expr_to_callee_name(callee);
                        if let Some(sig) = self.functions.get(&name) {
                            (name, self.resolve_call_args_against_sig(expr.span, args, sig)?)
                        } else {
                            let mut v = Vec::with_capacity(args.len());
                            for a in args {
                                v.push(call_arg_value(a));
                            }
                            (name, v)
                        }
                    }
                };

                // Type-check trailing block in the caller scope (Phase A.5 semantics).
                if let Some(tb) = trailing {
                    let _ = self.check_block(tb)?;
                }

                let sig = self.functions.get(&name).cloned();

                if let Some(trusted) = self.extern_cells.get(&name).copied() {
                    if !trusted && self.unsafe_depth == 0 {
                        return Err(SemanticError {
                            message: format!(
                                "calling extern cell '{}' requires an unsafe block",
                                name
                            ),
                            span: expr.span,
                        });
                    }
                }

                for a in &all_args {
                    let _ = self.infer_expr(a)?;
                }

                if let Some(sig) = sig {
                    // Minimal arity checking.
                    if sig.params.len() != all_args.len() {
                        return Err(SemanticError {
                            message: format!(
                                "wrong number of arguments for '{}': expected {}, got {}",
                                name,
                                sig.params.len(),
                                all_args.len()
                            ),
                            span: expr.span,
                        });
                    }

                    // Minimal type checking (range constraints may appear on expected params later).
                    for (i, (expected, arg)) in sig.params.iter().zip(all_args.iter()).enumerate() {
                        let actual = self.infer_expr(arg)?;
                        self.check_assignable(&expected.ty, &actual, arg).map_err(|mut e| {
                            e.message = format!("arg {i}: {}", e.message);
                            e
                        })?;

                        // Linear capability rule for calls:
                        // - Some builtins borrow (do not consume) non-copy values.
                        // - Otherwise, passing a non-copy identifier consumes (moves) it.
                        if let ExprKind::Ident(src) = &arg.kind {
                            if self.is_non_copy_type(&actual) {
                                match name.as_str() {
                                    // Read-only borrows.
                                    "tensor.len" | "tensor.get" | "collections.vector_len" | "collections.vector_get" => {
                                        let from = self.cap.ensure_alive(&src.node, src.span)?;
                                        let to = self.fresh_cap(arg.span);
                                        self.cap.lend_read(from, to, arg.span);
                                    }

                                    // Write borrow (must originate from a mutable binding).
                                    "tensor.set" | "collections.vector_set" => {
                                        if !self.is_mutable(&src.node) {
                                            return Err(SemanticError {
                                                message: format!(
                                                    "cannot pass immutable val '{}' as mutable tensor receiver",
                                                    src.node
                                                ),
                                                span: src.span,
                                            });
                                        }
                                        let from = self.cap.ensure_alive(&src.node, src.span)?;
                                        let to = self.fresh_cap(arg.span);
                                        self.cap.lend_write(from, to, arg.span);
                                    }

                                    // Default: move/consume.
                                    _ => {
                                        self.consume_move_from_value(&src.node, src.span)?;
                                    }
                                }
                            }
                        }
                    }

                    Ok(sig.ret)
                } else {
                    // Unknown call: allow but type becomes unknown.
                    Ok(Type::Unknown)
                }
            }
            ExprKind::ForAll { binders, body } | ExprKind::Exists { binders, body } => {
                self.push_scope();
                for b in binders {
                    let ty = if let Some(tr) = &b.ty {
                        self.resolve_type_ref(tr)?
                    } else {
                        Type::U32
                    };
                    self.define_val(&b.name, ty, false)?;
                }
                let body_ty = self.infer_expr(body)?;
                self.pop_scope();
                if body_ty != Type::Bool {
                    return Err(SemanticError {
                        message: format!("quantifier body must be bool, got {}", body_ty.display()),
                        span: body.span,
                    });
                }
                Ok(Type::Bool)
            }
            ExprKind::Lambda { op, body } => {
                if *op == aura_ast::FlowOp::Async {
                    self.async_lambda_bases.push(self.scopes.len());
                    let _ = self.check_block(body)?;
                    self.async_lambda_bases.pop();
                } else {
                    let _ = self.check_block(body)?;
                }
                Ok(Type::Unknown)
            }
            ExprKind::Flow { left, op: _, right } => {
                // Capability rules:
                // - Sync (->): transfer ownership (consume LHS capability)
                // - Async (~>): transfer ownership and require thread-safe-by-construction
                let op = match expr.kind {
                    ExprKind::Flow { op, .. } => op,
                    _ => unreachable!(),
                };

                // Type-check LHS first (also enforces capability liveness).
                let left_ty = self.infer_expr(left)?;

                // Apply linear capability transfer after the LHS is checked.
                self.apply_flow_capabilities(left, right, op, expr.span)?;

                if let ExprKind::Call { callee, args, .. } = &right.kind {
                    let (name, base_args): (String, Vec<&Expr>) = match &callee.kind {
                        ExprKind::Member { base, member }
                            if matches!(member.node.as_str(), "len" | "get" | "set")
                                && !matches!(&base.kind, ExprKind::Ident(id) if id.node == "tensor") =>
                        {
                            let mut v = Vec::with_capacity(args.len() + 1);
                            v.push(base.as_ref());
                            for a in args {
                                v.push(call_arg_value(a));
                            }
                            (format!("tensor.{}", member.node), v)
                        }
                        ExprKind::Member { base, member }
                            if member.node == "infer"
                                && !matches!(&base.kind, ExprKind::Ident(id) if id.node == "ai") =>
                        {
                            let mut v = Vec::with_capacity(args.len() + 1);
                            v.push(base.as_ref());
                            for a in args {
                                v.push(call_arg_value(a));
                            }
                            ("ai.infer".to_string(), v)
                        }
                        _ => {
                            let mut v = Vec::with_capacity(args.len());
                            for a in args {
                                v.push(call_arg_value(a));
                            }
                            (expr_to_callee_name(callee), v)
                        }
                    };

                    let sig = self.functions.get(&name).cloned();
                    if let Some(sig) = sig {
                        // Flow-injected call: expected args are [left] + base_args.
                        if sig.params.len() != base_args.len() + 1 {
                            return Err(SemanticError {
                                message: format!(
                                    "wrong number of arguments for '{}': expected {}, got {}",
                                    name,
                                    sig.params.len(),
                                    base_args.len() + 1
                                ),
                                span: expr.span,
                            });
                        }

                        // Arg0: left
                        self.check_assignable(&sig.params[0].ty, &left_ty, left).map_err(|mut e| {
                            e.message = format!("arg 0: {}", e.message);
                            e
                        })?;

                        // Remaining args.
                        for (i, (expected, arg)) in sig.params[1..]
                            .iter()
                            .zip(base_args.iter())
                            .enumerate()
                        {
                            let actual = self.infer_expr(arg)?;
                            self.check_assignable(&expected.ty, &actual, arg).map_err(|mut e| {
                                e.message = format!("arg {}: {}", i + 1, e.message);
                                e
                            })?;
                        }

                        Ok(sig.ret)
                    } else {
                        // Unknown call through flow: allow, but type becomes unknown.
                        Ok(Type::Unknown)
                    }
                } else {
                    self.infer_expr(right)
                }
            }
        }
    }

    fn apply_flow_capabilities(
        &mut self,
        left: &Expr,
        right: &Expr,
        op: aura_ast::FlowOp,
        span: Span,
    ) -> Result<(), SemanticError> {
        // Ensure any referenced values on the RHS are alive.
        // (RHS is evaluated after the LHS capability transfer.)
        let mut referenced = Vec::new();
        if let ExprKind::Call { args, .. } = &right.kind {
            for a in args {
                collect_value_idents(call_arg_value(a), &mut referenced);
            }
        } else {
            collect_value_idents(right, &mut referenced);
        }
        for id in referenced {
            let _ = self.cap.ensure_alive(&id.node, id.span)?;
        }

        // Linear capability transfer: consume the LHS if it's a named value.
        if let ExprKind::Ident(id) = &left.kind {
            // Do not treat module placeholders as linear resources.
            if let Some(ty) = self.lookup_val(&id.node) {
                if matches!(ty, Type::Named(n) if n.starts_with("<module:")) {
                    return Ok(());
                }
            }

            if op == aura_ast::FlowOp::Async && self.is_mutable(&id.node) {
                return Err(SemanticError {
                    message: format!(
                        "cannot async-transfer mutable capability '{}' via ~> (must be immutable or moved from an immutable binding)",
                        id.node
                    ),
                    span: id.span,
                });
            }

            let from = self.cap.ensure_alive(&id.node, id.span)?;
            let to = self.fresh_cap(span);
            self.cap.consume_move(from, to, span);
        }
        Ok(())
    }

    fn fresh_cap(&mut self, _span: Span) -> aura_ir::CapabilityId {
        let id = aura_ir::CapabilityId(self.cap_next);
        self.cap_next += 1;
        id
    }

    fn resolve_type_ref(&self, tr: &TypeRef) -> Result<Type, SemanticError> {
        let base = match tr.name.node.as_str() {
            "u32" => Type::U32,
            "Int" => Type::U32,
            "bool" => Type::Bool,
            "Tensor" => {
                // `Tensor<Elem, [d0, d1, ...]>` (shape optional)
                // `Tensor<Elem>` (element optional)
                let elem;
                let mut shape: Option<Vec<u64>> = None;

                match tr.args.as_slice() {
                    [] => {
                        elem = Type::Unknown;
                    }
                    [TypeArg::Type(t)] => {
                        elem = self.resolve_type_ref(t)?;
                    }
                    [TypeArg::Type(t), TypeArg::Shape(dims)] => {
                        elem = self.resolve_type_ref(t)?;
                        shape = Some(dims.clone());
                    }
                    [TypeArg::Shape(_)] => {
                        return Err(SemanticError {
                            message: "Tensor first type argument must be an element type".to_string(),
                            span: tr.span,
                        })
                    }
                    _ => {
                        return Err(SemanticError {
                            message: "Tensor expects `Tensor<Elem>` or `Tensor<Elem, [d0, d1, ...]>`".to_string(),
                            span: tr.span,
                        })
                    }
                }

                if let Some(dims) = &shape {
                    if dims.iter().any(|d| *d == 0) {
                        return Err(SemanticError {
                            message: "Tensor shape dimensions must be > 0".to_string(),
                            span: tr.span,
                        });
                    }
                }

                Type::Tensor {
                    elem: Box::new(elem),
                    shape,
                }
            }
            "String" => Type::String,
            "Style" => Type::Style,
            "Unit" => Type::Unit,
            "Model" => Type::Model,
            other => {
                if let Some(def) = self.record_defs.get(other) {
                    return self.resolve_nominal_type_ref(other, &def.params, &tr.args, tr.span);
                } else if let Some(def) = self.enum_defs.get(other) {
                    return self.resolve_nominal_type_ref(other, &def.params, &tr.args, tr.span);
                } else
                if let Some(entry) = self.type_aliases.get(other) {
                    match entry {
                        AliasEntry::Mono(ty) => {
                            if !tr.args.is_empty() {
                                return Err(SemanticError {
                                    message: format!(
                                        "type alias '{other}' does not take type arguments"
                                    ),
                                    span: tr.span,
                                });
                            }
                            ty.clone()
                        }
                        AliasEntry::Generic(def) => {
                            let instantiated = self.instantiate_type_alias(other, def, &tr.args)?;
                            self.resolve_type_ref(&instantiated)?
                        }
                    }
                } else {
                    Type::Named(other.to_string())
                }
            }
        };

        if let Some(range) = &tr.range {
            let lo = const_u64(&range.lo).ok_or_else(|| SemanticError {
                message: "range lower-bound must be a constant integer".to_string(),
                span: range.lo.span,
            })?;
            let hi = const_u64(&range.hi).ok_or_else(|| SemanticError {
                message: "range upper-bound must be a constant integer".to_string(),
                span: range.hi.span,
            })?;
            if lo > hi {
                return Err(SemanticError {
                    message: "range lower-bound is greater than upper-bound".to_string(),
                    span: range.span,
                });
            }

            Ok(Type::ConstrainedRange {
                base: Box::new(base),
                lo,
                hi,
            })
        } else {
            Ok(base)
        }
    }

    fn resolve_nominal_type_ref(
        &self,
        name: &str,
        params: &[aura_ast::TypeParam],
        args: &[TypeArg],
        span: Span,
    ) -> Result<Type, SemanticError> {
        if params.is_empty() {
            if !args.is_empty() {
                return Err(SemanticError {
                    message: format!("type '{name}' does not take type arguments"),
                    span,
                });
            }
            return Ok(Type::Named(name.to_string()));
        }

        if args.is_empty() {
            return Err(SemanticError {
                message: format!("type '{name}' expects {} type arguments", params.len()),
                span,
            });
        }

        if args.len() != params.len() {
            return Err(SemanticError {
                message: format!(
                    "type '{name}' expects {} type arguments, got {}",
                    params.len(),
                    args.len()
                ),
                span,
            });
        }

        let mut resolved_args: Vec<Type> = Vec::with_capacity(args.len());
        for (param, arg) in params.iter().zip(args.iter()) {
            match arg {
                TypeArg::Type(t) => {
                    if let Some(bound) = &param.bound {
                        if !self.traits.contains(&bound.node) {
                            return Err(SemanticError {
                                message: format!(
                                    "unknown trait '{}' in type parameter constraint",
                                    bound.node
                                ),
                                span,
                            });
                        }

                        let resolved = self.resolve_type_ref(t)?;
                        if !type_satisfies_trait(&resolved, &bound.node) {
                            return Err(SemanticError {
                                message: format!(
                                    "type argument does not satisfy trait bound '{}'",
                                    bound.node
                                ),
                                span,
                            });
                        }
                    }
                    resolved_args.push(self.resolve_type_ref(t)?);
                }
                TypeArg::Shape(_) => {
                    return Err(SemanticError {
                        message: "value generics are not supported for nominal types".to_string(),
                        span,
                    })
                }
            }
        }

        Ok(Type::Applied {
            name: name.to_string(),
            args: resolved_args,
        })
    }

    fn resolve_type_ref_with_type_params(
        &self,
        tr: &TypeRef,
        type_params: &HashMap<String, Type>,
    ) -> Result<Type, SemanticError> {
        if tr.args.is_empty() && tr.range.is_none() {
            if let Some(t) = type_params.get(&tr.name.node) {
                return Ok(t.clone());
            }
        }

        let base = match tr.name.node.as_str() {
            "u32" | "Int" => Type::U32,
            "bool" => Type::Bool,
            "String" => Type::String,
            "Style" => Type::Style,
            "Unit" => Type::Unit,
            "Model" => Type::Model,
            "Tensor" => {
                let elem;
                let mut shape: Option<Vec<u64>> = None;

                match tr.args.as_slice() {
                    [] => {
                        elem = Type::Unknown;
                    }
                    [TypeArg::Type(t)] => {
                        elem = self.resolve_type_ref_with_type_params(t, type_params)?;
                    }
                    [TypeArg::Type(t), TypeArg::Shape(dims)] => {
                        elem = self.resolve_type_ref_with_type_params(t, type_params)?;
                        shape = Some(dims.clone());
                    }
                    [TypeArg::Shape(_)] => {
                        return Err(SemanticError {
                            message: "Tensor first type argument must be an element type".to_string(),
                            span: tr.span,
                        })
                    }
                    _ => {
                        return Err(SemanticError {
                            message: "Tensor expects `Tensor<Elem>` or `Tensor<Elem, [d0, d1, ...]>`".to_string(),
                            span: tr.span,
                        })
                    }
                }

                if let Some(dims) = &shape {
                    if dims.iter().any(|d| *d == 0) {
                        return Err(SemanticError {
                            message: "Tensor shape dimensions must be > 0".to_string(),
                            span: tr.span,
                        });
                    }
                }

                Type::Tensor {
                    elem: Box::new(elem),
                    shape,
                }
            }
            other => {
                if tr.args.is_empty() {
                    return self.resolve_type_ref(tr);
                }

                let resolved_args = tr
                    .args
                    .iter()
                    .map(|a| match a {
                        TypeArg::Type(t) => self.resolve_type_ref_with_type_params(t, type_params),
                        TypeArg::Shape(_) => Err(SemanticError {
                            message: "value generics are not supported in this context".to_string(),
                            span: tr.span,
                        }),
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Type::Applied {
                    name: other.to_string(),
                    args: resolved_args,
                }
            }
        };

        if let Some(range) = &tr.range {
            let lo = const_u64(&range.lo).ok_or_else(|| SemanticError {
                message: "range lower-bound must be a constant integer".to_string(),
                span: range.lo.span,
            })?;
            let hi = const_u64(&range.hi).ok_or_else(|| SemanticError {
                message: "range upper-bound must be a constant integer".to_string(),
                span: range.hi.span,
            })?;
            if lo > hi {
                return Err(SemanticError {
                    message: "range lower-bound is greater than upper-bound".to_string(),
                    span: range.span,
                });
            }

            Ok(Type::ConstrainedRange {
                base: Box::new(base),
                lo,
                hi,
            })
        } else {
            Ok(base)
        }
    }

    fn infer_type_args_from_record_literal(
        &mut self,
        def: &RecordDef,
        fields: &[(Ident, Expr)],
    ) -> Result<HashMap<String, Type>, SemanticError> {
        let mut subst: HashMap<String, Type> = HashMap::new();

        for (k, v) in fields {
            let Some(field_def) = def.fields.iter().find(|f| f.name.node == k.node) else {
                continue;
            };

            let actual_ty = self.infer_expr(v)?;
            self.unify_type_params_in_typeref(&field_def.ty, &actual_ty, &def.params, &mut subst)
                .map_err(|mut e| {
                    e.message = format!("field '{}': {}", k.node, e.message);
                    e
                })?;
        }

        Ok(subst)
    }

    fn infer_type_args_from_enum_ctor(
        &mut self,
        def: &EnumDef,
        variant_name: &str,
        arg_exprs: &[&Expr],
    ) -> Result<HashMap<String, Type>, SemanticError> {
        let mut subst: HashMap<String, Type> = HashMap::new();
        let Some(variant) = def.variants.iter().find(|v| v.name.node == variant_name) else {
            return Err(SemanticError {
                message: format!("unknown variant '{variant_name}' for enum '{}'", def.name.node),
                span: def.span,
            });
        };

        for (field_def, arg_expr) in variant.fields.iter().zip(arg_exprs.iter()) {
            let actual_ty = self.infer_expr(arg_expr)?;
            self.unify_type_params_in_typeref(&field_def.ty, &actual_ty, &def.params, &mut subst)?;
        }

        Ok(subst)
    }

    fn unify_type_params_in_typeref(
        &self,
        expected: &TypeRef,
        actual: &Type,
        params: &[aura_ast::TypeParam],
        subst: &mut HashMap<String, Type>,
    ) -> Result<(), SemanticError> {
        let is_param = params.iter().any(|p| p.name.node == expected.name.node);
        if is_param && expected.args.is_empty() && expected.range.is_none() {
            let key = expected.name.node.clone();
            match subst.get(&key) {
                None => {
                    subst.insert(key, actual.clone());
                    return Ok(());
                }
                Some(existing) => {
                    if existing == actual {
                        return Ok(());
                    }
                    if *existing == Type::Unknown {
                        subst.insert(key, actual.clone());
                        return Ok(());
                    }
                    if *actual == Type::Unknown {
                        return Ok(());
                    }
                    return Err(SemanticError {
                        message: format!(
                            "conflicting type argument inference for '{}': saw {} and {}",
                            key,
                            existing.display(),
                            actual.display()
                        ),
                        span: expected.span,
                    });
                }
            }
        }

        if expected.name.node == "Tensor" {
            let Type::Tensor {
                elem: act_elem,
                shape: act_shape,
            } = base_type(actual)
            else {
                return Ok(());
            };

            match expected.args.as_slice() {
                [] => Ok(()),
                [TypeArg::Type(t)] => self.unify_type_params_in_typeref(t, act_elem, params, subst),
                [TypeArg::Type(t), TypeArg::Shape(dims)] => {
                    if let Some(act_dims) = act_shape {
                        if act_dims != dims {
                            return Err(SemanticError {
                                message: "tensor shape mismatch during generic inference".to_string(),
                                span: expected.span,
                            });
                        }
                    }
                    self.unify_type_params_in_typeref(t, act_elem, params, subst)
                }
                _ => Ok(()),
            }
        } else {
            Ok(())
        }
    }

    fn define_val(&mut self, name: &Ident, ty: Type, mutable: bool) -> Result<(), SemanticError> {
        let scope = self.scopes.last_mut().expect("scope stack");
        if scope.contains_key(&name.node) {
            return Err(SemanticError {
                message: format!("val '{}' already defined in this scope", name.node),
                span: name.span,
            });
        }
        scope.insert(name.node.clone(), ty.clone());

        if mutable {
            let m = self.mut_scopes.last_mut().expect("mut scope stack");
            m.insert(name.node.clone());
        }

        // Initialize ownership state (all new bindings start as Owned)
        if let Some(own_scope) = self.ownership_states.last_mut() {
            own_scope.insert(name.node.clone(), OwnershipState::Owned);
        }

        // Capability root.
        let cap_id = self.fresh_cap(name.span);
        self.cap.alloc_root(cap_id, &name.node);
        Ok(())
    }

    fn is_mutable(&self, name: &str) -> bool {
        for scope in self.mut_scopes.iter().rev() {
            if scope.contains(name) {
                return true;
            }
        }
        false
    }

    fn lookup_val(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        None
    }

    fn lookup_scope_index(&self, name: &str) -> Option<usize> {
        for idx in (0..self.scopes.len()).rev() {
            if self.scopes[idx].contains_key(name) {
                return Some(idx);
            }
        }
        None
    }

    fn check_async_capture(&self, name: &str, span: Span) -> Result<(), SemanticError> {
        let Some(&base) = self.async_lambda_bases.last() else {
            return Ok(());
        };
        let Some(def_idx) = self.lookup_scope_index(name) else {
            return Ok(());
        };

        // Captures are names that resolve from an outer scope.
        if def_idx < base {
            if self
                .mut_scopes
                .get(def_idx)
                .is_some_and(|s| s.contains(name))
            {
                return Err(SemanticError {
                    message: format!(
                        "async lambda cannot capture mutable binding '{}' (would be a data race)",
                        name
                    ),
                    span,
                });
            }
        }
        Ok(())
    }

    fn is_non_copy_type(&self, ty: &Type) -> bool {
        fn is_linear_nominal_name(name: &str) -> bool {
            matches!(
                name,
                // Region-based ownership model primitives.
                "Region" | "Socket" | "File" | "Stream" |
                // Collections that will be region-backed (treat as unique until proven otherwise).
                "Vector" | "HashMap" |
                // Namespaced fallbacks (some code paths may keep module-ish prefixes).
                "collections.Vector" | "collections.HashMap"
            )
        }

        match base_type(ty) {
            Type::Named(n) if n.starts_with("<module:") => false,
            Type::Tensor { .. } | Type::Model | Type::Style => true,
            Type::Named(n) => is_linear_nominal_name(n.as_str()),
            Type::Applied { name, .. } => is_linear_nominal_name(name.as_str()),
            _ => false,
        }
    }

    fn consume_move_from_value(&mut self, value_name: &str, span: Span) -> Result<(), SemanticError> {
        // Check linear ownership constraints first
        if let Some(ty) = self.lookup_val(value_name) {
            self.check_not_consumed(value_name, span)?;
            
            // Only mark as consumed if non-copy type
            if self.is_non_copy_type(&ty) {
                self.mark_consumed(value_name, span)?;
            }
        }

        // Also update capability graph
        let from = self.cap.ensure_alive(value_name, span)?;
        let to = self.fresh_cap(span);
        self.cap.consume_move(from, to, span);
        Ok(())
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.mut_scopes.push(HashSet::new());
        self.ownership_states.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
        let _ = self.mut_scopes.pop();
        let _ = self.ownership_states.pop();
    }

    fn instantiate_type_alias(
        &self,
        name: &str,
        def: &TypeAliasDef,
        args: &[TypeArg],
    ) -> Result<TypeRef, SemanticError> {
        if args.len() != def.params.len() {
            return Err(SemanticError {
                message: format!(
                    "type alias '{name}' expects {} type arguments, got {}",
                    def.params.len(),
                    args.len()
                ),
                span: def.target.span,
            });
        }

        let mut subst: HashMap<String, TypeRef> = HashMap::new();
        for (param, arg) in def.params.iter().zip(args.iter()) {
            match arg {
                TypeArg::Type(t) => {
                    // Check trait bound (if any) against the resolved type.
                    if let Some(bound) = &param.bound {
                        if !self.traits.contains(bound) {
                            return Err(SemanticError {
                                message: format!(
                                    "unknown trait '{bound}' in type parameter constraint"
                                ),
                                span: def.target.span,
                            });
                        }

                        let resolved = self.resolve_type_ref(t)?;
                        if !type_satisfies_trait(&resolved, bound) {
                            return Err(SemanticError {
                                message: format!(
                                    "type argument does not satisfy trait bound '{bound}'"
                                ),
                                span: def.target.span,
                            });
                        }
                    }

                    subst.insert(param.name.clone(), (**t).clone());
                }
                TypeArg::Shape(_) => {
                    return Err(SemanticError {
                        message: "generic type arguments must be types; value generics are not supported yet"
                            .to_string(),
                        span: def.target.span,
                    })
                }
            }
        }

        Ok(subst_type_ref(&def.target, &subst))
    }

    fn define_type_placeholder(&mut self, name: &Ident) -> Result<(), SemanticError> {
        // Treat type names as non-assignable placeholders in the value namespace.
        // This enables constructor names like `Option::Some(...)` to type-check as member/call
        // expressions without requiring a separate namespace system.
        self.define_module_placeholder(name)
    }

    // ========== Linear Type Ownership Tracking ==========
    
    /// Get the current ownership state of a variable.
    /// Returns Owned if not yet tracked (first use).
    fn get_ownership(&self, name: &str) -> OwnershipState {
        for scope in self.ownership_states.iter().rev() {
            if let Some(&state) = scope.get(name) {
                return state;
            }
        }
        // Default to Owned if not tracked (shouldn't happen in normal flow)
        OwnershipState::Owned
    }

    /// Set the ownership state of a variable in the current scope.
    fn set_ownership(&mut self, name: &str, state: OwnershipState) {
        if let Some(scope) = self.ownership_states.last_mut() {
            scope.insert(name.to_string(), state);
        }
    }

    /// Mark a variable as consumed (moved) and check it wasn't already consumed.
    /// Only enforces for non-copy types.
    fn mark_consumed(&mut self, name: &str, span: Span) -> Result<(), SemanticError> {
        let current_state = self.get_ownership(name);
        
        // Consumed values cannot be used again (unless type is copy)
        if current_state == OwnershipState::Consumed {
            return Err(SemanticError {
                message: format!("value '{}' used after move (was consumed by previous use)", name),
                span,
            });
        }

        self.set_ownership(name, OwnershipState::Consumed);
        Ok(())
    }

    /// Check that a non-copy value hasn't been consumed yet.
    /// Called before reading/using a value.
    fn check_not_consumed(&self, name: &str, span: Span) -> Result<(), SemanticError> {
        let current_state = self.get_ownership(name);
        
        if current_state == OwnershipState::Consumed {
            return Err(SemanticError {
                message: format!("value '{}' used after move", name),
                span,
            });
        }

        if current_state == OwnershipState::Returned {
            return Err(SemanticError {
                message: format!("value '{}' used after return", name),
                span,
            });
        }

        Ok(())
    }

    /// Enforce linear type rules when using an identifier.
    /// 
    /// For non-copy types, track the use and potentially mark as consumed.
    /// For copy types, allow unrestricted reuse.
    fn enforce_linear_use(&mut self, name: &str, ty: &Type, span: Span) -> Result<(), SemanticError> {
        // Skip enforcement for copy types (u32, bool, etc.)
        if !self.is_non_copy_type(ty) {
            return Ok(());
        }

        // Check value is still usable
        self.check_not_consumed(name, span)?;

        // Track this use for future consumption checks
        // Note: Actual consumption happens in specific contexts (function args, moves, etc.)
        Ok(())
    }

    pub fn enum_variant_info(&self, ty_name: &str, variant: &str) -> Option<(u32, usize)> {
        let def = self.enum_defs.get(ty_name)?;
        let (idx, v) = def
            .variants
            .iter()
            .enumerate()
            .find(|(_i, v)| v.name.node == variant)?;
        Some((idx as u32, v.fields.len()))
    }
}

fn subst_type_ref(tr: &TypeRef, subst: &HashMap<String, TypeRef>) -> TypeRef {
    if tr.args.is_empty() && tr.range.is_none() {
        if let Some(repl) = subst.get(&tr.name.node) {
            return repl.clone();
        }
    }

    let args = tr
        .args
        .iter()
        .map(|a| match a {
            TypeArg::Type(t) => TypeArg::Type(Box::new(subst_type_ref(t, subst))),
            TypeArg::Shape(dims) => TypeArg::Shape(dims.clone()),
        })
        .collect::<Vec<_>>();

    TypeRef {
        span: tr.span,
        name: tr.name.clone(),
        args,
        range: tr.range.clone(),
    }
}

fn refinement_u32_range_from_where(var: &str, expr: &Expr) -> Option<(u64, u64)> {
    // MVP: support conjunctions of comparisons against a literal:
    // - `x >= N`, `x > N`, `x <= N`, `x < N`, `x == N`
    // - combined with `&&`
    fn one(var: &str, expr: &Expr) -> Option<(u64, u64)> {
        let ExprKind::Binary { left, op, right } = &expr.kind else {
            return None;
        };

        let (id, lit, flipped) = match (&left.kind, &right.kind) {
            (ExprKind::Ident(id), ExprKind::IntLit(n)) => (id, *n, false),
            (ExprKind::IntLit(n), ExprKind::Ident(id)) => (id, *n, true),
            _ => return None,
        };

        if id.node != var {
            return None;
        }

        let (op, n) = if flipped {
            // Reverse operator when swapping sides.
            let rop = match op {
                BinOp::Lt => BinOp::Gt,
                BinOp::Le => BinOp::Ge,
                BinOp::Gt => BinOp::Lt,
                BinOp::Ge => BinOp::Le,
                BinOp::Eq => BinOp::Eq,
                BinOp::Ne => BinOp::Ne,
                other => *other,
            };
            (rop, lit)
        } else {
            (*op, lit)
        };

        match op {
            BinOp::Ge => Some((n, U32_MAX)),
            BinOp::Gt => Some((n.saturating_add(1), U32_MAX)),
            BinOp::Le => Some((0, n)),
            BinOp::Lt => Some((0, n.saturating_sub(1))),
            BinOp::Eq => Some((n, n)),
            _ => None,
        }
    }

    match &expr.kind {
        ExprKind::Binary { op: BinOp::And, left, right } => {
            let (l_lo, l_hi) = one(var, left)?;
            let (r_lo, r_hi) = one(var, right)?;
            Some((l_lo.max(r_lo), l_hi.min(r_hi)))
        }
        _ => one(var, expr),
    }
}

fn type_satisfies_trait(ty: &Type, tr: &str) -> bool {
    // MVP built-in trait satisfaction table.
    match tr {
        "Numeric" => matches!(base_type(ty), Type::U32),
        "Scalar" => matches!(base_type(ty), Type::U32 | Type::Bool | Type::String | Type::Style),
        "Eq" => matches!(base_type(ty), Type::U32 | Type::Bool | Type::String),
        _ => false,
    }
}

impl Checker {
    fn resolve_call_args_against_sig<'a>(
        &self,
        call_span: Span,
        args: &'a [CallArg],
        sig: &FnSig,
    ) -> Result<Vec<&'a Expr>, SemanticError> {
        let mut ordered: Vec<Option<&'a Expr>> = vec![None; sig.params.len()];
        let mut next_pos = 0usize;

        for a in args {
            match a {
                CallArg::Positional(e) => {
                    while next_pos < ordered.len() && ordered[next_pos].is_some() {
                        next_pos += 1;
                    }
                    if next_pos >= ordered.len() {
                        return Err(SemanticError {
                            message: format!(
                                "wrong number of arguments: expected {}, got {}",
                                sig.params.len(),
                                args.len()
                            ),
                            span: call_span,
                        });
                    }
                    ordered[next_pos] = Some(e);
                    next_pos += 1;
                }
                CallArg::Named { name, value } => {
                    let idx = sig
                        .params
                        .iter()
                        .position(|p| p.name == name.node)
                        .ok_or_else(|| SemanticError {
                            message: format!("unknown named argument '{}'", name.node),
                            span: name.span,
                        })?;
                    if ordered[idx].is_some() {
                        return Err(SemanticError {
                            message: format!("duplicate argument '{}'", name.node),
                            span: name.span,
                        });
                    }
                    ordered[idx] = Some(value);
                }
            }
        }

        if ordered.iter().any(|o| o.is_none()) {
            return Err(SemanticError {
                message: format!(
                    "wrong number of arguments: expected {}, got {}",
                    sig.params.len(),
                    args.len()
                ),
                span: call_span,
            });
        }

        Ok(ordered.into_iter().map(|o| o.expect("filled")).collect())
    }
}

fn const_u64(expr: &Expr) -> Option<u64> {
    match expr.kind {
        ExprKind::IntLit(n) => Some(n),
        _ => None,
    }
}

fn expr_to_callee_name(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Ident(id) => id.node.clone(),
        ExprKind::Member { base, member } => format!("{}.{}", expr_to_callee_name(base), member.node),
        _ => "<unknown>".to_string(),
    }
}

fn collect_value_idents(expr: &Expr, out: &mut Vec<Ident>) {
    match &expr.kind {
        ExprKind::Ident(id) => out.push(id.clone()),
        ExprKind::StyleLit { fields } => {
            for (_k, v) in fields {
                collect_value_idents(v, out);
            }
        }
        ExprKind::Unary { expr, .. } => collect_value_idents(expr, out),
        ExprKind::Binary { left, right, .. } => {
            collect_value_idents(left, out);
            collect_value_idents(right, out);
        }
        ExprKind::Member { base, .. } => collect_value_idents(base, out),
        ExprKind::Call { args, trailing, .. } => {
            for a in args {
                collect_value_idents(call_arg_value(a), out);
            }
            if let Some(tb) = trailing {
                for s in &tb.stmts {
                    if let Stmt::ExprStmt(e) = s {
                        collect_value_idents(e, out);
                    }
                }
                if let Some(y) = &tb.yield_expr {
                    collect_value_idents(y, out);
                }
            }
        }
        ExprKind::Lambda { body, .. } => {
            for s in &body.stmts {
                if let Stmt::ExprStmt(e) = s {
                    collect_value_idents(e, out);
                }
            }
            if let Some(y) = &body.yield_expr {
                collect_value_idents(y, out);
            }
        }
        ExprKind::Flow { left, right, .. } => {
            collect_value_idents(left, out);
            collect_value_idents(right, out);
        }
        ExprKind::RecordLit { fields, .. } => {
            for (_k, v) in fields {
                collect_value_idents(v, out);
            }
        }
        ExprKind::ForAll { binders, body } | ExprKind::Exists { binders, body } => {
            let mut tmp = Vec::new();
            collect_value_idents(body, &mut tmp);
            let bound: std::collections::BTreeSet<String> = binders
                .iter()
                .map(|b| b.name.node.clone())
                .collect();
            out.extend(tmp.into_iter().filter(|id| !bound.contains(&id.node)));
        }
        ExprKind::IntLit(_) | ExprKind::StringLit(_) => {}
    }
}

fn call_arg_value(arg: &CallArg) -> &Expr {
    match arg {
        CallArg::Positional(e) => e,
        CallArg::Named { value, .. } => value,
    }
}
