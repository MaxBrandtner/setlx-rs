use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::iter::block_iterator_push;
use crate::ir::lower::stmt::block_populate;
use crate::ir::lower::util::block_get;

pub fn block_for_push(
    f: &CSTFor,
    block_idx: &mut NodeIndex,
    ret_idx: NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    struct ExprModArgs<'a> {
        block: &'a CSTBlock,
        ret_idx: NodeIndex,
    }

    fn expr_mod(
        expr_idx: NodeIndex,
        backtrack_idx: NodeIndex,
        follow_idx: NodeIndex,
        args: &ExprModArgs,
        proc: &mut IRProcedure,
        shared_proc: &mut IRSharedProc,
        cfg: &mut IRCfg,
    ) {
        let mut expr_idx = expr_idx;
        let terminated = block_populate(
            &mut expr_idx,
            args.block,
            Some(backtrack_idx),
            Some(follow_idx),
            args.ret_idx,
            proc,
            shared_proc,
            cfg,
        );

        if !terminated {
            block_get(proc, expr_idx).push(IRStmt::Goto(follow_idx));
            proc.blocks.add_edge(expr_idx, follow_idx, ());
        }
    }

    block_iterator_push(
        block_idx,
        &f.params,
        &f.condition,
        expr_mod,
        &ExprModArgs {
            block: &f.block,
            ret_idx,
        },
        proc,
        shared_proc,
        cfg,
    );
}
