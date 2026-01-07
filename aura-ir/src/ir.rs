#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use aura_ast::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunctionId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionHint {
    Sequential,
    Parallel,
    Predictive,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlowKind {
    Sync,
    Async,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Unit,
    Bool,
    U32,
    String,
    Tensor,
    Opaque(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,

    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,

    And,
    Or,
}

#[derive(Clone, Debug)]
pub struct ModuleIR {
    pub functions: BTreeMap<String, FunctionIR>,
    pub externs: BTreeMap<String, ExternFnSig>,
}

impl ModuleIR {
    pub fn new() -> Self {
        Self {
            functions: BTreeMap::new(),
            externs: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternFnSig {
    pub params: Vec<Type>,
    pub ret: Type,
    pub call_conv: CallConv,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallConv {
    C,
    Stdcall,
}

#[derive(Clone, Debug)]
pub struct FunctionIR {
    pub name: String,
    pub span: Span,
    pub params: Vec<Param>,
    pub ret: Type,
    pub blocks: Vec<BasicBlock>,
    pub entry: BlockId,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub span: Span,
    pub value: ValueId,
}

#[derive(Clone, Debug)]
pub struct BasicBlock {
    pub id: BlockId,
    pub span: Span,
    pub hint: ExecutionHint,
    pub insts: Vec<Inst>,
    pub term: Terminator,
}

#[derive(Clone, Debug)]
pub struct Local {
    pub name: String,
    pub value: ValueId,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum InstKind {
    /// Create a capability handle for a local value.
    AllocCapability { name: String },

    /// Bind a strand (immutable value) to an SSA value.
    BindStrand { name: String, expr: RValue },

    /// Call a function/cell.
    Call {
        callee: String,
        args: Vec<ValueId>,
    },

    /// A compute kernel / async flow node.
    ComputeKernel {
        callee: String,
        args: Vec<ValueId>,
    },

    /// Range/bounds check inserted by verifier.
    RangeCheckU32 { value: ValueId, lo: u64, hi: u64 },

    /// Unary operator.
    Unary { op: UnaryOp, operand: ValueId },

    /// Binary operator.
    Binary {
        op: BinOp,
        left: ValueId,
        right: ValueId,
    },

    /// Merge values from predecessor blocks.
    Phi {
        incomings: Vec<(BlockId, ValueId)>,
    },
}

#[derive(Clone, Debug)]
pub struct Inst {
    pub span: Span,
    pub dest: Option<ValueId>,
    pub kind: InstKind,
}

#[derive(Clone, Debug)]
pub enum RValue {
    ConstU32(u64),
    ConstBool(bool),
    ConstString(String),
    Local(ValueId),
}

#[derive(Clone, Debug)]
pub enum Terminator {
    Return(Option<ValueId>),

    Br(BlockId),

    CondBr {
        cond: ValueId,
        then_bb: BlockId,
        else_bb: BlockId,
    },

    Switch {
        scrut: ValueId,
        default_bb: BlockId,
        cases: Vec<(u64, BlockId)>,
    },
}

#[derive(Default, Debug)]
pub struct IdGen {
    next_block: u32,
    next_value: u32,
    next_cap: u32,
}

impl IdGen {
    pub fn fresh_block(&mut self) -> BlockId {
        let id = BlockId(self.next_block);
        self.next_block += 1;
        id
    }

    pub fn fresh_value(&mut self) -> ValueId {
        let id = ValueId(self.next_value);
        self.next_value += 1;
        id
    }

    pub fn fresh_capability(&mut self) -> CapabilityId {
        let id = CapabilityId(self.next_cap);
        self.next_cap += 1;
        id
    }
}
