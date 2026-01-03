use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::iter::block_iterator_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_quant_push(
    q: &CSTQuantifier,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    let target = if let IRTarget::Ignore = target {
        IRTarget::Variable(tmp_var_new(proc))
    } else {
        target
    };

    match q.kind {
        // target := false;
        CSTQuantifierKind::Exists => block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target,
            types: IRType::BOOL,
            source: IRValue::Bool(false),
            op: IROp::Assign,
        })),
        // target := true;
        CSTQuantifierKind::Forall => block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target,
            types: IRType::BOOL,
            source: IRValue::Bool(true),
            op: IROp::Assign,
        })),
    }

    struct ExprModArgs<'a> {
        target: &'a IRTarget,
        condition: &'a CSTExpression,
        kind: &'a CSTQuantifierKind,
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
        let mut main_idx = expr_idx;

        let tmp = tmp_var_new(proc);
        block_expr_push(
            args.condition,
            &mut main_idx,
            *args.target,
            proc,
            shared_proc,
            cfg,
        );

        match args.kind {
            CSTQuantifierKind::Exists => {
                /* target := true;
                 * goto <follow_idx>
                 */
                let success_idx = proc.blocks.add_node(vec![
                    IRStmt::Assign(IRAssign {
                        target: *args.target,
                        types: IRType::BOOL,
                        source: IRValue::Bool(true),
                        op: IROp::Assign,
                    }),
                    IRStmt::Goto(follow_idx),
                ]);

                proc.blocks.add_edge(success_idx, follow_idx, ());

                /* if tmp
                 *    goto <success_idx>
                 * else
                 *    goto <backtrack_idx>
                 */
                block_get(proc, main_idx).push(IRStmt::Branch(IRBranch {
                    cond: IRValue::Variable(tmp),
                    success: success_idx,
                    failure: backtrack_idx,
                }));

                proc.blocks.add_edge(main_idx, success_idx, ());
                proc.blocks.add_edge(main_idx, backtrack_idx, ());
            }
            CSTQuantifierKind::Forall => {
                /* target := false;
                 * goto <follow_idx>
                 */
                let failure_idx = proc.blocks.add_node(vec![
                    IRStmt::Assign(IRAssign {
                        target: *args.target,
                        types: IRType::BOOL,
                        source: IRValue::Bool(false),
                        op: IROp::Assign,
                    }),
                    IRStmt::Goto(follow_idx),
                ]);

                proc.blocks.add_edge(failure_idx, follow_idx, ());

                /* if tmp
                 *   goto <backtrack_idx>
                 * else
                 *   goto <failure_idx>
                 */
                block_get(proc, main_idx).push(IRStmt::Branch(IRBranch {
                    cond: IRValue::Variable(tmp),
                    success: backtrack_idx,
                    failure: failure_idx,
                }));

                proc.blocks.add_edge(main_idx, backtrack_idx, ());
                proc.blocks.add_edge(main_idx, failure_idx, ());
            }
        }
    }

    block_iterator_push(
        block_idx,
        &q.iterators,
        &None,
        expr_mod,
        &ExprModArgs {
            target: &target,
            condition: &q.condition,
            kind: &q.kind,
        },
        proc,
        shared_proc,
        cfg,
    );
}
