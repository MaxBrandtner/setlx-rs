use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::stmt::block_populate;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_if_push(
    c: &CSTIf,
    block_idx: &mut NodeIndex,
    continue_idx: Option<NodeIndex>,
    break_idx: Option<NodeIndex>,
    ret_idx: NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    let end_idx = proc.blocks.add_node(Vec::new());

    let mut else_init_idx = if c.alternative.is_some() {
        proc.blocks.add_node(Vec::new())
    } else {
        end_idx
    };
    let mut else_idx = else_init_idx;

    if let Some(alt) = &c.alternative {
        let terminated = block_populate(
            &mut else_idx,
            alt,
            continue_idx,
            break_idx,
            ret_idx,
            proc,
            shared_proc,
            cfg,
        );

        if !terminated {
            block_get(proc, else_idx).push(IRStmt::Goto(end_idx));
            proc.blocks.add_edge(else_idx, end_idx, ());
        }
    }

    for branch in c.branches.iter().rev() {
        let t_cond = tmp_var_new(proc);
        let mut current_idx = proc.blocks.add_node(Vec::new());
        let current_init_idx = current_idx;

        let mut branch_idx = proc.blocks.add_node(Vec::new());

        block_expr_push(
            &branch.condition,
            &mut current_idx,
            IRTarget::Variable(t_cond),
            proc,
            shared_proc,
            cfg,
        );

        block_get(proc, current_idx).push(IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cond),
            success: branch_idx,
            failure: else_init_idx,
        }));

        proc.blocks.add_edge(current_idx, branch_idx, ());
        proc.blocks.add_edge(current_idx, else_init_idx, ());

        let terminated = block_populate(
            &mut branch_idx,
            &branch.block,
            continue_idx,
            break_idx,
            ret_idx,
            proc,
            shared_proc,
            cfg,
        );

        if !terminated {
            block_get(proc, branch_idx).push(IRStmt::Goto(end_idx));
            proc.blocks.add_edge(branch_idx, end_idx, ());
        }

        else_init_idx = current_init_idx;
    }

    block_get(proc, *block_idx).push(IRStmt::Goto(else_init_idx));
    proc.blocks.add_edge(*block_idx, else_init_idx, ());
    *block_idx = end_idx;
}
