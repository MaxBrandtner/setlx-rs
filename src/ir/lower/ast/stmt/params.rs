use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::*;
use crate::ir::def::*;
use crate::ir::lower::ast::expr::block_cst_expr_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_cst_params_push(
    params: &[CSTParam],
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    /* t_list := list_new(params.len());
     *
     * t_default := // expr
     * t_param := ast_node_new("param", i.name, i.is_rw, t_default);
     * _ := list_push(t_list, t_param);
     * target := t_list;
     */
    let t_list = tmp_var_new(proc);

    block_get(proc, block_idx).push(
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_list),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(vec![IRValue::Number(params.len().into())]),
        })
    );

    params.iter().for_each(|i| {
        let t_default = tmp_var_new(proc);
        if let Some(expr) = &i.default {
            block_cst_expr_push(expr, block_idx, target, proc);
        } else {
            block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_default),
                types: IRType::UNDEFINED,
                source: IRValue::Undefined,
                op: IROp::Assign,
            }));
        }

        let t_param = tmp_var_new(proc);

        block_get(proc, block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_param),
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String(i.name.to_string()),
                    IRValue::Bool(i.is_rw),
                    IRValue::Variable(t_default),
                ]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(t_list),
                    IRValue::Variable(t_param),
                ]),
            }),
        ]);
    });

    block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::LIST,
        source: IRValue::Variable(t_list),
        op: IROp::Assign,
    }));
}

pub fn block_cst_iter_params_push(
    p: &[CSTIterParam],
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    /* t_list := list_new(p.len());
     *
     * t_var := // expr
     * t_coll := // expr
     * t_i := ast_node_new("iterParam", t_var, t_coll);
     * _ := list_push(t_list, t_i);
     *
     * target := t_list;
     */
    let t_list = tmp_var_new(proc);

    block_get(proc, block_idx).push(
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_list),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(vec![IRValue::Number(p.len().into())]),
        })
    );

    p.iter().for_each(|i| {
        let t_var = tmp_var_new(proc);
        let t_coll = tmp_var_new(proc);
        let t_i = tmp_var_new(proc);

        block_cst_expr_push(&i.variable, block_idx, IRTarget::Variable(t_var), proc);
        block_cst_expr_push(&i.collection, block_idx, IRTarget::Variable(t_coll), proc);

        block_get(proc, block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_i),
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("iterParam".to_string()),
                    IRValue::Variable(t_var),
                    IRValue::Variable(t_coll),
                ]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                op: IROp::NativeCall(vec![IRValue::Variable(t_list), IRValue::Variable(t_i)]),
            }),
        ]);
    });

    block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::LIST,
        source: IRValue::Variable(t_list),
        op: IROp::Assign,
    }));
}
