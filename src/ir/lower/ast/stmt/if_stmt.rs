use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::*;
use crate::ir::def::*;
use crate::ir::lower::ast::block_cst_block_push;
use crate::ir::lower::ast::expr::block_cst_expr_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_cst_if_push(
    name: String,
    i: &CSTIf,
    block_idx: NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    /* t_branches := //branches
     * t_alt := // expr
     * target := ast_node_new(name, t_branches, t_alt);
     */
    let t_branches = tmp_var_new(proc);
    let t_alt = tmp_var_new(proc);

    block_get(proc, block_idx).push(
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_branches),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(vec![IRValue::Number(i.branches.len().into())]),
        })
    );

    i.branches.iter().for_each(|i| {
        /* t_cond := // expr
         * t_block := // block
         * t_i := ast_node_new("ifBranch", t_cond, t_block);
         * _ := list_push(t_branches, t_i);
         */
        let t_cond = tmp_var_new(proc);
        let t_block = tmp_var_new(proc);
        let t_i = tmp_var_new(proc);

        block_cst_expr_push(&i.condition, block_idx, IRTarget::Variable(t_cond), proc);
        block_cst_block_push(&i.block, block_idx, IRTarget::Variable(t_block), proc);

        block_get(proc, block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_i),
                types: IRType::AST,
                source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
                op: IROp::NativeCall(vec![
                    IRValue::String("ifBranch".to_string()),
                    IRValue::Variable(t_cond),
                    IRValue::Variable(t_block),
                ]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(t_branches),
                    IRValue::Variable(t_i),
                ]),
            }),
        ]);
    });

    if let Some(alt) = &i.alternative {
        block_cst_block_push(alt, block_idx, IRTarget::Variable(t_alt), proc);
    } else {
        block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_alt),
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
            IRValue::Variable(t_branches),
            IRValue::Variable(t_alt),
        ]),
    }));
}
