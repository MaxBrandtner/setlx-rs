pub mod access_expr;
mod call_expr;
mod collection_expr;
mod lambda_expr;
pub mod op_expr;
mod quant_expr;
mod set_mem_ops;
pub mod term_expr;
mod unary_op_expr;
pub mod var_expr;

use petgraph::stable_graph::NodeIndex;

use access_expr::block_access_push;
use call_expr::block_call_push;
use collection_expr::block_collection_push;
use lambda_expr::block_lambda_push;
use op_expr::block_op_push;
use quant_expr::block_quant_push;
use term_expr::block_term_push;
use unary_op_expr::block_unary_op_push;
use var_expr::block_var_push;

use crate::ast::*;
use crate::builtin::*;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::ast::expr::block_cst_expr_push;
use crate::ir::lower::proc::procedure_new;
use crate::ir::lower::util::{block_get, tmp_var_new};

/// Compiles a CST expression into IR, appending statements to the current
/// block. The result of the expression is written into `target`.
///
/// - `expr` — the CST expression node to compile
/// - `block_idx` — the current block being written to; may be advanced
///   forward if the expression requires new blocks
/// - `target` — where to store the result of the compiled expression
pub fn block_expr_push(
    expr: &CSTExpression,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) -> bool /* owned target  */ {
    let lhs_old = shared_proc.code_lhs;
    let rhs_old = shared_proc.code_rhs;
    if !shared_proc.disable_annotations {
        block_get(proc, *block_idx).push(IRStmt::Annotate(expr.lhs, expr.rhs));
    }
    shared_proc.code_lhs = expr.lhs;
    shared_proc.code_rhs = expr.rhs;

    let out = match &expr.kind {
        CSTExpressionKind::Lambda(_) => {
            block_lambda_push(expr, block_idx, target, proc, cfg);
            true
        }
        CSTExpressionKind::Op(c) => {
            block_op_push(c, block_idx, target, proc, shared_proc, cfg);
            true
        }
        CSTExpressionKind::UnaryOp(c) => {
            block_unary_op_push(c, block_idx, target, proc, shared_proc, cfg);
            true
        }
        CSTExpressionKind::Procedure(p) => {
            /* t_info := // cst expr
             * if p.kind == CSTProcedureKind::Closure {
             *  t_stack := stack_copy();
             * // }
             * target := procedure_new(_1, t_info);
             */
            let p_idx = procedure_new(
                &p.block,
                &p.kind,
                &p.params,
                &p.list_param,
                shared_proc.disable_annotations,
                cfg,
            );
            let t_info = tmp_var_new(proc);
            block_cst_expr_push(expr, *block_idx, IRTarget::Variable(t_info), proc);

            let t_stack = if matches!(&p.kind, CSTProcedureKind::Closure) {
                let t_stack = tmp_var_new(proc);

                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_stack),
                    types: IRType::STACK_IMAGE,
                    source: IRValue::BuiltinProc(BuiltinProc::StackCopy),
                    op: IROp::NativeCall(vec![]),
                }));

                Some(t_stack)
            } else {
                None
            };

            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::PROCEDURE,
                source: IRValue::BuiltinProc(BuiltinProc::ProcedureNew),
                op: IROp::NativeCall(vec![
                    IRValue::Procedure(p_idx),
                    IRValue::Variable(t_info),
                    if let Some(t_stack) = t_stack {
                        IRValue::Variable(t_stack)
                    } else {
                        IRValue::Undefined
                    },
                    IRValue::Bool(true),
                ]),
            }));
            true
        }
        CSTExpressionKind::Call(c) => {
            block_call_push(c, block_idx, target, proc, shared_proc, cfg);
            true
        }
        CSTExpressionKind::Term(t) => {
            block_term_push(t, block_idx, target, proc, shared_proc, cfg);
            true
        }
        CSTExpressionKind::Variable(c) => {
            block_var_push(c, block_idx, target, proc, shared_proc);
            false
        }
        CSTExpressionKind::Accessible(a) => {
            block_access_push(a, block_idx, target, proc, shared_proc, cfg)
        }
        CSTExpressionKind::String(_) => {
            panic!("strings should have been converted to literals during CST passes");
        }
        CSTExpressionKind::Literal(s) => {
            // target := s;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::STRING,
                source: IRValue::String(s.clone()),
                op: IROp::Assign,
            }));
            true
        }
        CSTExpressionKind::Bool(b) => {
            // target := b;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::BOOL,
                source: IRValue::Bool(*b),
                op: IROp::Assign,
            }));
            false
        }
        CSTExpressionKind::Double(f) => {
            // target := f;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::DOUBLE,
                source: IRValue::Double(*f),
                op: IROp::Assign,
            }));
            false
        }
        CSTExpressionKind::Number(i) => {
            // target := i;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::NUMBER,
                source: IRValue::Number(i.clone()),
                op: IROp::Assign,
            }));
            true
        }
        CSTExpressionKind::Collection(c) => {
            block_collection_push(c, block_idx, target, proc, shared_proc, cfg);
            true
        }
        CSTExpressionKind::Quantifier(q) => {
            block_quant_push(q, block_idx, target, proc, shared_proc, cfg);
            true
        }
        CSTExpressionKind::Matrix(m) => {
            let ir_mat: Vec<Vec<IRValue>> = m
                .iter()
                .map(|i| {
                    i.iter()
                        .map(|j| {
                            let tmp = tmp_var_new(proc);
                            block_expr_push(
                                j,
                                block_idx,
                                IRTarget::Variable(tmp),
                                proc,
                                shared_proc,
                                cfg,
                            );
                            IRValue::Variable(tmp)
                        })
                        .collect()
                })
                .collect();

            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::MATRIX,
                source: IRValue::Matrix(ir_mat),
                op: IROp::Assign,
            }));
            true
        }
        CSTExpressionKind::Vector(v) => {
            let ir_vec: Vec<IRValue> = v
                .iter()
                .map(|i| {
                    let tmp = tmp_var_new(proc);
                    block_expr_push(
                        i,
                        block_idx,
                        IRTarget::Variable(tmp),
                        proc,
                        shared_proc,
                        cfg,
                    );
                    IRValue::Variable(tmp)
                })
                .collect();

            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::VECTOR,
                source: IRValue::Vector(ir_vec),
                op: IROp::Assign,
            }));
            true
        }
        CSTExpressionKind::Om | CSTExpressionKind::Ignore => {
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::UNDEFINED,
                source: IRValue::Undefined,
                op: IROp::Assign,
            }));
            true
        }
        CSTExpressionKind::Serialize(e) => {
            /* t_tmp := // e.expr;
             * target := serialize(t_tmp);
             * // if is_owned {
             *  _ := invalidate(t_tmp);
             * // }
             */
            let t_tmp = tmp_var_new(proc);

            let is_owned = block_expr_push(
                e,
                block_idx,
                IRTarget::Variable(t_tmp),
                proc,
                shared_proc,
                cfg,
            );
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::STRING,
                source: IRValue::BuiltinProc(BuiltinProc::Serialize),
                op: IROp::NativeCall(vec![IRValue::Variable(t_tmp)]),
            }));

            if is_owned {
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_tmp)]),
                }));
            }

            true
        }
    };

    if !shared_proc.disable_annotations {
        block_get(proc, *block_idx).push(IRStmt::Annotate(lhs_old, rhs_old));
    }
    shared_proc.code_lhs = lhs_old;
    shared_proc.code_rhs = rhs_old;

    out
}
