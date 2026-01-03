use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::util::{block_get, stack_get, tmp_var_new};

pub fn assign_parse(
    block_idx: &mut NodeIndex,
    tmp: IRVar,
    is_owned: bool,
    t_succeeded: Option<IRVar>,
    cond_rest: bool,
    target: &CSTExpression,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    match target {
        CSTExpression::Accessible(a) => {
            assert!(t_succeeded.is_none());

            let mut t_n: IRVar;
            if let CSTExpression::Variable(v) = *a.head.clone() {
                if let Some(tmp) = stack_get(shared_proc, &v) {
                    t_n = tmp;
                } else {
                    // t_n := stack_get_assert(v);
                    t_n = tmp_var_new(proc);
                    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_n),
                        types: IRType::PTR,
                        source: IRValue::BuiltinProc(BuiltinProc::StackGetAssert),
                        op: IROp::NativeCall(vec![IRValue::String(v.clone())]),
                    }));
                }
            } else {
                panic!("assign target accessible head must be variable");
            }

            for i in &a.body {
                match i {
                    CSTExpression::Variable(v) => {
                        /* //native call
                         * t_o := object_get_assert(t_n, v);
                         */
                        let t_o = tmp_var_new(proc);
                        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_o),
                            types: IRType::PTR,
                            source: IRValue::BuiltinProc(BuiltinProc::ObjectGetAssert),
                            op: IROp::NativeCall(vec![
                                IRValue::Variable(t_n),
                                IRValue::String(v.clone()),
                            ]),
                        }));
                        t_n = t_o;
                    }
                    CSTExpression::Collection(l) => {
                        if let CSTCollection::Set(s) = l
                            && s.range.is_none()
                            && s.rest.is_none()
                            && s.expressions.len() == 1
                        {
                            /* t_expr := // s.expressions[0]
                             * t_o := set_list_get(t_n, t_expr);
                             * _ := invalidate(t_expr);
                             */
                            let t_expr = tmp_var_new(proc);
                            let t_o = tmp_var_new(proc);

                            let expr_owned = block_expr_push(
                                &s.expressions[0],
                                block_idx,
                                IRTarget::Variable(t_expr),
                                proc,
                                shared_proc,
                                cfg,
                            );

                            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_o),
                                types: IRTypes!("any"),
                                source: IRValue::BuiltinProc(BuiltinProc::SetListGet),
                                op: IROp::NativeCall(vec![
                                    IRValue::Variable(t_n),
                                    IRValue::Variable(t_expr),
                                ]),
                            }));

                            if expr_owned {
                                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                                    target: IRTarget::Ignore,
                                    types: IRType::UNDEFINED,
                                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                    op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
                                }));
                            }

                            t_n = t_o;
                        } else if let CSTCollection::List(s) = l
                            && s.range.is_none()
                            && s.rest.is_none()
                        {
                            if s.expressions.len() == 1 {
                                /* t_expr := // s.expressions[0]
                                 * t_expr_new := t_expr - 1;
                                 * t_o := t_n[t_expr_new];
                                 * _ := invalidate(t_expr);
                                 * _ := invalidate(t_expr_new);
                                 */
                                let t_expr = tmp_var_new(proc);
                                let t_expr_new = tmp_var_new(proc);
                                let t_o = tmp_var_new(proc);
                                block_expr_push(
                                    &s.expressions[0],
                                    block_idx,
                                    IRTarget::Variable(t_expr),
                                    proc,
                                    shared_proc,
                                    cfg,
                                );
                                block_get(proc, *block_idx).extend(vec![
                                    IRStmt::Assign(IRAssign {
                                        target: IRTarget::Variable(t_expr_new),
                                        types: IRTypes!("minus"),
                                        source: IRValue::Variable(t_expr),
                                        op: IROp::Minus(IRValue::Number(1.into())),
                                    }),
                                    IRStmt::Assign(IRAssign {
                                        target: IRTarget::Variable(t_o),
                                        types: IRType::PTR,
                                        source: IRValue::Variable(t_n),
                                        op: IROp::AccessArray(IRValue::Variable(t_expr_new)),
                                    }),
                                    IRStmt::Assign(IRAssign {
                                        target: IRTarget::Ignore,
                                        types: IRType::UNDEFINED,
                                        source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                        op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
                                    }),
                                    IRStmt::Assign(IRAssign {
                                        target: IRTarget::Ignore,
                                        types: IRType::UNDEFINED,
                                        source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                        op: IROp::NativeCall(vec![IRValue::Variable(t_expr_new)]),
                                    }),
                                ]);
                                t_n = t_o;
                            } else {
                                /* t_list := list_new(s.expressions.len());
                                 * t_list_addr := &t_list;
                                 * t_expr := // s.expressions[0];
                                 * t_list_elem := t_list_addr[0];
                                 * *t_list_elem := t_expr;
                                 * //...
                                 * t_o := set_list_get(t_n, t_list);
                                 * _ := invalidate(t_list);
                                 */
                                let t_list = tmp_var_new(proc);
                                let t_list_addr = tmp_var_new(proc);
                                block_get(proc, *block_idx).extend(vec![IRStmt::Assign(
                                    IRAssign {
                                        target: IRTarget::Variable(t_list),
                                        types: IRType::LIST,
                                        source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                                        op: IROp::NativeCall(vec![IRValue::Number(
                                            s.expressions.len().into(),
                                        )]),
                                    },
                                )]);

                                for (idx, i) in s.expressions.iter().enumerate() {
                                    let t_expr = tmp_var_new(proc);
                                    let t_list_elem = tmp_var_new(proc);
                                    block_expr_push(
                                        i,
                                        block_idx,
                                        IRTarget::Variable(t_expr),
                                        proc,
                                        shared_proc,
                                        cfg,
                                    );
                                    block_get(proc, *block_idx).extend(vec![
                                        IRStmt::Assign(IRAssign {
                                            target: IRTarget::Variable(t_list_elem),
                                            types: IRType::PTR,
                                            source: IRValue::Variable(t_list_addr),
                                            op: IROp::AccessArray(IRValue::Variable(idx)),
                                        }),
                                        IRStmt::Assign(IRAssign {
                                            target: IRTarget::Deref(t_list_elem),
                                            types: IRTypes!("any"),
                                            source: IRValue::Variable(t_expr),
                                            op: IROp::Assign,
                                        }),
                                    ]);
                                }

                                let t_o = tmp_var_new(proc);
                                block_get(proc, *block_idx).extend(vec![
                                    IRStmt::Assign(IRAssign {
                                        target: IRTarget::Variable(t_o),
                                        types: IRType::PTR,
                                        source: IRValue::BuiltinProc(BuiltinProc::SetListGet),
                                        op: IROp::NativeCall(vec![
                                            IRValue::Variable(t_n),
                                            IRValue::Variable(t_list),
                                        ]),
                                    }),
                                    IRStmt::Assign(IRAssign {
                                        target: IRTarget::Ignore,
                                        types: IRType::UNDEFINED,
                                        source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                        op: IROp::NativeCall(vec![IRValue::Variable(t_list)]),
                                    }),
                                ]);

                                t_n = t_o;
                            }
                        } else {
                            panic!(
                                "assign target accessible body collection must be list singleton containing a non-zero number"
                            );
                        }
                    }
                    _ => {
                        panic!("assign target accessible body must be variable or list");
                    }
                }
            }

            if !is_owned {
                // *t_n := copy(tmp);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Deref(t_n),
                    types: IRTypes!("any"),
                    source: IRValue::BuiltinProc(BuiltinProc::Copy),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }));
            } else {
                // *t_n := tmp;
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Deref(t_n),
                    types: IRTypes!("any"),
                    source: IRValue::Variable(tmp),
                    op: IROp::Assign,
                }));
            }
        }
        CSTExpression::Collection(c) => {
            let (expr, rest) = if let CSTCollection::List(l) = c
                && l.range.is_none()
            {
                (
                    l.expressions.iter().collect::<Vec<&CSTExpression>>(),
                    &l.rest,
                )
            } else if let CSTCollection::Set(l) = c
                && l.range.is_none()
            {
                (
                    l.expressions.iter().collect::<Vec<&CSTExpression>>(),
                    &l.rest,
                )
            } else {
                panic!("assign target must be list or variable");
            };

            let t_len = if t_succeeded.is_some() {
                tmp_var_new(proc)
            } else {
                0
            };
            let t_len_cond = if t_succeeded.is_some() {
                tmp_var_new(proc)
            } else {
                0
            };

            if t_succeeded.is_some() {
                // t_len := amount(tmp);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_len),
                    types: IRType::NUMBER,
                    source: IRValue::BuiltinProc(BuiltinProc::Amount),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }));

                if cond_rest || rest.is_some() {
                    /* t_1 := expr.len() < t_len;
                     * t_2 := expr.len() == t_len;
                     * t_len_cond := t1 || t2;
                     */
                    let t_1 = tmp_var_new(proc);
                    let t_2 = tmp_var_new(proc);
                    block_get(proc, *block_idx).extend(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_1),
                            types: IRType::BOOL,
                            source: IRValue::Number(expr.len().into()),
                            op: IROp::Less(IRValue::Variable(t_len)),
                        }),
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_2),
                            types: IRType::BOOL,
                            source: IRValue::Number(expr.len().into()),
                            op: IROp::Equal(IRValue::Variable(t_len)),
                        }),
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_len_cond),
                            types: IRType::BOOL,
                            source: IRValue::Variable(t_1),
                            op: IROp::Or(IRValue::Variable(t_2)),
                        }),
                    ]);
                } else {
                    // t_len_cond := t_len == expr.len();
                    block_get(proc, *block_idx).extend(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_len_cond),
                            types: IRType::BOOL,
                            source: IRValue::Number(expr.len().into()),
                            op: IROp::Equal(IRValue::Variable(t_len)),
                        }),
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Ignore,
                            types: IRType::UNDEFINED,
                            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                            op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
                        }),
                    ]);
                }

                // _ := invalidate(t_len);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
                }));
            }

            let mut assign_idx = if let Some(t_succeeded_val) = t_succeeded {
                proc.blocks.add_node(vec![IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_succeeded_val),
                    types: IRType::BOOL,
                    source: IRValue::Bool(true),
                    op: IROp::Assign,
                })])
            } else {
                *block_idx
            };
            let mut follow_idx = NodeIndex::new(0);
            if let Some(t_succeeded_val) = t_succeeded {
                follow_idx = proc.blocks.add_node(Vec::new());
                let fail_idx = proc.blocks.add_node(vec![
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_succeeded_val),
                        types: IRType::BOOL,
                        source: IRValue::Bool(false),
                        op: IROp::Assign,
                    }),
                    IRStmt::Goto(follow_idx),
                ]);

                proc.blocks.add_edge(fail_idx, follow_idx, ());

                block_get(proc, *block_idx).push(IRStmt::Branch(IRBranch {
                    cond: IRValue::Variable(t_len_cond),
                    success: assign_idx,
                    failure: fail_idx,
                }));

                proc.blocks.add_edge(*block_idx, assign_idx, ());
                proc.blocks.add_edge(*block_idx, fail_idx, ());
            }

            for (idx, i) in expr.iter().enumerate() {
                // t_1 := tmp[idx];
                let t_1 = tmp_var_new(proc);
                block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_1),
                    types: IRTypes!("any"),
                    source: IRValue::Variable(tmp),
                    op: IROp::AccessArray(IRValue::Number(idx.into())),
                }));

                assign_parse(
                    &mut assign_idx,
                    t_1,
                    false,
                    t_succeeded,
                    false,
                    i,
                    proc,
                    shared_proc,
                    cfg,
                );
            }

            if let Some(r) = rest {
                // t_1 := slice(tmp, expr.len(), -1);
                let t_1 = tmp_var_new(proc);
                block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_1),
                    types: IRType::LIST | IRType::SET,
                    source: IRValue::BuiltinProc(BuiltinProc::Slice),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(tmp),
                        IRValue::Number(expr.len().into()),
                        IRValue::Number((-1).into()),
                    ]),
                }));

                assign_parse(
                    &mut assign_idx,
                    t_1,
                    false,
                    t_succeeded,
                    false,
                    r,
                    proc,
                    shared_proc,
                    cfg,
                );
            }

            if is_owned {
                // _ := invalidate(tmp);
                block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }));
            }

            if t_succeeded.is_some() {
                block_get(proc, assign_idx).push(IRStmt::Goto(follow_idx));
                proc.blocks.add_edge(assign_idx, follow_idx, ());
                *block_idx = follow_idx;
            }
        }
        CSTExpression::Variable(v) => {
            let var_idx = if let Some(t) = stack_get(shared_proc, v) {
                t
            } else {
                let t = tmp_var_new(proc);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t),
                    types: IRType::PTR,
                    source: IRValue::BuiltinProc(BuiltinProc::StackGetAssert),
                    op: IROp::NativeCall(vec![IRValue::String(v.to_string())]),
                }));
                t
            };
            if is_owned {
                // *var_idx := tmp;
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Deref(var_idx),
                    types: IRTypes!("any"),
                    source: IRValue::Variable(tmp),
                    op: IROp::Assign,
                }));
            } else {
                // *var_idx := copy(tmp);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Deref(var_idx),
                    types: IRTypes!("any"),
                    source: IRValue::BuiltinProc(BuiltinProc::Copy),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }));
            }
        }
        CSTExpression::Term(t) => {
            /*  t_kind := term_kind_eq(tmp, t.is_tterm, t.name);
             *  t_len := amount(tmp);
             *  t_len_check := t_len == t.params.len() + 1;
             *  t_check := t_kind && t_len_check;
             *  t_succeeded := t_check;
             *  if t_check
             *    goto <assign_idx>
             *  else
             *    goto <follow_idx>
             *
             * <assign_idx>:
             *  // assign parse
             *  goto <follow_idx>
             *
             * <follow_idx>:
             * _ := invalidate(t_len);
             * _ := invalidate(tmp);
             */
            let t_kind = tmp_var_new(proc);
            let t_len = tmp_var_new(proc);
            let t_len_check = tmp_var_new(proc);
            let t_check = tmp_var_new(proc);

            let mut assign_idx = if t_succeeded.is_some() {
                proc.blocks.add_node(Vec::new())
            } else {
                *block_idx
            };
            let mut follow_idx = if t_succeeded.is_some() {
                proc.blocks.add_node(Vec::new())
            } else {
                *block_idx
            };

            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_kind),
                    types: IRType::BOOL,
                    source: IRValue::BuiltinProc(BuiltinProc::TermKindEq),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(tmp),
                        IRValue::Bool(t.is_tterm),
                        IRValue::String(t.name.to_string()),
                    ]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_len),
                    types: IRType::NUMBER,
                    source: IRValue::BuiltinProc(BuiltinProc::Amount),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_len_check),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_len),
                    op: IROp::Equal(IRValue::Number((t.params.len() + 1).into())),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_check),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_kind),
                    op: IROp::And(IRValue::Variable(t_len_check)),
                }),
                if let Some(t_s) = t_succeeded {
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_s),
                        types: IRType::BOOL,
                        source: IRValue::Variable(t_check),
                        op: IROp::Assign,
                    })
                } else {
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Ignore,
                        types: IRType::UNDEFINED,
                        source: IRValue::BuiltinProc(BuiltinProc::Assert),
                        op: IROp::NativeCall(vec![IRValue::Variable(t_check)]),
                    })
                },
            ]);

            if t_succeeded.is_some() {
                block_get(proc, *block_idx).push(IRStmt::Branch(IRBranch {
                    cond: IRValue::Variable(t_check),
                    success: assign_idx,
                    failure: follow_idx,
                }));

                proc.blocks.add_edge(*block_idx, assign_idx, ());
                proc.blocks.add_edge(*block_idx, follow_idx, ());
            }

            t.params.iter().enumerate().for_each(|(idx, i)| {
                // t_1 := tmp[idx + 1];
                let t_1 = tmp_var_new(proc);
                block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_1),
                    types: IRTypes!("any"),
                    source: IRValue::Variable(tmp),
                    op: IROp::AccessArray(IRValue::Number((idx + 1).into())),
                }));

                assign_parse(
                    &mut assign_idx,
                    t_1,
                    false,
                    t_succeeded,
                    false,
                    i,
                    proc,
                    shared_proc,
                    cfg,
                );
            });

            if t_succeeded.is_some() {
                block_get(proc, assign_idx).push(IRStmt::Goto(follow_idx));
                proc.blocks.add_edge(assign_idx, follow_idx, ());
                *block_idx = follow_idx;
            } else {
                follow_idx = assign_idx;
                *block_idx = assign_idx;
            }

            block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
            }));

            if is_owned {
                block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }));
            }
        }
        CSTExpression::Op(o) => {
            /*  t_kind_check := ast_node_kind_str_eq(o.op.to_string(), tmp);
             *  t_len := amount(tmp);
             *  t_len_check := t_len == 3;
             *  t_check := t_kind_check && t_len_check;
             *  t_succeeded := t_check;
             *  if t_check
             *   goto <assign_idx>
             *  else
             *   goto <follow_idx>
             * <assign_idx>:
             *  t_lhs := tmp[1];
             *  // assign parse
             *  t_rhs := tmp[2];
             *  // assign parse
             *  goto <follow idx>
             *
             * <follow_idx>:
             * _ := invalidate(t_len);
             * _ := invalidate(tmp);
             */
            let t_kind_check = tmp_var_new(proc);
            let t_len = tmp_var_new(proc);
            let t_len_check = tmp_var_new(proc);
            let t_check = tmp_var_new(proc);

            let mut assign_idx = if t_succeeded.is_some() {
                proc.blocks.add_node(Vec::new())
            } else {
                *block_idx
            };
            let mut follow_idx = if t_succeeded.is_some() {
                proc.blocks.add_node(Vec::new())
            } else {
                *block_idx
            };

            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_kind_check),
                    types: IRType::BOOL,
                    source: IRValue::BuiltinProc(BuiltinProc::AstNodeKindStrEq),
                    op: IROp::NativeCall(vec![
                        IRValue::String(o.op.to_string()),
                        IRValue::Variable(tmp),
                    ]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_len),
                    types: IRType::NUMBER,
                    source: IRValue::BuiltinProc(BuiltinProc::Amount),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_len_check),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_len),
                    op: IROp::Equal(IRValue::Number(3.into())),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_check),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_kind_check),
                    op: IROp::And(IRValue::Variable(t_len_check)),
                }),
                if let Some(t_s) = t_succeeded {
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_s),
                        types: IRType::BOOL,
                        source: IRValue::Variable(t_check),
                        op: IROp::Assign,
                    })
                } else {
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Ignore,
                        types: IRType::UNDEFINED,
                        source: IRValue::BuiltinProc(BuiltinProc::Assert),
                        op: IROp::NativeCall(vec![IRValue::Variable(t_check)]),
                    })
                },
            ]);

            if t_succeeded.is_some() {
                block_get(proc, *block_idx).push(IRStmt::Branch(IRBranch {
                    cond: IRValue::Variable(t_check),
                    success: assign_idx,
                    failure: follow_idx,
                }));

                proc.blocks.add_edge(*block_idx, assign_idx, ());
                proc.blocks.add_edge(*block_idx, follow_idx, ());
            }

            let t_lhs = tmp_var_new(proc);
            let t_rhs = tmp_var_new(proc);

            block_get(proc, assign_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_lhs),
                    types: IRTypes!("any"),
                    source: IRValue::Variable(tmp),
                    op: IROp::AccessArray(IRValue::Number(1.into())),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_rhs),
                    types: IRTypes!("any"),
                    source: IRValue::Variable(tmp),
                    op: IROp::AccessArray(IRValue::Number(2.into())),
                }),
            ]);

            assign_parse(
                &mut assign_idx,
                t_lhs,
                false,
                t_succeeded,
                false,
                &o.left,
                proc,
                shared_proc,
                cfg,
            );

            assign_parse(
                &mut assign_idx,
                t_rhs,
                false,
                t_succeeded,
                false,
                &o.right,
                proc,
                shared_proc,
                cfg,
            );

            if t_succeeded.is_some() {
                block_get(proc, assign_idx).push(IRStmt::Goto(follow_idx));
                proc.blocks.add_edge(assign_idx, follow_idx, ());
                *block_idx = follow_idx;
            } else {
                *block_idx = assign_idx;
                follow_idx = assign_idx;
            }

            block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
            }));

            if is_owned {
                block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }));
            }
        }
        CSTExpression::UnaryOp(o) => {
            /*  t_kind_check := ast_node_kind_str_eq(o.op.to_string(), tmp);
             *  t_len := amount(tmp);
             *  t_len_check := t_len == 2;
             *  t_check := t_kind_check && t_len_check;
             *  t_succeeded := t_check;
             *  if t_check
             *   goto <assign_idx>
             *  else
             *   goto <follow_idx>
             * <assign_idx>:
             *  t_lhs := tmp[1];
             *  // assign parse
             *  goto <follow idx>
             *
             * <follow_idx>:
             * _ := invalidate(t_len);
             * _ := invalidate(tmp);
             */
            let t_kind_check = tmp_var_new(proc);
            let t_len = tmp_var_new(proc);
            let t_len_check = tmp_var_new(proc);
            let t_check = tmp_var_new(proc);

            let mut assign_idx = if t_succeeded.is_some() {
                proc.blocks.add_node(Vec::new())
            } else {
                *block_idx
            };
            let mut follow_idx = if t_succeeded.is_some() {
                proc.blocks.add_node(Vec::new())
            } else {
                *block_idx
            };

            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_kind_check),
                    types: IRType::BOOL,
                    source: IRValue::BuiltinProc(BuiltinProc::AstNodeKindStrEq),
                    op: IROp::NativeCall(vec![
                        IRValue::String(o.op.to_string()),
                        IRValue::Variable(tmp),
                    ]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_len),
                    types: IRType::NUMBER,
                    source: IRValue::BuiltinProc(BuiltinProc::Amount),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_len_check),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_len),
                    op: IROp::Equal(IRValue::Number(2.into())),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_check),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_kind_check),
                    op: IROp::And(IRValue::Variable(t_len_check)),
                }),
                if let Some(t_s) = t_succeeded {
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_s),
                        types: IRType::BOOL,
                        source: IRValue::Variable(t_check),
                        op: IROp::Assign,
                    })
                } else {
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Ignore,
                        types: IRType::UNDEFINED,
                        source: IRValue::BuiltinProc(BuiltinProc::Assert),
                        op: IROp::NativeCall(vec![IRValue::Variable(t_check)]),
                    })
                },
            ]);

            if t_succeeded.is_some() {
                block_get(proc, *block_idx).push(IRStmt::Branch(IRBranch {
                    cond: IRValue::Variable(t_check),
                    success: assign_idx,
                    failure: follow_idx,
                }));

                proc.blocks.add_edge(*block_idx, assign_idx, ());
                proc.blocks.add_edge(*block_idx, follow_idx, ());
            }

            let t_expr = tmp_var_new(proc);

            block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_expr),
                types: IRTypes!("any"),
                source: IRValue::Variable(tmp),
                op: IROp::AccessArray(IRValue::Number(1.into())),
            }));

            assign_parse(
                &mut assign_idx,
                t_expr,
                false,
                t_succeeded,
                false,
                &o.expr,
                proc,
                shared_proc,
                cfg,
            );

            if t_succeeded.is_some() {
                block_get(proc, assign_idx).push(IRStmt::Goto(follow_idx));
                proc.blocks.add_edge(assign_idx, follow_idx, ());
                *block_idx = follow_idx;
            } else {
                *block_idx = assign_idx;
                follow_idx = assign_idx;
            }

            block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
            }));

            if is_owned {
                block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }));
            }
        }
        CSTExpression::Literal(_) | CSTExpression::Bool(_) | CSTExpression::Number(_) => {
            /* t_e := //expr
             * t_s := tmp == t_e
             * t_succeeded := t_s;
             * _ := invalidate(t_e);
             * _ := invalidate(tmp);
             */
            let t_e = tmp_var_new(proc);

            block_expr_push(
                target,
                block_idx,
                IRTarget::Variable(t_e),
                proc,
                shared_proc,
                cfg,
            );

            let t_s = tmp_var_new(proc);
            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_s),
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp),
                    op: IROp::Equal(IRValue::Variable(t_e)),
                }),
                if let Some(t_suc) = t_succeeded {
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_suc),
                        types: IRType::BOOL,
                        source: IRValue::Variable(t_s),
                        op: IROp::Assign,
                    })
                } else {
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Ignore,
                        types: IRType::UNDEFINED,
                        source: IRValue::BuiltinProc(BuiltinProc::Assert),
                        op: IROp::NativeCall(vec![IRValue::Variable(t_s)]),
                    })
                },
            ]);

            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_e)]),
            }));

            if is_owned {
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }));
            }
        }
        CSTExpression::Call(c) => {
            /*  *t_fnc := ast_node_new("var", c.name);
             *  t_check := ast_node_kind_str_eq("call", tmp);
             *  if t_check
             *   goto <check_idx>
             *  else
             *   goto <follow_idx>
             *
             * <check_idx>:
             *  t_params := tmp[2];
             *  t_params_t := type_of(t_params);
             *  t_params_check := t_params_t == TYPE_LIST;
             *  t_check := t_check && t_params_check;
             *  if t_params_check
             *   goto <check_params_idx>:
             *  else
             *   goto <follow_idx>
             *
             * <check_params_idx>
             *  t_param := t_params[0];
             *  t_check_param := false;
             *  // assign parse
             *  t_check := t_check && t_check_param;
             *  t_rest := tmp[3];
             *  t_check_rest := false;
             *  // assign parse
             *  t_check := t_check && t_check_rest;
             *
             * <follow_idx>:
             *  t_success := t_check;
             *  _ := invalidate(tmp);
             */
            let t_fnc = if let Some(t) = stack_get(shared_proc, &c.name) {
                t
            } else {
                let t = tmp_var_new(proc);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t),
                    types: IRType::PTR,
                    source: IRValue::BuiltinProc(BuiltinProc::StackGetAssert),
                    op: IROp::NativeCall(vec![IRValue::String(c.name.to_string())]),
                }));
                t
            };

            let t_check = tmp_var_new(proc);

            let mut check_idx = proc.blocks.add_node(Vec::new());
            let follow_idx = proc.blocks.add_node(Vec::new());

            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Deref(t_fnc),
                    types: IRType::AST,
                    source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                    op: IROp::NativeCall(vec![
                        IRValue::String("var".to_string()),
                        IRValue::String(c.name.to_string()),
                    ]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_check),
                    types: IRType::AST,
                    source: IRValue::BuiltinProc(BuiltinProc::AstNodeKindStrEq),
                    op: IROp::NativeCall(vec![
                        IRValue::String("call".to_string()),
                        IRValue::Variable(tmp),
                    ]),
                }),
                IRStmt::Branch(IRBranch {
                    cond: IRValue::Variable(t_check),
                    success: check_idx,
                    failure: follow_idx,
                }),
            ]);

            proc.blocks.add_edge(*block_idx, check_idx, ());
            proc.blocks.add_edge(*block_idx, follow_idx, ());

            let check_params_idx = proc.blocks.add_node(Vec::new());

            let t_params = tmp_var_new(proc);
            let t_params_t = tmp_var_new(proc);
            let t_params_check = tmp_var_new(proc);
            block_get(proc, check_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_params),
                    types: IRTypes!("any"),
                    source: IRValue::Variable(tmp),
                    op: IROp::AccessArray(IRValue::Number(2.into())),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_params_t),
                    types: IRType::TYPE,
                    source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_params)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_params_check),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_params_t),
                    op: IROp::Equal(IRValue::Type(IRType::LIST)),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_check),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_check),
                    op: IROp::And(IRValue::Variable(t_params_check)),
                }),
                IRStmt::Branch(IRBranch {
                    cond: IRValue::Variable(t_params_check),
                    success: check_params_idx,
                    failure: follow_idx,
                }),
            ]);

            proc.blocks.add_edge(check_idx, check_params_idx, ());
            proc.blocks.add_edge(check_idx, follow_idx, ());

            check_idx = check_params_idx;

            c.params.iter().enumerate().for_each(|(idx, i)| {
                let t_param = tmp_var_new(proc);
                let t_check_param = tmp_var_new(proc);
                block_get(proc, check_idx).extend(vec![
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_param),
                        types: IRTypes!("any"),
                        source: IRValue::Variable(t_params),
                        op: IROp::AccessArray(IRValue::Number(idx.into())),
                    }),
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_check_param),
                        types: IRType::BOOL,
                        source: IRValue::Bool(false),
                        op: IROp::Assign,
                    }),
                ]);

                assign_parse(
                    &mut check_idx,
                    t_param,
                    false,
                    Some(t_check_param),
                    false,
                    i,
                    proc,
                    shared_proc,
                    cfg,
                );
                block_get(proc, check_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_check),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_check),
                    op: IROp::And(IRValue::Variable(t_check_param)),
                }));
            });

            if let Some(rest_param) = &c.rest_param {
                let t_rest = tmp_var_new(proc);
                let t_rest_check = tmp_var_new(proc);

                block_get(proc, check_idx).extend(vec![
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_rest),
                        types: IRTypes!("any"),
                        source: IRValue::Variable(tmp),
                        op: IROp::AccessArray(IRValue::Number(3.into())),
                    }),
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_rest_check),
                        types: IRType::BOOL,
                        source: IRValue::Bool(false),
                        op: IROp::Assign,
                    }),
                ]);

                assign_parse(
                    &mut check_idx,
                    t_rest,
                    false,
                    Some(t_rest_check),
                    false,
                    rest_param,
                    proc,
                    shared_proc,
                    cfg,
                );

                block_get(proc, check_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_check),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_check),
                    op: IROp::And(IRValue::Variable(t_rest_check)),
                }));
            }

            block_get(proc, check_idx).push(IRStmt::Goto(follow_idx));
            proc.blocks.add_edge(check_idx, follow_idx, ());

            if let Some(t_s) = t_succeeded {
                block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_s),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_check),
                    op: IROp::Assign,
                }));
            } else {
                block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Assert),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_check)]),
                }));
            }

            if is_owned {
                block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }));
            }

            *block_idx = follow_idx;
        }
        CSTExpression::Ignore => {}
        _ => panic!("invalid assign target {:?}", target),
    }
}
