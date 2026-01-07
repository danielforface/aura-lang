use aura_ir::{
    BinOp, BlockId, ExecutionHint, FunctionIR, Inst, InstKind, ModuleIR, Param, RValue, Terminator,
    Type, UnaryOp, ValueId,
};

fn span0() -> aura_ast::Span {
    aura_ast::Span::new(miette::SourceOffset::from(0usize), 0usize)
}

fn module_with_single_function(insts: Vec<Inst>, term: Terminator) -> ModuleIR {
    let mut m = ModuleIR::new();

    let entry = BlockId(0);
    let f = FunctionIR {
        name: "main".to_string(),
        span: span0(),
        params: Vec::new(),
        ret: Type::Unit,
        entry,
        blocks: vec![aura_ir::BasicBlock {
            id: entry,
            span: span0(),
            hint: ExecutionHint::Sequential,
            insts,
            term,
        }],
    };

    m.functions.insert("main".to_string(), f);
    m
}

fn backend_ok_or_explicit_reject(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("unsupported") || lower.contains("unimplemented") || lower.contains("not enabled")
}

#[test]
fn conformance_emit_c_backend_for_ir_features() {
    let mut cases: Vec<(&str, ModuleIR)> = Vec::new();

    // BindStrand const + unary + binary.
    cases.push((
        "const_unary_binary",
        module_with_single_function(
            vec![
                Inst {
                    span: span0(),
                    dest: Some(ValueId(0)),
                    kind: InstKind::BindStrand {
                        name: "x".to_string(),
                        expr: RValue::ConstU32(5),
                    },
                },
                Inst {
                    span: span0(),
                    dest: Some(ValueId(1)),
                    kind: InstKind::Unary {
                        op: UnaryOp::Neg,
                        operand: ValueId(0),
                    },
                },
                Inst {
                    span: span0(),
                    dest: Some(ValueId(2)),
                    kind: InstKind::Binary {
                        op: BinOp::Add,
                        left: ValueId(0),
                        right: ValueId(0),
                    },
                },
            ],
            Terminator::Return(None),
        ),
    ));

    // Range check.
    cases.push((
        "range_check_u32",
        module_with_single_function(
            vec![
                Inst {
                    span: span0(),
                    dest: Some(ValueId(0)),
                    kind: InstKind::BindStrand {
                        name: "x".to_string(),
                        expr: RValue::ConstU32(1),
                    },
                },
                Inst {
                    span: span0(),
                    dest: None,
                    kind: InstKind::RangeCheckU32 {
                        value: ValueId(0),
                        lo: 0,
                        hi: 10,
                    },
                },
            ],
            Terminator::Return(None),
        ),
    ));

    // Call to a known builtin symbol.
    cases.push((
        "call_builtin_println",
        {
            let mut m = ModuleIR::new();
            let entry = BlockId(0);
            let f = FunctionIR {
                name: "main".to_string(),
                span: span0(),
                params: Vec::new(),
                ret: Type::Unit,
                entry,
                blocks: vec![aura_ir::BasicBlock {
                    id: entry,
                    span: span0(),
                    hint: ExecutionHint::Sequential,
                    insts: vec![
                        Inst {
                            span: span0(),
                            dest: Some(ValueId(0)),
                            kind: InstKind::BindStrand {
                                name: "s".to_string(),
                                expr: RValue::ConstString("hello".to_string()),
                            },
                        },
                        Inst {
                            span: span0(),
                            dest: None,
                            kind: InstKind::Call {
                                callee: "io.println".to_string(),
                                args: vec![ValueId(0)],
                            },
                        },
                    ],
                    term: Terminator::Return(None),
                }],
            };
            m.functions.insert("main".to_string(), f);
            m
        },
    ));

    for (name, module) in cases {
        match aura_backend_c::emit_module(&module, None) {
            Ok(_a) => {}
            Err(e) => {
                assert!(
                    backend_ok_or_explicit_reject(&e.message),
                    "C backend must either implement or explicitly reject IR feature set '{name}'; got: {e:?}"
                );
            }
        }
    }
}

#[test]
fn conformance_emit_llvm_backend_for_ir_features_or_explicit_reject() {
    let module = module_with_single_function(
        vec![Inst {
            span: span0(),
            dest: Some(ValueId(0)),
            kind: InstKind::BindStrand {
                name: "x".to_string(),
                expr: RValue::ConstU32(1),
            },
        }],
        Terminator::Return(None),
    );

    match aura_backend_llvm::emit_llvm_ir(&module, None) {
        Ok(_a) => {}
        Err(e) => {
            assert!(
                backend_ok_or_explicit_reject(&e.message),
                "LLVM backend must either implement or explicitly reject; got: {e:?}"
            );
        }
    }
}

#[test]
fn conformance_phi_and_branches_do_not_panic_in_c_backend() {
    // Build a tiny CFG: entry -> (then/else) -> join, with a phi in join.
    let mut m = ModuleIR::new();

    let entry = BlockId(0);
    let then_bb = BlockId(1);
    let else_bb = BlockId(2);
    let join_bb = BlockId(3);

    let cond_v = ValueId(0);
    let then_v = ValueId(1);
    let else_v = ValueId(2);
    let phi_v = ValueId(3);

    let blocks = vec![
        aura_ir::BasicBlock {
            id: entry,
            span: span0(),
            hint: ExecutionHint::Sequential,
            insts: vec![Inst {
                span: span0(),
                dest: Some(cond_v),
                kind: InstKind::BindStrand {
                    name: "c".to_string(),
                    expr: RValue::ConstBool(true),
                },
            }],
            term: Terminator::CondBr {
                cond: cond_v,
                then_bb,
                else_bb,
            },
        },
        aura_ir::BasicBlock {
            id: then_bb,
            span: span0(),
            hint: ExecutionHint::Sequential,
            insts: vec![Inst {
                span: span0(),
                dest: Some(then_v),
                kind: InstKind::BindStrand {
                    name: "t".to_string(),
                    expr: RValue::ConstU32(1),
                },
            }],
            term: Terminator::Br(join_bb),
        },
        aura_ir::BasicBlock {
            id: else_bb,
            span: span0(),
            hint: ExecutionHint::Sequential,
            insts: vec![Inst {
                span: span0(),
                dest: Some(else_v),
                kind: InstKind::BindStrand {
                    name: "e".to_string(),
                    expr: RValue::ConstU32(2),
                },
            }],
            term: Terminator::Br(join_bb),
        },
        aura_ir::BasicBlock {
            id: join_bb,
            span: span0(),
            hint: ExecutionHint::Sequential,
            insts: vec![Inst {
                span: span0(),
                dest: Some(phi_v),
                kind: InstKind::Phi {
                    incomings: vec![(then_bb, then_v), (else_bb, else_v)],
                },
            }],
            term: Terminator::Return(None),
        },
    ];

    let f = FunctionIR {
        name: "main".to_string(),
        span: span0(),
        params: vec![Param {
            name: "dummy".to_string(),
            ty: Type::Unit,
            span: span0(),
            value: ValueId(999),
        }]
        .into_iter()
        .filter(|_| false)
        .collect(),
        ret: Type::Unit,
        blocks,
        entry,
    };

    m.functions.insert("main".to_string(), f);

    // Must not panic; error is allowed only if explicit.
    match aura_backend_c::emit_module(&m, None) {
        Ok(_a) => {}
        Err(e) => {
            assert!(
                backend_ok_or_explicit_reject(&e.message),
                "C backend must either implement or explicitly reject; got: {e:?}"
            );
        }
    }
}
