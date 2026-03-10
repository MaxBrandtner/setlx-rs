use petgraph::stable_graph::{NodeIndex, StableGraph};
use std::cell::RefCell;
use std::rc::Rc;

use crate::ast::*;
use crate::builtin::{BuiltinProc, BuiltinVar};
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::access_expr::block_access_ref_push;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::stmt::block_populate;
use crate::ir::lower::util::{block_get, stack_get, tmp_var_new};

/// Emits IR to assemble the parameter list for a call to `t_proc` into `target`.
///
/// Prepends the procedure's embedded stack image if present. Variable arguments
/// pass their stack pointer directly; expression arguments are evaluated into
/// owned temporaries whose addresses are pushed. Owned temporaries are marked
/// persistent before the call. Returns the owned temporary pointers that must
/// be invalidated after the call via `call_params_invalidate_push`.
pub fn call_params_push(
    c: &CSTProcedureCall,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    t_proc: IRVar,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) -> Vec<IRVar> /* IRVar ptrs */ {
    let t_params = match target {
        IRTarget::Variable(v) => v,
        _ => unreachable!(),
    };

    // target := list_new();
    let t_params_addr = tmp_var_new(proc);
    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::LIST,
        source: IRValue::BuiltinProc(BuiltinProc::ListNew),
        op: IROp::NativeCall(Vec::new()),
    }));

    /*  t_stack := procedure_stack_get(t_proc);
     *  _ := mark_persist(t_stack);
     *  t_stack_check := t_stack == om;
     *  if t_stack_check;
     *   goto <follow_idx>
     *  else
     *   goto <assign_idx>
     *
     * <assign_idx>:
     *  t_stack_addr := &t_stack;
     *  list_push(t_params, t_stack_addr);
     *  goto <follow_idx>
     *
     * <follow_idx>:
     */

    let follow_idx = proc.blocks.add_node(Vec::new());
    let assign_idx = proc.blocks.add_node(Vec::new());

    let t_stack = tmp_var_new(proc);
    let t_stack_check = tmp_var_new(proc);
    let t_stack_addr = tmp_var_new(proc);

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_stack),
            types: IRType::STACK_IMAGE | IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ProcedureStackGet),
            op: IROp::NativeCall(vec![IRValue::Variable(t_proc)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::MarkPersist),
            op: IROp::NativeCall(vec![IRValue::Variable(t_stack)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_stack_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_stack),
            op: IROp::Equal(IRValue::Undefined),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_stack_check),
            success: follow_idx,
            failure: assign_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, follow_idx, ());
    proc.blocks.add_edge(*block_idx, assign_idx, ());

    block_get(proc, assign_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_stack_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_stack),
            op: IROp::PtrAddress,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ListPush),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_params),
                IRValue::Variable(t_stack_addr),
            ]),
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(*block_idx, follow_idx, ());

    *block_idx = follow_idx;

    let mut out_vars: Vec<IRVar> = c
        .params
        .iter()
        .filter_map(|i| {
            let tmp_addr = tmp_var_new(proc);
            let is_owned = match &i.kind {
                CSTExpressionKind::Variable(v) => {
                    if let Some(tmp) = stack_get(shared_proc, v) {
                        // t_a := tmp;
                        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(tmp_addr),
                            types: IRType::PTR,
                            source: IRValue::Variable(tmp),
                            op: IROp::Assign,
                        }));
                    } else {
                        // t_a := stack_get_or_new(v);
                        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(tmp_addr),
                            types: IRType::PTR,
                            source: IRValue::BuiltinProc(BuiltinProc::StackGetOrNew),
                            op: IROp::NativeCall(vec![IRValue::String(v.to_string())]),
                        }));
                    }
                    None
                }
                CSTExpressionKind::Accessible(a) => {
                    // t_a := // access ref
                    block_access_ref_push(
                        a,
                        block_idx,
                        IRTarget::Variable(tmp_addr),
                        proc,
                        shared_proc,
                        cfg,
                    )
                    .map(|tmp| (tmp, tmp_addr))
                }
                _ => {
                    //t_n := expr;
                    let tmp = tmp_var_new(proc);
                    let expr_owned = block_expr_push(
                        i,
                        block_idx,
                        IRTarget::Variable(tmp),
                        proc,
                        shared_proc,
                        cfg,
                    );

                    /* t_a := &t_n;
                     */
                    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(tmp_addr),
                        types: IRType::PTR,
                        source: IRValue::Variable(tmp),
                        op: IROp::PtrAddress,
                    }));
                    if expr_owned {
                        Some((tmp, tmp_addr))
                    } else {
                        None
                    }
                }
            };
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(t_params),
                    IRValue::Variable(tmp_addr),
                ]),
            }));

            if let Some(o) = is_owned {
                // _ := mark_persist(o.0);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::MarkPersist),
                    op: IROp::NativeCall(vec![IRValue::Variable(o.0)]),
                }));
            }

            is_owned.map(|o| o.1)
        })
        .collect();

    if let Some(rest) = &c.rest_param {
        /* t_rest := //expr
         * t_rest_addr := &t_rest;
         * t_rest_type := type_of(t_rest);
         * t_rest_type_list := t_rest_type == TYPE_LIST;
         * if t_rest_type_list
         *  goto <assign_idx>
         * else
         *  goto <throw_idx>
         *
         * <throw_idx>:
         * _ := throw(1, "invalid type for rest parameters");
         * unreachable
         *
         * <assign_idx>:
         * t_rest_len := amount(t_rest);
         * t_params_len := params.len() + t_rest_len;
         * _ := list_resize(t_params, t_params_len);
         * _ := invalidate(t_params_len);
         * t_i := 0;
         * goto <len_check_bb>
         *
         * <len_check_bb>
         * t_check := t_i < t_rest_len;
         * if t_check
         *   goto <loop_bb>
         * else
         *   goto <follow_bb>
         *
         * <loop_bb>
         * t_offset := t_params_addr[0];
         * *t_offset := t_rest[t_i];
         * t_i_new := t_i + 1;
         * _ := invalidate(t_i);
         * _ := invalidate(t_check);
         * t_i := t_i_new;
         * goto <len_check_bb>
         *
         * <follow_bb>
         * _ := invalidate(t_i);
         * _ := invalidate(t_rest_len);
         * _ := invalidate(t_check);
         */
        let t_rest = tmp_var_new(proc);
        let rest_owned = block_expr_push(
            rest,
            block_idx,
            IRTarget::Variable(t_rest),
            proc,
            shared_proc,
            cfg,
        );

        if rest_owned {
            let t_rest_addr = tmp_var_new(proc);
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_rest_addr),
                types: IRType::PTR,
                source: IRValue::Variable(t_rest),
                op: IROp::PtrAddress,
            }));
            out_vars.push(t_rest_addr);
        }

        let assign_idx = proc.blocks.add_node(Vec::new());
        let throw_idx = proc.blocks.add_node(Vec::new());
        let len_check_idx = proc.blocks.add_node(Vec::new());
        let loop_idx = proc.blocks.add_node(Vec::new());
        let follow_idx = proc.blocks.add_node(Vec::new());

        let t_rest_type = tmp_var_new(proc);
        let t_rest_type_list = tmp_var_new(proc);
        let t_rest_len = tmp_var_new(proc);
        let t_params_len = tmp_var_new(proc);
        let t_i = tmp_var_new(proc);

        block_get(proc, *block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_rest_type),
                types: IRType::TYPE,
                source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
                op: IROp::NativeCall(vec![IRValue::Variable(t_rest)]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_rest_type_list),
                types: IRType::BOOL,
                source: IRValue::Variable(t_rest_type_list),
                op: IROp::Equal(IRValue::Type(IRType::LIST)),
            }),
            IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_rest_type_list),
                success: assign_idx,
                failure: throw_idx,
            }),
        ]);

        proc.blocks.add_edge(*block_idx, assign_idx, ());
        proc.blocks.add_edge(*block_idx, throw_idx, ());

        block_get(proc, throw_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Throw),
                op: IROp::NativeCall(vec![
                    IRValue::Number(1.into()),
                    IRValue::String("invalid type for rest parameter".to_string()),
                ]),
            }),
            IRStmt::Unreachable,
        ]);

        block_get(proc, assign_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_rest_len),
                types: IRType::NUMBER,
                source: IRValue::BuiltinProc(BuiltinProc::Amount),
                op: IROp::NativeCall(vec![IRValue::Variable(t_rest)]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_params_len),
                types: IRType::NUMBER,
                source: IRValue::Variable(t_rest_len),
                op: IROp::Plus(IRValue::Number(c.params.len().into())),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::ListResize),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(t_params),
                    IRValue::Variable(t_params_len),
                ]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_params_len)]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_i),
                types: IRType::NUMBER,
                source: IRValue::Number(0.into()),
                op: IROp::Assign,
            }),
            IRStmt::Goto(len_check_idx),
        ]);

        proc.blocks.add_edge(*block_idx, len_check_idx, ());

        let t_check = tmp_var_new(proc);

        block_get(proc, len_check_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_check),
                types: IRType::BOOL,
                source: IRValue::Variable(t_i),
                op: IROp::Less(IRValue::Variable(t_rest_len)),
            }),
            IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_check),
                success: loop_idx,
                failure: follow_idx,
            }),
        ]);

        proc.blocks.add_edge(len_check_idx, loop_idx, ());
        proc.blocks.add_edge(len_check_idx, follow_idx, ());

        let t_offset = tmp_var_new(proc);
        let t_i_new = tmp_var_new(proc);

        block_get(proc, loop_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_offset),
                types: IRType::PTR,
                source: IRValue::Variable(t_params_addr),
                op: IROp::AccessArray(IRValue::Variable(t_i)),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Deref(t_offset),
                types: IRTypes!("any"),
                source: IRValue::Variable(t_rest),
                op: IROp::AccessArray(IRValue::Variable(t_i)),
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
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_check)]),
            }),
            IRStmt::Goto(len_check_idx),
        ]);

        proc.blocks.add_edge(loop_idx, len_check_idx, ());

        block_get(proc, follow_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_rest_len)]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_check)]),
            }),
        ]);

        *block_idx = follow_idx;
    }

    out_vars
}

/// Emits IR to invalidate the owned temporaries returned by `call_params_push`.
///
/// Called after a procedure call on both the normal and exception paths to
/// clean up any owned expression temporaries whose addresses were passed as
/// arguments.
pub fn call_params_invalidate_push(
    block_idx: NodeIndex,
    out_vars: &[IRVar],
    proc: &mut IRProcedure,
) {
    /* // for i in out_vars {
     *  t_val := *i;
     *  _ := invalidate(t_val);
     * // }
     */
    for i in out_vars {
        let t_val = tmp_var_new(proc);
        block_get(proc, block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_val),
                types: IRTypes!("any"),
                source: IRValue::Variable(*i),
                op: IROp::PtrDeref,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_val)]),
            }),
        ]);
    }
}

pub fn expr_vars_push(e: &CSTExpression, out: &mut Vec<String>) {
    match &e.kind {
        CSTExpressionKind::Term(t) => {
            t.params.iter().for_each(|i| expr_vars_push(i, out));
        }
        CSTExpressionKind::Variable(v) => {
            if !out.contains(v) {
                out.push(v.to_string());
            }
        }
        CSTExpressionKind::Collection(c) => match c {
            CSTCollection::List(l) => {
                for i in &l.expressions {
                    expr_vars_push(i, out);
                }

                if let Some(rest) = &l.rest {
                    expr_vars_push(rest, out);
                }
            }
            CSTCollection::Set(l) => {
                for i in &l.expressions {
                    expr_vars_push(i, out);
                }

                if let Some(rest) = &l.rest {
                    expr_vars_push(rest, out);
                }
            }
            _ => (),
        },
        CSTExpressionKind::Op(o) => {
            expr_vars_push(&o.left, out);
            expr_vars_push(&o.right, out);
        }
        CSTExpressionKind::UnaryOp(o) => {
            expr_vars_push(&o.expr, out);
        }
        CSTExpressionKind::Call(c) => {
            out.push(c.name.to_string());
            c.params.iter().for_each(|i| expr_vars_push(i, out));
            if let Some(rest_param) = &c.rest_param {
                expr_vars_push(rest_param, out);
            }
        }
        _ => (),
    }
}

pub fn procedure_vars_aggregate(cst: &CSTBlock) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for i in cst {
        match &i.kind {
            CSTStatementKind::If(c) | CSTStatementKind::Switch(c) => {
                for j in &c.branches {
                    out.extend(procedure_vars_aggregate(&j.block));
                }

                if let Some(alt) = &c.alternative {
                    out.extend(procedure_vars_aggregate(alt));
                }
            }
            CSTStatementKind::Match(m) => {
                for j in &m.branches {
                    match j {
                        CSTMatchBranch::Case(c) => {
                            out.extend(procedure_vars_aggregate(&c.statements));
                        }
                        CSTMatchBranch::Regex(c) => {
                            out.extend(procedure_vars_aggregate(&c.statements));
                        }
                    }
                }
                out.extend(procedure_vars_aggregate(&m.default));
            }
            CSTStatementKind::Scan(s) => {
                for j in &s.branches {
                    match j {
                        CSTMatchBranch::Case(c) => {
                            out.extend(procedure_vars_aggregate(&c.statements));
                        }
                        CSTMatchBranch::Regex(c) => {
                            out.extend(procedure_vars_aggregate(&c.statements));
                        }
                    }
                }
            }
            CSTStatementKind::For(f) => {
                out.extend(procedure_vars_aggregate(&f.block));
            }
            CSTStatementKind::While(w) | CSTStatementKind::DoWhile(w) => {
                out.extend(procedure_vars_aggregate(&w.block));
            }
            CSTStatementKind::TryCatch(t) => {
                for j in &t.catch_branches {
                    out.extend(procedure_vars_aggregate(&j.block));
                }
                out.extend(procedure_vars_aggregate(&t.try_branch));
            }
            CSTStatementKind::Check(c) => {
                out.extend(procedure_vars_aggregate(&c.block));
                out.extend(procedure_vars_aggregate(&c.after_backtrack));
            }
            CSTStatementKind::Assign(a) => {
                expr_vars_push(&a.assign, &mut out);
                let mut e = &*a.expr;
                while let CSTStatementKind::Assign(a) = &e.kind {
                    expr_vars_push(&a.assign, &mut out);
                    e = &*a.expr;
                }
            }
            _ => (),
        }
    }

    out
}

pub fn procedure_object_vars_aggregate(cst: &CSTBlock) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for i in cst {
        match &i.kind {
            CSTStatementKind::If(c) | CSTStatementKind::Switch(c) => {
                for j in &c.branches {
                    out.extend(procedure_object_vars_aggregate(&j.block));
                }

                if let Some(alt) = &c.alternative {
                    out.extend(procedure_object_vars_aggregate(alt));
                }
            }
            CSTStatementKind::Match(m) => {
                for j in &m.branches {
                    match j {
                        CSTMatchBranch::Case(c) => {
                            out.extend(procedure_object_vars_aggregate(&c.statements));
                        }
                        CSTMatchBranch::Regex(c) => {
                            out.extend(procedure_object_vars_aggregate(&c.statements));
                        }
                    }
                }
                out.extend(procedure_object_vars_aggregate(&m.default));
            }
            CSTStatementKind::Scan(s) => {
                for j in &s.branches {
                    match j {
                        CSTMatchBranch::Case(c) => {
                            out.extend(procedure_object_vars_aggregate(&c.statements));
                        }
                        CSTMatchBranch::Regex(c) => {
                            out.extend(procedure_object_vars_aggregate(&c.statements));
                        }
                    }
                }
            }
            CSTStatementKind::For(f) => {
                out.extend(procedure_object_vars_aggregate(&f.block));
            }
            CSTStatementKind::While(w) | CSTStatementKind::DoWhile(w) => {
                out.extend(procedure_object_vars_aggregate(&w.block));
            }
            CSTStatementKind::TryCatch(t) => {
                for j in &t.catch_branches {
                    out.extend(procedure_object_vars_aggregate(&j.block));
                }
                out.extend(procedure_object_vars_aggregate(&t.try_branch));
            }
            CSTStatementKind::Check(c) => {
                out.extend(procedure_object_vars_aggregate(&c.block));
                out.extend(procedure_object_vars_aggregate(&c.after_backtrack));
            }
            CSTStatementKind::Assign(a) => {
                if let CSTExpressionKind::Accessible(acc) = &a.assign.kind
                    && let CSTExpressionKind::Variable(v) = &acc.head.kind
                    && v.as_str() == "this"
                    && acc.body.len() == 1
                    && let CSTExpressionKind::Variable(v_fin) = &acc.body[0].kind
                {
                    out.push(v_fin.clone());
                }
            }
            _ => (),
        }
    }

    out
}

pub fn proc_params_push(
    start_idx: &mut NodeIndex,
    params: &[CSTParam],
    list_param: &Option<String>,
    is_closure: bool,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    for (idx, i) in params.iter().enumerate() {
        let tmp = tmp_var_new(proc);
        if i.is_rw {
            /* t_n := params[i];
             * _ := stack_alias("a", t_n);
             */
            block_get(proc, *start_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp),
                    types: IRType::PTR,
                    source: IRValue::BuiltinVar(BuiltinVar::Params),
                    op: IROp::AccessArray(IRValue::Number(
                        (idx + if is_closure { 1 } else { 0 }).into(),
                    )),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::StackAlias),
                    op: IROp::NativeCall(vec![
                        IRValue::String(i.name.clone()),
                        IRValue::Variable(tmp),
                        IRValue::Bool(false),
                    ]),
                }),
            ]);
            shared_proc.definitions.push((i.name.clone(), tmp));
        } else {
            /* t_1 := params[i];
             * t_2 := stack_add("a");
             * t_3 := *t_1;
             * *t_2 := copy(t_3);
             */
            let tmp_1 = tmp_var_new(proc);
            let tmp = tmp_var_new(proc);
            let t_3 = tmp_var_new(proc);

            block_get(proc, *start_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp_1),
                    types: IRType::PTR,
                    source: IRValue::BuiltinVar(BuiltinVar::Params),
                    op: IROp::AccessArray(IRValue::Number(
                        (idx + if is_closure { 1 } else { 0 }).into(),
                    )),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp),
                    types: IRType::PTR,
                    source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
                    op: IROp::NativeCall(vec![IRValue::String(i.name.clone())]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_3),
                    types: IRTypes!("any"),
                    source: IRValue::Variable(tmp_1),
                    op: IROp::PtrDeref,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Deref(tmp),
                    types: IRTypes!("any"),
                    source: IRValue::BuiltinProc(BuiltinProc::Copy),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_3)]),
                }),
            ]);
            shared_proc.definitions.push((i.name.clone(), tmp));
        }

        if let Some(def) = &i.default {
            /* t_1 := *t_n;
             * t_2 := t_1 == undefined;
             * if t_2
             *   <bb1>
             * else
             *   <bb2> // follow block
             * <bb1>:
             * t_3 = expr;
             * *t_n = t_3;
             * goto <bb2>;
             */
            let t_1 = tmp_var_new(proc);
            let t_2 = tmp_var_new(proc);

            let follow_idx = proc.blocks.add_node(Vec::new());
            let mut set_idx = proc.blocks.add_node(Vec::new());

            block_get(proc, *start_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_1),
                    types: IRTypes!("any"),
                    source: IRValue::Variable(tmp),
                    op: IROp::PtrDeref,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_2),
                    types: IRType::BOOL,
                    source: IRValue::Variable(t_1),
                    op: IROp::Equal(IRValue::Undefined),
                }),
                IRStmt::Branch(IRBranch {
                    cond: IRValue::Variable(t_2),
                    success: set_idx,
                    failure: follow_idx,
                }),
            ]);

            proc.blocks.add_edge(*start_idx, set_idx, ());
            proc.blocks.add_edge(*start_idx, follow_idx, ());

            block_expr_push(
                def,
                &mut set_idx,
                IRTarget::Deref(tmp),
                proc,
                shared_proc,
                cfg,
            );
            block_get(proc, set_idx).push(IRStmt::Goto(follow_idx));
            proc.blocks.add_edge(set_idx, follow_idx, ());

            *start_idx = follow_idx;
        }
    }

    if let Some(list_param_val) = list_param {
        /* //native call
         * t_0 := stack_add(list_param);
         * t_1 := slice(params, p.len(), -1);
         * *t_0 = copy(t_1);
         */
        let t_list_param = tmp_var_new(proc);
        let t_slice = tmp_var_new(proc);
        block_get(proc, *start_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_list_param),
                types: IRType::PTR,
                source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
                op: IROp::NativeCall(vec![IRValue::String(list_param_val.clone())]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_slice),
                types: IRType::LIST,
                source: IRValue::BuiltinProc(BuiltinProc::Slice),
                op: IROp::NativeCall(vec![
                    IRValue::BuiltinVar(BuiltinVar::Params),
                    IRValue::Number((params.len() + if is_closure { 1 } else { 0 }).into()),
                    IRValue::Number((-1_i8).into()),
                ]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Deref(t_list_param),
                types: IRType::LIST,
                source: IRValue::BuiltinProc(BuiltinProc::Copy),
                op: IROp::NativeCall(vec![IRValue::Variable(t_slice)]),
            }),
        ]);
    }
}

pub fn proc_vars_push(
    block_idx: &mut NodeIndex,
    cst: &CSTBlock,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
) {
    let obj_vars = procedure_object_vars_aggregate(cst);

    if !obj_vars.is_empty() {
        /*  t_exists := stack_exists("this");
         *  if t_exists
         *   goto <add_idx>
         *  else
         *   goto <follow_idx>
         *
         * <add_idx>:
         *  t_obj_addr := stack_get_or_new("this");
         *  t_obj := *t_obj_addr;
         *  // for i in obj_vars {
         *   t_var := object_add(t_obj, i);
         *   _ := stack_alias(i, t_var, true);
         *  // }
         *  goto <follow_idx>
         *
         * <follow_idx>:
         */
        let add_idx = proc.blocks.add_node(vec![]);
        let follow_idx = proc.blocks.add_node(vec![]);

        let t_exists = tmp_var_new(proc);
        let t_obj_addr = tmp_var_new(proc);
        let t_obj = tmp_var_new(proc);

        block_get(proc, *block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_exists),
                types: IRType::BOOL,
                source: IRValue::BuiltinProc(BuiltinProc::StackInScope),
                op: IROp::NativeCall(vec![IRValue::String(String::from("this"))]),
            }),
            IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_exists),
                success: add_idx,
                failure: follow_idx,
            }),
        ]);

        proc.blocks.add_edge(*block_idx, add_idx, ());
        proc.blocks.add_edge(*block_idx, follow_idx, ());

        block_get(proc, add_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_obj_addr),
                types: IRType::PTR,
                source: IRValue::BuiltinProc(BuiltinProc::StackGetOrNew),
                op: IROp::NativeCall(vec![IRValue::String(String::from("this"))]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_obj),
                types: IRTypes!("any"),
                source: IRValue::Variable(t_obj_addr),
                op: IROp::PtrDeref,
            }),
        ]);

        for i in obj_vars {
            let t_var = tmp_var_new(proc);
            block_get(proc, add_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_var),
                    types: IRType::PTR,
                    source: IRValue::BuiltinProc(BuiltinProc::ObjectAdd),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_obj), IRValue::String(i.clone())]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::StackAlias),
                    op: IROp::NativeCall(vec![IRValue::String(i.clone()), IRValue::Variable(t_var), IRValue::Bool(true)]),
                }),
            ]);

            shared_proc.definitions.push((i.clone(), t_var));
         }

        block_get(proc, add_idx).push(IRStmt::Goto(follow_idx));
        proc.blocks.add_edge(add_idx, follow_idx, ());

        *block_idx = follow_idx;
    }

    procedure_vars_aggregate(cst).iter().for_each(|i| {
        let t_var = tmp_var_new(proc);
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_var),
            types: IRType::PTR,
            source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
            op: IROp::NativeCall(vec![IRValue::String(i.clone())]),
        }));
        shared_proc.definitions.push((i.clone(), t_var));
    });
}

fn params_to_cache_key_push(block_idx: &mut NodeIndex, proc: &mut IRProcedure) -> IRVar {
    /*  t_iter := iter_new(params);
     *  t_out := list_new();
     *  goto <iter_idx>
     *
     * <iter_idx>:
     *  t_i := om;
     *  t_i_addr := &t_i;
     *  t_cond := iter_next(t_iter, t_i_addr);
     *  if t_cond
     *   goto <push_idx>
     *  else
     *   goto <follow_idx>
     *
     * <push_idx>:
     *  t_val := *t_i;
     *  t_val_insert := copy(t_val);
     *  _ := list_push(t_out, t_val_insert);
     *  goto <iter_idx>
     *
     * <follow_idx>:
     */
    let iter_idx = proc.blocks.add_node(Vec::new());
    let push_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    let t_iter = tmp_var_new(proc);
    let t_out = tmp_var_new(proc);
    let t_i = tmp_var_new(proc);
    let t_i_addr = tmp_var_new(proc);
    let t_cond = tmp_var_new(proc);
    let t_val = tmp_var_new(proc);
    let t_val_insert = tmp_var_new(proc);

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_iter),
            types: IRType::ITERATOR,
            source: IRValue::BuiltinProc(BuiltinProc::IterNew),
            op: IROp::NativeCall(vec![IRValue::BuiltinVar(BuiltinVar::Params)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(vec![]),
        }),
        IRStmt::Goto(iter_idx),
    ]);

    proc.blocks.add_edge(*block_idx, iter_idx, ());

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
            success: push_idx,
            failure: follow_idx,
        }),
    ]);

    proc.blocks.add_edge(iter_idx, push_idx, ());
    proc.blocks.add_edge(iter_idx, follow_idx, ());

    block_get(proc, push_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_val),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_i),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_val_insert),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_val)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ListPush),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_out),
                IRValue::Variable(t_val_insert),
            ]),
        }),
        IRStmt::Goto(iter_idx),
    ]);

    proc.blocks.add_edge(push_idx, iter_idx, ());

    *block_idx = follow_idx;

    t_out
}

/// Compiles a CST procedure body into an `IRProcedure` registered in the CFG.
///
/// Supported kinds:
/// - `Procedure` — standard procedure with a stack frame
/// - `Closure` — restores a captured stack image on entry
/// - `Cached` — wraps the body with a cache lookup at entry and insert at exit
///
/// Local variables are pre-allocated via `proc_vars_push`. Parameters are set
/// up via `proc_params_push`. If the body does not terminate with an explicit
/// return, a default `om` return is appended.
pub fn procedure_new(
    cst: &CSTBlock,
    kind: &CSTProcedureKind,
    params: &[CSTParam],
    list_param: &Option<String>,
    disable_annotations: bool,
    cfg: &mut IRCfg,
) -> Rc<RefCell<IRProcedure>> {
    let proc = Rc::new(RefCell::new(IRProcedure {
        blocks: StableGraph::new(),
        start_block: NodeIndex::from(0),
        end_block: NodeIndex::from(0),
        vars: Vec::new(),
        tag: String::from(""),
    }));

    let mut shared_proc = IRSharedProc {
        ret_var: tmp_var_new(&mut proc.borrow_mut()),
        disable_annotations,
        ..Default::default()
    };

    let ret_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::StackFramePop),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Return(IRValue::Variable(shared_proc.ret_var)),
    ]);

    let mut main_idx = proc.borrow_mut().blocks.add_node(vec![]);

    if matches!(kind, CSTProcedureKind::Closure) {
        /* t_stack_param := params[0];
         * t_stack := *t_stack_param;
         * _ := stack_frame_restore(t_stack);
         */
        let t_stack_param = tmp_var_new(&mut proc.borrow_mut());
        let t_stack = tmp_var_new(&mut proc.borrow_mut());

        block_get(&mut proc.borrow_mut(), main_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_stack_param),
                types: IRType::PTR,
                source: IRValue::BuiltinVar(BuiltinVar::Params),
                op: IROp::AccessArray(IRValue::Number(0.into())),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_stack),
                types: IRType::STACK_IMAGE,
                source: IRValue::Variable(t_stack_param),
                op: IROp::PtrDeref,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::StackFrameRestore),
                op: IROp::NativeCall(vec![IRValue::Variable(t_stack)]),
            }),
        ]);
    }

    let start_idx = if matches!(kind, CSTProcedureKind::Cached) {
        let t_ret_addr = tmp_var_new(&mut proc.borrow_mut());
        let t_lookup_res = tmp_var_new(&mut proc.borrow_mut());

        let s_idx = proc.borrow_mut().blocks.add_node(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::StackFrameAdd),
                op: IROp::NativeCall(Vec::new()),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_ret_addr),
                types: IRTypes!("any"),
                source: IRValue::Variable(shared_proc.ret_var),
                op: IROp::PtrAddress,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_lookup_res),
                types: IRType::BOOL,
                source: IRValue::BuiltinProc(BuiltinProc::CacheLookup),
                op: IROp::NativeCall(vec![
                    IRValue::Procedure(proc.clone()),
                    IRValue::BuiltinVar(BuiltinVar::Params),
                    IRValue::Variable(t_ret_addr),
                ]),
            }),
            IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_lookup_res),
                success: ret_idx,
                failure: main_idx,
            }),
        ]);

        proc.borrow_mut().blocks.add_edge(s_idx, ret_idx, ());
        proc.borrow_mut().blocks.add_edge(s_idx, main_idx, ());

        s_idx
    } else {
        let idx = proc.borrow_mut().blocks.add_node(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::StackFrameAdd),
                op: IROp::NativeCall(Vec::new()),
            }),
            IRStmt::Goto(main_idx),
        ]);
        proc.borrow_mut().blocks.add_edge(idx, main_idx, ());
        idx
    };

    let t_cache = match kind {
        CSTProcedureKind::Cached => Some(params_to_cache_key_push(
            &mut main_idx,
            &mut proc.borrow_mut(),
        )),
        _ => None,
    };

    let end_idx = if matches!(kind, CSTProcedureKind::Cached) {
        let t_ret = tmp_var_new(&mut proc.borrow_mut());
        let idx = proc.borrow_mut().blocks.add_node(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_ret),
                types: IRTypes!("any"),
                source: IRValue::BuiltinProc(BuiltinProc::Copy),
                op: IROp::NativeCall(vec![IRValue::Variable(shared_proc.ret_var)]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::CacheAdd),
                op: IROp::NativeCall(vec![
                    IRValue::Procedure(proc.clone()),
                    IRValue::Variable(t_cache.unwrap()),
                    IRValue::Variable(t_ret),
                ]),
            }),
            IRStmt::Goto(ret_idx),
        ]);
        proc.borrow_mut().blocks.add_edge(idx, ret_idx, ());
        idx
    } else {
        ret_idx
    };

    proc_vars_push(&mut main_idx, cst, &mut proc.borrow_mut(), &mut shared_proc);

    proc_params_push(
        &mut main_idx,
        params,
        list_param,
        matches!(kind, CSTProcedureKind::Closure),
        &mut proc.borrow_mut(),
        &mut shared_proc,
        cfg,
    );

    let terminated = block_populate(
        &mut main_idx,
        cst,
        None,
        None,
        end_idx,
        &mut proc.borrow_mut(),
        &mut shared_proc,
        cfg,
    );

    if !terminated {
        let next_idx = proc.borrow_mut().blocks.add_node(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(shared_proc.ret_var),
                types: IRType::UNDEFINED,
                source: IRValue::Undefined,
                op: IROp::Assign,
            }),
            IRStmt::Goto(end_idx),
        ]);
        proc.borrow_mut().blocks.add_edge(next_idx, end_idx, ());

        block_get(&mut proc.borrow_mut(), main_idx).push(IRStmt::Goto(next_idx));
        proc.borrow_mut().blocks.add_edge(main_idx, next_idx, ());
    }

    proc.borrow_mut().start_block = start_idx;
    proc.borrow_mut().end_block = ret_idx;

    let idx = cfg.procedures.add_node(proc.clone());
    proc.borrow_mut().tag = format!("proc{}", idx.index());
    proc
}
