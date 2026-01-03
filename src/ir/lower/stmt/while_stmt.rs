use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::stmt::block_populate;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_while_push(
    w: &CSTWhile,
    block_idx: &mut NodeIndex,
    ret_idx: NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /*
     * goto <bb1>;
     * <bb1>:
     * t_1 := cond;
     * if t_1
     *   goto <bb2>
     * else
     *   goto <bb3> // follow block
     * <bb2>:
     *   // while block
     *   goto <bb1>;
     */
    let mut cond_idx = proc.blocks.add_node(Vec::new());
    let cond_init_idx = cond_idx;
    let follow_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, *block_idx).push(IRStmt::Goto(cond_init_idx));
    proc.blocks.add_edge(*block_idx, cond_init_idx, ());

    let t_cond = tmp_var_new(proc);
    block_expr_push(
        &w.condition,
        &mut cond_idx,
        IRTarget::Variable(t_cond),
        proc,
        shared_proc,
        cfg,
    );

    let mut loop_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, cond_idx).push(IRStmt::Branch(IRBranch {
        cond: IRValue::Variable(t_cond),
        success: loop_idx,
        failure: follow_idx,
    }));
    proc.blocks.add_edge(cond_idx, loop_idx, ());
    proc.blocks.add_edge(cond_idx, follow_idx, ());

    let loop_terminated = block_populate(
        &mut loop_idx,
        &w.block,
        Some(cond_init_idx),
        Some(follow_idx),
        ret_idx,
        proc,
        shared_proc,
        cfg,
    );

    if !loop_terminated {
        block_get(proc, loop_idx).push(IRStmt::Goto(cond_init_idx));
        proc.blocks.add_edge(loop_idx, cond_init_idx, ());
    }

    *block_idx = follow_idx;
}

pub fn block_do_while_push(
    w: &CSTWhile,
    block_idx: &mut NodeIndex,
    ret_idx: NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /*
     * goto <bb1>;
     * <bb1>:
     * t_1 := // cond;
     * if t_1
     *   goto <bb2>
     * else
     *   goto <bb3> // follow block
     * <bb2>:
     *   // while block
     *   goto <bb1>;
     */
    let mut cond_idx = proc.blocks.add_node(Vec::new());
    let cond_init_idx = cond_idx;
    let follow_idx = proc.blocks.add_node(Vec::new());
    let mut loop_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, *block_idx).push(IRStmt::Goto(loop_idx));
    proc.blocks.add_edge(*block_idx, loop_idx, ());

    let t_cond = tmp_var_new(proc);
    block_expr_push(
        &w.condition,
        &mut cond_idx,
        IRTarget::Variable(t_cond),
        proc,
        shared_proc,
        cfg,
    );

    block_get(proc, cond_idx).push(IRStmt::Branch(IRBranch {
        cond: IRValue::Variable(t_cond),
        success: loop_idx,
        failure: follow_idx,
    }));
    proc.blocks.add_edge(cond_idx, loop_idx, ());
    proc.blocks.add_edge(cond_idx, follow_idx, ());

    let loop_terminated = block_populate(
        &mut loop_idx,
        &w.block,
        Some(cond_init_idx),
        Some(follow_idx),
        ret_idx,
        proc,
        shared_proc,
        cfg,
    );

    if !loop_terminated {
        block_get(proc, loop_idx).push(IRStmt::Goto(cond_init_idx));
        proc.blocks.add_edge(loop_idx, cond_init_idx, ());
    }

    *block_idx = follow_idx;
}
