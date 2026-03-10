use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::iter::block_iterator_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_list_truncate_push(
    block_idx: &mut NodeIndex,
    t_list: IRVar,
    proc: &mut IRProcedure,
) {
    /*   t_i := amount(t_list);
     *   goto <cond_idx>
     *
     * <cond_idx>:
     *  t_i_zero := t_i == 0;
     *  if t_i_zero
     *   goto <inv_list_idx>
     *  else
     *   goto <om_idx>
     *
     * <inv_list_idx>:
     *  _ := invalidate(t_list);
     *  t_list := list_new();
     *  goto <follow_idx>;
     *
     * <om_idx>:
     *  t_i_sub_one := t_i - 1;
     *  _ := invalidate(t_i);
     *  t_i := t_i_sub_one;
     *  t_entry := t_list[t_i];
     *  t_entry_om := t_entry == om;
     *  if t_entry_om
     *   goto <cond_idx>
     *  else
     *   goto <resize_idx>
     *
     *  <resize_idx>:
     *   t_i_plus_one := t_i + 1;
     *   _ := invalidate(t_i);
     *   t_i := t_i_plus_one;
     *   _ := list_resize(t_list, t_i);
     *   _ := invalidate(t_i);
     *   goto <follow_idx>
     *
     * <follow_idx>:
     */
    let cond_idx = proc.blocks.add_node(Vec::new());
    let om_idx = proc.blocks.add_node(Vec::new());
    let resize_idx = proc.blocks.add_node(Vec::new());
    let inv_list_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    let t_i = tmp_var_new(proc);
    let t_i_zero = tmp_var_new(proc);
    let t_i_sub_one = tmp_var_new(proc);
    let t_i_plus_one = tmp_var_new(proc);
    let t_entry = tmp_var_new(proc);
    let t_entry_om = tmp_var_new(proc);

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_list)]),
        }),
        IRStmt::Goto(cond_idx),
    ]);

    proc.blocks.add_edge(*block_idx, cond_idx, ());

    block_get(proc, cond_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_zero),
            types: IRType::BOOL,
            source: IRValue::Variable(t_i),
            op: IROp::Equal(IRValue::Number(0.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_i_zero),
            success: inv_list_idx,
            failure: om_idx,
        }),
    ]);

    proc.blocks.add_edge(cond_idx, inv_list_idx, ());
    proc.blocks.add_edge(cond_idx, om_idx, ());

    block_get(proc, inv_list_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_list)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_list),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(inv_list_idx, follow_idx, ());

    block_get(proc, om_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_sub_one),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_i),
            op: IROp::Minus(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_i_sub_one),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_entry),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_list),
            op: IROp::AccessArray(IRValue::Variable(t_i)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_entry_om),
            types: IRType::BOOL,
            source: IRValue::Variable(t_entry),
            op: IROp::Equal(IRValue::Undefined),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_entry_om),
            success: cond_idx,
            failure: resize_idx,
        }),
    ]);

    proc.blocks.add_edge(om_idx, resize_idx, ());
    proc.blocks.add_edge(om_idx, cond_idx, ());

    block_get(proc, resize_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_plus_one),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_i),
            op: IROp::Plus(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_i_plus_one),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ListResize),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_list),
                IRValue::Variable(t_i),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Goto(follow_idx),
    ]);

    *block_idx = follow_idx;
}

/// Emits IR to construct the collection described by `c` and write it into `target`.
///
/// Supports literal sets, literal lists, set comprehensions, and list
/// comprehensions.
pub fn block_collection_push(
    c: &CSTCollection,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    fn set_push(
        expr: &CSTSet,
        block_idx: &mut NodeIndex,
        target: IRTarget,
        is_set: bool,
        proc: &mut IRProcedure,
        shared_proc: &mut IRSharedProc,
        cfg: &mut IRCfg,
    ) {
        let target_var = if let IRTarget::Variable(t_var) = target {
            t_var
        } else {
            tmp_var_new(proc)
        };

        if let Some(range) = &expr.range {
            let tmp_left = tmp_var_new(proc);
            let tmp_right = tmp_var_new(proc);

            /* t_1 := expr_left;
             * t_2 := expr_right;
             */
            let left_owned = if let Some(left) = &range.left {
                block_expr_push(
                    left,
                    block_idx,
                    IRTarget::Variable(tmp_left),
                    proc,
                    shared_proc,
                    cfg,
                )
            } else {
                panic!("standalone collection must contain left and right range delimiter");
            };

            let right_owned = if let Some(right) = &range.right {
                block_expr_push(
                    right,
                    block_idx,
                    IRTarget::Variable(tmp_right),
                    proc,
                    shared_proc,
                    cfg,
                )
            } else {
                panic!("standalone collection must contain left and right range delimiter");
            };

            if is_set {
                // target := set_range(t_1, t_2);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(target_var),
                    types: IRType::SET,
                    source: IRValue::BuiltinProc(BuiltinProc::SetRange),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(tmp_left),
                        IRValue::Variable(tmp_right),
                    ]),
                }));
            } else {
                // target := list_range(t_1, t_2);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(target_var),
                    types: IRType::LIST,
                    source: IRValue::BuiltinProc(BuiltinProc::ListRange),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(tmp_left),
                        IRValue::Variable(tmp_right),
                    ]),
                }));
            }

            if left_owned {
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp_left)]),
                }));
            }

            if right_owned {
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp_right)]),
                }));
            }
        } else if is_set {
            // target := set_new();
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(target_var),
                types: IRType::SET,
                source: IRValue::BuiltinProc(BuiltinProc::SetNew),
                op: IROp::NativeCall(Vec::new()),
            }));
        } else {
            // target := list_new();
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(target_var),
                types: IRType::LIST,
                source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                op: IROp::NativeCall(Vec::new()),
            }));
        }

        for i in &expr.expressions {
            // t_n := expr;
            let t_n = tmp_var_new(proc);
            let t_n_owned = block_expr_push(
                i,
                block_idx,
                IRTarget::Variable(t_n),
                proc,
                shared_proc,
                cfg,
            );

            if !t_n_owned {
                // t_n := copy(t_n);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_n),
                    types: IRTypes!("any"),
                    source: IRValue::BuiltinProc(BuiltinProc::Copy),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_n)]),
                }));
            }

            if is_set {
                /* //native call
                 * _ := set_insert(t_3, t_n);
                 */
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::SetInsert),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(target_var),
                        IRValue::Variable(t_n),
                    ]),
                }));
            } else {
                //_ := list_push(t_3, t_n);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(target_var),
                        IRValue::Variable(t_n),
                    ]),
                }));
            }
        }

        if let Some(rest) = &expr.rest {
            // t_n := expr;
            let t_n = tmp_var_new(proc);
            let t_n_owned = block_expr_push(
                rest,
                block_idx,
                IRTarget::Variable(t_n),
                proc,
                shared_proc,
                cfg,
            );

            if !t_n_owned {
                // t_n := copy(t_n);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_n),
                    types: IRTypes!("any"),
                    source: IRValue::BuiltinProc(BuiltinProc::Copy),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_n)]),
                }));
            }

            /*  t_n_iter := iter_new(t_n);
             *  goto <set_iter_idx>:
             *
             * <set_iter_idx>:
             *  t_i := om;
             *  t_i_addr := &t_i;
             *  t_cond := iter_next(t_n_iter, t_i_addr);
             *  if t_cond
             *   goto <set_insert_idx>
             *  else
             *   goto <follow_idx>
             *
             * <set_insert_idx>:
             *  t_i_copy := copy(t_i);
             *  // if is_set {
             *  _ := set_insert(target_var, t_n);
             *  // } else {
             *  _ := list_push(target_var, t_n);
             *  // }
             *  goto <set_iter_idx>
             *
             * <follow_idx>:
             *  _ := invalidate(t_n);
             */
            let t_n_iter = tmp_var_new(proc);
            let t_i = tmp_var_new(proc);
            let t_i_addr = tmp_var_new(proc);
            let t_cond = tmp_var_new(proc);
            let t_i_copy = tmp_var_new(proc);

            let set_iter_idx = proc.blocks.add_node(Vec::new());
            let set_insert_idx = proc.blocks.add_node(Vec::new());
            let follow_idx = proc.blocks.add_node(Vec::new());

            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_n_iter),
                    types: IRType::ITERATOR,
                    source: IRValue::BuiltinProc(BuiltinProc::IterNew),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_n)]),
                }),
                IRStmt::Goto(set_iter_idx),
            ]);

            proc.blocks.add_edge(*block_idx, set_iter_idx, ());

            block_get(proc, set_iter_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_i),
                    types: IRType::UNDEFINED,
                    source: IRValue::Undefined,
                    op: IROp::Assign,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_i_addr),
                    types: IRType::PTR,
                    source: IRValue::Variable(t_i),
                    op: IROp::PtrAddress,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_cond),
                    types: IRType::BOOL,
                    source: IRValue::BuiltinProc(BuiltinProc::IterNext),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(t_n_iter),
                        IRValue::Variable(t_i_addr),
                    ]),
                }),
                IRStmt::Branch(IRBranch {
                    cond: IRValue::Variable(t_cond),
                    success: set_insert_idx,
                    failure: follow_idx,
                }),
            ]);

            proc.blocks.add_edge(set_iter_idx, set_insert_idx, ());
            proc.blocks.add_edge(set_iter_idx, follow_idx, ());

            block_get(proc, set_insert_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_i_copy),
                types: IRTypes!("any"),
                source: IRValue::BuiltinProc(BuiltinProc::Copy),
                op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
            }));

            if is_set {
                block_get(proc, set_insert_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::SetInsert),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(target_var),
                        IRValue::Variable(t_i_copy),
                    ]),
                }))
            } else {
                block_get(proc, set_insert_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(target_var),
                        IRValue::Variable(t_i_copy),
                    ]),
                }));
            }

            block_get(proc, set_insert_idx).push(IRStmt::Goto(set_iter_idx));

            proc.blocks.add_edge(set_insert_idx, set_iter_idx, ());

            block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_n)]),
            }));

            *block_idx = follow_idx;
        }

        if !is_set {
            block_list_truncate_push(block_idx, target_var, proc);
        }
    }

    fn comprehension_push(
        expr: &CSTComprehension,
        block_idx: &mut NodeIndex,
        target: IRTarget,
        is_set: bool,
        proc: &mut IRProcedure,
        shared_proc: &mut IRSharedProc,
        cfg: &mut IRCfg,
    ) {
        let target_var = if let IRTarget::Variable(v) = target {
            v
        } else if let IRTarget::Deref(_) = target {
            usize::MAX
        } else {
            tmp_var_new(proc)
        };

        if let IRTarget::Deref(_) = target {
            if is_set {
                // *target := set_new();
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target,
                    types: IRType::SET,
                    source: IRValue::BuiltinProc(BuiltinProc::SetNew),
                    op: IROp::NativeCall(Vec::new()),
                }));
            } else {
                // *target := list_new(0);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target,
                    types: IRType::LIST,
                    source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                    op: IROp::NativeCall(vec![IRValue::Number(0.into())]),
                }));
            }
        } else if is_set {
            // target := set_new();
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(target_var),
                types: IRType::SET,
                source: IRValue::BuiltinProc(BuiltinProc::SetNew),
                op: IROp::NativeCall(Vec::new()),
            }));
        } else {
            // target := list_new();
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(target_var),
                types: IRType::LIST,
                source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                op: IROp::NativeCall(Vec::new()),
            }));
        }

        // target_var := *target;
        let target_var = if let IRTarget::Variable(d) = target {
            Some(d)
        } else if let IRTarget::Deref(target_var) = target {
            let t = tmp_var_new(proc);
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t),
                types: IRTypes!("any"),
                source: IRValue::Variable(target_var),
                op: IROp::PtrDeref,
            }));
            Some(t)
        } else {
            None
        };

        struct ExprModArg<'a> {
            is_set: bool,
            target_var: Option<IRVar>,
            expression: &'a CSTExpression,
        }

        fn expr_mod(
            expr_idx: NodeIndex,
            backtrack_idx: NodeIndex,
            _follow_idx: NodeIndex,
            _ret_idx: Option<NodeIndex>,
            arg: &ExprModArg,
            proc: &mut IRProcedure,
            shared_proc: &mut IRSharedProc,
            cfg: &mut IRCfg,
        ) {
            let mut expr_idx = expr_idx;
            let t_expr = tmp_var_new(proc);
            let expr_owned = block_expr_push(
                arg.expression,
                &mut expr_idx,
                IRTarget::Variable(t_expr),
                proc,
                shared_proc,
                cfg,
            );

            if !expr_owned {
                block_get(proc, expr_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_expr),
                    types: IRTypes!("any"),
                    source: IRValue::BuiltinProc(BuiltinProc::Copy),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
                }));
            }

            if let Some(target_var) = arg.target_var {
                if arg.is_set {
                    /* _ := set_insert(target_var, t_expr);
                     * goto <backtrack_idx>
                     */
                    block_get(proc, expr_idx).extend(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Ignore,
                            types: IRType::UNDEFINED,
                            source: IRValue::BuiltinProc(BuiltinProc::SetInsert),
                            op: IROp::NativeCall(vec![
                                IRValue::Variable(target_var),
                                IRValue::Variable(t_expr),
                            ]),
                        }),
                        IRStmt::Goto(backtrack_idx),
                    ]);
                    proc.blocks.add_edge(expr_idx, backtrack_idx, ());
                } else {
                    /* _ := list_push(target_addr, t_expr);
                     * goto <backtrack_idx>
                     */
                    block_get(proc, expr_idx).extend(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Ignore,
                            types: IRType::UNDEFINED,
                            source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                            op: IROp::NativeCall(vec![
                                IRValue::Variable(target_var),
                                IRValue::Variable(t_expr),
                            ]),
                        }),
                        IRStmt::Goto(backtrack_idx),
                    ]);
                    proc.blocks.add_edge(expr_idx, backtrack_idx, ());
                }
            } else {
                block_get(proc, expr_idx).push(IRStmt::Goto(backtrack_idx));
                proc.blocks.add_edge(expr_idx, backtrack_idx, ());
            }
        }

        let arg = ExprModArg {
            is_set,
            target_var,
            expression: &expr.expression,
        };
        block_iterator_push(
            block_idx,
            &expr.iterators,
            &expr.condition,
            None,
            expr_mod,
            &arg,
            proc,
            shared_proc,
            cfg,
        );

        if !is_set && let Some(t_var) = target_var {
            block_list_truncate_push(block_idx, t_var, proc);
        }
    }

    match &c {
        CSTCollection::Set(s) => {
            set_push(s, block_idx, target, true, proc, shared_proc, cfg);
        }
        CSTCollection::List(l) => {
            set_push(l, block_idx, target, false, proc, shared_proc, cfg);
        }
        CSTCollection::SetComprehension(sc) => {
            comprehension_push(sc, block_idx, target, true, proc, shared_proc, cfg);
        }
        CSTCollection::ListComprehension(lc) => {
            comprehension_push(lc, block_idx, target, false, proc, shared_proc, cfg);
        }
    }
}
