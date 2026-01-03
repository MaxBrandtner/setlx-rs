use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::assign::assign_parse;
use crate::ir::lower::ast::expr::block_cst_expr_push;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::expr::var_expr::block_var_push;
use crate::ir::lower::proc::expr_vars_push;
use crate::ir::lower::stmt::block_populate;
use crate::ir::lower::util::{block_get, stack_pop, tmp_var_new};

fn block_case_equiv_push(
    t_expr: IRVar,
    t_matched: IRVar,
    current_idx: &mut NodeIndex,
    pop_vars: &mut Vec<String>,
    c: &CSTMatchBranchCase,
    pop_follow_idx: &mut NodeIndex,
    pop_end_idx: &mut NodeIndex,
    pop_continue_idx: &mut Option<NodeIndex>,
    pop_break_idx: &mut Option<NodeIndex>,
    pop_ret_idx: &mut NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    let e = &c.expressions[0];

    match e {
        CSTExpression::Literal(_) | CSTExpression::Bool(_) | CSTExpression::Number(_) => {
            /* t_e := // expr
             * t_matched := t_expr == t_e
             * _ := invalidate(t_e);
             */
            let t_e = tmp_var_new(proc);
            let t_e_owned = block_expr_push(
                e,
                current_idx,
                IRTarget::Variable(t_e),
                proc,
                shared_proc,
                cfg,
            );
            block_get(proc, *current_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_matched),
                types: IRType::BOOL,
                source: IRValue::Variable(t_expr),
                op: IROp::Equal(IRValue::Variable(t_e)),
            }));

            if t_e_owned {
                block_get(proc, *current_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_e)]),
                }));
            }
        }
        CSTExpression::Collection(_) => {
            /* // list_vars_push
             * // assign parse
             */
            expr_vars_push(e, pop_vars);

            pop_vars.iter().for_each(|i| {
                shared_proc
                    .definitions
                    .push((i.to_string(), tmp_var_new(proc)));
            });

            assign_parse(
                current_idx,
                t_expr,
                false,
                Some(t_matched),
                false,
                e,
                proc,
                shared_proc,
                cfg,
            );

            *pop_follow_idx = pop_block_new(pop_vars, *pop_follow_idx, proc);
            *pop_end_idx = pop_block_new(pop_vars, *pop_end_idx, proc);
            *pop_ret_idx = pop_block_new(pop_vars, *pop_ret_idx, proc);

            if let Some(b_idx) = *pop_continue_idx {
                *pop_continue_idx = Some(pop_block_new(pop_vars, b_idx, proc));
            }
            if let Some(b_idx) = *pop_break_idx {
                *pop_break_idx = Some(pop_block_new(pop_vars, b_idx, proc));
            }
        }
        CSTExpression::Term(_) | CSTExpression::Op(_) | CSTExpression::UnaryOp(_) => {
            /* // expr_vars_push
             * // assign parse
             */
            expr_vars_push(e, pop_vars);

            pop_vars.iter().for_each(|i| {
                shared_proc
                    .definitions
                    .push((i.to_string(), tmp_var_new(proc)));
            });

            assign_parse(
                current_idx,
                t_expr,
                false,
                Some(t_matched),
                false,
                e,
                proc,
                shared_proc,
                cfg,
            );

            *pop_follow_idx = pop_block_new(pop_vars, *pop_follow_idx, proc);
            *pop_end_idx = pop_block_new(pop_vars, *pop_end_idx, proc);
            *pop_ret_idx = pop_block_new(pop_vars, *pop_ret_idx, proc);

            if let Some(b_idx) = *pop_continue_idx {
                *pop_continue_idx = Some(pop_block_new(pop_vars, b_idx, proc));
            }
            if let Some(b_idx) = *pop_break_idx {
                *pop_break_idx = Some(pop_block_new(pop_vars, b_idx, proc));
            }
        }
        CSTExpression::Variable(v) => {
            /* t_var := // var
             * t_ast_match := ast_assign_eq(t_expr, t_var);
             * t_eq := t_var == t_expr;
             * t_matched := t_ast_match || t_eq;
             */
            let t_var = tmp_var_new(proc);
            let t_ast_match = tmp_var_new(proc);
            let t_eq = tmp_var_new(proc);

            block_var_push(v, current_idx, IRTarget::Variable(t_var), proc, shared_proc);
            block_get(proc, *current_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_ast_match),
                    types: IRType::BOOL,
                    source: IRValue::BuiltinProc(BuiltinProc::AstAssignEq),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_expr), IRValue::Variable(t_var)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_eq),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_var),
                    op: IROp::Equal(IRValue::Variable(t_expr)),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_matched),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_ast_match),
                    op: IROp::Equal(IRValue::Variable(t_eq)),
                }),
            ]);
        }
        _ => {
            /* t_e_cst := // convert e to cst
             * t_matched := ast_node_kind_eq(t_expr, t_e_cst);
             * _ := invalidate(t_e_cst);
             */
            let t_e_cst = tmp_var_new(proc);
            block_cst_expr_push(e, *current_idx, IRTarget::Variable(t_e_cst), proc);
            block_get(proc, *current_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_matched),
                    types: IRType::BOOL,
                    source: IRValue::BuiltinProc(BuiltinProc::AstNodeKindEq),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(t_expr),
                        IRValue::Variable(t_e_cst),
                    ]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_e_cst)]),
                }),
            ]);
        }
    }
}

fn block_match_case_push(
    t_expr: IRVar,
    c: &CSTMatchBranchCase,
    follow_idx: &mut NodeIndex,
    end_idx: NodeIndex,
    continue_idx: Option<NodeIndex>,
    break_idx: Option<NodeIndex>,
    ret_idx: NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /* <current_idx>:
     *  t_matched := false;
     *  // case-dependent
     *  if t_matched:
     *   goto <cond_idx>
     *  else
     *   goto <pop_follow_idx>
     *
     * <cond_idx>:
     *  t_cond := // cond
     *  if t_cond
     *   goto <block_idx>
     *  else
     *   goto <pop_follow_idx>
     *
     * <block_idx>:
     *  // block
     */
    let mut pop_follow_idx = *follow_idx;
    let mut pop_end_idx = end_idx;
    let mut pop_ret_idx = ret_idx;
    let mut pop_continue_idx = continue_idx;
    let mut pop_break_idx = break_idx;

    let mut pop_vars: Vec<String> = Vec::new();

    let t_matched = tmp_var_new(proc);
    let mut current_idx = proc.blocks.add_node(vec![IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_matched),
        types: IRType::BOOL,
        source: IRValue::Bool(false),
        op: IROp::Assign,
    })]);
    let end_idx = current_idx;

    block_case_equiv_push(
        t_expr,
        t_matched,
        &mut current_idx,
        &mut pop_vars,
        c,
        &mut pop_follow_idx,
        &mut pop_end_idx,
        &mut pop_continue_idx,
        &mut pop_break_idx,
        &mut pop_ret_idx,
        proc,
        shared_proc,
        cfg,
    );
    let mut cond_idx = proc.blocks.add_node(Vec::new());
    block_get(proc, current_idx).push(IRStmt::Branch(IRBranch {
        cond: IRValue::Variable(t_matched),
        success: cond_idx,
        failure: pop_follow_idx,
    }));

    proc.blocks.add_edge(current_idx, cond_idx, ());
    proc.blocks.add_edge(current_idx, pop_follow_idx, ());

    let block_idx = proc.blocks.add_node(Vec::new());
    let mut block_changed_idx = block_idx;
    block_populate(
        &mut block_changed_idx,
        &c.statements,
        pop_continue_idx,
        pop_break_idx,
        pop_ret_idx,
        proc,
        shared_proc,
        cfg,
    );

    block_get(proc, block_changed_idx).push(IRStmt::Goto(pop_end_idx));
    proc.blocks.add_edge(block_changed_idx, pop_end_idx, ());

    if let Some(cond) = &c.condition {
        let t_cond = tmp_var_new(proc);

        block_expr_push(
            cond,
            &mut cond_idx,
            IRTarget::Variable(t_cond),
            proc,
            shared_proc,
            cfg,
        );
        block_get(proc, cond_idx).push(IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cond),
            success: block_idx,
            failure: pop_follow_idx,
        }));
        proc.blocks.add_edge(cond_idx, block_idx, ());
        proc.blocks.add_edge(cond_idx, pop_follow_idx, ());
    } else {
        block_get(proc, cond_idx).push(IRStmt::Goto(block_idx));
        proc.blocks.add_edge(cond_idx, block_idx, ());
    }

    for i in pop_vars.iter().rev() {
        stack_pop(shared_proc, i);
    }

    *follow_idx = end_idx;
}

pub fn pop_block_new(out: &[String], masked_idx: NodeIndex, proc: &mut IRProcedure) -> NodeIndex {
    if out.is_empty() {
        return masked_idx;
    }

    let out = proc.blocks.add_node(
        out.iter()
            .map(|i| {
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::StackPop),
                    op: IROp::NativeCall(vec![IRValue::String(i.to_string())]),
                })
            })
            .collect::<Vec<IRStmt>>(),
    );

    block_get(proc, out).push(IRStmt::Goto(masked_idx));
    proc.blocks.add_edge(out, masked_idx, ());

    out
}

pub fn block_match_regex_push(
    t_expr: usize,
    r: &CSTMatchBranchRegex,
    is_multiline: bool,
    follow_idx: &mut NodeIndex,
    end_idx: NodeIndex,
    continue_idx: Option<NodeIndex>,
    break_idx: Option<NodeIndex>,
    ret_idx: NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /* <current_idx>:
     *  t_pattern := // pattern
     *  t_regex := regex_compile(t_pattern, 1 | is_multiline << 1)
     *  t_matched := false;
     *  t_matched_addr := &t_matched;
     *  t_assign := regex_match_groups(t_expr, t_regex, t_matched_addr);
     *  if t_matches
     *    goto <assign_idx>
     *  else
     *    goto <pop_follow_idx>
     *
     * <assign_idx>:
     *  // aggregate parameters and push them to the stack
     *  t_groups := stack_add("group_var");
     *  t_assign_success := false;
     *  // assign_parse
     *  *t_groups := t_assign;
     *  if t_assign_success:
     *   goto <cond_idx>
     *  else
     *   goto <pop_follow_idx>
     *
     * <cond_idx>:
     *  t_cond := // cond
     *  if t_cond
     *    goto <block_idx>
     *  else
     *    goto <pop_follow_idx>
     *
     * <block_idx>:
     *   // block
     *
     * <pop_follow_idx>:
     *  _ := invalidate(t_pattern);
     *  _ := invalidate(t_regex);
     *  _ := stack_pop("group_var");
     *  goto <follow_idx>
     *
     * <pop_next_idx>:
     *  _ := invalidate(t_pattern);
     *  _ := invalidate(t_regex);
     *  _ := stack_pop("group_var");
     *  goto <next_idx>
     *
     * <pop_ret_idx>:
     *  _ := invalidate(t_pattern);
     *  _ := invalidate(t_regex);
     *  _ := stack_pop("group_var");
     *  goto <ret_idx>
     *
     * // if break_idx.is_some()
     * <pop_break_idx>:
     *  _ := invalidate(t_pattern);
     *  _ := invalidate(t_regex);
     *  _ := stack_pop("group_var");
     *  goto <break_idx>
     */
    let t_pattern = tmp_var_new(proc);
    let mut current_idx = proc.blocks.add_node(Vec::new());

    let pattern_owned = block_expr_push(
        &r.pattern,
        &mut current_idx,
        IRTarget::Variable(t_pattern),
        proc,
        shared_proc,
        cfg,
    );

    let t_regex = tmp_var_new(proc);
    block_get(proc, current_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_regex),
        types: IRType::NATIVE_REGEX,
        source: IRValue::BuiltinProc(BuiltinProc::RegexCompile),
        op: IROp::NativeCall(vec![
            IRValue::Variable(t_pattern),
            IRValue::Number(if is_multiline { 3.into() } else { 1.into() }),
        ]),
    }));

    fn invalidate_block_new(
        t_pattern: IRVar,
        pattern_owned: bool,
        t_regex: IRVar,
        proc: &mut IRProcedure,
        block_idx: NodeIndex,
    ) -> NodeIndex {
        let new_idx = proc.blocks.add_node(Vec::new());

        if pattern_owned {
            block_get(proc, new_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_pattern)]),
            }));
        }

        block_get(proc, new_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_regex)]),
            }),
            IRStmt::Goto(block_idx),
        ]);

        proc.blocks.add_edge(new_idx, block_idx, ());

        new_idx
    }

    let mut pop_follow_idx =
        invalidate_block_new(t_pattern, pattern_owned, t_regex, proc, *follow_idx);
    let mut pop_end_idx = invalidate_block_new(t_pattern, pattern_owned, t_regex, proc, end_idx);
    let mut pop_continue_idx =
        continue_idx.map(|idx| invalidate_block_new(t_pattern, pattern_owned, t_regex, proc, idx));
    let mut pop_ret_idx = invalidate_block_new(t_pattern, pattern_owned, t_regex, proc, ret_idx);
    let mut pop_break_idx =
        break_idx.map(|idx| invalidate_block_new(t_pattern, pattern_owned, t_regex, proc, idx));

    let mut cond_idx = proc.blocks.add_node(Vec::new());
    let mut pop_vars: Vec<String> = Vec::new();

    if let Some(pattern_out) = &r.pattern_out {
        let t_matched = tmp_var_new(proc);
        let t_matched_addr = tmp_var_new(proc);
        let t_assign = tmp_var_new(proc);

        let mut assign_idx = proc.blocks.add_node(Vec::new());

        block_get(proc, current_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_matched),
                types: IRType::BOOL,
                source: IRValue::Bool(false),
                op: IROp::Assign,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_matched_addr),
                types: IRType::PTR,
                source: IRValue::Variable(t_matched),
                op: IROp::PtrAddress,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_assign),
                types: IRType::BOOL,
                source: IRValue::BuiltinProc(BuiltinProc::RegexMatchGroups),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(t_expr),
                    IRValue::Variable(t_regex),
                    IRValue::Variable(t_matched_addr),
                ]),
            }),
            IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_matched),
                success: assign_idx,
                failure: pop_follow_idx,
            }),
        ]);

        proc.blocks.add_edge(current_idx, assign_idx, ());
        proc.blocks.add_edge(current_idx, pop_follow_idx, ());

        if let CSTExpression::Collection(_) = pattern_out {
            expr_vars_push(pattern_out, &mut pop_vars);
            for i in &pop_vars {
                let t_var = tmp_var_new(proc);
                shared_proc.definitions.push((i.to_string(), t_var));
                block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_var),
                    types: IRType::PTR,
                    source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
                    op: IROp::NativeCall(vec![IRValue::String(i.to_string())]),
                }));
            }
        } else if let CSTExpression::Variable(v) = pattern_out {
            let t_var = tmp_var_new(proc);

            shared_proc.definitions.push((v.to_string(), t_var));
            block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_var),
                types: IRType::PTR,
                source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
                op: IROp::NativeCall(vec![IRValue::String(v.to_string())]),
            }));

            pop_vars.push(v.to_string());
        } else {
            panic!("regex groups must be list or variable");
        }

        pop_follow_idx = pop_block_new(&pop_vars, pop_follow_idx, proc);
        pop_end_idx = pop_block_new(&pop_vars, pop_end_idx, proc);
        pop_ret_idx = pop_block_new(&pop_vars, pop_ret_idx, proc);
        pop_continue_idx = pop_continue_idx.map(|b_idx| pop_block_new(&pop_vars, b_idx, proc));
        pop_break_idx = pop_break_idx.map(|b_idx| pop_block_new(&pop_vars, b_idx, proc));

        let t_assign_success = tmp_var_new(proc);

        block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_assign_success),
            types: IRType::BOOL,
            source: IRValue::Bool(false),
            op: IROp::Assign,
        }));

        assign_parse(
            &mut assign_idx,
            t_assign,
            true,
            Some(t_assign_success),
            false,
            pattern_out,
            proc,
            shared_proc,
            cfg,
        );

        block_get(proc, assign_idx).push(IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_assign_success),
            success: cond_idx,
            failure: pop_follow_idx,
        }));
        proc.blocks.add_edge(assign_idx, cond_idx, ());
        proc.blocks.add_edge(assign_idx, pop_follow_idx, ());
    } else {
        let t_matched = tmp_var_new(proc);

        block_get(proc, current_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_matched),
                types: IRType::BOOL,
                source: IRValue::BuiltinProc(BuiltinProc::RegexMatch),
                op: IROp::NativeCall(vec![IRValue::Variable(t_expr), IRValue::Variable(t_regex)]),
            }),
            IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_matched),
                success: cond_idx,
                failure: pop_follow_idx,
            }),
        ]);

        proc.blocks.add_edge(current_idx, cond_idx, ());
        proc.blocks.add_edge(current_idx, pop_follow_idx, ());
    }

    let mut block_idx = proc.blocks.add_node(Vec::new());
    let block_init_idx = block_idx;

    let block_terminated = block_populate(
        &mut block_idx,
        &r.statements,
        pop_continue_idx,
        pop_break_idx,
        pop_ret_idx,
        proc,
        shared_proc,
        cfg,
    );

    if !block_terminated {
        block_get(proc, block_idx).push(IRStmt::Goto(pop_end_idx));
        proc.blocks.add_edge(block_idx, pop_end_idx, ());
    }

    if let Some(cond) = &r.condition {
        let t_cond = tmp_var_new(proc);

        block_expr_push(
            cond,
            &mut cond_idx,
            IRTarget::Variable(t_cond),
            proc,
            shared_proc,
            cfg,
        );
        block_get(proc, cond_idx).push(IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cond),
            success: block_init_idx,
            failure: pop_follow_idx,
        }));
        proc.blocks.add_edge(cond_idx, block_init_idx, ());
        proc.blocks.add_edge(cond_idx, pop_follow_idx, ());
    } else {
        block_get(proc, cond_idx).push(IRStmt::Goto(block_init_idx));
        proc.blocks.add_edge(cond_idx, block_init_idx, ());
    }

    for i in pop_vars.iter().rev() {
        stack_pop(shared_proc, i);
    }

    *follow_idx = current_idx;
}

pub fn block_match_push(
    m: &CSTMatch,
    block_idx: &mut NodeIndex,
    continue_idx: Option<NodeIndex>,
    break_idx: Option<NodeIndex>,
    ret_idx: NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    let t_expr = tmp_var_new(proc);
    let is_owned = block_expr_push(
        &m.expression,
        block_idx,
        IRTarget::Variable(t_expr),
        proc,
        shared_proc,
        cfg,
    );

    let mut follow_idx = proc.blocks.add_node(Vec::new());

    let follow_terminated = block_populate(
        &mut follow_idx,
        &m.default,
        continue_idx,
        break_idx,
        ret_idx,
        proc,
        shared_proc,
        cfg,
    );

    let end_idx = proc.blocks.add_node(Vec::new());

    if is_owned {
        // _ := invalidate(t_expr);
        block_get(proc, end_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
        }));
    }

    if !follow_terminated {
        block_get(proc, follow_idx).push(IRStmt::Goto(end_idx));
        proc.blocks.add_edge(follow_idx, end_idx, ());
    }

    for branch in m.branches.iter().rev() {
        match branch {
            CSTMatchBranch::Case(c) => {
                block_match_case_push(
                    t_expr,
                    c,
                    &mut follow_idx,
                    end_idx,
                    continue_idx,
                    break_idx,
                    ret_idx,
                    proc,
                    shared_proc,
                    cfg,
                );
            }
            CSTMatchBranch::Regex(r) => {
                block_match_regex_push(
                    t_expr,
                    r,
                    false,
                    &mut follow_idx,
                    end_idx,
                    continue_idx,
                    break_idx,
                    ret_idx,
                    proc,
                    shared_proc,
                    cfg,
                );
            }
        }
    }

    block_get(proc, *block_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(*block_idx, follow_idx, ());

    *block_idx = end_idx;
}
