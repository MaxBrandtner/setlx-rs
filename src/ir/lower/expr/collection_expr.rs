use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::iter::block_iterator_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

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
            // target := list_new(expr.expressions.len());
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(target_var),
                types: IRType::LIST,
                source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                op: IROp::NativeCall(vec![IRValue::Number(expr.expressions.len().into())]),
            }));
        }

        if !expr.expressions.is_empty() || expr.rest.is_some() {
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
                    /* //native call
                     * _ := list_push(t_3, t_n);
                     */
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

                if is_set {
                    /* //native call
                     * _ := set_extend(t_3, t_n);
                     */
                    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                        target: IRTarget::Ignore,
                        types: IRType::UNDEFINED,
                        source: IRValue::BuiltinProc(BuiltinProc::SetExtend),
                        op: IROp::NativeCall(vec![
                            IRValue::Variable(target_var),
                            IRValue::Variable(t_n),
                        ]),
                    }));
                } else {
                    /* //native call
                     * _ := list_extend(t_3, t_n);
                     */
                    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                        target: IRTarget::Ignore,
                        types: IRType::UNDEFINED,
                        source: IRValue::BuiltinProc(BuiltinProc::ListExtend),
                        op: IROp::NativeCall(vec![
                            IRValue::Variable(target_var),
                            IRValue::Variable(t_n),
                        ]),
                    }));
                }
            }
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
            // target := list_new(0);
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(target_var),
                types: IRType::LIST,
                source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                op: IROp::NativeCall(vec![IRValue::Number(0.into())]),
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
            expr_mod,
            &arg,
            proc,
            shared_proc,
            cfg,
        );
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
