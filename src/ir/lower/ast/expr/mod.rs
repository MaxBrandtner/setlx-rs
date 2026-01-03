mod collection;
mod iter_param;

use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::*;
use crate::ir::def::*;
use crate::ir::lower::ast::block_cst_block_push;
use crate::ir::lower::ast::stmt::params::{block_cst_iter_params_push, block_cst_params_push};
use crate::ir::lower::util::{block_get, tmp_var_new};

use collection::block_cst_collection_push;

pub fn block_cst_expr_opt_box_push(
    expr: &Option<Box<CSTExpression>>,
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    if let Some(e) = &expr {
        block_cst_expr_push(e, block_idx, target, proc);
    } else {
        block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
            target,
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }));
    }
}

pub fn block_cst_expr_opt_push(
    expr: &Option<CSTExpression>,
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    if let Some(e) = &expr {
        block_cst_expr_push(e, block_idx, target, proc);
    } else {
        block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
            target,
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }));
    }
}

pub fn block_cst_expr_push(
    expr: &CSTExpression,
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    match expr {
        CSTExpression::Lambda(l) => {
            /* t_params := // collection
             * t_expr := // expr
             * target := ast_node_new("lambda", t_params, l.is_closure, t_expr);
             */
            let t_params = tmp_var_new(proc);
            let t_expr = tmp_var_new(proc);
            block_cst_collection_push(&l.params, block_idx, IRTarget::Variable(t_params), proc);
            block_cst_expr_push(&l.expr, block_idx, IRTarget::Variable(t_expr), proc);
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("lambda".to_string()),
                    IRValue::Variable(t_params),
                    IRValue::Bool(l.is_closure),
                    IRValue::Variable(t_expr),
                ]),
            }));
        }
        CSTExpression::Op(o) => {
            /* t_lhs := //expr
             * t_rhs := //expr
             * target := ast_node_new(o.op.to_string(), t_lhs, t_rhs);
             */
            let t_lhs = tmp_var_new(proc);
            let t_rhs = tmp_var_new(proc);

            block_cst_expr_push(&o.left, block_idx, IRTarget::Variable(t_lhs), proc);
            block_cst_expr_push(&o.right, block_idx, IRTarget::Variable(t_rhs), proc);
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String(o.op.to_string()),
                    IRValue::Variable(t_lhs),
                    IRValue::Variable(t_rhs),
                ]),
            }));
        }
        CSTExpression::UnaryOp(o) => {
            /* t_expr := // expr
             * target := ast_node_new(o.op.to_string(), t_expr);
             */
            let t_expr = tmp_var_new(proc);

            block_cst_expr_push(&o.expr, block_idx, IRTarget::Variable(t_expr), proc);
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String(o.op.to_string()),
                    IRValue::Variable(t_expr),
                ]),
            }));
        }
        CSTExpression::Procedure(p) => {
            /* t_params := // params
             * t_list_param := // list_param
             * t_block := // block
             * target := ast_node_new(p.kind.to_string(), t_params, t_list_params, t_block);
             */
            let t_params = tmp_var_new(proc);
            let t_list_param = tmp_var_new(proc);
            let t_block = tmp_var_new(proc);

            block_cst_params_push(&p.params, block_idx, IRTarget::Variable(t_params), proc);

            if let Some(lp) = &p.list_param {
                block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_list_param),
                    types: IRType::STRING,
                    source: IRValue::String(lp.to_string()),
                    op: IROp::Assign,
                }));
            } else {
                block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_list_param),
                    types: IRType::UNDEFINED,
                    source: IRValue::Undefined,
                    op: IROp::Assign,
                }));
            }

            block_cst_block_push(&p.block, block_idx, IRTarget::Variable(t_block), proc);

            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String(p.kind.to_string()),
                    IRValue::Variable(t_params),
                    IRValue::Variable(t_list_param),
                    IRValue::Variable(t_block),
                ]),
            }));
        }
        CSTExpression::Call(c) => {
            /* t_params := // exprs
             * t_rest_param := // expr
             * target := ast_node_new("call", c.name, t_params, t_rest_param);
             */
            let t_params = tmp_var_new(proc);
            let t_rest_param = tmp_var_new(proc);

            block_cst_expr_vec_push(&c.params, block_idx, IRTarget::Variable(t_params), proc);
            if let Some(rp) = &c.rest_param {
                block_cst_expr_push(rp, block_idx, IRTarget::Variable(t_rest_param), proc);
            } else {
                block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_rest_param),
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
                    IRValue::String("call".to_string()),
                    IRValue::String(c.name.to_string()),
                    IRValue::Variable(t_params),
                    IRValue::Variable(t_rest_param),
                ]),
            }));
        }
        CSTExpression::Term(t) => {
            /* t_params := //exprs
             * target := ast_node_new("term" || "tterm", t.name, t_params);
             */
            let t_params = tmp_var_new(proc);

            block_cst_expr_vec_push(&t.params, block_idx, IRTarget::Variable(t_params), proc);

            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types:IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String(if t.is_tterm { "tterm" } else { "term" }.to_string()),
                    IRValue::String(t.name.to_string()),
                    IRValue::Variable(t_params),
                ]),
            }));
        }
        CSTExpression::Variable(v) => {
            // target := ast_node_new("var", v);
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("var".to_string()),
                    IRValue::String(v.to_string()),
                ]),
            }));
        }
        CSTExpression::Accessible(a) => {
            /* t_head := // expr
             * t_body := // exprs
             * target := ast_node_new("accessible", t_head, t_body);
             */
            let t_head = tmp_var_new(proc);
            let t_body = tmp_var_new(proc);

            block_cst_expr_push(&a.head, block_idx, IRTarget::Variable(t_head), proc);
            block_cst_expr_vec_push(&a.body, block_idx, IRTarget::Variable(t_body), proc);

            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("accessible".to_string()),
                    IRValue::Variable(t_head),
                    IRValue::Variable(t_body),
                ]),
            }));
        }
        CSTExpression::String(s) => {
            // target := ast_node_new("string", s);
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("string".to_string()),
                    IRValue::String(s.to_string()),
                ]),
            }));
        }
        CSTExpression::Literal(l) => {
            // target := ast_node_new("literal", l);
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("literal".to_string()),
                    IRValue::String(l.to_string()),
                ]),
            }));
        }
        CSTExpression::Bool(b) => {
            // target := ast_node_new("bool", b);
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![IRValue::String("bool".to_string()), IRValue::Bool(*b)]),
            }));
        }
        CSTExpression::Double(d) => {
            // target := ast_node_new("double", d);
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("double".to_string()),
                    IRValue::Double(*d),
                ]),
            }));
        }
        CSTExpression::Number(n) => {
            // target := ast_node_new("number", n);
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("number".to_string()),
                    IRValue::Number(n.clone()),
                ]),
            }));
        }
        CSTExpression::Collection(c) => {
            block_cst_collection_push(c, block_idx, target, proc);
        }
        CSTExpression::Matrix(m) => {
            /* t_list := list_new(m.len());
             *
             * t_i := // exprs
             * _ := list_push(t_list, t_i);
             *
             * target := ast_node_new("matrix", t_list);
             */
            let t_list = tmp_var_new(proc);

            block_get(proc, block_idx).push(
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_list),
                    types: IRType::LIST,
                    source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                    op: IROp::NativeCall(vec![IRValue::Number(m.len().into())]),
                })
            );

            m.iter().for_each(|i| {
                let t_i = tmp_var_new(proc);

                block_cst_expr_vec_push(i, block_idx, IRTarget::Variable(t_i), proc);

                block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(t_list),
                        IRValue::Variable(t_i),
                    ]),
                }));
            });

            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("matrix".to_string()),
                    IRValue::Variable(t_list),
                ]),
            }));
        }
        CSTExpression::Vector(v) => {
            /* t_exprs := // exprs
             * target := ast_node_new("vector", t_exprs);
             */
            let t_exprs = tmp_var_new(proc);

            block_cst_expr_vec_push(v, block_idx, IRTarget::Variable(t_exprs), proc);
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("vector".to_string()),
                    IRValue::Variable(t_exprs),
                ]),
            }));
        }
        CSTExpression::Quantifier(q) => {
            /* t_iter := // params
             * t_cond := // expr
             * target := ast_node_new(q.kind.to_string(), t_iter, t_cond);
             */
            let t_iter = tmp_var_new(proc);
            let t_cond = tmp_var_new(proc);

            block_cst_iter_params_push(&q.iterators, block_idx, IRTarget::Variable(t_iter), proc);
            block_cst_expr_push(&q.condition, block_idx, IRTarget::Variable(t_cond), proc);
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String(q.kind.to_string()),
                    IRValue::Variable(t_iter),
                    IRValue::Variable(t_cond),
                ]),
            }));
        }
        CSTExpression::Om => {
            // target := ast_node_new("om");
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![IRValue::String("om".to_string())]),
            }));
        }
        CSTExpression::Ignore => {
            // target := ast_node_new("ignore");
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![IRValue::String("ignore".to_string())]),
            }));
        }
    }
}

pub fn block_cst_expr_vec_push(
    exprs: &[CSTExpression],
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    /* t_list := list_new(exprs.len());
     *
     * t_expr := // expr
     * _ := list_push(t_list, t_expr);
     *
     * target := t_list;
     */
    let t_list = tmp_var_new(proc);

    block_get(proc, block_idx).push(
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_list),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(vec![IRValue::Number(exprs.len().into())]),
        })
    );

    exprs.iter().for_each(|i| {
        let t_expr = tmp_var_new(proc);

        block_cst_expr_push(i, block_idx, IRTarget::Variable(t_expr), proc);
        block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ListPush),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_list),
                IRValue::Variable(t_expr),
            ]),
        }));
    });

    block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::LIST,
        source: IRValue::Variable(t_list),
        op: IROp::Assign,
    }));
}
