use petgraph::stable_graph::{NodeIndex, StableGraph};

use crate::ast::*;
use crate::builtin::{BuiltinProc, BuiltinVar};
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::access_expr::block_access_ref_push;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::stmt::block_populate;
use crate::ir::lower::util::{block_get, stack_get, tmp_var_new};

pub fn call_params_push(
    c: &CSTProcedureCall,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) -> Vec<IRVar> {
    let mut out_vars: Vec<IRVar> = Vec::new();

    let t_params = match target {
        IRTarget::Variable(v) => v,
        _ => unreachable!(),
    };

    // target := list_new(c.params.len());
    let t_params_addr = tmp_var_new(proc);
    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::LIST,
        source: IRValue::BuiltinProc(BuiltinProc::ListNew),
        op: IROp::NativeCall(vec![IRValue::Number(c.params.len().into())]),
    }));
    for i in &c.params {
        let tmp_addr = tmp_var_new(proc);
        if let CSTExpression::Variable(v) = i {
            if let Some(tmp) = stack_get(shared_proc, v) {
                // t_a := tmp;
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp_addr),
                    types: IRType::PTR,
                    source: IRValue::Variable(tmp),
                    op: IROp::Assign,
                }));
            } else if let CSTExpression::Accessible(a) = i {
                // t_a := // access ref
                block_access_ref_push(
                    a,
                    block_idx,
                    IRTarget::Variable(tmp_addr),
                    proc,
                    shared_proc,
                    cfg,
                );
            } else {
                // t_a := stack_get_or_new(v);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp_addr),
                    types: IRType::PTR,
                    source: IRValue::BuiltinProc(BuiltinProc::StackGetOrNew),
                    op: IROp::NativeCall(vec![IRValue::String(v.to_string())]),
                }));
            }
        } else {
            //t_n := expr;
            let tmp = tmp_var_new(proc);
            out_vars.push(tmp);
            block_expr_push(
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
        }
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ListPush),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_params),
                IRValue::Variable(tmp_addr),
            ]),
        }));
    }
    if let Some(rest) = &c.rest_param {
        /* t_rest := //expr
         * _ := type_assert(t_rest, IRType::List);
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
        out_vars.push(t_rest);
        block_expr_push(
            rest,
            block_idx,
            IRTarget::Variable(t_rest),
            proc,
            shared_proc,
            cfg,
        );

        let len_check_idx = proc.blocks.add_node(Vec::new());
        let loop_idx = proc.blocks.add_node(Vec::new());
        let follow_idx = proc.blocks.add_node(Vec::new());

        let t_rest_len = tmp_var_new(proc);
        let t_params_len = tmp_var_new(proc);
        let t_i = tmp_var_new(proc);

        block_get(proc, *block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::TypeAssert),
                op: IROp::NativeCall(vec![IRValue::Variable(t_rest)]),
            }),
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

pub fn expr_vars_push(e: &CSTExpression, out: &mut Vec<String>) {
    match e {
        CSTExpression::Term(t) => {
            t.params.iter().for_each(|i| expr_vars_push(i, out));
        }
        CSTExpression::Variable(v) => {
            if !out.contains(v) {
                out.push(v.to_string());
            }
        }
        CSTExpression::Collection(c) => match c {
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
        CSTExpression::Op(o) => {
            expr_vars_push(&o.left, out);
            expr_vars_push(&o.right, out);
        }
        CSTExpression::UnaryOp(o) => {
            expr_vars_push(&o.expr, out);
        }
        CSTExpression::Call(c) => {
            out.push(c.name.to_string());
            c.params.iter().for_each(|i| expr_vars_push(i, out));
            if let Some(rest_param) = &c.rest_param {
                expr_vars_push(rest_param, out);
            }
        }
        _ => (),
    }
}

fn procedure_vars_aggregate(cst: &CSTBlock) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for i in cst {
        match i {
            CSTStatement::If(c) | CSTStatement::Switch(c) => {
                for j in &c.branches {
                    out.extend(procedure_vars_aggregate(&j.block));
                }

                if let Some(alt) = &c.alternative {
                    out.extend(procedure_vars_aggregate(alt));
                }
            }
            CSTStatement::Match(m) => {
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
            CSTStatement::Scan(s) => {
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
            CSTStatement::For(f) => {
                out.extend(procedure_vars_aggregate(&f.block));
            }
            CSTStatement::While(w) | CSTStatement::DoWhile(w) => {
                out.extend(procedure_vars_aggregate(&w.block));
            }
            CSTStatement::TryCatch(t) => {
                for j in &t.catch_branches {
                    out.extend(procedure_vars_aggregate(&j.block));
                }
                out.extend(procedure_vars_aggregate(&t.try_branch));
            }
            CSTStatement::Check(c) => {
                out.extend(procedure_vars_aggregate(&c.block));
                out.extend(procedure_vars_aggregate(&c.after_backtrack));
            }
            CSTStatement::Assign(a) => expr_vars_push(&a.assign, &mut out),
            _ => (),
        }
    }

    out
}

pub fn proc_params_push(
    start_idx: &mut NodeIndex,
    params: &[CSTParam],
    list_param: &Option<String>,
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
                    op: IROp::AccessArray(IRValue::Number(idx.into())),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::StackAlias),
                    op: IROp::NativeCall(vec![
                        IRValue::String(i.name.clone()),
                        IRValue::Variable(tmp),
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
                    op: IROp::AccessArray(IRValue::Number(idx.into())),
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
                    IRValue::Number(params.len().into()),
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

pub fn procedure_new(
    cst: &CSTBlock,
    kind: &CSTProcedureKind,
    params: &[CSTParam],
    list_param: &Option<String>,
    cfg: &mut IRCfg,
) -> NodeIndex {
    let mut proc = IRProcedure {
        blocks: StableGraph::new(),
        start_block: NodeIndex::from(0),
        end_block: NodeIndex::from(0),
        vars: Vec::new(),
    };

    let mut shared_proc = IRSharedProc {
        definitions: Vec::new(),
        ret_var: 0,
    };

    shared_proc.ret_var = tmp_var_new(&mut proc);
    let ret_idx = proc.blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::StackFramePop),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Return(IRValue::Variable(shared_proc.ret_var)),
    ]);

    let mut main_idx = proc.blocks.add_node(vec![IRStmt::Assign(IRAssign {
        target: IRTarget::Ignore,
        types: IRType::UNDEFINED,
        source: IRValue::BuiltinProc(BuiltinProc::StackFrameAdd),
        op: IROp::NativeCall(Vec::new()),
    })]);

    if matches!(kind, CSTProcedureKind::Closure) {
        /* v1 := params[0];
         * _ := stack_frame_restore(v1);
         */
        let t_stack_param = tmp_var_new(&mut proc);

        block_get(&mut proc, main_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_stack_param),
                types: IRType::LIST,
                source: IRValue::BuiltinVar(BuiltinVar::Params),
                op: IROp::AccessArray(IRValue::Number(0.into())),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::StackFrameRestore),
                op: IROp::NativeCall(vec![IRValue::Variable(t_stack_param)]),
            }),
        ]);
    }

    let cached_idx = cfg.n_cached;
    if matches!(kind, CSTProcedureKind::Cached) {
        cfg.n_cached += 1;
    }

    let end_idx = if matches!(kind, CSTProcedureKind::Cached) {
        proc.blocks.add_node(vec![IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::CacheAdd),
            op: IROp::NativeCall(vec![
                IRValue::Number(cached_idx.into()),
                IRValue::Variable(shared_proc.ret_var),
            ]),
        })])
    } else {
        ret_idx
    };

    let start_idx = if matches!(kind, CSTProcedureKind::Cached) {
        let t_ret_addr = tmp_var_new(&mut proc);
        let t_lookup_res = tmp_var_new(&mut proc);

        let s_idx = proc.blocks.add_node(vec![
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
                    IRValue::Number(cached_idx.into()),
                    IRValue::BuiltinVar(BuiltinVar::Params),
                    IRValue::Variable(t_ret_addr),
                ]),
            }),
            IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_lookup_res),
                success: end_idx,
                failure: main_idx,
            }),
        ]);

        proc.blocks.add_edge(s_idx, end_idx, ());
        proc.blocks.add_edge(s_idx, main_idx, ());

        s_idx
    } else {
        main_idx
    };

    proc_params_push(
        &mut main_idx,
        params,
        list_param,
        &mut proc,
        &mut shared_proc,
        cfg,
    );

    procedure_vars_aggregate(cst).iter().for_each(|i| {
        let t_var = tmp_var_new(&mut proc);
        block_get(&mut proc, main_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_var),
            types: IRType::PTR,
            source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
            op: IROp::NativeCall(vec![IRValue::String(i.clone())]),
        }));
        shared_proc.definitions.push((i.clone(), t_var));
    });

    let terminated = block_populate(
        &mut main_idx,
        cst,
        None,
        None,
        end_idx,
        &mut proc,
        &mut shared_proc,
        cfg,
    );

    if !terminated {
        let next_idx = proc.blocks.add_node(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(shared_proc.ret_var),
                types: IRType::UNDEFINED,
                source: IRValue::Undefined,
                op: IROp::Assign,
            }),
            IRStmt::Goto(end_idx),
        ]);
        proc.blocks.add_edge(next_idx, end_idx, ());

        block_get(&mut proc, main_idx).push(IRStmt::Goto(next_idx));
        proc.blocks.add_edge(main_idx, next_idx, ());
    }

    proc.start_block = start_idx;
    proc.end_block = ret_idx;

    cfg.procedures.add_node(proc)
}
