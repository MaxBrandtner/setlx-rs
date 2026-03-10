use petgraph::stable_graph::NodeIndex;
use std::collections::BTreeMap;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::assign::assign_parse;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::proc::expr_vars_push;
use crate::ir::lower::util::{block_get, stack_pop, tmp_var_new};

/// iterator template function
///
/// # Arguments
///
/// * `params` - the iterator parameters, each binding a variable pattern to a collection
/// * `condition` - optional filter expression ANDed with the equality constraints
/// * `ret_idx` - if provided, stack variables are cleaned up before jumping to this block on return
/// * `expr_mod` - callback that emits the loop body; receives `backtrack_idx` to
///   continue to the next iteration and `follow_idx` to break out of the loop
/// * `arg` - additional argument forwarded unchanged to `expr_mod`
pub fn block_iterator_push<T>(
    block_idx: &mut NodeIndex,
    params: &[CSTIterParam],
    condition: &Option<Box<CSTExpression>>,
    ret_idx: Option<NodeIndex>,
    expr_mod: fn(
        expr_idx: NodeIndex,
        backtrack_idx: NodeIndex,
        follow_idx: NodeIndex,
        ret_idx: Option<NodeIndex>,
        T,
        &mut IRProcedure,
        &mut IRSharedProc,
        &mut IRCfg,
    ),
    arg: T,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    let ret_pop_idx = ret_idx.map(|_|proc.blocks.add_node(Vec::new()));
    let mut varnames: Vec<String> = Vec::new();
    let mut vars_map: BTreeMap<String, Vec<IRVar>> = BTreeMap::new();

    let mut current_idx = proc.blocks.add_node(Vec::new());
    let mut next_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());
    let mut backtrack_idx = follow_idx;

    block_get(proc, *block_idx).push(IRStmt::Goto(current_idx));
    proc.blocks.add_edge(*block_idx, current_idx, ());

    for (idx, param) in params.iter().enumerate() {
        /* <current_idx>:
         * t_expr := // expr
         * t_iter := iter_new(t_expr);
         * goto <iter_idx>
         *
         * <iter_idx>:
         * t_i := om;
         * t_i_addr := &t_i;
         * t_cond := iter_next(t_iter);
         * if t_cond
         *  goto <next_idx>
         * else
         *  goto <backtrack_expr_idx>
         *
         * <backtrack_expr_idx>:
         * _ := invalidate(t_expr);
         *  goto <backtrack_idx>
         *
         * <next_idx>:
         *  // stack add
         *  // assign_parse
         */
        let t_expr = tmp_var_new(proc);
        let expr_init_idx = proc.blocks.add_node(Vec::new());
        let mut expr_idx = expr_init_idx;
        block_get(proc, current_idx).push(IRStmt::Goto(expr_init_idx));
        proc.blocks.add_edge(current_idx, expr_idx, ());

        let expr_owned = block_expr_push(
            &param.collection,
            &mut expr_idx,
            IRTarget::Variable(t_expr),
            proc,
            shared_proc,
            cfg,
        );

        if expr_owned && let Some(ret_pop_idx_val) = ret_pop_idx {
            block_get(proc, ret_pop_idx_val).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
            }));
        }

        let backtrack_expr_idx = if expr_owned {
            let backtrack_expr_idx = proc.blocks.add_node(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
                }),
                IRStmt::Goto(backtrack_idx),
            ]);
            proc.blocks.add_edge(backtrack_expr_idx, backtrack_idx, ());
            backtrack_expr_idx
        } else {
            backtrack_idx
        };

        current_idx = expr_idx;

        let iter_idx = proc.blocks.add_node(Vec::new());

        let t_iter = tmp_var_new(proc);
        block_get(proc, current_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_iter),
                types: IRType::ITERATOR,
                source: IRValue::BuiltinProc(BuiltinProc::IterNew),
                op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
            }),
            IRStmt::Goto(iter_idx),
        ]);
        proc.blocks.add_edge(current_idx, iter_idx, ());

        let t_i = tmp_var_new(proc);
        let t_i_addr = tmp_var_new(proc);
        let t_cond = tmp_var_new(proc);

        block_get(proc, iter_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_i),
                types: IRType::UNDEFINED,
                source: IRValue::Undefined,
                op: IROp::Assign,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_i_addr),
                types: IRType::PTR,
                source: IRValue::Variable(t_i),
                op: IROp::PtrAddress,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_cond),
                types: IRType::BOOL,
                source: IRValue::BuiltinProc(BuiltinProc::IterNext),
                op: IROp::NativeCall(vec![IRValue::Variable(t_iter), IRValue::Variable(t_i_addr)]),
            }),
            IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_cond),
                success: next_idx,
                failure: backtrack_expr_idx,
            }),
        ]);
        proc.blocks.add_edge(iter_idx, next_idx, ());
        proc.blocks.add_edge(iter_idx, backtrack_expr_idx, ());

        let mut vars = Vec::new();
        expr_vars_push(&param.variable, &mut vars);

        vars.iter().for_each(|i| {
            let t_var = tmp_var_new(proc);

            block_get(proc, next_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_var),
                types: IRType::PTR,
                source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
                op: IROp::NativeCall(vec![IRValue::String(i.to_string())]),
            }));

            if let Some(ret_pop_idx_val) = ret_pop_idx {
                block_get(proc, ret_pop_idx_val).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::StackPop),
                    op: IROp::NativeCall(vec![IRValue::String(i.to_string())]),
                }));
            }

            shared_proc.definitions.push((i.to_string(), t_var));
            vars_map.entry(i.to_string())
                .or_default()
                .push(t_var);
        });

        varnames.extend(vars);

        assign_parse(
            &mut next_idx,
            t_i,
            false,
            None,
            &params[idx].variable,
            proc,
            shared_proc,
            cfg,
        );

        backtrack_idx = iter_idx;
        current_idx = next_idx;
        next_idx = proc.blocks.add_node(Vec::new());
    }

    // t_equal_all := true;
    let t_equal_all = tmp_var_new(proc);
    block_get(proc, current_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_equal_all),
        types: IRType::BOOL,
        source: IRValue::Bool(true),
        op: IROp::Assign,
    }));

    for (_, t_vars) in vars_map {
        // t_var_first_val := *t_vars[0];
        let t_var_first_val = tmp_var_new(proc);
        block_get(proc, current_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_var_first_val),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_vars[0]),
            op: IROp::PtrDeref,
        }));

        for t_var in t_vars.iter().skip(1) {
            /* t_var_val = *t_var;
             * t_equal := t_var_first_val == t_var_val;
             * t_equal_all := t_equal_all && t_equal;
             */
            let t_var_val = tmp_var_new(proc);
            let t_equal = tmp_var_new(proc);
            block_get(proc, current_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_var_val),
                    types: IRTypes!("any"),
                    source: IRValue::Variable(*t_var),
                    op: IROp::PtrDeref,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_equal),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_var_first_val),
                    op: IROp::Equal(IRValue::Variable(t_var_val)),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_equal_all),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_equal_all),
                    op: IROp::And(IRValue::Variable(t_equal)),
                }),
            ]);
        }
    }

    if let Some(cond) = condition {
        /* t_cond := //expr
         * t_equal_all := t_equal_all && t_cond;
         */
        let t_cond = tmp_var_new(proc);
        // NOTE: cond must return a boolean value which doesn't require memory
        let _ = block_expr_push(
            cond,
            &mut current_idx,
            IRTarget::Variable(t_cond),
            proc,
            shared_proc,
            cfg,
        );
        block_get(proc, current_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_equal_all),
            types: IRType::BOOL,
            source: IRValue::Variable(t_equal_all),
            op: IROp::And(IRValue::Variable(t_cond)),
        }));
    }

    let expr_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, current_idx).push(IRStmt::Branch(IRBranch {
        cond: IRValue::Variable(t_equal_all),
        success: expr_idx,
        failure: backtrack_idx,
    }));
    proc.blocks.add_edge(current_idx, expr_idx, ());
    proc.blocks.add_edge(current_idx, backtrack_idx, ());

    if let Some(pop_ret_idx_val) = ret_pop_idx {
        // goto <ret_idx>
        block_get(proc, pop_ret_idx_val).push(IRStmt::Goto(ret_idx.unwrap()));
        proc.blocks.add_edge(pop_ret_idx_val, ret_idx.unwrap(), ());
    }

    expr_mod(
        expr_idx,
        backtrack_idx,
        follow_idx,
        ret_pop_idx,
        arg,
        proc,
        shared_proc,
        cfg,
    );

    varnames.iter().rev().for_each(|i| stack_pop(shared_proc, i));

    *block_idx = follow_idx;
}
