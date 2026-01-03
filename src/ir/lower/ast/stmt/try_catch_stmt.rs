use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::*;
use crate::ir::def::*;
use crate::ir::lower::ast::block_cst_block_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_cst_try_catch_push(
    c: &CSTTryCatch,
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    /* t_try := // block
     * t_catch := list_new(c.catch_branches.len());
     *
     * t_catch_block := //block
     * t_i := ast_node_new(i.kind.to_string(), i.exception, t_catch_block);
     * _ := list_push(t_catch, t_i);
     *
     * target := ast_node_new("tryCatch", t_try, t_catch);
     */
    let t_try = tmp_var_new(proc);
    let t_catch = tmp_var_new(proc);

    block_cst_block_push(&c.try_branch, block_idx, IRTarget::Variable(t_try), proc);
    block_get(proc, block_idx).push(
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_catch),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(vec![IRValue::Number(c.catch_branches.len().into())]),
        })
    );

    c.catch_branches.iter().for_each(|i| {
        let t_catch_block = tmp_var_new(proc);
        let t_i = tmp_var_new(proc);

        block_cst_block_push(&i.block, block_idx, IRTarget::Variable(t_catch_block), proc);
        block_get(proc, block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_i),
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String(i.kind.to_string()),
                    IRValue::String(i.exception.to_string()),
                    IRValue::Variable(t_catch_block),
                ]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(t_catch),
                    IRValue::Variable(t_i),
                ]),
            }),
        ]);
    });

    block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::AST,
        source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
        op: IROp::NativeCall(vec![
            IRValue::String("tryCatch".to_string()),
            IRValue::Variable(t_try),
            IRValue::Variable(t_catch),
        ]),
    }));
}
