use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::access_expr::block_arr_access_push;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::expr::term_expr::tterm_ast_tag_get;
use crate::ir::lower::util::{block_get, stack_get, tmp_var_new};

fn assign_arr_access(
    block_idx: &mut NodeIndex,
    t_n: IRVar,
    e: &CSTExpression,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) -> IRVar {
    /* t_expr := // s.expressions[0]
     * t_n_val := *t_n;
     * t_n_val_type := type_of(t_n_val);
     * t_n_val_set := t_n_val_type == TYPE_SET;
     * if t_n_val_set
     *  goto <set_assign_idx>
     * else
     *  goto <arr_idx>
     *
     * <set_assign_idx>:
     *  t_o := set_get_tag(t_n_val, t_expr);
     *  t_o_om := t_o == om;
     *  t_expr_om := t_expr == om;
     *  t_expr_nom := !t_expr_type_om;
     *  t_o_om := t_o_om && t_expr_type_nom;
     *  if t_o_om
     *   goto <set_insert_idx>
     *  else
     *   goto <inval_idx>
     *
     * <set_insert_idx>:
     *  t_insert_list := list_new();
     *  // if expr_owned {
     *  t_expr_copy := t_expr;
     *  // } else {
     *  t_expr_copy := copy(t_expr);
     *  // }
     *  _ := list_push(t_insert_list, t_expr_copy);
     *  _ := list_push(t_insert_list, om);
     *  // NOTE: set_insert returns undefined if the inserted entry is
     *  // undefined. This is guaranteed not to be the case here
     *  t_o := set_insert(t_n_val, t_insert_list);
     *  t_o := t_o[1];
     *  goto <follow_idx>
     *
     * <inval_idx>
     *  // if expr_owned {
     *  _ := invalidate(t_expr);
     *  // }
     *  goto <follow_idx>
     *
     * <arr_idx>:
     *  // block_arr_access_push
     *  t_n_amount := amount(t_n_val);
     *  t_expr_fits := t_expr < t_n_amount;
     *  if t_expr_fits
     *   goto <arr_assign_idx>
     *  else
     *   goto <fill_idx>
     *
     * <fill_idx>:
     *  t_n_diff := t_expr - t_n_amount;
     *  t_n_diff := t_n_diff + 1;
     *  t_n_val_list := t_n_val_type == TYPE_LIST;
     *  if t_n_val_list:
     *   goto <list_push_loop_idx>
     *  else
     *   goto <fill_str_check_idx>
     *
     * <list_push_loop_idx>:
     *  t_i := 0;
     *  goto <list_push_cond_idx>
     *
     * <list_push_cond_idx>:
     *  t_lp_cond := t_i < t_n_diff;
     *  if t_lp_cond
     *   goto <list_push_idx>
     *  else
     *   goto <inv_i_idx>
     *
     * <list_push_idx>:
     *  _ := list_push(t_n_val, om);
     *  t_i_new := t_i + 1;
     *  _ := invalidate(t_i);
     *  t_i := t_i_new;
     *  goto <list_push_cond_idx>
     *
     * <inv_i_idx>:
     *  _ := invalidate(t_i);
     *  goto <arr_assign_idx>
     *
     * <fill_str_check_idx>:
     *  t_n_val_str := t_n_val_type == TYPE_STRING;
     *  if t_n_val_str
     *   goto <fill_str_idx>
     *  else
     *   goto <throw_arr_idx>
     *
     * <throw_arr_idx>:
     *  _ := throw(1, "access array not defined for type");
     *  unreachable;
     *
     * <fill_str_idx>:
     *  t_extend := " " * t_n_diff;
     *  *t_n := t_n_val + t_extend; // invalidates t_n_val
     *  _ := invalidate(t_extend);
     *  goto <arr_assign_idx>
     *
     * <arr_assign_idx>
     *  t_o := t_n[t_expr];
     *  // if expr_owned {
     *  _ := invalidate(t_expr);
     *  // }
     *  goto <follow_idx>
     *
     * <follow_idx>:
     */
    let t_expr = tmp_var_new(proc);
    let t_o = tmp_var_new(proc);
    let expr_owned = block_expr_push(
        e,
        block_idx,
        IRTarget::Variable(t_expr),
        proc,
        shared_proc,
        cfg,
    );

    let t_n_val = tmp_var_new(proc);
    let t_n_val_type = tmp_var_new(proc);
    let t_n_val_set = tmp_var_new(proc);
    let t_o_om = tmp_var_new(proc);
    let t_expr_om = tmp_var_new(proc);
    let t_expr_nom = tmp_var_new(proc);
    let t_insert_list = tmp_var_new(proc);
    let t_expr_copy = tmp_var_new(proc);
    let t_n_amount = tmp_var_new(proc);
    let t_expr_fits = tmp_var_new(proc);
    let t_n_diff = tmp_var_new(proc);
    let t_n_val_list = tmp_var_new(proc);
    let t_i = tmp_var_new(proc);
    let t_i_new = tmp_var_new(proc);
    let t_lp_cond = tmp_var_new(proc);
    let t_n_val_str = tmp_var_new(proc);
    let t_extend = tmp_var_new(proc);

    let set_assign_idx = proc.blocks.add_node(Vec::new());
    let set_insert_idx = proc.blocks.add_node(Vec::new());
    let inval_idx = proc.blocks.add_node(Vec::new());
    let mut arr_idx = proc.blocks.add_node(Vec::new());
    let fill_idx = proc.blocks.add_node(Vec::new());
    let list_push_loop_idx = proc.blocks.add_node(Vec::new());
    let list_push_cond_idx = proc.blocks.add_node(Vec::new());
    let list_push_idx = proc.blocks.add_node(Vec::new());
    let inv_i_idx = proc.blocks.add_node(Vec::new());
    let fill_str_check_idx = proc.blocks.add_node(Vec::new());
    let fill_str_idx = proc.blocks.add_node(Vec::new());
    let throw_arr_idx = proc.blocks.add_node(Vec::new());
    let arr_assign_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_n_val),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_n),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_n_val_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_n_val)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_n_val_set),
            types: IRType::BOOL,
            source: IRValue::Variable(t_n_val_type),
            op: IROp::Equal(IRValue::Type(IRType::SET)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_n_val_set),
            success: set_assign_idx,
            failure: arr_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, set_assign_idx, ());
    proc.blocks.add_edge(*block_idx, arr_idx, ());

    block_get(proc, set_assign_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_o),
            types: IRType::PTR | IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::SetGetTag),
            op: IROp::NativeCall(vec![IRValue::Variable(t_n_val), IRValue::Variable(t_expr)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_o_om),
            types: IRType::BOOL,
            source: IRValue::Variable(t_o),
            op: IROp::Equal(IRValue::Undefined),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_expr_om),
            types: IRType::BOOL,
            source: IRValue::Variable(t_expr),
            op: IROp::Equal(IRValue::Undefined),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_expr_nom),
            types: IRType::BOOL,
            source: IRValue::Variable(t_expr_om),
            op: IROp::Not,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_o_om),
            types: IRType::BOOL,
            source: IRValue::Variable(t_o_om),
            op: IROp::And(IRValue::Variable(t_expr_nom)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_o_om),
            success: set_insert_idx,
            failure: inval_idx,
        }),
    ]);

    proc.blocks.add_edge(set_assign_idx, set_insert_idx, ());
    proc.blocks.add_edge(set_assign_idx, inval_idx, ());

    block_get(proc, set_insert_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_insert_list),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(Vec::new()),
        }),
        if expr_owned {
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_expr_copy),
                types: IRTypes!("any"),
                source: IRValue::Variable(t_expr),
                op: IROp::Assign,
            })
        } else {
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_expr_copy),
                types: IRTypes!("any"),
                source: IRValue::BuiltinProc(BuiltinProc::Copy),
                op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
            })
        },
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ListPush),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_insert_list),
                IRValue::Variable(t_expr_copy),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ListPush),
            op: IROp::NativeCall(vec![IRValue::Variable(t_insert_list), IRValue::Undefined]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_o),
            types: IRType::PTR,
            source: IRValue::BuiltinProc(BuiltinProc::SetInsert),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_n_val),
                IRValue::Variable(t_insert_list),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_o),
            types: IRType::PTR,
            source: IRValue::Variable(t_o),
            op: IROp::AccessArray(IRValue::Number(1.into())),
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(set_insert_idx, follow_idx, ());

    if expr_owned {
        block_get(proc, inval_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
            }),
            IRStmt::Goto(follow_idx),
        ])
    } else {
        block_get(proc, inval_idx).push(IRStmt::Goto(follow_idx));
    };

    proc.blocks.add_edge(inval_idx, follow_idx, ());

    // FIXME: memory-leak
    if !expr_owned {
        block_get(proc, arr_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_expr),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
        }));
    }

    block_arr_access_push(&mut arr_idx, t_expr, t_n, false, proc);

    block_get(proc, arr_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_n_amount),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_n_val)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_expr_fits),
            types: IRType::BOOL,
            source: IRValue::Variable(t_expr),
            op: IROp::Less(IRValue::Variable(t_n_amount)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_expr_fits),
            success: arr_assign_idx,
            failure: fill_idx,
        }),
    ]);

    proc.blocks.add_edge(arr_idx, arr_assign_idx, ());
    proc.blocks.add_edge(arr_idx, fill_idx, ());

    block_get(proc, fill_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_n_diff),
            types: IRType::NUMBER | IRType::DOUBLE,
            source: IRValue::Variable(t_expr),
            op: IROp::Minus(IRValue::Variable(t_n_amount)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_n_diff),
            types: IRType::NUMBER | IRType::DOUBLE,
            source: IRValue::Variable(t_n_diff),
            op: IROp::Plus(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_n_val_list),
            types: IRType::BOOL,
            source: IRValue::Variable(t_n_val_type),
            op: IROp::Equal(IRValue::Type(IRType::LIST)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_n_val_list),
            success: list_push_loop_idx,
            failure: fill_str_check_idx,
        }),
    ]);

    proc.blocks.add_edge(fill_idx, list_push_loop_idx, ());
    proc.blocks.add_edge(fill_idx, fill_str_check_idx, ());

    block_get(proc, list_push_loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRType::NUMBER,
            source: IRValue::Number(0.into()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(list_push_cond_idx),
    ]);

    proc.blocks
        .add_edge(list_push_loop_idx, list_push_cond_idx, ());

    block_get(proc, list_push_cond_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_lp_cond),
            types: IRType::BOOL,
            source: IRValue::Variable(t_i),
            op: IROp::Less(IRValue::Variable(t_n_diff)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_lp_cond),
            success: list_push_idx,
            failure: inv_i_idx,
        }),
    ]);

    proc.blocks.add_edge(list_push_cond_idx, list_push_idx, ());
    proc.blocks.add_edge(list_push_cond_idx, inv_i_idx, ());

    block_get(proc, list_push_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ListPush),
            op: IROp::NativeCall(vec![IRValue::Variable(t_n_val), IRValue::Undefined]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_new),
            types: IRType::BOOL,
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
            types: IRType::NUMBER | IRType::DOUBLE,
            source: IRValue::Variable(t_i_new),
            op: IROp::Assign,
        }),
        IRStmt::Goto(list_push_cond_idx),
    ]);

    proc.blocks.add_edge(list_push_idx, list_push_cond_idx, ());

    block_get(proc, inv_i_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Goto(arr_assign_idx),
    ]);

    proc.blocks.add_edge(inv_i_idx, arr_assign_idx, ());

    block_get(proc, fill_str_check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_n_val_str),
            types: IRType::BOOL,
            source: IRValue::Variable(t_n_val_type),
            op: IROp::Equal(IRValue::Type(IRType::STRING)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_n_val_str),
            success: fill_str_idx,
            failure: throw_arr_idx,
        }),
    ]);

    proc.blocks.add_edge(fill_str_check_idx, fill_str_idx, ());
    proc.blocks.add_edge(fill_str_check_idx, throw_arr_idx, ());

    block_get(proc, fill_str_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_extend),
            types: IRType::STRING,
            source: IRValue::String("".to_string()),
            op: IROp::Mult(IRValue::Variable(t_n_diff)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Deref(t_n),
            types: IRType::STRING,
            source: IRValue::Variable(t_n_val),
            op: IROp::Plus(IRValue::Variable(t_extend)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_extend)]),
        }),
        IRStmt::Goto(arr_assign_idx),
    ]);

    proc.blocks.add_edge(fill_str_idx, arr_assign_idx, ());

    block_get(proc, throw_arr_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Throw),
            op: IROp::NativeCall(vec![
                IRValue::Number(1.into()),
                IRValue::String("access array not defined for type".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    block_get(proc, arr_assign_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_o),
        types: IRType::PTR,
        source: IRValue::Variable(t_n),
        op: IROp::AccessArray(IRValue::Variable(t_expr)),
    }));

    if expr_owned {
        block_get(proc, arr_assign_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
        }));
    }

    block_get(proc, arr_assign_idx).push(IRStmt::Goto(follow_idx));

    proc.blocks.add_edge(arr_assign_idx, follow_idx, ());

    *block_idx = follow_idx;

    t_o
}

fn assign_accessible(
    block_idx: &mut NodeIndex,
    tmp: IRVar,
    is_owned: bool,
    a: &CSTAccessible,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    let mut t_n: IRVar;
    if let CSTExpressionKind::Variable(v) = &a.head.kind {
        if let Some(tmp) = stack_get(shared_proc, v) {
            t_n = tmp;
        } else {
            // t_n := stack_get_or_new(v);
            t_n = tmp_var_new(proc);
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_n),
                types: IRType::PTR,
                source: IRValue::BuiltinProc(BuiltinProc::StackGetOrNew),
                op: IROp::NativeCall(vec![IRValue::String(v.clone())]),
            }));
        }
    } else {
        panic!("assign target accessible head must be variable");
    }

    for i in &a.body {
        match &i.kind {
            CSTExpressionKind::Variable(v) => {
                /* t_n_val := *t_n;
                 * t_o := object_get_or_new(t_n_val, v);
                 */
                let t_n_val = tmp_var_new(proc);
                let t_o = tmp_var_new(proc);
                block_get(proc, *block_idx).extend(vec![
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_n_val),
                        types: IRTypes!("any"),
                        source: IRValue::Variable(t_n),
                        op: IROp::PtrDeref,
                    }),
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_o),
                        types: IRType::PTR,
                        source: IRValue::BuiltinProc(BuiltinProc::ObjectGetOrNew),
                        op: IROp::NativeCall(vec![
                            IRValue::Variable(t_n_val),
                            IRValue::String(v.clone()),
                        ]),
                    }),
                ]);
                t_n = t_o;
            }
            CSTExpressionKind::Collection(l) => {
                if let CSTCollection::List(s) = l
                    && s.range.is_none()
                    && s.rest.is_none()
                {
                    if s.expressions.len() == 1 {
                        t_n = assign_arr_access(
                            block_idx,
                            t_n,
                            &s.expressions[0],
                            proc,
                            shared_proc,
                            cfg,
                        );
                    } else {
                        /* t_list := list_new();
                         * t_expr := // s.expressions[0];
                         * _ := list_push(t_list, t_expr);
                         * //...
                         * t_n_val := *t_n;
                         * t_o := set_get_tag(t_n_val, t_list);
                         * t_o_om := t_o == om;
                         * if t_o_om
                         *  goto <set_insert_idx>
                         * else
                         *  goto <inval_idx>
                         *
                         * <insert_idx>:
                         *  t_insert_list := list_new();
                         *  _ := list_push(t_insert_list, t_list);
                         *  _ := list_push(t_insert_list, om);
                         *  // NOTE: set_insert returns undefined if the inserted entry is
                         *  // undefined. This is guaranteed not to be the case here
                         *  t_o := set_insert(t_n_val, t_insert_list);
                         *  t_o := t_o[1];
                         *  goto <follow_idx>
                         *
                         * <inval_idx>:
                         *  _ := invalidate(t_list);
                         *  goto <follow_idx>
                         *
                         * <follow_idx>:
                         */
                        let t_list = tmp_var_new(proc);
                        block_get(proc, *block_idx).extend(vec![IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_list),
                            types: IRType::LIST,
                            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                            op: IROp::NativeCall(Vec::new()),
                        })]);

                        for i in &s.expressions {
                            let t_expr = tmp_var_new(proc);

                            let expr_owned = block_expr_push(
                                i,
                                block_idx,
                                IRTarget::Variable(t_expr),
                                proc,
                                shared_proc,
                                cfg,
                            );

                            if !expr_owned {
                                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                                    target: IRTarget::Variable(t_expr),
                                    types: IRTypes!("any"),
                                    source: IRValue::BuiltinProc(BuiltinProc::Copy),
                                    op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
                                }));
                            }

                            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                                target: IRTarget::Ignore,
                                types: IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                                op: IROp::NativeCall(vec![
                                    IRValue::Variable(t_list),
                                    IRValue::Variable(t_expr),
                                ]),
                            }));
                        }

                        let t_n_val = tmp_var_new(proc);
                        let t_o = tmp_var_new(proc);
                        let t_o_is_om = tmp_var_new(proc);
                        let t_insert_list = tmp_var_new(proc);

                        let insert_idx = proc.blocks.add_node(Vec::new());
                        let inval_idx = proc.blocks.add_node(Vec::new());
                        let follow_idx = proc.blocks.add_node(Vec::new());

                        block_get(proc, *block_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_n_val),
                                types: IRTypes!("any"),
                                source: IRValue::Variable(t_n),
                                op: IROp::PtrDeref,
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_o),
                                types: IRType::PTR | IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::SetGetTag),
                                op: IROp::NativeCall(vec![
                                    IRValue::Variable(t_n_val),
                                    IRValue::Variable(t_list),
                                ]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_o_is_om),
                                types: IRType::BOOL,
                                source: IRValue::Variable(t_o),
                                op: IROp::Equal(IRValue::Undefined),
                            }),
                            IRStmt::Branch(IRBranch {
                                cond: IRValue::Variable(t_o_is_om),
                                success: insert_idx,
                                failure: inval_idx,
                            }),
                        ]);

                        proc.blocks.add_edge(*block_idx, insert_idx, ());
                        proc.blocks.add_edge(*block_idx, inval_idx, ());

                        block_get(proc, insert_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_insert_list),
                                types: IRType::LIST,
                                source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                                op: IROp::NativeCall(Vec::new()),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Ignore,
                                types: IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                                op: IROp::NativeCall(vec![
                                    IRValue::Variable(t_insert_list),
                                    IRValue::Variable(t_list),
                                ]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Ignore,
                                types: IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                                op: IROp::NativeCall(vec![
                                    IRValue::Variable(t_insert_list),
                                    IRValue::Undefined,
                                ]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_o),
                                types: IRType::PTR,
                                source: IRValue::BuiltinProc(BuiltinProc::SetInsert),
                                op: IROp::NativeCall(vec![
                                    IRValue::Variable(t_n_val),
                                    IRValue::Variable(t_insert_list),
                                ]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_o),
                                types: IRType::PTR,
                                source: IRValue::Variable(t_o),
                                op: IROp::AccessArray(IRValue::Number(1.into())),
                            }),
                            IRStmt::Goto(follow_idx),
                        ]);

                        proc.blocks.add_edge(insert_idx, follow_idx, ());

                        block_get(proc, inval_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Ignore,
                                types: IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                op: IROp::NativeCall(vec![IRValue::Variable(t_list)]),
                            }),
                            IRStmt::Goto(follow_idx),
                        ]);

                        proc.blocks.add_edge(inval_idx, follow_idx, ());

                        *block_idx = follow_idx;

                        t_n = t_o;
                    }
                } else {
                    panic!(
                        "assign target accessible body collection must be list singleton containing a non-zero number"
                    );
                }
            }
            _ => {
                panic!("assign target accessible body must be variable or list");
            }
        }
    }

    let t_n_val = tmp_var_new(proc);
    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_n_val),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_n),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_n_val)]),
        }),
    ]);

    if !is_owned {
        // *t_n := copy(tmp);
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Deref(t_n),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }));
    } else {
        // *t_n := tmp;
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Deref(t_n),
            types: IRTypes!("any"),
            source: IRValue::Variable(tmp),
            op: IROp::Assign,
        }));
    }
}

fn assign_list(
    block_idx: &mut NodeIndex,
    tmp: IRVar,
    is_owned: bool,
    t_succeeded: Option<IRVar>,
    s: &CSTSet,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /*  t_type := type_of(tmp);
     *  t_type_list := t_type == TYPE_LIST;
     *  t_type_string := t_type == TYPE_STRING;
     *  t_type_fits := t_type_list || t_type_string;
     *  if t_type_fits
     *   goto <assign_idx>
     *  else
     *   goto <except_idx>
     *
     * <assign_idx>:
     *  // if let Some(t_s) = t_succeeded {
     *   t_s := true;
     *  //}
     *  // for (idx, i) in expr.iter().enumerate() {
     *   t_i := tmp[idx];
     *   // assign parse (t_i, i)
     *   // if let Some(t_s) = t_succeeded {
     *    if t_s
     *     goto <next_idx>
     *    else
     *     goto <follow_idx>
     *   // }
     *   // assign_idx = next_idx
     *  // }
     *  // if let Some(rest) = rest {
     *   t_len := amount(tmp);
     *   t_s := slice(tmp, expr.len(), t_len);
     *   // assign parse (t_s, rest)
     *  // }
     *  // if is_owned {
     *   _ := invalidate(tmp);
     *  // }
     *  goto <follow_idx>
     *
     * <except_idx>:
     * // if let Some(t_s) = t_succeded {
     *  t_s := false;
     *  goto <follow_idx>
     * // } else {
     *  _ := exception_throw("assign", "unassignable amount of collection members");
     *  unreachable;
     * // }
     *
     * <follow_idx>:
     */
    let t_type = tmp_var_new(proc);
    let t_type_list = tmp_var_new(proc);
    let t_type_string = tmp_var_new(proc);
    let t_type_fits = tmp_var_new(proc);

    let mut assign_idx = proc.blocks.add_node(Vec::new());
    let except_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type_list),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type),
            op: IROp::Equal(IRValue::Type(IRType::LIST)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type_string),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type),
            op: IROp::Equal(IRValue::Type(IRType::STRING)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type_fits),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type_list),
            op: IROp::Or(IRValue::Variable(t_type_string)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_type_fits),
            success: assign_idx,
            failure: except_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, assign_idx, ());
    proc.blocks.add_edge(*block_idx, except_idx, ());

    if let Some(t_s) = t_succeeded {
        block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_s),
            types: IRType::BOOL,
            source: IRValue::Bool(true),
            op: IROp::Assign,
        }));
    }

    for (idx, i) in s.expressions.iter().enumerate() {
        let t_i = tmp_var_new(proc);
        block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRTypes!("any"),
            source: IRValue::Variable(tmp),
            op: IROp::AccessArray(IRValue::Number(idx.into())),
        }));

        assign_parse(
            &mut assign_idx,
            t_i,
            false,
            t_succeeded,
            i,
            proc,
            shared_proc,
            cfg,
        );

        if let Some(t_s) = t_succeeded {
            let next_idx = proc.blocks.add_node(Vec::new());

            block_get(proc, assign_idx).push(IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_s),
                success: next_idx,
                failure: follow_idx,
            }));

            proc.blocks.add_edge(assign_idx, next_idx, ());
            proc.blocks.add_edge(assign_idx, follow_idx, ());

            assign_idx = next_idx;
        }
    }

    if let Some(rest) = &s.rest {
        let t_len = tmp_var_new(proc);
        let t_slice = tmp_var_new(proc);

        block_get(proc, assign_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_len),
                types: IRType::NUMBER,
                source: IRValue::BuiltinProc(BuiltinProc::Amount),
                op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_slice),
                types: IRType::STRING | IRType::LIST,
                source: IRValue::BuiltinProc(BuiltinProc::Slice),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(tmp),
                    IRValue::Number(s.expressions.len().into()),
                    IRValue::Variable(t_len),
                ]),
            }),
        ]);

        assign_parse(
            &mut assign_idx,
            t_slice,
            false,
            t_succeeded,
            rest,
            proc,
            shared_proc,
            cfg,
        );
    }

    if is_owned {
        block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }));
    }

    block_get(proc, assign_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(assign_idx, follow_idx, ());

    if let Some(t_s) = t_succeeded {
        block_get(proc, except_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_s),
                types: IRType::BOOL,
                source: IRValue::Bool(false),
                op: IROp::Assign,
            }),
            IRStmt::Goto(follow_idx),
        ]);

        proc.blocks.add_edge(assign_idx, follow_idx, ());
    } else {
        block_get(proc, except_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
                op: IROp::NativeCall(vec![
                    IRValue::String("assign".to_string()),
                    IRValue::String("unassignable amount of collection members".to_string()),
                ]),
            }),
            IRStmt::Unreachable,
        ]);
    }

    *block_idx = follow_idx;
}

fn assign_set(
    block_idx: &mut NodeIndex,
    tmp: IRVar,
    is_owned: bool,
    t_succeeded: Option<IRVar>,
    s: &CSTSet,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /*  t_type := type_of(tmp);
     *  t_type_set := t_type == TYPE_SET;
     *  if t_type_set
     *   goto <assign_idx>
     *  else
     *   goto <except_idx>
     *
     * <assign_idx>:
     *  // if let Some(t_s) = t_succeeded {
     *   t_s := true;
     *  //}
     *  // if !is_owned {
     *   tmp := copy(tmp);
     *  // }
     *  // for (idx, i) in expr.iter().enumerate() {
     *   t_i := set_take(tmp, true);
     *   // assign parse (t_i, i)
     *   // if let Some(t_s) = t_succeeded {
     *    if t_s
     *     goto <next_idx>
     *    else
     *     goto <follow_idx>
     *   //}
     *   // assign_idx = next_idx;
     *  //
     *  // if let Some(rest) = rest {
     *   // assign parse (tmp, rest)
     *  //} else {
     *   _ := invalidate(tmp);
     *  //}
     *  goto <follow_idx>
     *
     * <except_idx>:
     * // if let Some(t_s) = t_succeded {
     *  t_s := false;
     *  goto <follow_idx>
     * // } else {
     *  _ := exception_throw("assign", "unassignable amount of collection members");
     *  unreachable;
     * // }
     *
     * <follow_idx>:
     */
    let t_type = tmp_var_new(proc);
    let t_type_set = tmp_var_new(proc);

    let mut assign_idx = proc.blocks.add_node(Vec::new());
    let except_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type_set),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type),
            op: IROp::Equal(IRValue::Type(IRType::SET)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_type_set),
            success: assign_idx,
            failure: except_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, assign_idx, ());
    proc.blocks.add_edge(*block_idx, except_idx, ());

    if let Some(t_s) = t_succeeded {
        block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_s),
            types: IRType::BOOL,
            source: IRValue::Bool(true),
            op: IROp::Assign,
        }));
    }

    if !is_owned {
        block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(tmp),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }));
    }

    for i in &s.expressions {
        let t_i = tmp_var_new(proc);
        block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::SetTake),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp), IRValue::Bool(true)]),
        }));

        assign_parse(
            &mut assign_idx,
            t_i,
            false,
            t_succeeded,
            i,
            proc,
            shared_proc,
            cfg,
        );

        if let Some(t_s) = t_succeeded {
            let next_idx = proc.blocks.add_node(Vec::new());

            block_get(proc, assign_idx).push(IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_s),
                success: next_idx,
                failure: follow_idx,
            }));

            proc.blocks.add_edge(assign_idx, next_idx, ());
            proc.blocks.add_edge(assign_idx, follow_idx, ());

            assign_idx = next_idx;
        }
    }

    if let Some(rest) = &s.rest {
        assign_parse(
            &mut assign_idx,
            tmp,
            true,
            t_succeeded,
            rest,
            proc,
            shared_proc,
            cfg,
        );
    } else {
        block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }));
    }

    block_get(proc, assign_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(assign_idx, follow_idx, ());

    if let Some(t_s) = t_succeeded {
        block_get(proc, except_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_s),
                types: IRType::BOOL,
                source: IRValue::Bool(false),
                op: IROp::Assign,
            }),
            IRStmt::Goto(follow_idx),
        ]);

        proc.blocks.add_edge(assign_idx, follow_idx, ());
    } else {
        block_get(proc, except_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
                op: IROp::NativeCall(vec![
                    IRValue::String("assign".to_string()),
                    IRValue::String("unassignable amount of collection members".to_string()),
                ]),
            }),
            IRStmt::Unreachable,
        ]);
    }

    *block_idx = follow_idx;
}

fn assign_collection(
    block_idx: &mut NodeIndex,
    tmp: IRVar,
    is_owned: bool,
    t_succeeded: Option<IRVar>,
    c: &CSTCollection,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /* t_len := amount(tmp);
     * t_len_cond := expr.len() == t_len;
     * // if rest.is_some() {
     *  t_1 := expr.len() < t_len;
     *  t_len_cond := t_1 || t_len_cond;
     * // }
     * _ := invalidate(t_len);
     * if t_len_cond
     *  goto <coll_idx>
     * else
     *  goto <fail_idx>
     *
     * <coll_idx>:
     *  // assign set | assign list
     *  goto <follow_idx>
     *
     * <fail_idx>:
     * // if let Some(t_s) = t_succeded {
     *  t_s := false;
     *  goto <follow_idx>
     * // } else {
     *  _ := exception_throw("assign", "unassignable amount of collection members");
     *  unreachable;
     * // }
     *
     * <follow_idx>:
     */
    let s = match c {
        CSTCollection::Set(s) | CSTCollection::List(s) => s,
        _ => unreachable!(),
    };

    let t_len = tmp_var_new(proc);
    let t_len_cond = tmp_var_new(proc);

    let mut coll_idx = proc.blocks.add_node(Vec::new());
    let fail_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len_cond),
            types: IRType::BOOL,
            source: IRValue::Number(s.expressions.len().into()),
            op: IROp::Equal(IRValue::Variable(t_len)),
        }),
    ]);

    if s.rest.is_some() {
        let t_1 = tmp_var_new(proc);
        block_get(proc, *block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_1),
                types: IRType::BOOL,
                source: IRValue::Number(s.expressions.len().into()),
                op: IROp::Less(IRValue::Variable(t_len)),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_len_cond),
                types: IRType::BOOL,
                source: IRValue::Variable(t_1),
                op: IROp::Or(IRValue::Variable(t_len_cond)),
            }),
        ]);
    }

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_len_cond),
            success: coll_idx,
            failure: fail_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, coll_idx, ());
    proc.blocks.add_edge(*block_idx, fail_idx, ());

    match c {
        CSTCollection::Set(s) => assign_set(
            &mut coll_idx,
            tmp,
            is_owned,
            t_succeeded,
            s,
            proc,
            shared_proc,
            cfg,
        ),
        CSTCollection::List(s) => assign_list(
            &mut coll_idx,
            tmp,
            is_owned,
            t_succeeded,
            s,
            proc,
            shared_proc,
            cfg,
        ),
        _ => unreachable!(),
    }

    block_get(proc, coll_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(coll_idx, follow_idx, ());

    if let Some(t_s) = t_succeeded {
        block_get(proc, fail_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_s),
                types: IRType::BOOL,
                source: IRValue::Bool(false),
                op: IROp::Assign,
            }),
            IRStmt::Goto(follow_idx),
        ]);

        proc.blocks.add_edge(fail_idx, follow_idx, ());
    } else {
        block_get(proc, fail_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
                op: IROp::NativeCall(vec![
                    IRValue::String("assign".to_string()),
                    IRValue::String("unassignable amount of collection members".to_string()),
                ]),
            }),
            IRStmt::Unreachable,
        ]);
    }

    *block_idx = follow_idx;
}

fn assign_var(
    block_idx: &mut NodeIndex,
    tmp: IRVar,
    is_owned: bool,
    t_succeeded: Option<IRVar>,
    v: &str,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
) {
    let var_idx = if let Some(t) = stack_get(shared_proc, v) {
        t
    } else {
        let t = tmp_var_new(proc);
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t),
            types: IRType::PTR,
            source: IRValue::BuiltinProc(BuiltinProc::StackGetOrNew),
            op: IROp::NativeCall(vec![IRValue::String(v.to_string())]),
        }));
        t
    };

    let t_insert = tmp_var_new(proc);
    if is_owned {
        // t_insert := tmp;
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_insert),
            types: IRTypes!("any"),
            source: IRValue::Variable(tmp),
            op: IROp::Assign,
        }));
    } else {
        // t_insert := copy(tmp);
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_insert),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }));
    }
    /* t_var_val := *var_idx;
     * _ := invalidate(t_var_val);
     */
    let t_var_val = tmp_var_new(proc);
    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_var_val),
            types: IRTypes!("any"),
            source: IRValue::Variable(var_idx),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_var_val)]),
        }),
    ]);

    /* *var_idx := t_insert;
     * _ := mark_persist(t_insert);
     */
    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Deref(var_idx),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_insert),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::MarkPersist),
            op: IROp::NativeCall(vec![IRValue::Variable(t_insert)]),
        }),
    ]);

    // t_succeeded := true;
    if let Some(t_s) = t_succeeded {
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_s),
            types: IRType::BOOL,
            source: IRValue::Bool(true),
            op: IROp::Assign,
        }));
    }
}

fn assign_term(
    block_idx: &mut NodeIndex,
    tmp: IRVar,
    is_owned: bool,
    t_succeeded: Option<IRVar>,
    t: &CSTTerm,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /* t_type := type_of(tmp);
     * t_type_ast := t_type == TYPE_AST;
     * t_type_term := t_type == TYPE_TERM;
     * t_type_tterm := t_type == TYPE_TTERM;
     * t_type_p := t_type_ast || t_type_term;
     * t_type_p := t_type_p || t_type_tterm;
     * if t_type_p
     *  goto <tag_idx>
     * else
     *  goto <fail_idx>
     *
     * <tag_idx>
     *  t_tag := tmp[0];
     *  // if t.is_tterm {
     *   // if let Some(tag) = tterm_ast_tag_get(t.tag, t.expressions.len()) {
     *    t_type_eq := t_type == TYPE_AST;
     *    t_tag_eq := t_tag == tag;
     *   // } else {
     *    t_type_eq := t_type == TYPE_TTERM;
     *    t_tag_eq := t_tag == t.tag;
     *   // }
     *  // } else {
     *   t_type_eq := t_type == TYPE_TERM;
     *   t_tag_eq := t_tag == t.tag;
     *  // }
     *
     *  t_len := amount(tmp);
     *  t_len_eq := t_len == t.expressions.len();
     *  t_cond := t_type_eq && t_tag_eq;
     *  t_cond := t_cond && t_len_eq;
     *  if t_cond
     *   goto <check_mem_idx>
     *  else
     *   goto <fail_idx>
     *
     * <check_mem_idx>:
     *  // if let Some(t_s) = t_succeeded {
     *   t_s := true;
     *  // }
     *  // for (idx, expr) in t.expressions.iter().enumerate() {
     *   t_mem := tmp[idx + 1];
     *   // assign parse (t_mem)
     *   // if let Some(t_s) = t_succeeded {
     *     if t_s
     *      goto <next_idx>
     *     else
     *      goto <follow_idx>
     *     // check_mem_idx = next_idx
     *   // }
     *  // }
     *  goto <follow_idx>
     *
     * <fail_idx>:
     *  // if let Some(t_s) = t_succeeded {
     *   t_s := false;
     *   goto <follow_idx>
     *  // } else {
     *  _ := exception_throw("assign", "cannot assign to different kind of terms");
     *  unreachable;
     *  // }
     *
     * <follow_idx>:
     *  // if is_owned {
     *   _ := invalidate(tmp);
     *  //
     */
    let tag_idx = proc.blocks.add_node(Vec::new());
    let mut check_mem_idx = proc.blocks.add_node(Vec::new());
    let fail_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    let t_type = tmp_var_new(proc);
    let t_type_ast = tmp_var_new(proc);
    let t_type_term = tmp_var_new(proc);
    let t_type_tterm = tmp_var_new(proc);
    let t_type_p = tmp_var_new(proc);
    let t_tag = tmp_var_new(proc);
    let t_type_eq = tmp_var_new(proc);
    let t_tag_eq = tmp_var_new(proc);
    let t_len = tmp_var_new(proc);
    let t_len_eq = tmp_var_new(proc);
    let t_cond = tmp_var_new(proc);

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type_ast),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type),
            op: IROp::Equal(IRValue::Type(IRType::AST)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type_term),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type),
            op: IROp::Equal(IRValue::Type(IRType::TERM)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type_tterm),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type),
            op: IROp::Equal(IRValue::Type(IRType::TTERM)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type_p),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type_ast),
            op: IROp::Or(IRValue::Variable(t_type_term)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type_p),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type_p),
            op: IROp::Or(IRValue::Variable(t_type_tterm)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_type_p),
            success: tag_idx,
            failure: fail_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, tag_idx, ());
    proc.blocks.add_edge(*block_idx, fail_idx, ());

    block_get(proc, tag_idx).extend(vec![IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_tag),
        types: IRTypes!("any"),
        source: IRValue::Variable(tmp),
        op: IROp::AccessArray(IRValue::Number(0.into())),
    })]);

    if t.is_tterm {
        if let Some(tag) = tterm_ast_tag_get(&t.name, t.params.len()) {
            block_get(proc, tag_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_type_eq),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_type),
                    op: IROp::Equal(IRValue::Type(IRType::AST)),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_tag_eq),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_tag),
                    op: IROp::Equal(IRValue::String(tag)),
                }),
            ]);
        } else {
            block_get(proc, tag_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_type_eq),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_type),
                    op: IROp::Equal(IRValue::Type(IRType::TTERM)),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_tag_eq),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_tag),
                    op: IROp::Equal(IRValue::String(t.name.clone())),
                }),
            ]);
        }
    } else {
        block_get(proc, tag_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_type_eq),
                types: IRType::BOOL,
                source: IRValue::Variable(t_type),
                op: IROp::Equal(IRValue::Type(IRType::TERM)),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_tag_eq),
                types: IRType::BOOL,
                source: IRValue::Variable(t_tag),
                op: IROp::Equal(IRValue::String(t.name.clone())),
            }),
        ]);
    }

    block_get(proc, tag_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len_eq),
            types: IRType::BOOL,
            source: IRValue::Variable(t_len),
            op: IROp::Equal(IRValue::Number(t.params.len().into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cond),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type_eq),
            op: IROp::And(IRValue::Variable(t_tag_eq)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cond),
            types: IRType::BOOL,
            source: IRValue::Variable(t_cond),
            op: IROp::And(IRValue::Variable(t_len_eq)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cond),
            success: check_mem_idx,
            failure: fail_idx,
        }),
    ]);

    proc.blocks.add_edge(tag_idx, check_mem_idx, ());
    proc.blocks.add_edge(tag_idx, fail_idx, ());

    if let Some(t_s) = t_succeeded {
        block_get(proc, check_mem_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_s),
            types: IRType::BOOL,
            source: IRValue::Bool(true),
            op: IROp::Assign,
        }));
    }

    for (idx, expr) in t.params.iter().enumerate() {
        let t_mem = tmp_var_new(proc);

        block_get(proc, check_mem_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_mem),
            types: IRTypes!("any"),
            source: IRValue::Variable(tmp),
            op: IROp::AccessArray(IRValue::Number((idx + 1).into())),
        }));

        assign_parse(
            &mut check_mem_idx,
            t_mem,
            false,
            t_succeeded,
            expr,
            proc,
            shared_proc,
            cfg,
        );

        if let Some(t_s) = t_succeeded {
            let next_idx = proc.blocks.add_node(Vec::new());
            block_get(proc, check_mem_idx).push(IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_s),
                success: next_idx,
                failure: follow_idx,
            }));

            proc.blocks.add_edge(check_mem_idx, next_idx, ());
            proc.blocks.add_edge(check_mem_idx, follow_idx, ());

            check_mem_idx = next_idx;
        }
    }

    block_get(proc, check_mem_idx).push(IRStmt::Goto(follow_idx));

    if let Some(t_s) = t_succeeded {
        block_get(proc, fail_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_s),
                types: IRType::BOOL,
                source: IRValue::Bool(false),
                op: IROp::Assign,
            }),
            IRStmt::Goto(follow_idx),
        ]);

        proc.blocks.add_edge(fail_idx, follow_idx, ());
    } else {
        block_get(proc, fail_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
                op: IROp::NativeCall(vec![
                    IRValue::String("assign".to_string()),
                    IRValue::String("cannot assign to different kind of terms".to_string()),
                ]),
            }),
            IRStmt::Unreachable,
        ]);
    }

    if is_owned {
        block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }));
    }

    *block_idx = follow_idx;
}

fn ast_kind_str_eq_push(
    block_idx: &mut NodeIndex,
    t_expr: IRVar,
    op: String,
    t_target: IRVar,
    proc: &mut IRProcedure,
) {
    /*  t_expr_type := type_of(t_expr);
     *  t_expr_ast := t_expr_type == TYPE_AST;
     *  if t_expr_ast
     *   goto <ast_check_idx>
     *  else
     *   goto <fail_idx>
     *
     * <ast_check_idx>:
     *  t_expr_tag := t_expr[0];
     *  t_target := t_expr_tag == op;
     *  goto <follow_idx>
     *
     * <fail_idx>:
     *  t_target := false;
     *  goto <follow_idx>
     *
     * <follow_idx>:
     */
    let t_expr_type = tmp_var_new(proc);
    let t_expr_ast = tmp_var_new(proc);
    let t_expr_tag = tmp_var_new(proc);

    let ast_check_idx = proc.blocks.add_node(Vec::new());
    let fail_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, *block_idx).extend(vec![
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
            success: ast_check_idx,
            failure: fail_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, ast_check_idx, ());
    proc.blocks.add_edge(*block_idx, fail_idx, ());

    block_get(proc, ast_check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_expr_tag),
            types: IRType::STRING,
            source: IRValue::Variable(t_expr),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_target),
            types: IRType::BOOL,
            source: IRValue::Variable(t_expr_tag),
            op: IROp::Equal(IRValue::String(op)),
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(ast_check_idx, follow_idx, ());

    block_get(proc, fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_target),
            types: IRType::BOOL,
            source: IRValue::Bool(false),
            op: IROp::Assign,
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(fail_idx, follow_idx, ());

    *block_idx = follow_idx;
}

fn assign_op(
    block_idx: &mut NodeIndex,
    tmp: IRVar,
    is_owned: bool,
    t_succeeded: Option<IRVar>,
    o: &CSTExpressionOp,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /*  t_kind_check := // ast_kind_str_eq_push
     *  if t_kind_check
     *   goto <assign_lhs_idx>
     *  else
     *   goto <fail_idx>
     *
     * <assign_lhs_idx>:
     *  t_lhs := tmp[1];
     *  // assign parse
     *  if t_s
     *   goto <assign_rhs_idx>
     *  else
     *   goto <follow_idx>
     *
     * <assign_rhs_idx>:
     *  t_rhs := tmp[2];
     *  // assign parse
     *  goto <follow_idx>:
     *
     * <fail_idx>:
     *  t_s := false;
     *  goto <follow_idx>
     *
     * <follow_idx>:
     * // if is_owned {
     *  _ := invalidate(tmp);
     * // }
     */
    let t_kind_check = tmp_var_new(proc);
    let t_lhs = tmp_var_new(proc);
    let t_rhs = tmp_var_new(proc);

    let mut assign_lhs_idx = proc.blocks.add_node(Vec::new());
    let mut assign_rhs_idx = proc.blocks.add_node(Vec::new());
    let fail_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    ast_kind_str_eq_push(block_idx, tmp, o.op.to_string(), t_kind_check, proc);

    block_get(proc, *block_idx).push(IRStmt::Branch(IRBranch {
        cond: IRValue::Variable(t_kind_check),
        success: assign_lhs_idx,
        failure: fail_idx,
    }));

    proc.blocks.add_edge(*block_idx, assign_lhs_idx, ());
    proc.blocks.add_edge(*block_idx, fail_idx, ());

    block_get(proc, assign_lhs_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_lhs),
        types: IRTypes!("any"),
        source: IRValue::Variable(tmp),
        op: IROp::AccessArray(IRValue::Number(1.into())),
    }));

    assign_parse(
        &mut assign_lhs_idx,
        t_lhs,
        false,
        t_succeeded,
        &o.left,
        proc,
        shared_proc,
        cfg,
    );
    block_get(proc, assign_lhs_idx).push(IRStmt::Branch(IRBranch {
        cond: IRValue::Variable(t_succeeded.unwrap()),
        success: assign_rhs_idx,
        failure: follow_idx,
    }));

    proc.blocks.add_edge(assign_lhs_idx, assign_rhs_idx, ());
    proc.blocks.add_edge(assign_lhs_idx, follow_idx, ());

    block_get(proc, assign_rhs_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_rhs),
        types: IRTypes!("any"),
        source: IRValue::Variable(tmp),
        op: IROp::AccessArray(IRValue::Number(2.into())),
    }));

    assign_parse(
        &mut assign_rhs_idx,
        t_rhs,
        false,
        t_succeeded,
        &o.right,
        proc,
        shared_proc,
        cfg,
    );
    block_get(proc, assign_rhs_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(assign_rhs_idx, follow_idx, ());

    block_get(proc, fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_succeeded.unwrap()),
            types: IRType::BOOL,
            source: IRValue::Bool(false),
            op: IROp::Assign,
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(fail_idx, follow_idx, ());

    if is_owned {
        block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }));
    }

    *block_idx = follow_idx;
}

fn assign_unary_op(
    block_idx: &mut NodeIndex,
    tmp: IRVar,
    is_owned: bool,
    t_succeeded: Option<IRVar>,
    o: &CSTExpressionUnaryOp,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /*  t_kind_check := // ast_kind_str_eq_push
     *  if t_check_kind
     *   goto <assign_idx>
     *  else
     *   goto <fail_idx>
     *
     * <assign_idx>:
     *  t_lhs := tmp[1];
     *  // assign parse
     *  goto <follow idx>
     *
     * <fail_idx>:
     *  t_s := false;
     *  goto <follow_idx>
     *
     * <follow_idx>:
     * _ := invalidate(tmp);
     */
    let t_lhs = tmp_var_new(proc);
    let t_kind_check = tmp_var_new(proc);

    let mut assign_idx = proc.blocks.add_node(Vec::new());
    let fail_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    ast_kind_str_eq_push(block_idx, tmp, o.op.to_string(), t_kind_check, proc);

    block_get(proc, *block_idx).push(IRStmt::Branch(IRBranch {
        cond: IRValue::Variable(t_kind_check),
        success: assign_idx,
        failure: fail_idx,
    }));

    proc.blocks.add_edge(*block_idx, assign_idx, ());
    proc.blocks.add_edge(*block_idx, fail_idx, ());

    block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_lhs),
        types: IRTypes!("any"),
        source: IRValue::Variable(tmp),
        op: IROp::AccessArray(IRValue::Number(1.into())),
    }));

    assign_parse(
        &mut assign_idx,
        t_lhs,
        false,
        t_succeeded,
        &o.expr,
        proc,
        shared_proc,
        cfg,
    );

    block_get(proc, assign_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(assign_idx, follow_idx, ());

    block_get(proc, fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_succeeded.unwrap()),
            types: IRType::BOOL,
            source: IRValue::Bool(false),
            op: IROp::Assign,
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(fail_idx, follow_idx, ());

    if is_owned {
        block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }));
    }

    *block_idx = follow_idx;
}

fn assign_literal(
    block_idx: &mut NodeIndex,
    tmp: IRVar,
    is_owned: bool,
    t_succeeded: Option<IRVar>,
    target: &CSTExpression,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /* t_e := //expr
     * t_s := tmp == t_e
     * t_succeeded := t_s;
     * _ := invalidate(t_e);
     * _ := invalidate(tmp);
     */
    let t_e = tmp_var_new(proc);

    block_expr_push(
        target,
        block_idx,
        IRTarget::Variable(t_e),
        proc,
        shared_proc,
        cfg,
    );

    let t_s = tmp_var_new(proc);
    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_s),
            types: IRType::BOOL,
            source: IRValue::Variable(tmp),
            op: IROp::Equal(IRValue::Variable(t_e)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_succeeded.unwrap()),
            types: IRType::BOOL,
            source: IRValue::Variable(t_s),
            op: IROp::Assign,
        }),
    ]);

    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Ignore,
        types: IRType::UNDEFINED,
        source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
        op: IROp::NativeCall(vec![IRValue::Variable(t_e)]),
    }));

    if is_owned {
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }));
    }
}

//FIXME: call parameters need to be pushed in match case
fn assign_call(
    block_idx: &mut NodeIndex,
    tmp: IRVar,
    is_owned: bool,
    t_succeeded: Option<IRVar>,
    c: &CSTProcedureCall,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /*  // if c.rest_param.is_some() && let Some(t_s) = t_succeeded {
     *   t_s := false;
     *  // } else {
     *   t_kind := //ast_kind_str_eq_push
     *   if t_kind
     *    goto <ast_check_idx>
     *   else
     *    goto <fail_idx>
     *
     *  <ast_check_idx>
     *   t_params := tmp[2];
     *   t_params_len := amount();
     *   t_params_eq := t_params_len == c.params.len();
     *   if t_params_eq
     *    goto <ast_idx>
     *  else
     *   goto <fail_idx>
     *
     *  <ast_idx>:
     *   t_var_name := // var
     *   *t_var_name := "name";
     *   // for (idx, i) in c.params.iter().enumerate() {
     *    t_i := t_params[idx];
     *    // assign_parse
     *    if t_s
     *     goto <next_idx>
     *    else
     *     goto <follow_idx>
     *    // ast_idx = next_idx;
     *   // }
     *   goto <follow_idx>
     *
     *  <fail_idx>:
     *   // if let Some(t_s) = t_succeeded {
     *    t_s := false;
     *    goto <follow_idx>
     *   // }
     *
     *  <follow_idx>:
     *   // if is_owned {
     *    _ := invalidate(tmp);
     *   // }
     * // }
     */
    if c.rest_param.is_some() {
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_succeeded.unwrap()),
            types: IRType::BOOL,
            source: IRValue::Bool(false),
            op: IROp::Assign,
        }));

        return;
    }

    let ast_check_idx = proc.blocks.add_node(Vec::new());
    let mut ast_idx = proc.blocks.add_node(Vec::new());
    let fail_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    let t_kind = tmp_var_new(proc);

    ast_kind_str_eq_push(block_idx, tmp, String::from("call"), t_kind, proc);
    block_get(proc, *block_idx).push(IRStmt::Branch(IRBranch {
        cond: IRValue::Variable(t_kind),
        success: ast_check_idx,
        failure: fail_idx,
    }));

    proc.blocks.add_edge(*block_idx, ast_check_idx, ());
    proc.blocks.add_edge(*block_idx, fail_idx, ());

    let t_params = tmp_var_new(proc);
    let t_params_len = tmp_var_new(proc);
    let t_params_eq = tmp_var_new(proc);

    block_get(proc, ast_check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_params),
            types: IRType::LIST,
            source: IRValue::Variable(tmp),
            op: IROp::AccessArray(IRValue::Number(2.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_params_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_params)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_params_eq),
            types: IRType::BOOL,
            source: IRValue::Variable(t_params_len),
            op: IROp::Equal(IRValue::Number(c.params.len().into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_params_eq),
            success: ast_idx,
            failure: fail_idx,
        }),
    ]);

    proc.blocks.add_edge(ast_check_idx, ast_idx, ());
    proc.blocks.add_edge(ast_check_idx, fail_idx, ());

    let t_var_name = if let Some(t) = stack_get(shared_proc, &c.name) {
        t
    } else {
        let t = tmp_var_new(proc);
        block_get(proc, ast_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t),
            types: IRType::PTR,
            source: IRValue::BuiltinProc(BuiltinProc::StackGetOrNew),
            op: IROp::NativeCall(vec![IRValue::String(c.name.to_string())]),
        }));
        t
    };

    block_get(proc, ast_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Deref(t_var_name),
        types: IRType::STRING,
        source: IRValue::String(c.name.to_string()),
        op: IROp::Assign,
    }));

    for (idx, i) in c.params.iter().enumerate() {
        let t_i = tmp_var_new(proc);
        block_get(proc, ast_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_params),
            op: IROp::AccessArray(IRValue::Number(idx.into())),
        }));
        assign_parse(
            &mut ast_idx,
            t_i,
            false,
            t_succeeded,
            i,
            proc,
            shared_proc,
            cfg,
        );
        let next_idx = proc.blocks.add_node(Vec::new());
        block_get(proc, ast_idx).push(IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_succeeded.unwrap()),
            success: next_idx,
            failure: follow_idx,
        }));

        proc.blocks.add_edge(ast_idx, next_idx, ());
        proc.blocks.add_edge(ast_idx, follow_idx, ());

        ast_idx = next_idx;
    }

    block_get(proc, ast_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(ast_idx, follow_idx, ());

    block_get(proc, fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_succeeded.unwrap()),
            types: IRType::BOOL,
            source: IRValue::Bool(false),
            op: IROp::Assign,
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(fail_idx, follow_idx, ());

    if is_owned {
        block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }));
    }

    *block_idx = follow_idx;
}

/// assign `tmp` to the pattern described by `target`.
///
/// Supported targets: variables, accessible chains, list/set destructuring,
/// terms, operator and unary operator patterns, literals, and call patterns.
///
/// # Arguments
///
/// * `tmp` - the source value to assign or match against
/// * `is_owned` - whether `tmp` is owned; if true it will be invalidated after use
/// * `t_succeeded` - optional flag variable set to true on success and false on
///   failure; when absent a mismatch throws an exception
/// * `target` - the CST expression describing the assignment or pattern target
pub fn assign_parse(
    block_idx: &mut NodeIndex,
    tmp: IRVar,
    is_owned: bool,
    t_succeeded: Option<IRVar>,
    target: &CSTExpression,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    match &target.kind {
        CSTExpressionKind::Accessible(a) => {
            assert!(t_succeeded.is_none());
            assign_accessible(block_idx, tmp, is_owned, a, proc, shared_proc, cfg);
        }
        CSTExpressionKind::Collection(c) => {
            assign_collection(
                block_idx,
                tmp,
                is_owned,
                t_succeeded,
                c,
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTExpressionKind::Variable(v) => {
            assign_var(block_idx, tmp, is_owned, t_succeeded, v, proc, shared_proc);
        }
        CSTExpressionKind::Term(t) => {
            assign_term(
                block_idx,
                tmp,
                is_owned,
                t_succeeded,
                t,
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTExpressionKind::Op(o) => {
            assign_op(
                block_idx,
                tmp,
                is_owned,
                t_succeeded,
                o,
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTExpressionKind::UnaryOp(o) => {
            assign_unary_op(
                block_idx,
                tmp,
                is_owned,
                t_succeeded,
                o,
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTExpressionKind::Literal(_)
        | CSTExpressionKind::Bool(_)
        | CSTExpressionKind::Om
        | CSTExpressionKind::Number(_) => {
            assign_literal(
                block_idx,
                tmp,
                is_owned,
                t_succeeded,
                target,
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTExpressionKind::Call(c) => {
            assign_call(
                block_idx,
                tmp,
                is_owned,
                t_succeeded,
                c,
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTExpressionKind::Ignore => {
            if is_owned {
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
                }));
            }
        }
        _ => panic!("invalid assign target {:?}", target),
    }
}
