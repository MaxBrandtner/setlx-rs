mod if_stmt;
mod match_stmt;
pub mod params;
mod try_catch_stmt;

use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::*;
use crate::ir::def::*;
use crate::ir::lower::ast::block_cst_block_push;
use crate::ir::lower::ast::expr::{
    block_cst_expr_opt_box_push, block_cst_expr_opt_push, block_cst_expr_push,
};
use crate::ir::lower::util::{block_get, tmp_var_new};

use if_stmt::block_cst_if_push;
use match_stmt::{block_cst_match_branches_push, block_cst_match_push};
use params::{block_cst_iter_params_push, block_cst_params_push};
use try_catch_stmt::block_cst_try_catch_push;

pub fn block_cst_stmt_push(
    stmt: &CSTStatement,
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    match stmt {
        CSTStatement::Class(c) => {
            /* t_params := // params
             * t_block := // block
             * t_static_block := // block
             * target := ast_node_new("class", c.name, t_params, t_block, t_static_block);
             */
            let t_params = tmp_var_new(proc);
            let t_block = tmp_var_new(proc);
            let t_static_block = tmp_var_new(proc);

            block_cst_params_push(&c.params, block_idx, IRTarget::Variable(t_params), proc);
            block_cst_block_push(&c.block, block_idx, IRTarget::Variable(t_block), proc);
            if let Some(s_block) = &c.static_block {
                block_cst_block_push(s_block, block_idx, IRTarget::Variable(t_static_block), proc);
            } else {
                block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_static_block),
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
                    IRValue::String("class".to_string()),
                    IRValue::String(c.name.to_string()),
                    IRValue::Variable(t_params),
                    IRValue::Variable(t_block),
                    IRValue::Variable(t_static_block),
                ]),
            }));
        }
        CSTStatement::If(i) => {
            block_cst_if_push("if".to_string(), i, block_idx, target, proc);
        }
        CSTStatement::Switch(i) => {
            block_cst_if_push("switch".to_string(), i, block_idx, target, proc);
        }
        CSTStatement::Match(m) => {
            block_cst_match_push(m, block_idx, target, proc);
        }
        CSTStatement::Scan(s) => {
            /* t_expr := //expr
             * t_branches := // branches
             * target := ast_node_new("scan", t_expr, s.variable, t_branches);
             */
            let t_expr = tmp_var_new(proc);
            let t_branches = tmp_var_new(proc);

            block_cst_expr_push(&s.expression, block_idx, IRTarget::Variable(t_expr), proc);
            block_cst_match_branches_push(
                &s.branches,
                block_idx,
                IRTarget::Variable(t_branches),
                proc,
            );
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("scan".to_string()),
                    IRValue::Variable(t_expr),
                    if let Some(var) = &s.variable {
                        IRValue::String(var.to_string())
                    } else {
                        IRValue::Undefined
                    },
                    IRValue::Variable(t_branches),
                ]),
            }));
        }
        CSTStatement::For(f) => {
            /* t_params := // iter params
             * t_cond := // expr
             * t_block := // block
             * target := ast_node_new("for", t_params, t_cond, t_block);
             */
            let t_params = tmp_var_new(proc);
            let t_cond = tmp_var_new(proc);
            let t_block = tmp_var_new(proc);

            block_cst_iter_params_push(&f.params, block_idx, IRTarget::Variable(t_params), proc);
            block_cst_expr_opt_box_push(
                &f.condition,
                block_idx,
                IRTarget::Variable(t_params),
                proc,
            );
            block_cst_block_push(&f.block, block_idx, IRTarget::Variable(t_block), proc);

            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("for".to_string()),
                    IRValue::Variable(t_params),
                    IRValue::Variable(t_cond),
                    IRValue::Variable(t_block),
                ]),
            }));
        }
        CSTStatement::While(w) => {
            /* t_cond := // expr
             * t_block := //block
             * target := ast_node_new("while", t_cond, t_block);
             */
            let t_cond = tmp_var_new(proc);
            let t_block = tmp_var_new(proc);

            block_cst_expr_push(&w.condition, block_idx, IRTarget::Variable(t_cond), proc);
            block_cst_block_push(&w.block, block_idx, IRTarget::Variable(t_block), proc);

            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("while".to_string()),
                    IRValue::Variable(t_cond),
                    IRValue::Variable(t_block),
                ]),
            }));
        }
        CSTStatement::DoWhile(w) => {
            /* t_cond := // expr
             * t_block := //block
             * target := ast_node_new("doWhile", t_cond, t_block);
             */
            let t_cond = tmp_var_new(proc);
            let t_block = tmp_var_new(proc);

            block_cst_expr_push(&w.condition, block_idx, IRTarget::Variable(t_cond), proc);
            block_cst_block_push(&w.block, block_idx, IRTarget::Variable(t_block), proc);

            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("doWhile".to_string()),
                    IRValue::Variable(t_cond),
                    IRValue::Variable(t_block),
                ]),
            }));
        }
        CSTStatement::TryCatch(c) => {
            block_cst_try_catch_push(c, block_idx, target, proc);
        }
        CSTStatement::Check(c) => {
            /* t_block := // block
             * t_ab := // block
             * target := ast_node_new("check", t_block, t_ab);
             */
            let t_block = tmp_var_new(proc);
            let t_ab = tmp_var_new(proc);

            block_cst_block_push(&c.block, block_idx, IRTarget::Variable(t_block), proc);
            block_cst_block_push(
                &c.after_backtrack,
                block_idx,
                IRTarget::Variable(t_ab),
                proc,
            );
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("check".to_string()),
                    IRValue::Variable(t_block),
                    IRValue::Variable(t_ab),
                ]),
            }));
        }
        CSTStatement::Return(e) => {
            /* t_ret := //expr
             *  target := ast_node_new("return", t_ret);
             */
            let t_ret = tmp_var_new(proc);
            block_cst_expr_opt_push(&e.val, block_idx, IRTarget::Variable(t_ret), proc);
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("return".to_string()),
                    IRValue::Variable(t_ret),
                ]),
            }));
        }
        CSTStatement::Assign(a) => {
            /* t_assign := //expr
             * t_expr := //stmt
             * target := ast_node_new("assign", t_assign, t_expr);
             */
            let t_assign = tmp_var_new(proc);
            let t_expr = tmp_var_new(proc);

            block_cst_expr_push(&a.assign, block_idx, IRTarget::Variable(t_assign), proc);
            block_cst_stmt_push(&*a.expr, block_idx, IRTarget::Variable(t_expr), proc);
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("assign".to_string()),
                    IRValue::Variable(t_assign),
                    IRValue::Variable(t_expr),
                ]),
            }));
        }
        CSTStatement::AssignMod(a) => {
            /* t_assign := //expr
             * t_expr := //expr
             * target := ast_node_new(a.kind.to_string(), t_assign, t_expr);
             */
            let t_assign = tmp_var_new(proc);
            let t_expr = tmp_var_new(proc);

            block_cst_expr_push(&a.assign, block_idx, IRTarget::Variable(t_assign), proc);
            block_cst_expr_push(&a.expr, block_idx, IRTarget::Variable(t_expr), proc);
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String(a.kind.to_string()),
                    IRValue::Variable(t_assign),
                    IRValue::Variable(t_expr),
                ]),
            }));
        }
        CSTStatement::Expression(e) => {
            block_cst_expr_push(e, block_idx, target, proc);
        }
        CSTStatement::Backtrack => {
            // target := ast_node_new("backtrack");
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("backtrack".to_string()),
                ]),
            }));
        }
        CSTStatement::Break => {
            // target := ast_node_new("break");
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("break".to_string()),
                ]),
            }));
        }
        CSTStatement::Continue => {
            // target := ast_node_new("continue");
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("Continue".to_string()),
                ]),
            }));
        }
        CSTStatement::Exit => {
            // target := ast_node_new("exit");
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("exit".to_string()),
                ]),
            }));
        }
    }
}
