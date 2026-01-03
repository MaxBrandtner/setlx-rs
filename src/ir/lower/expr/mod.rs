pub mod access_expr;
mod call_expr;
mod collection_expr;
mod lambda_expr;
mod op_expr;
mod quant_expr;
mod term_expr;
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

pub fn block_expr_push(
    expr: &CSTExpression,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) -> bool /* owned target  */ {
    match expr {
        CSTExpression::Lambda(_) => {
            block_lambda_push(expr, block_idx, target, proc, cfg);
            true
        }
        CSTExpression::Op(c) => {
            block_op_push(c, block_idx, target, proc, shared_proc, cfg);
            true
        }
        CSTExpression::UnaryOp(c) => {
            block_unary_op_push(c, block_idx, target, proc, shared_proc, cfg);
            true
        }
        CSTExpression::Procedure(p) => {
            /* t_info := // cst expr
             * target := procedure_new(_1, t_info);
             */
            let p_idx = procedure_new(&p.block, &p.kind, &p.params, &p.list_param, cfg);
            let t_info = tmp_var_new(proc);
            block_cst_expr_push(expr, *block_idx, IRTarget::Variable(t_info), proc);
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::PROCEDURE,
                source: IRValue::BuiltinProc(BuiltinProc::ProcedureNew),
                op: IROp::NativeCall(vec![IRValue::Procedure(p_idx), IRValue::Variable(t_info)]),
            }));
            true
        }
        CSTExpression::Call(c) => {
            block_call_push(c, block_idx, target, proc, shared_proc, cfg);
            true
        }
        CSTExpression::Term(t) => {
            block_term_push(t, block_idx, target, proc, shared_proc, cfg);
            true
        }
        CSTExpression::Variable(c) => {
            block_var_push(c, block_idx, target, proc, shared_proc);
            false
        }
        CSTExpression::Accessible(a) => {
            block_access_push(a, block_idx, target, proc, shared_proc, cfg)
        }
        CSTExpression::String(_) => {
            panic!("strings should have been converted to literals during CST passes");
        }
        CSTExpression::Literal(s) => {
            // target := s;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::STRING,
                source: IRValue::String(s.clone()),
                op: IROp::Assign,
            }));
            true
        }
        CSTExpression::Bool(b) => {
            // target := b;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::BOOL,
                source: IRValue::Bool(*b),
                op: IROp::Assign,
            }));
            false
        }
        CSTExpression::Double(f) => {
            // target := f;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::DOUBLE,
                source: IRValue::Double(*f),
                op: IROp::Assign,
            }));
            false
        }
        CSTExpression::Number(i) => {
            // target := i;
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::NUMBER,
                source: IRValue::Number(i.clone()),
                op: IROp::Assign,
            }));
            true
        }
        CSTExpression::Collection(c) => {
            block_collection_push(c, block_idx, target, proc, shared_proc, cfg);
            true
        }
        CSTExpression::Quantifier(q) => {
            block_quant_push(q, block_idx, target, proc, shared_proc, cfg);
            true
        }
        CSTExpression::Matrix(m) => {
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
        CSTExpression::Vector(v) => {
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
        CSTExpression::Om | CSTExpression::Ignore => {
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::UNDEFINED,
                source: IRValue::Undefined,
                op: IROp::Assign,
            }));
            false
        }
    }
}
