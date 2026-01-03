use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::*;
use crate::ir::def::*;
use crate::ir::lower::ast::expr::iter_param::block_cst_iter_params_push;
use crate::ir::lower::ast::expr::{block_cst_expr_push, block_cst_expr_vec_push};
use crate::ir::lower::util::{block_get, tmp_var_new};

fn block_cst_set_push(
    name: String,
    s: &CSTSet,
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    /* t_range := list_new(2);
     * t_range_addr := &t_range;
     * t_range_lhs := t_range_addr[0];
     * *t_range_lhs := // expr;
     * t_range_rhs := t_range_addr[1];
     * *t_range_rhs := // expr;
     * t_expressions := // exprs
     * t_rest := // expr
     * target := ast_node_new(name, t_range, t_expressions, t_rest);
     */
    let t_range = tmp_var_new(proc);

    if let Some(range) = &s.range {
        let t_range_addr = tmp_var_new(proc);
        let t_range_lhs = tmp_var_new(proc);
        let t_range_rhs = tmp_var_new(proc);

        block_get(proc, block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_range),
                types: IRType::LIST,
                source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                op: IROp::NativeCall(vec![IRValue::Number(2.into())]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_range_addr),
                types: IRType::PTR,
                source: IRValue::Variable(t_range),
                op: IROp::PtrAddress,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_range_lhs),
                types: IRType::PTR,
                source: IRValue::Variable(t_range_addr),
                op: IROp::AccessArray(IRValue::Number(0.into())),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_range_rhs),
                types: IRType::PTR,
                source: IRValue::Variable(t_range_addr),
                op: IROp::AccessArray(IRValue::Number(1.into())),
            }),
        ]);

        if let Some(lhs) = &range.left {
            block_cst_expr_push(&*lhs, block_idx, IRTarget::Deref(t_range_lhs), proc);
        } else {
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Deref(t_range_lhs),
                types: IRType::UNDEFINED,
                source: IRValue::Undefined,
                op: IROp::Assign,
            }));
        }

        if let Some(rhs) = &range.right {
            block_cst_expr_push(&*rhs, block_idx, IRTarget::Deref(t_range_rhs), proc);
        } else {
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Deref(t_range_rhs),
                types: IRType::UNDEFINED,
                source: IRValue::Undefined,
                op: IROp::Assign,
            }));
        }
    } else {
        block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_range),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }));
    }

    let t_expressions = tmp_var_new(proc);
    block_cst_expr_vec_push(
        &s.expressions,
        block_idx,
        IRTarget::Variable(t_expressions),
        proc,
    );

    let t_rest = tmp_var_new(proc);
    if let Some(rest) = &s.rest {
        block_cst_expr_push(&*rest, block_idx, IRTarget::Variable(t_rest), proc);
    } else {
        block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_rest),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }));
    }

    block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::AST,
        source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
        op: IROp::NativeCall(vec![
            IRValue::String(name),
            IRValue::Variable(t_range),
            IRValue::Variable(t_expressions),
            IRValue::Variable(t_rest),
        ]),
    }));
}

fn block_cst_comprehension_push(
    name: String,
    s: &CSTComprehension,
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    /* t_expression = // expr
     * t_iterators = // iter params
     * t_cond = // expr
     * target := ast_node_new(name, t_expression, t_iterators, t_cond);
     */
    let t_expression = tmp_var_new(proc);
    let t_iterators = tmp_var_new(proc);
    let t_cond = tmp_var_new(proc);

    block_cst_expr_push(
        &*s.expression,
        block_idx,
        IRTarget::Variable(t_expression),
        proc,
    );
    block_cst_iter_params_push(
        &s.iterators,
        block_idx,
        IRTarget::Variable(t_iterators),
        proc,
    );
    if let Some(cond) = &s.condition {
        block_cst_expr_push(&*cond, block_idx, IRTarget::Variable(t_cond), proc);
    } else {
        block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cond),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }));
    }

    block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::AST,
        source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
        op: IROp::NativeCall(vec![
            IRValue::String(name),
            IRValue::Variable(t_expression),
            IRValue::Variable(t_iterators),
            IRValue::Variable(t_cond),
        ]),
    }));
}

pub fn block_cst_collection_push(
    c: &CSTCollection,
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    match c {
        CSTCollection::Set(s) => {
            block_cst_set_push("set".to_string(), s, block_idx, target, proc);
        }
        CSTCollection::List(l) => {
            block_cst_set_push("list".to_string(), l, block_idx, target, proc);
        }
        CSTCollection::SetComprehension(c) => {
            block_cst_comprehension_push(
                "setComprehension".to_string(),
                c,
                block_idx,
                target,
                proc,
            );
        }
        CSTCollection::ListComprehension(c) => {
            block_cst_comprehension_push(
                "listComprehension".to_string(),
                c,
                block_idx,
                target,
                proc,
            );
        }
    }
}
