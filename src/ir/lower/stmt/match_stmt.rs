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
    e: &CSTExpression,
    pop_follow_idx: &mut NodeIndex,
    pop_end_idx: &mut NodeIndex,
    pop_continue_idx: &mut Option<NodeIndex>,
    pop_break_idx: &mut Option<NodeIndex>,
    pop_ret_idx: &mut NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    if !shared_proc.disable_annotations {
        block_get(proc, *current_idx).push(IRStmt::Annotate(e.lhs, e.rhs));
    }
    shared_proc.code_lhs = e.lhs;
    shared_proc.code_rhs = e.rhs;

    match &e.kind {
        CSTExpressionKind::Literal(_)
        | CSTExpressionKind::Bool(_)
        | CSTExpressionKind::Om
        | CSTExpressionKind::Number(_) => {
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
        CSTExpressionKind::Collection(_) => {
            /* // list_vars_push
             * // assign parse
             */
            let mut pop_vars_raw = Vec::new();
            expr_vars_push(e, &mut pop_vars_raw);
            let pop_vars_new = pop_vars_raw
                .iter()
                .filter(|i| !pop_vars.contains(i))
                .cloned()
                .collect::<Vec<_>>();

            pop_vars_new.iter().for_each(|i| {
                let t_var = tmp_var_new(proc);

                block_get(proc, *current_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_var),
                    types: IRType::PTR,
                    source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
                    op: IROp::NativeCall(vec![IRValue::String(i.to_string())]),
                }));

                shared_proc.definitions.push((i.to_string(), t_var));
            });

            pop_vars.extend(pop_vars_new);

            assign_parse(
                current_idx,
                t_expr,
                false,
                Some(t_matched),
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
        CSTExpressionKind::Variable(_)
        | CSTExpressionKind::Term(_)
        | CSTExpressionKind::Call(_)
        | CSTExpressionKind::Op(_)
        | CSTExpressionKind::UnaryOp(_) => {
            /* // expr_vars_push
             * // assign parse
             */
            let mut pop_vars_raw = Vec::new();
            expr_vars_push(e, &mut pop_vars_raw);
            if let CSTExpressionKind::Call(c) = &e.kind {
                pop_vars_raw.push(c.name.to_string());
                c.params
                    .iter()
                    .for_each(|i| expr_vars_push(i, &mut pop_vars_raw));
            }

            let pop_vars_new = pop_vars_raw
                .iter()
                .filter(|i| !pop_vars.contains(i))
                .cloned()
                .collect::<Vec<_>>();

            pop_vars_new
                .iter()
                .map(|i| {
                    (
                        i,
                        block_stack_copied_add_push(current_idx, i, proc, shared_proc),
                    )
                })
                .collect::<Vec<_>>()
                .iter()
                .for_each(|(i, t_var)| shared_proc.definitions.push((i.to_string(), *t_var)));

            *pop_follow_idx = pop_block_new(&pop_vars_new, *pop_follow_idx, proc);
            *pop_end_idx = pop_block_new(&pop_vars_new, *pop_end_idx, proc);
            *pop_ret_idx = pop_block_new(&pop_vars_new, *pop_ret_idx, proc);
            if let Some(b_idx) = *pop_continue_idx {
                *pop_continue_idx = Some(pop_block_new(&pop_vars_new, b_idx, proc));
            }
            if let Some(b_idx) = *pop_break_idx {
                *pop_break_idx = Some(pop_block_new(&pop_vars_new, b_idx, proc));
            }

            pop_vars.extend(pop_vars_new);

            assign_parse(
                current_idx,
                t_expr,
                false,
                Some(t_matched),
                e,
                proc,
                shared_proc,
                cfg,
            );
        }
        _ => {
            /*  t_e_cst := // convert e to cst
             *  t_expr_type := type_of(t_expr);
             *  t_expr_ast := t_expr_type == TYPE_AST;
             *  if t_expr_ast
             *   goto <match_idx>
             *  else
             *   goto <fail_idx>
             *
             * <match_idx>:
             *  t_e_tag := t_e_cst[0];
             *  t_expr_tag := t_expr[0];
             *  t_matched := t_e_tag == t_expr_tag;
             *  goto <follow_idx>
             *
             * <fail_idx>:
             *  t_matched := false;
             *  goto <follow_idx>
             *
             * <follow_idx>:
             *  _ := invalidate(t_e_cst);
             */
            let match_idx = proc.blocks.add_node(Vec::new());
            let fail_idx = proc.blocks.add_node(Vec::new());
            let follow_idx = proc.blocks.add_node(Vec::new());

            let t_e_cst = tmp_var_new(proc);
            block_cst_expr_push(e, *current_idx, IRTarget::Variable(t_e_cst), proc);

            let t_expr_type = tmp_var_new(proc);
            let t_expr_ast = tmp_var_new(proc);

            block_get(proc, *current_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_expr_type),
                    types: IRType::TYPE,
                    source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_expr_ast),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_expr_type),
                    op: IROp::Equal(IRValue::Type(IRType::AST)),
                }),
                IRStmt::Branch(IRBranch {
                    cond: IRValue::Variable(t_expr_ast),
                    success: match_idx,
                    failure: fail_idx,
                }),
            ]);

            proc.blocks.add_edge(*current_idx, match_idx, ());
            proc.blocks.add_edge(*current_idx, fail_idx, ());

            let t_e_tag = tmp_var_new(proc);
            let t_expr_tag = tmp_var_new(proc);

            block_get(proc, match_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_e_tag),
                    types: IRType::STRING,
                    source: IRValue::Variable(t_e_cst),
                    op: IROp::AccessArray(IRValue::Number(0.into())),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_expr_tag),
                    types: IRType::STRING,
                    source: IRValue::Variable(t_expr),
                    op: IROp::AccessArray(IRValue::Number(0.into())),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_matched),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_e_tag),
                    op: IROp::Equal(IRValue::Variable(t_expr_tag)),
                }),
                IRStmt::Goto(follow_idx),
            ]);

            proc.blocks.add_edge(match_idx, follow_idx, ());

            block_get(proc, fail_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_matched),
                    types: IRType::BOOL,
                    source: IRValue::Bool(false),
                    op: IROp::Assign,
                }),
                IRStmt::Goto(follow_idx),
            ]);

            proc.blocks.add_edge(fail_idx, follow_idx, ());

            *current_idx = follow_idx;
        }
    }
}

fn block_stack_copied_add_push(
    block_idx: &mut NodeIndex,
    name: &str,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
) -> IRVar {
    /* t_exists := stack_in_scope(name);
     * if t_exists
     *  goto <assign_idx>
     * else
     *  goto <tmp_om_idx>
     *
     * <assign_idx>:
     *  t_tmp := // block_var_push
     *  goto <follow_idx>
     *
     * <tmp_om_idx>:
     *  t_tmp := om;
     *  goto <follow_idx>
     *
     * <follow_idx>:
     *  t_ptr := stack_add(name);
     *  *t_ptr := copy(t_tmp);
     */
    let t_exists = tmp_var_new(proc);
    let t_tmp = tmp_var_new(proc);
    let t_ptr = tmp_var_new(proc);

    let mut assign_idx = proc.blocks.add_node(Vec::new());
    let tmp_om_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_exists),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::StackInScope),
            op: IROp::NativeCall(vec![IRValue::String(name.to_string())]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_exists),
            success: assign_idx,
            failure: tmp_om_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, assign_idx, ());
    proc.blocks.add_edge(*block_idx, tmp_om_idx, ());

    block_var_push(
        name,
        &mut assign_idx,
        IRTarget::Variable(t_tmp),
        proc,
        shared_proc,
    );
    block_get(proc, assign_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(assign_idx, follow_idx, ());

    block_get(proc, tmp_om_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_tmp),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Goto(follow_idx),
    ]);
    proc.blocks.add_edge(tmp_om_idx, follow_idx, ());

    block_get(proc, follow_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ptr),
            types: IRType::PTR,
            source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
            op: IROp::NativeCall(vec![IRValue::String(name.to_string())]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Deref(t_ptr),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_tmp)]),
        }),
    ]);

    *block_idx = follow_idx;
    t_ptr
}

fn block_match_case_matches_push(
    block_idx: &mut NodeIndex,
    t_expr: IRVar,
    c: &CSTMatchBranchCase,
    pop_vars: &mut Vec<String>,
    pop_follow_idx: &mut NodeIndex,
    pop_end_idx: &mut NodeIndex,
    pop_continue_idx: &mut Option<NodeIndex>,
    pop_break_idx: &mut Option<NodeIndex>,
    pop_ret_idx: &mut NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) -> IRVar {
    /* t_matched := false;
     * // current_idx = *block_idx
     * // for i in c.expressions {
     *  <current_idx>:
     *  // case-dependent
     *  if t_matched
     *   goto <follow_idx>
     *  else
     *   goto <next_idx> // current_idx = next_idx
     * // }
     * <follow_idx>:
     */
    let t_matched = tmp_var_new(proc);
    let mut current_idx = *block_idx;
    let follow_idx = proc.blocks.add_node(Vec::new());

    for (idx, i) in c.expressions.iter().enumerate() {
        block_case_equiv_push(
            t_expr,
            t_matched,
            &mut current_idx,
            pop_vars,
            i,
            pop_follow_idx,
            pop_end_idx,
            pop_continue_idx,
            pop_break_idx,
            pop_ret_idx,
            proc,
            shared_proc,
            cfg,
        );

        if idx != c.expressions.len() - 1 {
            let next_idx = proc.blocks.add_node(Vec::new());
            block_get(proc, current_idx).push(IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_matched),
                success: follow_idx,
                failure: next_idx,
            }));
            proc.blocks.add_edge(current_idx, follow_idx, ());
            proc.blocks.add_edge(current_idx, next_idx, ());
            current_idx = next_idx;
        } else {
            block_get(proc, current_idx).push(IRStmt::Goto(follow_idx));
            proc.blocks.add_edge(current_idx, follow_idx, ());
        }
    }

    *block_idx = follow_idx;
    t_matched
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
     *  t_matched := // block_match_case_matches_push
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

    let mut current_idx = proc.blocks.add_node(Vec::new());
    let end_idx = current_idx;

    let t_matched = block_match_case_matches_push(
        &mut current_idx,
        t_expr,
        c,
        &mut pop_vars,
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

        if let CSTExpressionKind::Collection(_) = &pattern_out.kind {
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
        } else if let CSTExpressionKind::Variable(v) = &pattern_out.kind {
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

    let mut follow_end_idx = proc.blocks.add_node(Vec::new());
    let mut follow_idx = follow_end_idx;

    let follow_terminated = block_populate(
        &mut follow_end_idx,
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
        block_get(proc, follow_end_idx).push(IRStmt::Goto(end_idx));
        proc.blocks.add_edge(follow_end_idx, end_idx, ());
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
