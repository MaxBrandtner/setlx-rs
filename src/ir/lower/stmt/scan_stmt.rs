use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::assign::assign_parse;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::proc::expr_vars_push;
use crate::ir::lower::stmt::block_populate;
use crate::ir::lower::stmt::match_stmt::pop_block_new;
use crate::ir::lower::util::{block_get, stack_pop, tmp_var_new};

fn block_strline_push(
    overwrite_idx: &mut NodeIndex,
    t_expr: IRVar,
    t_chars: IRVar,
    t_line: IRVar,
    t_column: IRVar,
    proc: &mut IRProcedure,
) {
    /* <block_idx>:
     *  t_i := 0;
     *  t_line := 0;
     *  t_column := 0;
     *  goto <cond_idx>
     *
     * <cond_idx>:
     *  t_i_check := t_i < t_chars;
     *  if t_i_check
     *   goto <loop_idx>
     *  else
     *   goto <end_idx>
     *
     * <loop_idx>:
     *  t_c := t_expr[t_i];
     *  t_c_check := t_c == "\n";
     *  if t_c_check
     *   goto <add_line_idx>
     *  else
     *   goto <add_column_idx>
     *
     * <add_line_idx>:
     *  t_line_new := t_line + 1;
     *  _ := invalidate(t_line);
     *  t_line := t_line_new;
     *  _ := invalidate(t_column);
     *  t_column := 0;
     *  t_i_new := t_i + 1;
     *  _ := invalidate(t_i);
     *  t_i := t_i_new;
     *  goto <loop_idx>
     *
     * <add_column_idx>:
     *  t_column_new := t_column + 1;
     *  _ := invalidate(t_column);
     *  t_column := t_column_new;
     *  t_i_new := t_i + 1;
     *  _ := invalidate(t_i);
     *  t_i := t_i_new;
     *  goto <loop_idx>
     *
     * <end_idx>:
     *  _ := invalidate(t_i);
     */

    let t_i = tmp_var_new(proc);
    let t_c = tmp_var_new(proc);
    let t_i_check = tmp_var_new(proc);
    let t_c_check = tmp_var_new(proc);
    let t_i_new = tmp_var_new(proc);
    let t_line_new = tmp_var_new(proc);
    let t_column_new = tmp_var_new(proc);

    let cond_idx = proc.blocks.add_node(Vec::new());
    let loop_idx = proc.blocks.add_node(Vec::new());
    let add_line_idx = proc.blocks.add_node(Vec::new());
    let add_column_idx = proc.blocks.add_node(Vec::new());
    let end_idx = proc.blocks.add_node(vec![IRStmt::Assign(IRAssign {
        target: IRTarget::Ignore,
        types: IRType::UNDEFINED,
        source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
        op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
    })]);

    block_get(proc, *overwrite_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRType::NUMBER,
            source: IRValue::Number(0.into()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_line),
            types: IRType::NUMBER,
            source: IRValue::Number(0.into()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_column),
            types: IRType::NUMBER,
            source: IRValue::Number(0.into()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(cond_idx),
    ]);

    proc.blocks.add_edge(*overwrite_idx, cond_idx, ());

    block_get(proc, cond_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_i),
            op: IROp::Less(IRValue::Variable(t_chars)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_i_check),
            success: loop_idx,
            failure: end_idx,
        }),
    ]);

    proc.blocks.add_edge(cond_idx, loop_idx, ());
    proc.blocks.add_edge(cond_idx, end_idx, ());

    block_get(proc, loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_c),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_expr),
            op: IROp::AccessArray(IRValue::Variable(t_i)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_c_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_c),
            op: IROp::Equal(IRValue::String("\n".to_string())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_c_check),
            success: add_line_idx,
            failure: add_column_idx,
        }),
    ]);

    proc.blocks.add_edge(loop_idx, add_line_idx, ());
    proc.blocks.add_edge(loop_idx, add_column_idx, ());

    block_get(proc, add_line_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_line_new),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_line),
            op: IROp::Plus(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_line)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_line),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_line_new),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_column)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_column),
            types: IRType::NUMBER,
            source: IRValue::Number(0.into()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_new),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_i),
            op: IROp::Plus(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_i_new),
            op: IROp::Assign,
        }),
        IRStmt::Goto(cond_idx),
    ]);

    proc.blocks.add_edge(add_line_idx, cond_idx, ());

    block_get(proc, add_column_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_column_new),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_column),
            op: IROp::Plus(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_column)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_column),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_column_new),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_new),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_i),
            op: IROp::Plus(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_i_new),
            op: IROp::Assign,
        }),
        IRStmt::Goto(cond_idx),
    ]);

    proc.blocks.add_edge(add_column_idx, cond_idx, ());

    *overwrite_idx = end_idx;
}

pub fn block_scan_push(
    s: &CSTScan,
    block_idx: &mut NodeIndex,
    continue_idx: Option<NodeIndex>,
    break_idx: Option<NodeIndex>,
    ret_idx: NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /*  t_expr := // expr
     *  t_slice := slice(t_expr, 0, -1);
     *  t_regex_1 := regex_compile(s.branches[0].pattern, 0x02);
     *  t_var := stack_frame_add(s.variable);
     *  goto <check_idx>
     *
     * <check_idx>:
     *  t_len := amount(t_slice);
     *  t_len_check := t_len == 0;
     *  if t_len_check
     *   goto <loop_idx>
     *  else
     *   goto <end_idx>
     *
     * <loop_idx>:
     *  t_matched_branch := -1;
     *  t_matched_len := -1;
     *  t_matched_assign := list_new();
     *  t_len := -1;
     *  t_len_addr := &t_len;
     *
     *  t_chars := 0;
     *  t_chars_addr := &t_chars;
     *  t_matched := false;
     *  t_matched_addr := &t_matched;
     *  t_assign := regex_match_groups_len(t_slice, t_regex_1, t_matched_addr, t_len_addr, t_chars_addr);
     *  t_check_chars_zero := t_chars == 0;
     *  t_matched := t_matched && t_check_chars_zero;
     *  t_check_len_less := t_len < t_matched_len;
     *  t_check_len_geq := !t_check_len_less;
     *  t_check_len_eq := t_len == t_matched_len;
     *  t_check_len_neq := !t_check_len_eq;
     *  t_check_len := t_check_len_geq && t_check_len_neq;
     *  t_check := t_matched && t_check_len;
     *  t_check_assign := false;
     *  // list vars push
     *  // assign_parse
     *  // stack pop
     *  t_check := t_check && t_check_assign;
     *  if t_check
     *   goto <cond_idx>
     *  else
     *   goto <pop_follow_idx>
     *
     * <cond_idx>:
     *  t_cond := // cond
     *  if t_cond
     *   goto <overwrite_idx>
     *  else
     *   goto <pop_follow_idx>
     *
     * <overwrite_idx>:
     *  t_set := set_new();
     *  t_line, t_column := // block strline det push
     *  t_chars := t_chars + 1;
     *  t_list := list_new();
     *  _ := list_push(t_list, "chars");
     *  _ := list_push(t_list, t_chars);
     *  _ := set_insert(t_list);
     *  // same for t_line and t_column
     *  *var := t_set;
     *  t_matched_branch := 0;
     *  t_matched_len := copy(t_len);
     *  t_matched_assign := copy(t_assign);
     *  goto <pop_follow_idx>;
     *
     * <pop_follow_idx>:
     *  _ := invalidate(t_assign);
     *  _ := invalidate(t_len);
     *  // pop vars
     *  goto <follow_idx>
     *
     * <follow_idx>:
     *  t_check_matched_branch_eq := t_matched_branch == 0;
     *  if t_check_matched_branch_eq
     *   goto <branch_idx>
     *  else
     *   goto <end_loop_idx>:
     *
     * <branch_idx>:
     *  t_slice := slice(t_slice, t_len, -1);
     *  // list vars push
     *  // assign parse
     *  // block populate
     *  // stack pop
     *  goto <check_idx>
     *
     * <break_pop_idx>:
     *  // stack pop
     *  _ := invalidate(t_expr);
     *  _ := invalidate(t_regex_1);
     *  _ := invalidate(t_matched_assign);
     *  _ := invalidate(t_matched_len);
     *  _ := invalidate(t_matched_branch);
     *  _ := stack_frame_pop("var");
     *  goto <break_idx>
     *
     * <continue_pop_idx>:
     * // stack pop
     *  _ := invalidate(t_expr);
     *  _ := invalidate(t_regex_1);
     *  _ := invalidate(t_matched_assign);
     *  _ := invalidate(t_matched_len);
     *  _ := invalidate(t_matched_branch);
     *  _ := stack_frame_pop("var");
     *  goto <continue_idx>
     *
     * <end_loop_idx>:
     *  t_matched_branch_none := t_matched_branch == -1;
     *  if t_matched_branch_none:
     *   goto <end_idx>
     *  else
     *   goto <check_idx>
     *
     * <end_idx>:
     *  _ := invalidate(t_expr);
     *  _ := invalidate(t_regex_1);
     *  _ := invalidate(t_matched_assign);
     *  _ := invalidate(t_matched_len);
     *  _ := invalidate(t_matched_branch);
     *  _ := stack_frame_pop("var");
     */

    let t_expr = tmp_var_new(proc);
    let t_slice = tmp_var_new(proc);

    let expr_owned = block_expr_push(
        &s.expression,
        block_idx,
        IRTarget::Variable(t_expr),
        proc,
        shared_proc,
        cfg,
    );

    let t_expr_len = tmp_var_new(proc);
    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_expr_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_slice),
            types: IRType::STRING | IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_expr),
                IRValue::Number(0.into()),
                IRValue::Variable(t_expr_len),
            ]),
        }),
    ]);

    let regex_vars = std::iter::repeat_n((), s.branches.len())
        .map(|_| tmp_var_new(proc))
        .collect::<Vec<IRVar>>();

    regex_vars.iter().enumerate().for_each(|(idx, i)| {
        let branch = if let CSTMatchBranch::Regex(r) = &s.branches[idx] {
            r
        } else {
            panic!("scan statement must only contain regex branches")
        };

        let t_pattern = tmp_var_new(proc);
        let pattern_owned = block_expr_push(
            &branch.pattern,
            block_idx,
            IRTarget::Variable(t_pattern),
            proc,
            shared_proc,
            cfg,
        );

        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(*i),
            types: IRType::NATIVE_REGEX,
            source: IRValue::BuiltinProc(BuiltinProc::RegexCompile),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_pattern),
                IRValue::Number(2.into()),
            ]),
        }));

        if pattern_owned {
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_pattern)]),
            }));
        }
    });

    let t_var = if let Some(v) = &s.variable {
        let t_var = tmp_var_new(proc);
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_var),
            types: IRType::PTR,
            source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
            op: IROp::NativeCall(vec![IRValue::String(v.to_string())]),
        }));
        shared_proc.definitions.push((v.to_string(), t_var));
        t_var
    } else {
        0
    };

    let end_idx = proc.blocks.add_node(if let Some(v) = &s.variable {
        vec![IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::StackPop),
            op: IROp::NativeCall(vec![IRValue::String(v.to_string())]),
        })]
    } else {
        Vec::new()
    });

    if expr_owned {
        block_get(proc, end_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
        }));
    }

    regex_vars.iter().for_each(|i| {
        block_get(proc, end_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(*i)]),
        }))
    });

    let loop_idx = proc.blocks.add_node(Vec::new());

    let t_len = tmp_var_new(proc);
    let t_len_check = tmp_var_new(proc);

    let check_idx = proc.blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_slice)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_len),
            op: IROp::Equal(IRValue::Number(0.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_len_check),
            success: end_idx,
            failure: loop_idx,
        }),
    ]);
    proc.blocks.add_edge(check_idx, end_idx, ());
    proc.blocks.add_edge(check_idx, loop_idx, ());

    block_get(proc, *block_idx).push(IRStmt::Goto(check_idx));
    proc.blocks.add_edge(*block_idx, check_idx, ());

    let t_matched_branch = tmp_var_new(proc);
    let t_matched_len = tmp_var_new(proc);
    let t_matched_assign = tmp_var_new(proc);

    block_get(proc, end_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_matched_branch)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_matched_len)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_matched_assign)]),
        }),
    ]);

    block_get(proc, loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_matched_branch),
            types: IRType::NUMBER,
            source: IRValue::Number((-1).into()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_matched_len),
            types: IRType::NUMBER,
            source: IRValue::Number((-1).into()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_matched_assign),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(Vec::new()),
        }),
    ]);

    let mut current_idx = loop_idx;
    let mut follow_idx = proc.blocks.add_node(Vec::new());
    for (idx, i) in s.branches.iter().enumerate() {
        let branch = if let CSTMatchBranch::Regex(r) = i {
            r
        } else {
            panic!("all branches in scan statement must be regex branches")
        };

        let t_len = tmp_var_new(proc);
        let t_assign = if branch.pattern_out.is_some() {
            tmp_var_new(proc)
        } else {
            0
        };

        let t_set = tmp_var_new(proc);
        let t_chars = tmp_var_new(proc);
        let t_line = tmp_var_new(proc);
        let t_column = tmp_var_new(proc);

        let overwrite_init_idx = proc.blocks.add_node(Vec::new());
        let mut overwrite_idx = overwrite_init_idx;
        block_strline_push(&mut overwrite_idx, t_expr, t_chars, t_line, t_column, proc);

        let t_list = tmp_var_new(proc);

        if s.variable.is_some() {
            block_get(proc, overwrite_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_set),
                    types: IRType::SET,
                    source: IRValue::BuiltinProc(BuiltinProc::SetNew),
                    op: IROp::NativeCall(Vec::new()),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_chars),
                    types: IRType::NUMBER,
                    source: IRValue::Variable(t_chars),
                    op: IROp::Plus(IRValue::Number(1.into())),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_line),
                    types: IRType::NUMBER,
                    source: IRValue::Variable(t_line),
                    op: IROp::Plus(IRValue::Number(1.into())),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_column),
                    types: IRType::NUMBER,
                    source: IRValue::Variable(t_column),
                    op: IROp::Plus(IRValue::Number(1.into())),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_list),
                    types: IRType::LIST,
                    source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                    op: IROp::NativeCall(Vec::new()),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(t_list),
                        IRValue::String("char".to_string()),
                    ]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(t_list),
                        IRValue::Variable(t_chars),
                    ]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::SetInsert),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_set), IRValue::Variable(t_list)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_list),
                    types: IRType::LIST,
                    source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                    op: IROp::NativeCall(Vec::new()),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(t_list),
                        IRValue::String("line".to_string()),
                    ]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(t_list),
                        IRValue::Variable(t_line),
                    ]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::SetInsert),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_set), IRValue::Variable(t_list)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_list),
                    types: IRType::LIST,
                    source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                    op: IROp::NativeCall(Vec::new()),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(t_list),
                        IRValue::String("column".to_string()),
                    ]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(t_list),
                        IRValue::Variable(t_column),
                    ]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::SetInsert),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_set), IRValue::Variable(t_list)]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Deref(t_var),
                    types: IRType::SET,
                    source: IRValue::Variable(t_set),
                    op: IROp::Assign,
                }),
            ]);
        }

        block_get(proc, overwrite_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_matched_branch),
                types: IRType::NUMBER,
                source: IRValue::Number(idx.into()),
                op: IROp::Assign,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_matched_len),
                types: IRType::NUMBER,
                source: IRValue::BuiltinProc(BuiltinProc::Copy),
                op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
            }),
            if branch.pattern_out.is_some() {
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_matched_assign),
                    types: IRType::LIST,
                    source: IRValue::BuiltinProc(BuiltinProc::Copy),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_assign)]),
                })
            } else {
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_matched_assign),
                    types: IRType::LIST,
                    source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                    op: IROp::NativeCall(Vec::new()),
                })
            },
            IRStmt::Goto(follow_idx),
        ]);
        proc.blocks.add_edge(overwrite_idx, follow_idx, ());

        let t_matched = tmp_var_new(proc);
        let t_matched_addr = if branch.pattern_out.is_some() {
            tmp_var_new(proc)
        } else {
            0
        };

        let t_len_addr = tmp_var_new(proc);
        let t_chars_addr = tmp_var_new(proc);

        block_get(proc, current_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_len),
                types: IRType::NUMBER,
                source: IRValue::Number((-1).into()),
                op: IROp::Assign,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_len_addr),
                types: IRType::PTR,
                source: IRValue::Variable(t_len),
                op: IROp::PtrAddress,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_chars),
                types: IRType::NUMBER,
                source: IRValue::Number((0).into()),
                op: IROp::Assign,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_chars_addr),
                types: IRType::PTR,
                source: IRValue::Variable(t_chars),
                op: IROp::PtrAddress,
            }),
        ]);

        if branch.pattern_out.is_some() {
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
            ]);
        }

        block_get(proc, current_idx).push(if branch.pattern_out.is_some() {
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_assign),
                types: IRType::LIST,
                source: IRValue::BuiltinProc(BuiltinProc::RegexMatchGroupsLen),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(t_slice),
                    IRValue::Variable(regex_vars[idx]),
                    IRValue::Variable(t_matched_addr),
                    IRValue::Variable(t_len_addr),
                    IRValue::Variable(t_chars_addr),
                ]),
            })
        } else {
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_matched),
                types: IRType::BOOL,
                source: IRValue::BuiltinProc(BuiltinProc::RegexMatchLen),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(t_slice),
                    IRValue::Variable(regex_vars[idx]),
                    IRValue::Variable(t_len_addr),
                    IRValue::Variable(t_chars_addr),
                ]),
            })
        });

        let t_check_chars_zero = tmp_var_new(proc);
        let t_check_len_less = tmp_var_new(proc);
        let t_check_len_geq = tmp_var_new(proc);
        let t_check_len_eq = tmp_var_new(proc);
        let t_check_len_neq = tmp_var_new(proc);
        let t_check_len = tmp_var_new(proc);
        let t_check = tmp_var_new(proc);

        block_get(proc, current_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_check_chars_zero),
                types: IRType::BOOL,
                source: IRValue::Variable(t_chars),
                op: IROp::Equal(IRValue::Number(0.into())),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_matched),
                types: IRType::BOOL,
                source: IRValue::Variable(t_matched),
                op: IROp::And(IRValue::Variable(t_check_chars_zero)),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_check_len_less),
                types: IRType::BOOL,
                source: IRValue::Variable(t_len),
                op: IROp::Less(IRValue::Variable(t_matched_len)),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_check_len_geq),
                types: IRType::BOOL,
                source: IRValue::Variable(t_check_len_less),
                op: IROp::Not,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_check_len_eq),
                types: IRType::BOOL,
                source: IRValue::Variable(t_len),
                op: IROp::Equal(IRValue::Variable(t_matched_len)),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_check_len_neq),
                types: IRType::BOOL,
                source: IRValue::Variable(t_check_len_eq),
                op: IROp::Not,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_check_len),
                types: IRType::BOOL,
                source: IRValue::Variable(t_check_len_geq),
                op: IROp::And(IRValue::Variable(t_check_len_neq)),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_check),
                types: IRType::BOOL,
                source: IRValue::Variable(t_matched),
                op: IROp::And(IRValue::Variable(t_check_len)),
            }),
        ]);

        let mut pop_vars: Vec<(String, IRVar)> = Vec::new();

        if let Some(pattern_out) = &branch.pattern_out {
            let t_check_assign = tmp_var_new(proc);

            let mut pop_vars_str: Vec<String> = Vec::new();

            expr_vars_push(pattern_out, &mut pop_vars_str);
            pop_vars_str.iter().for_each(|i| {
                let t_i = tmp_var_new(proc);
                block_get(proc, current_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_i),
                    types: IRType::PTR,
                    source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
                    op: IROp::NativeCall(vec![IRValue::String(i.to_string())]),
                }));
                pop_vars.push((i.to_string(), t_i));
                shared_proc.definitions.push((i.to_string(), t_i));
            });

            block_get(proc, current_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_check_assign),
                types: IRType::BOOL,
                source: IRValue::Bool(true),
                op: IROp::Assign,
            }));

            assign_parse(
                &mut current_idx,
                t_assign,
                false,
                Some(t_check_assign),
                pattern_out,
                proc,
                shared_proc,
                cfg,
            );

            block_get(proc, current_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_check),
                types: IRType::BOOL,
                source: IRValue::Variable(t_check),
                op: IROp::And(IRValue::Variable(t_check_assign)),
            }));
        }

        block_get(proc, follow_idx).extend(
            pop_vars
                .iter()
                .map(|(i, _)| {
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Ignore,
                        types: IRType::UNDEFINED,
                        source: IRValue::BuiltinProc(BuiltinProc::StackPop),
                        op: IROp::NativeCall(vec![IRValue::String(i.to_string())]),
                    })
                })
                .collect::<Vec<IRStmt>>(),
        );

        let cond_idx = if let Some(cond) = &branch.condition {
            let mut idx = proc.blocks.add_node(Vec::new());

            let t_cond = tmp_var_new(proc);
            block_expr_push(
                cond,
                &mut idx,
                IRTarget::Variable(t_cond),
                proc,
                shared_proc,
                cfg,
            );

            pop_vars
                .iter()
                .rev()
                .for_each(|(v, _)| stack_pop(shared_proc, v));

            block_get(proc, idx).push(IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_cond),
                success: overwrite_init_idx,
                failure: follow_idx,
            }));
            proc.blocks.add_edge(idx, overwrite_init_idx, ());
            proc.blocks.add_edge(idx, follow_idx, ());

            idx
        } else {
            overwrite_init_idx
        };

        block_get(proc, follow_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_assign)]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
            }),
        ]);

        block_get(proc, current_idx).push(IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_check),
            success: cond_idx,
            failure: follow_idx,
        }));
        proc.blocks.add_edge(current_idx, cond_idx, ());
        proc.blocks.add_edge(current_idx, follow_idx, ());

        current_idx = follow_idx;
        follow_idx = proc.blocks.add_node(Vec::new());
    }

    let t_check_matched_branch_neg = tmp_var_new(proc);
    let new_current_idx = proc.blocks.add_node(Vec::new());

    let end_stmts = block_get(proc, end_idx).clone();
    let panic_idx = proc.blocks.add_node(end_stmts);
    block_get(proc, panic_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Throw),
            op: IROp::NativeCall(vec![
                IRValue::Number(1.into()),
                IRValue::String(String::from("Infinite loop in scan-statement detected.")),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    block_get(proc, current_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_check_matched_branch_neg),
            types: IRType::BOOL,
            source: IRValue::Variable(t_matched_branch),
            op: IROp::Less(IRValue::Number(0.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_check_matched_branch_neg),
            success: panic_idx,
            failure: new_current_idx,
        }),
    ]);

    proc.blocks.add_edge(current_idx, new_current_idx, ());
    proc.blocks.add_edge(current_idx, panic_idx, ());
    current_idx = new_current_idx;

    for (idx, i) in s.branches.iter().enumerate() {
        let branch = if let CSTMatchBranch::Regex(r) = i {
            r
        } else {
            panic!("all branches in scan statement must be regex branches")
        };

        let t_check_matched_branch_eq = tmp_var_new(proc);

        let t_slice_len = tmp_var_new(proc);
        let mut branch_idx = proc.blocks.add_node(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_slice_len),
                types: IRType::NUMBER,
                source: IRValue::BuiltinProc(BuiltinProc::Amount),
                op: IROp::NativeCall(vec![IRValue::Variable(t_slice)]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_slice),
                types: IRType::STRING | IRType::LIST,
                source: IRValue::BuiltinProc(BuiltinProc::Slice),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(t_slice),
                    IRValue::Variable(t_matched_len),
                    IRValue::Variable(t_slice_len),
                ]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_slice_len)]),
            }),
        ]);

        block_get(proc, current_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_check_matched_branch_eq),
                types: IRType::BOOL,
                source: IRValue::Variable(t_matched_branch),
                op: IROp::Equal(IRValue::Number(idx.into())),
            }),
            IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_check_matched_branch_eq),
                success: branch_idx,
                failure: follow_idx,
            }),
        ]);
        proc.blocks.add_edge(current_idx, branch_idx, ());
        proc.blocks.add_edge(current_idx, follow_idx, ());

        let mut pop_vars: Vec<String> = Vec::new();

        if let Some(pattern_out) = &branch.pattern_out {
            expr_vars_push(pattern_out, &mut pop_vars);
            pop_vars.iter().for_each(|i| {
                let t_i = tmp_var_new(proc);
                block_get(proc, branch_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_i),
                    types: IRType::PTR,
                    source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
                    op: IROp::NativeCall(vec![IRValue::String(i.to_string())]),
                }));
                shared_proc.definitions.push((i.to_string(), t_i));
            });

            assign_parse(
                &mut branch_idx,
                t_matched_assign,
                false,
                None,
                pattern_out,
                proc,
                shared_proc,
                cfg,
            );
        }

        let pop_check_idx = pop_block_new(&pop_vars, check_idx, proc);
        let pop_continue_idx = continue_idx.map(|b_idx| pop_block_new(&pop_vars, b_idx, proc));
        let pop_break_idx = break_idx.map(|b_idx| pop_block_new(&pop_vars, b_idx, proc));
        let pop_ret_idx = pop_block_new(&pop_vars, ret_idx, proc);

        let block_stmt_idx = proc.blocks.add_node(Vec::new());
        let mut block_stmt_changed_idx = block_stmt_idx;

        block_populate(
            &mut block_stmt_changed_idx,
            &branch.statements,
            pop_continue_idx,
            pop_break_idx,
            pop_ret_idx,
            proc,
            shared_proc,
            cfg,
        );
        block_get(proc, block_stmt_changed_idx).push(IRStmt::Goto(pop_check_idx));
        proc.blocks
            .add_edge(block_stmt_changed_idx, pop_check_idx, ());

        pop_vars
            .iter()
            .rev()
            .for_each(|v| stack_pop(shared_proc, v));

        block_get(proc, branch_idx).push(IRStmt::Goto(block_stmt_idx));
        proc.blocks.add_edge(branch_idx, block_stmt_idx, ());

        current_idx = follow_idx;
        follow_idx = proc.blocks.add_node(Vec::new());
    }

    block_get(proc, current_idx).push(IRStmt::Goto(end_idx));
    proc.blocks.add_edge(current_idx, end_idx, ());

    *block_idx = end_idx;
}
