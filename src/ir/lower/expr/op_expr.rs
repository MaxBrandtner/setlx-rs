use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_op_push(
    c: &CSTExpressionOp,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    let tmp_left = tmp_var_new(proc);
    let tmp_left_owned = block_expr_push(
        &c.left,
        block_idx,
        IRTarget::Variable(tmp_left),
        proc,
        shared_proc,
        cfg,
    );

    let tmp_right = tmp_var_new(proc);
    let tmp_right_owned = block_expr_push(
        &c.right,
        block_idx,
        IRTarget::Variable(tmp_right),
        proc,
        shared_proc,
        cfg,
    );

    match c.op {
        CSTOp::Imply => {
            /* t_1 := !tmp_left;
             * target := t_1 || tmp_right;
             * _ := invalidate(t_1);
             */
            let tmp = tmp_var_new(proc);
            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp),
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_left),
                    op: IROp::Not,
                }),
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp),
                    op: IROp::Or(IRValue::Variable(tmp_right)),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }),
            ]);
        }
        CSTOp::Or => {
            // target := tmp_left || tmp_right;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::BOOL,
                source: IRValue::Variable(tmp_left),
                op: IROp::Or(IRValue::Variable(tmp_right)),
            }));
        }
        CSTOp::And => {
            // target := tmp_left && tmp_right;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::BOOL,
                source: IRValue::Variable(tmp_left),
                op: IROp::And(IRValue::Variable(tmp_right)),
            }));
        }
        CSTOp::Eq | CSTOp::SetEq => {
            // target := tmp_left == tmp_right;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::BOOL,
                source: IRValue::Variable(tmp_left),
                op: IROp::Equal(IRValue::Variable(tmp_right)),
            }));
        }
        CSTOp::Neq | CSTOp::SetNeq => {
            /* t_1 := tmp_left == tmp_right;
             * target := !t_1;
             * _ := invalidate(t_1);
             */
            let tmp = tmp_var_new(proc);
            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp),
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_left),
                    op: IROp::Equal(IRValue::Variable(tmp_right)),
                }),
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp),
                    op: IROp::Not,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }),
            ]);
        }
        CSTOp::Less => {
            // target := tmp_left < tmp_right;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::BOOL,
                source: IRValue::Variable(tmp_left),
                op: IROp::Less(IRValue::Variable(tmp_right)),
            }));
        }
        CSTOp::Leq => {
            /* t_1 := tmp_left < tmp_right;
             * t_2 := tmp_left == tmp_right;
             * target := t_1 || t_2;
             * _ := invalidate(t_1);
             * _ := invalidate(t_2);
             */
            let tmp_1 = tmp_var_new(proc);
            let tmp_2 = tmp_var_new(proc);

            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp_1),
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_left),
                    op: IROp::Less(IRValue::Variable(tmp_right)),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp_2),
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_left),
                    op: IROp::Equal(IRValue::Variable(tmp_right)),
                }),
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_1),
                    op: IROp::Or(IRValue::Variable(tmp_2)),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp_1)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp_2)]),
                }),
            ]);
        }
        CSTOp::Greater => {
            // target := tmp_right < tmp_left;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::BOOL,
                source: IRValue::Variable(tmp_right),
                op: IROp::Less(IRValue::Variable(tmp_left)),
            }));
        }
        CSTOp::Geq => {
            /* t_1 := tmp_right < tmp_left;
             * t_2 := tmp_left == tmp_right;
             * target := t_1 || t_2;
             * _ := invalidate(t_1);
             * _ := invalidate(t_2);
             */
            let tmp_1 = tmp_var_new(proc);
            let tmp_2 = tmp_var_new(proc);

            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp_1),
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_right),
                    op: IROp::Less(IRValue::Variable(tmp_left)),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp_2),
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_left),
                    op: IROp::Equal(IRValue::Variable(tmp_right)),
                }),
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_1),
                    op: IROp::Or(IRValue::Variable(tmp_2)),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp_1)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp_2)]),
                }),
            ]);
        }
        CSTOp::In => {
            /* target := contains(tmp_left, tmp_right);
             */
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::BOOL,
                source: IRValue::BuiltinProc(BuiltinProc::Contains),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(tmp_left),
                    IRValue::Variable(tmp_right),
                ]),
            }));
        }
        CSTOp::NotIn => {
            /* t_1 := contains(tmp_left, tmp_right);
             * target := !t_1;
             * _ := invalidate(t_1);
             */
            let tmp = tmp_var_new(proc);
            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp),
                    types: IRType::BOOL,
                    source: IRValue::BuiltinProc(BuiltinProc::Contains),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(tmp_left),
                        IRValue::Variable(tmp_right),
                    ]),
                }),
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp),
                    op: IROp::Not,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }),
            ]);
        }
        CSTOp::Plus => {
            // target := tmp_left + tmp_right;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRTypes!("plus"),
                source: IRValue::Variable(tmp_left),
                op: IROp::Plus(IRValue::Variable(tmp_right)),
            }));
        }
        CSTOp::Minus => {
            // target := tmp_left - tmp_right;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRTypes!("minus"),
                source: IRValue::Variable(tmp_left),
                op: IROp::Minus(IRValue::Variable(tmp_right)),
            }));
        }
        CSTOp::Mult => {
            // target := tmp_left * tmp_right;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRTypes!("mul"),
                source: IRValue::Variable(tmp_left),
                op: IROp::Mult(IRValue::Variable(tmp_right)),
            }));
        }
        CSTOp::Div => {
            // target := tmp_left / tmp_right;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRTypes!("quot"),
                source: IRValue::Variable(tmp_left),
                op: IROp::Divide(IRValue::Variable(tmp_right)),
            }));
        }
        CSTOp::IntDiv => {
            // target := tmp_left \ tmp_right;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRTypes!("quot"),
                source: IRValue::Variable(tmp_left),
                op: IROp::IntDivide(IRValue::Variable(tmp_right)),
            }));
        }
        CSTOp::Mod => {
            // target := tmp_left % tmp_right;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::NUMBER | IRType::DOUBLE | IRType::MATRIX | IRType::VECTOR,
                source: IRValue::Variable(tmp_left),
                op: IROp::Mod(IRValue::Variable(tmp_right)),
            }));
        }
        CSTOp::Cartesian => {
            /* target := cartesian(tmp_left, tmp_right);
             */
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::SET,
                source: IRValue::BuiltinProc(BuiltinProc::Cartesian),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(tmp_left),
                    IRValue::Variable(tmp_right),
                ]),
            }));
        }
        CSTOp::Power => {
            /* target := pow(tmp_left, tmp_right);
             */
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::SET
                    | IRType::NUMBER
                    | IRType::DOUBLE
                    | IRType::MATRIX
                    | IRType::VECTOR,
                source: IRValue::BuiltinProc(BuiltinProc::Pow),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(tmp_left),
                    IRValue::Variable(tmp_right),
                ]),
            }));
        }
        CSTOp::SumMem => {
            /* t_target := tmp_left;
             * t_iter := iter_new(tmp_right);
             * goto <check_idx>
             *
             * <check_Idx>:
             * t_i := om;
             * t_i_addr := &t_i;
             * t_cond := iter_next(t_iter, t_i_addr);
             * if t_cond
             *  goto <loop_idx>
             * else
             *  goto <follow_idx>
             *
             * <loop_idx>:
             * t_target_new := t_target + t_i;
             * _ := invalidate(target);
             * _ := invalidate(t_cond);
             * t_target := t_target_new;
             * goto <check_idx>
             *
             * <follow_idx>:
             * _ := invalidate(t_cond);
             * _ := invalidate(t_iter);
             * target := t_target;
             */
            let check_idx = proc.blocks.add_node(Vec::new());
            let loop_idx = proc.blocks.add_node(Vec::new());
            let follow_idx = proc.blocks.add_node(Vec::new());

            let t_iter = tmp_var_new(proc);
            let t_target = tmp_var_new(proc);

            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRTypes!("any"),
                    source: IRValue::Variable(tmp_left),
                    op: IROp::Assign,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_iter),
                    types: IRType::ITERATOR,
                    source: IRValue::BuiltinProc(BuiltinProc::IterNew),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp_right)]),
                }),
                IRStmt::Goto(check_idx),
            ]);

            proc.blocks.add_edge(*block_idx, check_idx, ());

            let t_i = tmp_var_new(proc);
            let t_i_addr = tmp_var_new(proc);
            let t_cond = tmp_var_new(proc);

            block_get(proc, check_idx).extend(vec![
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
                        IRValue::Variable(t_iter),
                        IRValue::Variable(t_i_addr),
                    ]),
                }),
                IRStmt::Branch(IRBranch {
                    cond: IRValue::Variable(t_cond),
                    success: loop_idx,
                    failure: follow_idx,
                }),
            ]);

            proc.blocks.add_edge(check_idx, loop_idx, ());
            proc.blocks.add_edge(check_idx, follow_idx, ());

            let t_target_new = tmp_var_new(proc);

            block_get(proc, loop_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_target_new),
                    types: IRTypes!("plus"),
                    source: IRValue::Variable(t_target),
                    op: IROp::Plus(IRValue::Variable(t_i)),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_target)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_cond)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_target),
                    types: IRTypes!("plus"),
                    source: IRValue::Variable(t_target_new),
                    op: IROp::Assign,
                }),
                IRStmt::Goto(check_idx),
            ]);

            proc.blocks.add_edge(loop_idx, check_idx, ());

            block_get(proc, follow_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_iter)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_cond)]),
                }),
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRTypes!("any"),
                    source: IRValue::Variable(t_target),
                    op: IROp::Assign,
                }),
            ]);

            *block_idx = follow_idx;
        }
        CSTOp::ProdMem => {
            /* t_target := tmp_left;
             * t_iter := iter_new(tmp_right);
             * goto <check_idx>
             *
             * <check_Idx>:
             * t_i := om;
             * t_i_addr := &t_i;
             * t_cond := iter_next(t_iter, t_i_addr);
             * if t_cond
             *  goto <loop_idx>
             * else
             *  goto <follow_idx>
             *
             * <loop_idx>:
             * t_target_new := t_target + t_i;
             * _ := invalidate(target);
             * _ := invalidate(t_cond);
             * t_target := t_target_new;
             * goto <check_idx>
             *
             * <follow_idx>:
             * _ := invalidate(t_iter);
             * _ := invalidate(t_cond);
             * target := t_target;
             */
            let check_idx = proc.blocks.add_node(Vec::new());
            let loop_idx = proc.blocks.add_node(Vec::new());
            let follow_idx = proc.blocks.add_node(Vec::new());

            let t_iter = tmp_var_new(proc);
            let t_target = tmp_var_new(proc);

            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRTypes!("any"),
                    source: IRValue::Variable(tmp_left),
                    op: IROp::Assign,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_iter),
                    types: IRType::ITERATOR,
                    source: IRValue::BuiltinProc(BuiltinProc::IterNew),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp_right)]),
                }),
                IRStmt::Goto(check_idx),
            ]);

            proc.blocks.add_edge(*block_idx, check_idx, ());

            let t_i = tmp_var_new(proc);
            let t_i_addr = tmp_var_new(proc);
            let t_cond = tmp_var_new(proc);

            block_get(proc, check_idx).extend(vec![
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
                        IRValue::Variable(t_iter),
                        IRValue::Variable(t_i_addr),
                    ]),
                }),
                IRStmt::Branch(IRBranch {
                    cond: IRValue::Variable(t_cond),
                    success: loop_idx,
                    failure: follow_idx,
                }),
            ]);

            proc.blocks.add_edge(check_idx, loop_idx, ());
            proc.blocks.add_edge(check_idx, follow_idx, ());

            let t_target_new = tmp_var_new(proc);

            block_get(proc, loop_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_target_new),
                    types: IRTypes!("mul"),
                    source: IRValue::Variable(t_target),
                    op: IROp::Mult(IRValue::Variable(t_i)),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_target)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_cond)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_target),
                    types: IRTypes!("mul"),
                    source: IRValue::Variable(t_target_new),
                    op: IROp::Assign,
                }),
                IRStmt::Goto(check_idx),
            ]);

            proc.blocks.add_edge(loop_idx, check_idx, ());

            block_get(proc, follow_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_iter)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_cond)]),
                }),
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRTypes!("any"),
                    source: IRValue::Variable(t_target),
                    op: IROp::Assign,
                }),
            ]);

            *block_idx = follow_idx;
        }
    }

    /* _ := invalidate(tmp_left);
     * _ := invalidate(tmp_right);
     */
    if tmp_left_owned {
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_left)]),
        }));
    }
    if tmp_right_owned {
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_right)]),
        }));
    }
}
