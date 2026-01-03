use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::assign::assign_parse;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::proc::expr_vars_push;
use crate::ir::lower::util::{block_get, stack_pop, tmp_var_new};

pub fn block_iterator_push<T>(
    block_idx: &mut NodeIndex,
    params: &[CSTIterParam],
    condition: &Option<Box<CSTExpression>>,
    expr_mod: fn(
        expr_idx: NodeIndex,
        backtrack_idx: NodeIndex,
        follow_idx: NodeIndex,
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
    struct IterVecParams {
        vars: Vec<(String, IRVar)>,
        iter: IRVar,
    }

    let mut owned_expr_vars: Vec<IRVar> = Vec::new();

    let iter_vec = params
        .iter()
        .map(|i| {
            /* t_expr := // expr;
             * t_iter := iter_new(t_expr);
             */
            let t_expr = tmp_var_new(proc);
            let expr_owned = block_expr_push(
                &i.collection,
                block_idx,
                IRTarget::Variable(t_expr),
                proc,
                shared_proc,
                cfg,
            );

            if expr_owned {
                owned_expr_vars.push(t_expr);
            }

            let t_iter = tmp_var_new(proc);
            block_get(proc, *block_idx).push(
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_iter),
                    types: IRType::ITERATOR,
                    source: IRValue::BuiltinProc(BuiltinProc::IterNew),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
                })
            );

            let mut vars_name: Vec<String> = Vec::new();
            expr_vars_push(&i.variable, &mut vars_name);

            let vars: Vec<(String, IRVar)> = vars_name
                .into_iter()
                .map(|var| {
                    // t_var := stack_add(var);
                    let t_var = tmp_var_new(proc);
                    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_var),
                        types: IRType::PTR,
                        source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
                        op: IROp::NativeCall(vec![IRValue::String(var.clone())]),
                    }));

                    shared_proc.definitions.push((var.clone(), t_var));
                    (var, t_var)
                })
                .collect();

            IterVecParams {
                vars,
                iter: t_iter,
            }
        })
        .collect::<Vec<IterVecParams>>();

    let follow_idx = proc.blocks.add_node(
        iter_vec
            .iter()
            .flat_map(|i| {
                i.vars
                    .iter()
                    .map(|(var, _)| {
                        // _ := stack_pop(var);
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Ignore,
                            types: IRType::UNDEFINED,
                            source: IRValue::BuiltinProc(BuiltinProc::StackPop),
                            op: IROp::NativeCall(vec![IRValue::String(var.clone())]),
                        })
                    })
                    .collect::<Vec<IRStmt>>()
            })
            .collect::<Vec<IRStmt>>(),
    );

    let mut backtrack_idx = follow_idx;
    let mut next_idx = proc.blocks.add_node(Vec::new());

    // goto <next_idx>
    block_get(proc, *block_idx).push(IRStmt::Goto(next_idx));
    proc.blocks.add_edge(*block_idx, next_idx, ());

    for (idx, i) in iter_vec.iter().enumerate() {
        /* t_entry := om;
         * t_entry_addr := &t_entry;
         * t_has_next := iter_next(t_iter, t_entry_addr);
         * if t_has_next
         *  goto <next_idx>
         * else
         *  goto <backtrack_idx>
         */
        let t_entry = tmp_var_new(proc);
        let t_entry_addr = tmp_var_new(proc);
        let t_has_next = tmp_var_new(proc);

        let current_idx = next_idx;
        next_idx = proc.blocks.add_node(Vec::new());

        block_get(proc, backtrack_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_has_next)]),
        }));

        block_get(proc, next_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_has_next)]),
        }));

        block_get(proc, current_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_entry),
                types: IRType::UNDEFINED,
                source: IRValue::Undefined,
                op: IROp::Assign,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_entry_addr),
                types: IRType::PTR,
                source: IRValue::Variable(t_entry),
                op: IROp::PtrAddress,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_has_next),
                types: IRType::BOOL,
                source: IRValue::BuiltinProc(BuiltinProc::IterNext),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(i.iter),
                    IRValue::Variable(t_entry_addr),
                ]),
            }),
        ]);

        assign_parse(
            &mut next_idx,
            t_entry,
            false,
            None,
            false,
            &params[idx].variable,
            proc,
            shared_proc,
            cfg,
        );
        block_get(proc, current_idx).push(IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_has_next),
            success: next_idx,
            failure: backtrack_idx,
        }));

        proc.blocks.add_edge(current_idx, next_idx, ());
        proc.blocks.add_edge(current_idx, backtrack_idx, ());

        backtrack_idx = current_idx;
    }

    let mut found_dup = false;
    // collect all instances of duplicate vars
    let mut dups: Vec<(String, Vec<IRVar>)> = Vec::new();
    for iter in &iter_vec {
        for (var_name, t_var) in &iter.vars {
            let mut found_match = false;
            for v in &mut dups {
                if v.0 == *var_name {
                    v.1.push(*t_var);
                    found_dup = true;
                    found_match = true;
                    break;
                }
            }
            if !found_match {
                dups.push((var_name.clone(), vec![*t_var]));
            }
        }
    }

    if found_dup {
        // t_res := true;
        let t_res = tmp_var_new(proc);
        block_get(proc, next_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_res),
            types: IRType::BOOL,
            source: IRValue::Bool(true),
            op: IROp::Assign,
        }));
        for (_, vars) in &dups {
            if vars.len() < 2 {
                continue;
            }

            let current_idx = next_idx;

            // t_var_first_val := *t_var;
            let t_var_first_val = tmp_var_new(proc);
            block_get(proc, current_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_var_first_val),
                types: IRTypes!("any"),
                source: IRValue::Variable(vars[0]),
                op: IROp::PtrDeref,
            }));

            for (idx, t_var) in vars.iter().enumerate() {
                if idx == 0 {
                    // var[0] is compared against
                    continue;
                }

                /* t_var_val := *t_var;
                 * t_equal := t_var_first_val == t_var_val;
                 * t_res := t_equal && t_res;
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
                        target: IRTarget::Variable(t_res),
                        types: IRType::BOOL,
                        source: IRValue::Variable(t_equal),
                        op: IROp::And(IRValue::Variable(t_res)),
                    }),
                ]);
            }
        }

        let current_idx = next_idx;
        next_idx = proc.blocks.add_node(Vec::new());

        block_get(proc, next_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_res)]),
        }));
        block_get(proc, backtrack_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_res)]),
        }));

        /* if t_res
         *  goto <next_idx>
         * else
         *  goto <backtrack_idx>
         */
        block_get(proc, current_idx).push(IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_res),
            success: next_idx,
            failure: backtrack_idx,
        }));

        proc.blocks.add_edge(current_idx, next_idx, ());
        proc.blocks.add_edge(current_idx, backtrack_idx, ());
    }

    if let Some(cond) = &condition {
        /* t_cond := cond;
         * if t_cond
         *   goto <next_idx>
         * else
         *   goto <backtrack_idx>
         */
        let t_cond = tmp_var_new(proc);
        let mut current_idx = next_idx;
        next_idx = proc.blocks.add_node(Vec::new());
        let cond_owned = block_expr_push(
            cond,
            &mut current_idx,
            IRTarget::Variable(t_cond),
            proc,
            shared_proc,
            cfg,
        );

        if cond_owned {
            block_get(proc, next_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_cond)]),
            }));
            block_get(proc, backtrack_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_cond)]),
            }));
        }

        block_get(proc, current_idx).push(IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cond),
            success: next_idx,
            failure: backtrack_idx,
        }));

        proc.blocks.add_edge(current_idx, next_idx, ());
        proc.blocks.add_edge(current_idx, backtrack_idx, ());
    }

    expr_mod(
        next_idx,
        backtrack_idx,
        follow_idx,
        arg,
        proc,
        shared_proc,
        cfg,
    );

    iter_vec.iter().rev().for_each(|i| {
        i.vars.iter().rev().for_each(|(v, t)| {
            stack_pop(shared_proc, v);
            block_get(proc, follow_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(*t)]),
                }),
            ]);
        });
    });

    owned_expr_vars.iter().for_each(|i| {
         block_get(proc, follow_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(*i)]),
                }),
            ]);
    });

    *block_idx = follow_idx;
}
