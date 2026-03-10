use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::expr::op_expr::{
    ObjOverloadRhs, block_obj_overload_push, op_obj_overload_sym,
};
use crate::ir::lower::expr::set_mem_ops::{block_prod_mem_push, block_sum_mem_push};
use crate::ir::lower::expr::term_expr::block_term_unary_op_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_unary_op_impl_push<F>(
    block_idx: &mut NodeIndex,
    t_expr: IRVar,
    target: IRTarget,
    dfl_fn: F,
    op: &str,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) where
    F: Fn(&mut NodeIndex, IRVar, IRTarget, &mut IRProcedure),
{
    /*  // block_obj_overload_push
     *
     * <dfl_idx>:
     *  // block_term_unary_op_push
     *
     * <assign_idx>:
     *  target := dfl_fn();
     *  goto <follow_idx>
     *
     * <follow_idx>:
     */
    let dfl_idx = proc.blocks.add_node(Vec::new());
    let mut assign_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    block_obj_overload_push(
        *block_idx,
        dfl_idx,
        follow_idx,
        false,
        target,
        t_expr,
        ObjOverloadRhs::None,
        op_obj_overload_sym(op).unwrap(),
        proc,
        shared_proc,
        cfg,
    );

    block_term_unary_op_push(
        dfl_idx, assign_idx, follow_idx, t_expr, false, target, op, proc,
    );

    dfl_fn(&mut assign_idx, t_expr, target, proc);

    block_get(proc, assign_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(assign_idx, follow_idx, ());

    *block_idx = follow_idx;
}

/// Emits IR for the unary operator expression `c`, writing the result into `target`.
///
/// Dispatch order per operator:
/// - Object overload via `block_obj_overload_push`
/// - Term/AST construction if the operand is a term or AST node
/// - Default primitive implementation
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
            block_unary_op_impl_push(
                block_idx,
                source,
                target,
                |idx, source, target, proc| {
                    // target := 0 - source;
                    block_get(proc, *idx).push(IRStmt::Assign(IRAssign {
                        target,
                        types: IRTypes!("minus"),
                        source: IRValue::Number(0.into()),
                        op: IROp::Minus(IRValue::Variable(source)),
                    }));
                },
                &c.op.to_string(),
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTUnaryOp::Card => {
            block_unary_op_impl_push(
                block_idx,
                source,
                target,
                |idx, source, target, proc| {
                    // target := amount(source);
                    block_get(proc, *idx).push(IRStmt::Assign(IRAssign {
                        target,
                        types: IRType::NUMBER,
                        source: IRValue::BuiltinProc(BuiltinProc::Amount),
                        op: IROp::NativeCall(vec![IRValue::Variable(source)]),
                    }));
                },
                &c.op.to_string(),
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTUnaryOp::SumMem => {
            block_sum_mem_push(block_idx, source, target, proc, shared_proc, cfg);
        }
        CSTUnaryOp::ProdMem => {
            block_prod_mem_push(block_idx, source, target, proc, shared_proc, cfg);
        }
        CSTUnaryOp::Factor => {
            block_unary_op_impl_push(
                block_idx,
                source,
                target,
                |idx, source, target, proc| {
                    /* <dfl_idx>:
                     *  t_source_type := type_of(source);
                     *  t_source_type_num := t_source_type == TYPE_NUMBER;
                     *  if t_source_type_num
                     *   goto <factor_idx>
                     *  else
                     *   goto <assert_idx>
                     *
                     * <assert_idx>:
                     *  _ := throw(1, "factor is undefined for type");
                     *  unreachable
                     *
                     * <factor_idx>
                     *  t_assert_lz := source < 0;
                     *  if t_assert_lz
                     *   goto <assert_idx>
                     *  else
                     *   goto <factor_iter_idx>
                     *
                     * <factor_iter_idx>:
                     *  t_i := copy(source);
                     *  t_target := 1;
                     *  goto <check_idx>
                     *
                     * <check_idx>
                     *  t_check := 0 < t_i;
                     *  if t_check
                     *   goto <loop_idx>
                     *  else
                     *   goto <follow_dfl_idx>
                     *
                     * <loop_idx>
                     *  t_target_new := t_target * t_i;
                     *  t_i_new := t_i - 1;
                     *  _ := invalidate(t_target);
                     *  _ := invalidate(t_i);
                     *  _ := invalidate(t_check);
                     *  t_target := t_target_new;
                     *  t_i := t_i_new;
                     *  goto <check_idx>
                     *
                     * <follow_dfl_idx>
                     *  _ := invalidate(t_i);
                     *  _ := invalidate(t_check);
                     *  target := t_target;
                     */
                    if let IRTarget::Ignore = target {
                        return;
                    }

                    let assert_idx = proc.blocks.add_node(Vec::new());
                    let factor_idx = proc.blocks.add_node(Vec::new());
                    let factor_iter_idx = proc.blocks.add_node(Vec::new());
                    let check_idx = proc.blocks.add_node(Vec::new());
                    let loop_idx = proc.blocks.add_node(Vec::new());
                    let follow_dfl_idx = proc.blocks.add_node(Vec::new());

                    let t_assert_lz = tmp_var_new(proc);
                    let t_i = tmp_var_new(proc);
                    let t_target = tmp_var_new(proc);

                    let t_source_type = tmp_var_new(proc);
                    let t_source_type_num = tmp_var_new(proc);

                    block_get(proc, *idx).extend(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_source_type),
                            types: IRType::TYPE,
                            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
                            op: IROp::NativeCall(vec![IRValue::Variable(source)]),
                        }),
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_source_type_num),
                            types: IRType::BOOL,
                            source: IRValue::Variable(t_source_type),
                            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
                        }),
                        IRStmt::Branch(IRBranch {
                            cond: IRValue::Variable(t_source_type_num),
                            success: factor_idx,
                            failure: assert_idx,
                        }),
                    ]);

                    proc.blocks.add_edge(*idx, factor_idx, ());
                    proc.blocks.add_edge(*idx, assert_idx, ());

                    block_get(proc, assert_idx).extend(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Ignore,
                            types: IRType::UNDEFINED,
                            source: IRValue::BuiltinProc(BuiltinProc::Throw),
                            op: IROp::NativeCall(vec![
                                IRValue::Number(1.into()),
                                IRValue::String("factor is undefined for type".to_string()),
                            ]),
                        }),
                        IRStmt::Unreachable,
                    ]);

                    block_get(proc, factor_idx).extend(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_assert_lz),
                            types: IRType::BOOL,
                            source: IRValue::Variable(source),
                            op: IROp::Less(IRValue::Number(0.into())),
                        }),
                        IRStmt::Branch(IRBranch {
                            cond: IRValue::Variable(t_assert_lz),
                            success: assert_idx,
                            failure: factor_iter_idx,
                        }),
                    ]);

                    proc.blocks.add_edge(factor_idx, assert_idx, ());
                    proc.blocks.add_edge(factor_idx, factor_iter_idx, ());

                    block_get(proc, factor_iter_idx).extend(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_i),
                            types: IRType::NUMBER,
                            source: IRValue::BuiltinProc(BuiltinProc::Copy),
                            op: IROp::NativeCall(vec![IRValue::Variable(source)]),
                        }),
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_target),
                            types: IRType::NUMBER,
                            source: IRValue::Number(1.into()),
                            op: IROp::Assign,
                        }),
                        IRStmt::Goto(check_idx),
                    ]);

                    proc.blocks.add_edge(factor_iter_idx, check_idx, ());

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
                            failure: follow_dfl_idx,
                        }),
                    ]);

                    proc.blocks.add_edge(check_idx, loop_idx, ());
                    proc.blocks.add_edge(check_idx, follow_dfl_idx, ());

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

                    block_get(proc, follow_dfl_idx).extend(vec![
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

                    *idx = follow_dfl_idx;
                },
                &c.op.to_string(),
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTUnaryOp::Not => {
            block_unary_op_impl_push(
                block_idx,
                source,
                target,
                |idx, source, target, proc| {
                    // target := !source;
                    block_get(proc, *idx).push(IRStmt::Assign(IRAssign {
                        target,
                        types: IRType::BOOL,
                        source: IRValue::Variable(source),
                        op: IROp::Not,
                    }));
                },
                &c.op.to_string(),
                proc,
                shared_proc,
                cfg,
            );
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
