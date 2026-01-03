use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::*;
use crate::ir::def::*;
use crate::ir::lower::ast::block_cst_block_push;
use crate::ir::lower::ast::expr::{block_cst_expr_push, block_cst_expr_vec_push};
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_cst_match_branches_push(
    b: &[CSTMatchBranch],
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    /* t_list := list_new(b.len());
     *
     * // branch push
     *
     * target := t_list;
     */
    let t_list = tmp_var_new(proc);

    block_get(proc, block_idx).push(
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_list),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(vec![IRValue::Number(b.len().into())]),
        })
    );

    b.iter().for_each(|i| {
        match i {
            CSTMatchBranch::Case(c) => {
                /* t_exprs := // exprs
                 * t_cond := // expr
                 * t_stmt := // block
                 * t_i := ast_node_new("matchBranchCase", t_exprs, t_cond, t_stmt);
                 * _ := list_push(t_list, t_i);
                 */
                let t_exprs = tmp_var_new(proc);
                let t_cond = tmp_var_new(proc);
                let t_stmt = tmp_var_new(proc);
                let t_i = tmp_var_new(proc);

                block_cst_expr_vec_push(
                    &c.expressions,
                    block_idx,
                    IRTarget::Variable(t_exprs),
                    proc,
                );

                if let Some(cond) = &c.condition {
                    block_cst_expr_push(cond, block_idx, IRTarget::Variable(t_cond), proc);
                } else {
                    block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_cond),
                        types: IRType::UNDEFINED,
                        source: IRValue::Undefined,
                        op: IROp::Assign,
                    }));
                }

                block_cst_block_push(&c.statements, block_idx, IRTarget::Variable(t_stmt), proc);

                block_get(proc, block_idx).extend(vec![
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_i),
                        types: IRType::AST,
                        source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                        op: IROp::NativeCall(vec![
                            IRValue::String("matchBranchCase".to_string()),
                            IRValue::Variable(t_exprs),
                            IRValue::Variable(t_cond),
                            IRValue::Variable(t_stmt),
                        ]),
                    }),
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Ignore,
                        types: IRType::UNDEFINED,
                        source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                        op: IROp::NativeCall(vec![
                            IRValue::Variable(t_list),
                            IRValue::Variable(t_i),
                        ]),
                    }),
                ]);
            }
            CSTMatchBranch::Regex(r) => {
                /* t_pattern := //expr
                 * t_pattern_out := // expr
                 * t_cond := // expr
                 * t_stmt := // block
                 * t_i := ast_node_new("matchBranchRegex", t_pattern, t_pattern_out t_cond,
                 * t_stmt);
                 * _ := list_push(t_list, t_i);
                 */
                let t_pattern = tmp_var_new(proc);
                let t_pattern_out = tmp_var_new(proc);
                let t_cond = tmp_var_new(proc);
                let t_stmt = tmp_var_new(proc);
                let t_i = tmp_var_new(proc);

                block_cst_expr_push(&r.pattern, block_idx, IRTarget::Variable(t_pattern), proc);
                if let Some(po) = &r.pattern_out {
                    block_cst_expr_push(po, block_idx, IRTarget::Variable(t_pattern_out), proc);
                } else {
                    block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_pattern_out),
                        types: IRType::UNDEFINED,
                        source: IRValue::Undefined,
                        op: IROp::Assign,
                    }));
                }

                if let Some(cond) = &r.condition {
                    block_cst_expr_push(cond, block_idx, IRTarget::Variable(t_cond), proc);
                } else {
                    block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_cond),
                        types: IRType::UNDEFINED,
                        source: IRValue::Undefined,
                        op: IROp::Assign,
                    }));
                }

                block_cst_block_push(&r.statements, block_idx, IRTarget::Variable(t_stmt), proc);

                block_get(proc, block_idx).extend(vec![
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_i),
                        types: IRType::AST,
                        source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                        op: IROp::NativeCall(vec![
                            IRValue::String("matchBranchRegex".to_string()),
                            IRValue::Variable(t_pattern),
                            IRValue::Variable(t_pattern_out),
                            IRValue::Variable(t_cond),
                            IRValue::Variable(t_stmt),
                        ]),
                    }),
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Ignore,
                        types: IRType::UNDEFINED,
                        source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                        op: IROp::NativeCall(vec![
                            IRValue::Variable(t_list),
                            IRValue::Variable(t_i),
                        ]),
                    }),
                ]);
            }
        }
    });

    block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::LIST,
        source: IRValue::Variable(t_list),
        op: IROp::Assign,
    }));
}

pub fn block_cst_match_push(
    m: &CSTMatch,
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    /* t_expr := // expr
     * t_branches := // match_branches
     * t_default := // block
     * target := ast_node_new("match", t_expr, t_branches, t_default);
     */
    let t_expr = tmp_var_new(proc);
    let t_branches = tmp_var_new(proc);
    let t_default = tmp_var_new(proc);

    block_cst_expr_push(&m.expression, block_idx, IRTarget::Variable(t_expr), proc);
    block_cst_match_branches_push(&m.branches, block_idx, IRTarget::Variable(t_branches), proc);
    block_cst_block_push(&m.default, block_idx, IRTarget::Variable(t_default), proc);

    block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::AST,
        source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
        op: IROp::NativeCall(vec![
            IRValue::String("match".to_string()),
            IRValue::Variable(t_expr),
            IRValue::Variable(t_branches),
            IRValue::Variable(t_default),
        ]),
    }));
}
