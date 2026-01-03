use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_unary_op_push(
    c: &CSTExpressionUnaryOp,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    let source = tmp_var_new(proc);
    let source_owned = block_expr_push(
        &c.expr,
        block_idx,
        IRTarget::Variable(source),
        proc,
        shared_proc,
        cfg,
    );

    match c.op {
        CSTUnaryOp::Minus => {
            // target := 0 - source;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRTypes!("minus"),
                source: IRValue::Number(0.into()),
                op: IROp::Minus(IRValue::Variable(source)),
            }));
        }
        CSTUnaryOp::Card => {
            // target := amount(source);
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::NUMBER,
                source: IRValue::BuiltinProc(BuiltinProc::Amount),
                op: IROp::NativeCall(vec![IRValue::Variable(source)]),
            }));
        }
        CSTUnaryOp::SumMem => {
            /* t_target := 0;
             * t_iter := iter_new(source);
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
             * _ := invalidate(t_target);
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
                    target: IRTarget::Variable(t_target),
                    types: IRType::NUMBER,
                    source: IRValue::Number(0.into()),
                    op: IROp::Assign,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_iter),
                    types: IRType::ITERATOR,
                    source: IRValue::BuiltinProc(BuiltinProc::IterNew),
                    op: IROp::NativeCall(vec![IRValue::Variable(source)]),
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
        CSTUnaryOp::ProdMem => {
            /* t_target := 1;
             * t_iter := iter_new(source);
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
             * t_target_new := t_target * t_i;
             * _ := invalidate(t_target);
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
                    target: IRTarget::Variable(t_target),
                    types: IRType::NUMBER,
                    source: IRValue::Number(1.into()),
                    op: IROp::Assign,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_iter),
                    types: IRType::ITERATOR,
                    source: IRValue::BuiltinProc(BuiltinProc::IterNew),
                    op: IROp::NativeCall(vec![IRValue::Variable(source)]),
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
        CSTUnaryOp::Factor => {
            /* _ := type_assert(source, TYPE_NUMBER);
             * t_assert_lz := source < 0;
             * t_assert := !t_assert_lz;
             * _ := assert(t_assert);
             * _ := invaliate(t_assert_lz);
             * _ := invalidate(t_assert);
             * t_i := source;
             * t_target := 1;
             * goto <check_idx>
             *
             * <check_idx>
             * t_check := 0 < t_i;
             * if t_check
             *  goto <loop_idx>
             * else
             *  goto <follow_idx>
             *
             * <loop_idx>
             * t_target_new := t_target * t_i;
             * t_i_new := t_i - 1;
             * _ := invalidate(t_target);
             * _ := invalidate(t_i);
             * _ := invalidate(t_check);
             * t_target := t_target_new;
             * t_i := t_i_new;
             * goto <check_idx>
             *
             * <follow_idx>
             * _ := invalidate(t_i);
             * _ := invalidate(t_check);
             * target := t_target;
             */
            if let IRTarget::Ignore = target {
                return;
            }

            let check_idx = proc.blocks.add_node(Vec::new());
            let loop_idx = proc.blocks.add_node(Vec::new());
            let follow_idx = proc.blocks.add_node(Vec::new());

            let t_assert_lz = tmp_var_new(proc);
            let t_assert = tmp_var_new(proc);
            let t_i = tmp_var_new(proc);
            let t_target = tmp_var_new(proc);

            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::TypeAssert),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(source),
                        IRValue::Type(IRType::NUMBER),
                    ]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_assert_lz),
                    types: IRType::BOOL,
                    source: IRValue::Variable(source),
                    op: IROp::Less(IRValue::Number(0.into())),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_assert),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_assert_lz),
                    op: IROp::Not,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Assert),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_assert)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_assert_lz)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_assert)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_i),
                    types: IRType::NUMBER,
                    source: IRValue::Variable(source),
                    op: IROp::Assign,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_target),
                    types: IRType::NUMBER,
                    source: IRValue::Number(1.into()),
                    op: IROp::Assign,
                }),
                IRStmt::Goto(check_idx),
            ]);

            proc.blocks.add_edge(*block_idx, check_idx, ());

            let t_check = tmp_var_new(proc);
            block_get(proc, check_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_check),
                    types: IRType::BOOL,
                    source: IRValue::Number(0.into()),
                    op: IROp::Less(IRValue::Variable(t_i)),
                }),
                IRStmt::Branch(IRBranch {
                    cond: IRValue::Variable(t_check),
                    success: loop_idx,
                    failure: follow_idx,
                }),
            ]);

            proc.blocks.add_edge(check_idx, loop_idx, ());
            proc.blocks.add_edge(check_idx, follow_idx, ());

            let t_target_new = tmp_var_new(proc);
            let t_i_new = tmp_var_new(proc);

            block_get(proc, loop_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_target_new),
                    types: IRTypes!("mul"),
                    source: IRValue::Variable(t_target),
                    op: IROp::Mult(IRValue::Variable(t_i)),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_i_new),
                    types: IRTypes!("minus"),
                    source: IRValue::Variable(t_i),
                    op: IROp::Minus(IRValue::Number(1.into())),
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
                    op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_check)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_target),
                    types: IRTypes!("mul"),
                    source: IRValue::Variable(t_target_new),
                    op: IROp::Assign,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_i),
                    types: IRTypes!("minus"),
                    source: IRValue::Variable(t_i_new),
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
                    op: IROp::NativeCall(vec![IRValue::Variable(t_check)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
                }),
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRTypes!("mul"),
                    source: IRValue::Variable(t_target),
                    op: IROp::Assign,
                }),
            ]);

            *block_idx = follow_idx;
        }
        CSTUnaryOp::Not => {
            /* target := !source;
             */
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::BOOL,
                source: IRValue::Variable(source),
                op: IROp::Not,
            }));
        }
    }

    if source_owned {
        // _ := invalidate(source);
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(source)]),
        }));
    }
}
