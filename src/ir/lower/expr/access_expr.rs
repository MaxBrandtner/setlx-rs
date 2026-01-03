use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::proc::call_params_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_access_ref_push(
    a: &CSTAccessible,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) -> Option<IRVar> /* owned referenced */ {
    let t_head_var = tmp_var_new(proc);
    let head_owned = block_expr_push(
        &a.head,
        block_idx,
        IRTarget::Variable(t_head_var),
        proc,
        shared_proc,
        cfg,
    );

    let t_owned = if head_owned {
        Some(t_head_var)
    } else {
        None
    };

    let t_head = tmp_var_new(proc);
    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_head),
        types: IRType::PTR,
        source: IRValue::Variable(t_head_var),
        op: IROp::PtrAddress,
    }));

    for i in &a.body {
        let mut t_owned_new = None;

        match i {
            CSTExpression::Variable(v) => {
                // t_head := object_get_assert(t_head, v);
                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_head),
                    types: IRType::PTR,
                    source: IRValue::BuiltinProc(BuiltinProc::ObjectGetAssert),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(t_head),
                        IRValue::String(v.to_string()),
                    ]),
                }));
            }
            CSTExpression::Call(c) => {
                /* t_params := // call_params_push
                 * t_proc_addr := object_get_assert(t_head, c.name);
                 * t_proc := *t_proc_addr;
                 * t_var := t_proc(t_params);
                 * _ := invalidate(t_params);
                 * t_head := &t_var
                 */
                let t_params = tmp_var_new(proc);
                let inv_vars = call_params_push(c, block_idx, target, proc, shared_proc, cfg);

                let t_proc_addr = tmp_var_new(proc);
                let t_proc = tmp_var_new(proc);
                let t_var = tmp_var_new(proc);

                block_get(proc, *block_idx).extend(vec![
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_proc_addr),
                        types: IRType::PTR,
                        source: IRValue::BuiltinProc(BuiltinProc::ObjectGetAssert),
                        op: IROp::NativeCall(vec![
                            IRValue::Variable(t_head),
                            IRValue::String(c.name.to_string()),
                        ]),
                    }),
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_proc),
                        types: IRTypes!("any"),
                        source: IRValue::Variable(t_proc_addr),
                        op: IROp::PtrDeref,
                    }),
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_var),
                        types: IRTypes!("any"),
                        source: IRValue::Variable(t_proc),
                        op: IROp::Call(t_params),
                    }),
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Ignore,
                        types: IRType::UNDEFINED,
                        source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                        op: IROp::NativeCall(vec![IRValue::Variable(t_params)]),
                    }),
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_head),
                        types: IRType::PTR,
                        source: IRValue::Variable(t_var),
                        op: IROp::PtrAddress,
                    }),
                ]);

                inv_vars.iter().for_each(|i| {
                    // _ := invalidate(i);
                    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                        target: IRTarget::Ignore,
                        types: IRType::UNDEFINED,
                        source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                        op: IROp::NativeCall(vec![IRValue::Variable(*i)]),
                    }));
                });

                t_owned_new = Some(t_var);
            }
            CSTExpression::Collection(c) => match c {
                CSTCollection::Set(s) => {
                    /* t_var := //expr
                     * t_val := list_tagged_get(t_head, t_var)
                     * t_val := copy(t_val);
                     * t_head := &t_val;
                     * _ := invalidate(t_var);
                     */
                    let t_var = tmp_var_new(proc);
                    let t_val = tmp_var_new(proc);

                    let var_owned = block_expr_push(
                        &s.expressions[0],
                        block_idx,
                        IRTarget::Variable(t_var),
                        proc,
                        shared_proc,
                        cfg,
                    );

                    block_get(proc, *block_idx).extend(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_val),
                            types: IRTypes!("any"),
                            source: IRValue::BuiltinProc(BuiltinProc::ListTaggedGet),
                            op: IROp::NativeCall(vec![
                                IRValue::Variable(t_head),
                                IRValue::Variable(t_var),
                            ]),
                        }),
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_val),
                            types: IRTypes!("any"),
                            source: IRValue::BuiltinProc(BuiltinProc::Copy),
                            op: IROp::NativeCall(vec![IRValue::Variable(t_val)]),
                        }),
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_head),
                            types: IRType::PTR,
                            source: IRValue::Variable(t_val),
                            op: IROp::PtrAddress,
                        }),
                    ]);

                    if var_owned {
                        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                            target: IRTarget::Ignore,
                            types: IRType::UNDEFINED,
                            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                            op: IROp::NativeCall(vec![IRValue::Variable(t_var)]),
                        }));
                    }

                    t_owned_new = Some(t_val);
                }
                CSTCollection::List(l) => {
                    if let Some(range) = &l.range {
                        /* t_left = //expr
                         * t_right = //expr
                         * t_out = slice(t_head, t_left, t_right);
                         * _ := invalidate(t_left);
                         * _ := invalidate(t_right);
                         * t_head := &t_out;
                         */
                        let t_left = tmp_var_new(proc);
                        let t_right = tmp_var_new(proc);

                        if let Some(l_expr) = &range.left {
                            block_expr_push(
                                l_expr,
                                block_idx,
                                IRTarget::Variable(t_left),
                                proc,
                                shared_proc,
                                cfg,
                            );
                        } else {
                            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_left),
                                types: IRType::NUMBER,
                                source: IRValue::Number(0.into()),
                                op: IROp::Assign,
                            }));
                        }

                        if let Some(r_expr) = &range.right {
                            block_expr_push(
                                r_expr,
                                block_idx,
                                IRTarget::Variable(t_right),
                                proc,
                                shared_proc,
                                cfg,
                            );
                        } else {
                            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_right),
                                types: IRType::NUMBER,
                                source: IRValue::Number((-1).into()),
                                op: IROp::Assign,
                            }));
                        }

                        let t_out = tmp_var_new(proc);

                        block_get(proc, *block_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_out),
                                types: IRTypes!("any"),
                                source: IRValue::BuiltinProc(BuiltinProc::Slice),
                                op: IROp::NativeCall(vec![
                                    IRValue::Variable(t_head),
                                    IRValue::Variable(t_left),
                                    IRValue::Variable(t_right),
                                ]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Ignore,
                                types: IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                op: IROp::NativeCall(vec![IRValue::Variable(t_left)]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Ignore,
                                types: IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                op: IROp::NativeCall(vec![IRValue::Variable(t_right)]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head),
                                types: IRType::PTR,
                                source: IRValue::Variable(t_out),
                                op: IROp::PtrAddress,
                            }),
                        ]);

                        t_owned_new = t_owned;
                    } else {
                        /* t_expr = //expr
                         * t_head = t_head[t_expr];
                         * _ := invalidate(t_expr);
                         */
                        let t_expr = tmp_var_new(proc);
                        block_expr_push(
                            &l.expressions[0],
                            block_idx,
                            IRTarget::Variable(t_expr),
                            proc,
                            shared_proc,
                            cfg,
                        );

                        block_get(proc, *block_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_head),
                            types: IRTypes!("any"),
                            source: IRValue::Variable(t_head),
                            op: IROp::AccessArray(IRValue::Variable(t_expr)),
                        }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Ignore,
                                types: IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
                            }),
                        ]);

                        t_owned_new = t_owned;
                    }
                }
                _ => panic!("accessible collection should be list or set"),
            },
            _ => unreachable!(),
        }

        if let Some(t_o) = t_owned  && t_owned_new != t_owned {
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_o)]),
            }));
        }
    }

    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::PTR,
        source: IRValue::Variable(t_head),
        op: IROp::Assign,
    }));

    t_owned
}

pub fn block_access_push(
    a: &CSTAccessible,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) -> bool {
    /* tmp := // access ref
     * target := *tmp;
     */
    let tmp = tmp_var_new(proc);
    let t_owned = block_access_ref_push(
        a,
        block_idx,
        IRTarget::Variable(tmp),
        proc,
        shared_proc,
        cfg,
    );

    if let Some(t_o) = t_owned {
        let tmp_val = tmp_var_new(proc);
        block_get(proc, *block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(tmp_val),
                types: IRTypes!("any"),
                source: IRValue::Variable(tmp),
                op: IROp::PtrDeref,
            }),
            IRStmt::Assign(IRAssign {
                target,
                types: IRTypes!("any"),
                source: IRValue::BuiltinProc(BuiltinProc::Copy),
                op: IROp::NativeCall(vec![IRValue::Variable(tmp_val)]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_o)]),
            }),
        ]);
        true
    } else {
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target,
            types: IRTypes!("any"),
            source: IRValue::Variable(tmp),
            op: IROp::PtrDeref,
        }));
        false
    }
}
