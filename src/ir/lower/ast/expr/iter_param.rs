use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::*;
use crate::ir::def::*;
use crate::ir::lower::ast::expr::block_cst_expr_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_cst_iter_params_push(
    iter: &[CSTIterParam],
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    /* t_list := list_new(iter.len());
     *
     * t_variable := // expr
     * t_collection := // expr
     * t_i := ast_node_new("iterParam", t_variable, t_collection);
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
            op: IROp::NativeCall(vec![IRValue::Number(iter.len().into())]),
        })
    );

    iter.iter().for_each(|i| {
        let t_variable = tmp_var_new(proc);
        let t_collection = tmp_var_new(proc);
        let t_i = tmp_var_new(proc);

        block_cst_expr_push(&i.variable, block_idx, IRTarget::Variable(t_variable), proc);
        block_cst_expr_push(
            &i.collection,
            block_idx,
            IRTarget::Variable(t_collection),
            proc,
        );
        block_get(proc, block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_i),
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("iterParam".to_string()),
                    IRValue::Variable(t_variable),
                    IRValue::Variable(t_collection),
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
