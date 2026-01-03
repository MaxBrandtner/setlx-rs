pub mod expr;
pub mod stmt;

use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::*;
use crate::ir::def::*;
use crate::ir::lower::util::{block_get, tmp_var_new};

use stmt::block_cst_stmt_push;

pub fn block_cst_block_push(
    block: &CSTBlock,
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    /* t_list := list_new(block.len());
     *
     * t_stmt := //stmt
     * _ := list_push(t_list, t_stmt);
     *
     * target := t_list;
     */
    let t_list = tmp_var_new(proc);

    block_get(proc, block_idx).push(
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_list),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(vec![IRValue::Number(block.len().into())]),
        }),
    );

    block.iter().for_each(|i| {
        let t_stmt = tmp_var_new(proc);
        block_cst_stmt_push(i, block_idx, IRTarget::Variable(t_stmt), proc);
        block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ListPush),
            op: IROp::NativeCall(vec![IRValue::Variable(t_list), IRValue::Variable(t_stmt)]),
        }));
    });

    block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::LIST,
        source: IRValue::Variable(t_list),
        op: IROp::Assign,
    }));
}
