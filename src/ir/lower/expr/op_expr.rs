use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::access_expr::block_obj_call_impl_push;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::expr::set_mem_ops::{
    block_prod_mem_push, block_set_mem_op_dfl_push, block_sum_mem_push,
};
use crate::ir::lower::expr::term_expr::{block_term_op_push, block_term_type_check_push};
use crate::ir::lower::util::{block_get, tmp_var_new};

fn set_eq_push(
    block_idx: &mut NodeIndex,
    eq: bool,
    tmp_left: IRVar,
    tmp_left_owned: bool,
    tmp_right: IRVar,
    tmp_right_owned: bool,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /*  t_lhs_cond := // term_type_push
     *  t_rhs_cond := // term_type_push
     *  t_cond := t_lhs_cond || t_rhs_cond;
     *  if t_cond
     *   goto <ast_idx>;
     *  else
     *   goto <eq_idx>
     *
     * <ast_idx>:
     *  // if !tmp_left_owned {
     *   tmp_left := copy(tmp_left);
     *  // }
     *  // if !tmp_right_owned {
     *   tmp_right := copy(tmp_right);
     *  // }
     *  target := ast_node_new("setEq", tmp_left, tmp_right);
     *  goto <follow_idx>
     *
     * <eq_idx>:
     *  // block_obj_overload_push
     *
     * <dfl_eq_idx>:
     *  tmp := tmp_left == tmp_right;
     *  goto <follow_eq_idx>
     *
     * <follow_eq_idx>:
     *  // block_obj_overload_push
     *
     * <dfl_not_idx>:
     *  target := !tmp;
     *  goto <follow_not_idx>
     *
     * <follow_not_idx>:
     *  // if tmp_left_owned {
     *   _ := invalidate(tmp_left);
     *  // }
     *  // if tmp_right_owned {
     *   _ := invalidate(tmp_right);
     *  // }
     *  goto <follow_idx>
     *
     * <follow_idx>:
     */
    let t_lhs_cond = tmp_var_new(proc);
    let t_rhs_cond = tmp_var_new(proc);
    let t_cond = tmp_var_new(proc);
    let tmp = if !eq { Some(tmp_var_new(proc)) } else { None };

    let ast_idx = proc.blocks.add_node(Vec::new());
    let eq_idx = proc.blocks.add_node(Vec::new());
    let dfl_eq_idx = proc.blocks.add_node(Vec::new());
    let follow_eq_idx = proc.blocks.add_node(Vec::new());
    let dfl_not_idx = if !eq {
        Some(proc.blocks.add_node(Vec::new()))
    } else {
        None
    };
    let follow_not_idx = if !eq {
        proc.blocks.add_node(Vec::new())
    } else {
        follow_eq_idx
    };
    let follow_idx = proc.blocks.add_node(Vec::new());

    block_term_type_check_push(*block_idx, tmp_left, t_lhs_cond, proc);
    block_term_type_check_push(*block_idx, tmp_right, t_rhs_cond, proc);

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cond),
            types: IRType::BOOL,
            source: IRValue::Variable(t_lhs_cond),
            op: IROp::Or(IRValue::Variable(t_rhs_cond)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cond),
            success: ast_idx,
            failure: eq_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, ast_idx, ());
    proc.blocks.add_edge(*block_idx, eq_idx, ());

    if !tmp_left_owned {
        block_get(proc, ast_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(tmp_left),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_left)]),
        }));
    }

    if !tmp_right_owned {
        block_get(proc, ast_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(tmp_right),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_right)]),
        }));
    }

    block_get(proc, ast_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target,
            types: IRType::AST,
            source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
            op: IROp::NativeCall(vec![
                IRValue::String(if eq {
                    String::from("setEq")
                } else {
                    String::from("setNeq")
                }),
                IRValue::Variable(tmp_left),
                IRValue::Variable(tmp_right),
            ]),
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(ast_idx, follow_idx, ());

    block_obj_overload_push(
        eq_idx,
        dfl_eq_idx,
        follow_eq_idx,
        true,
        if !eq {
            IRTarget::Variable(tmp.unwrap())
        } else {
            target
        },
        tmp_left,
        ObjOverloadRhs::Var(tmp_right),
        "equals",
        proc,
        shared_proc,
        cfg,
    );

    block_get(proc, dfl_eq_idx).extend(vec![
        if !eq {
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(tmp.unwrap()),
                types: IRType::BOOL,
                source: IRValue::Variable(tmp_left),
                op: IROp::Equal(IRValue::Variable(tmp_right)),
            })
        } else {
            IRStmt::Assign(IRAssign {
                target,
                types: IRType::BOOL,
                source: IRValue::Variable(tmp_left),
                op: IROp::Equal(IRValue::Variable(tmp_right)),
            })
        },
        IRStmt::Goto(follow_eq_idx),
    ]);

    proc.blocks.add_edge(dfl_eq_idx, follow_eq_idx, ());

    if !eq {
        block_obj_overload_push(
            follow_eq_idx,
            dfl_not_idx.unwrap(),
            follow_not_idx,
            false,
            target,
            tmp.unwrap(),
            ObjOverloadRhs::None,
            "not",
            proc,
            shared_proc,
            cfg,
        );

        block_get(proc, dfl_not_idx.unwrap()).extend(vec![
            IRStmt::Assign(IRAssign {
                target,
                types: IRType::BOOL,
                source: IRValue::Variable(tmp.unwrap()),
                op: IROp::Not,
            }),
            IRStmt::Goto(follow_not_idx),
        ]);

        proc.blocks
            .add_edge(dfl_not_idx.unwrap(), follow_not_idx, ());
    }

    if tmp_left_owned {
        block_get(proc, follow_not_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_left)]),
        }));
    }

    if tmp_right_owned {
        block_get(proc, follow_not_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_right)]),
        }));
    }

    block_get(proc, follow_not_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(follow_not_idx, follow_idx, ());

    *block_idx = follow_idx;
}

pub enum ObjOverloadRhs<'a> {
    Expression(&'a CSTExpression),
    Var(IRVar),
    None,
}

/// Emits IR for object operator overloading, appending to `init_idx`.
///
/// If `t_lhs` is an object with a procedure member named `op`, calls it as a
/// method with `rhs` as the argument and jumps to `follow_idx`. If the member
/// is absent or not a procedure, jumps to `dfl_idx` when `dfl_on_nomem` is
/// true, otherwise throws an exception.
///
/// # Arguments
///
/// * `init_idx` - block to append the type check and dispatch to
/// * `dfl_idx` - block to jump to when overload is absent and `dfl_on_nomem` is true
/// * `follow_idx` - block to jump to after a successful overloaded call
/// * `dfl_on_nomem` - if true, fall through to `dfl_idx` on a missing member instead of throwing
/// * `t_lhs` - the left-hand side value to check for object type and member lookup
/// * `rhs` - the right-hand side argument passed to the overload procedure
/// * `op` - the member name to look up on the object
pub fn block_obj_overload_push(
    init_idx: NodeIndex,
    dfl_idx: NodeIndex,
    follow_idx: NodeIndex,
    dfl_on_nomem: bool,
    target: IRTarget,
    t_lhs: IRVar,
    rhs: ObjOverloadRhs,
    op: &str,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /* <init_idx>:
     *  t_lhs_type := type_of(t_lhs);
     *  t_lhs_obj := t_lhs_type == TYPE_OBJECT;
     *  if t_lhs_obj
     *   goto <obj_check_idx>
     *  else
     *   goto <dfl_idx>
     *
     * <obj_check_idx>:
     *  t_p := object_get(t_lhs, op);
     *  t_p_om := t_p == om;
     *  if t_p_om
     *   goto <fail_idx>
     *  else
     *   goto <obj_check_type_idx>
     *
     * <obj_check_type_idx>:
     *  t_proc := *t_p;
     *  t_proc_type := type_of(t_proc);
     *  t_proc_proc := t_proc_type == TYPE_PROCEDURE;
     *  if t_proc_proc
     *   goto <obj_call_idx>
     *  else
     *   goto <fail_idx>
     *
     * <obj_call_idx>:
     *  // match rhs {
     *   // ObjOverloadRhs::Expression(e) => {
     *    t_rhs := // block_expr_push
     *    t_params := list_new();
     *    t_rhs_addr := &t_rhs;
     *    _ := list_push(t_params);
     *    // block_obj_call_impl_push
     *    // if rhs_owned {
     *     _ := invalidate(t_rhs);
     *    // }
     *   // }
     *   // ObjOverloadRhs::Var(t_rhs) => {
     *    t_params := list_new();
     *    t_rhs_addr := &t_rhs;
     *    _ := list_push(t_params);
     *    // block_obj_call_impl_push
     *   // }
     *  // }
     *  goto <follow_idx>
     *
     * <fail_idx>:
     *  _ := exception_throw("op", "{op} is not implemented for object");
     *  unreachable;
     */
    let t_lhs_type = tmp_var_new(proc);
    let t_lhs_obj = tmp_var_new(proc);
    let t_lhs_addr = tmp_var_new(proc);
    let t_rhs_addr = tmp_var_new(proc);
    let t_p = tmp_var_new(proc);
    let t_p_om = tmp_var_new(proc);
    let t_proc = tmp_var_new(proc);
    let t_proc_type = tmp_var_new(proc);
    let t_proc_proc = tmp_var_new(proc);
    let t_params = tmp_var_new(proc);

    let obj_check_idx = proc.blocks.add_node(Vec::new());
    let obj_check_type_idx = proc.blocks.add_node(Vec::new());
    let mut obj_call_idx = proc.blocks.add_node(Vec::new());
    let fail_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_lhs_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_lhs)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_lhs_obj),
            types: IRType::BOOL,
            source: IRValue::Variable(t_lhs_type),
            op: IROp::Equal(IRValue::Type(IRType::OBJECT)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_lhs_obj),
            success: obj_check_idx,
            failure: dfl_idx,
        }),
    ]);

    block_get(proc, obj_check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p),
            types: IRType::PTR | IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ObjectGet),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_lhs),
                IRValue::String(op.to_string()),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p_om),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p),
            op: IROp::Equal(IRValue::Undefined),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p_om),
            success: fail_idx,
            failure: obj_check_type_idx,
        }),
    ]);

    proc.blocks.add_edge(obj_check_idx, fail_idx, ());
    proc.blocks.add_edge(obj_check_type_idx, fail_idx, ());

    if dfl_on_nomem {
        block_get(proc, fail_idx).push(IRStmt::Goto(dfl_idx));
        proc.blocks.add_edge(fail_idx, dfl_idx, ());
    } else {
        block_get(proc, fail_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
                op: IROp::NativeCall(vec![
                    IRValue::String("operator overloading".to_string()),
                    IRValue::String(format!("{op} is not implemented for object")),
                ]),
            }),
            IRStmt::Unreachable,
        ]);
    }

    block_get(proc, obj_check_type_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_proc),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_proc_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_proc)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_proc_proc),
            types: IRType::BOOL,
            source: IRValue::Variable(t_proc_type),
            op: IROp::Equal(IRValue::Type(IRType::PROCEDURE)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_proc_proc),
            success: obj_call_idx,
            failure: fail_idx,
        }),
    ]);

    proc.blocks.add_edge(obj_check_type_idx, obj_call_idx, ());
    proc.blocks.add_edge(obj_check_type_idx, fail_idx, ());

    let (rhs_owned, t_rhs) = match rhs {
        ObjOverloadRhs::Expression(e) => {
            let t_rhs = tmp_var_new(proc);
            let owned = block_expr_push(
                e,
                &mut obj_call_idx,
                IRTarget::Variable(t_rhs),
                proc,
                shared_proc,
                cfg,
            );
            (owned, Some(t_rhs))
        }
        ObjOverloadRhs::Var(i) => (false, Some(i)),
        ObjOverloadRhs::None => (false, None),
    };

    block_get(proc, obj_call_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_lhs_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_lhs),
            op: IROp::PtrAddress,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_params),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(Vec::new()),
        }),
    ]);

    if let Some(t_rhs) = t_rhs {
        block_get(proc, obj_call_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_rhs_addr),
                types: IRType::PTR,
                source: IRValue::Variable(t_rhs),
                op: IROp::PtrAddress,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::ListPush),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(t_params),
                    IRValue::Variable(t_rhs_addr),
                ]),
            }),
        ]);
    }

    block_obj_call_impl_push(
        &mut obj_call_idx,
        t_lhs_addr,
        t_proc,
        t_params,
        target,
        proc,
    );

    if rhs_owned && let Some(t_rhs) = t_rhs {
        block_get(proc, obj_call_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_rhs)]),
        }));
    }

    block_get(proc, obj_call_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(obj_call_idx, follow_idx, ());
}

pub fn block_op_rhs_deferred_push<F, FRhs>(
    block_idx: &mut NodeIndex,
    tmp_left: IRVar,
    tmp_left_owned: bool,
    rhs: &CSTExpression,
    fallthrough_success: bool,
    lhs_check: F,
    rhs_assign: FRhs,
    target: IRTarget,
    op: &str,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) where
    F: Fn(&mut NodeIndex, IRVar /* tmp_left */, IRVar /* target */, &mut IRProcedure),
    FRhs: Fn(
        &mut NodeIndex,
        IRVar, /* tmp_left */
        IRVar, /* tmp_right */
        IRTarget,
        &mut IRProcedure,
    ),
{
    /* // block_obj_overload_push
     *
     * <dfl_impl_idx>:
     *  t_lhs_t := // block_term_type_check_push
     *  if t_lhs_t
     *   goto <rhs_idx>
     *  else
     *   goto <not_idx>
     *
     * <not_idx>:
     *  t_1 := // lhs_check
     *  // if fallthrough_success {
     *   if t_1
     *    goto <fallthrough_idx>
     *   else
     *    goto <rhs_idx>
     *  // } else {
     *   if t_1
     *    goto <rhs_idx>
     *   else
     *    goto <fallthrough_idx>
     *  // }
     *
     * <fallthrough_idx>:
     *  target := fallthrough_success;
     *  // if tmp_left_owned {
     *   _ := invalidate(tmp_left);
     *  //}
     *  goto <follow_idx>
     *
     * <rhs_idx>:
     *  tmp_right = // block_expr_push
     *  t_rhs_t := // block_term_type_check_push
     *  t_rhs_t := t_lhs_t || t_rhs_t;
     *  if t_rhs_t
     *   goto <build_term_idx>
     *  else
     *   goto <assign_idx>
     *
     * <build_term_idx>:
     *  // if !tmp_left_owned {
     *   tmp_left := copy(tmp_left);
     *  // }
     *  // if !rhs_owned {
     *   tmp_right := copy(tmp_right);
     *  // }
     *  target := ast_node_new(op.to_string(), tmp_left, tmp_right);
     *  goto <follow_idx>
     *
     * <assign_idx>:
     *  target := // rhs_assign();
     *  // if tmp_left_owned {
     *   _ := invalidate(tmp_left);
     *  // }
     *  // if rhs_owned {
     *   _ := invalidate(tmp_right);
     *  // }
     *  goto <follow_idx>
     *
     * <follow_idx>:
     */
    let dfl_impl_idx = proc.blocks.add_node(Vec::new());
    let mut not_idx = proc.blocks.add_node(Vec::new());
    let fallthrough_idx = proc.blocks.add_node(Vec::new());
    let mut rhs_idx = proc.blocks.add_node(Vec::new());
    let build_term_idx = proc.blocks.add_node(Vec::new());
    let mut assign_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    block_obj_overload_push(
        *block_idx,
        dfl_impl_idx,
        follow_idx,
        false,
        target,
        tmp_left,
        ObjOverloadRhs::Expression(rhs),
        op_obj_overload_sym(op).unwrap(),
        proc,
        shared_proc,
        cfg,
    );

    let t_lhs_t = tmp_var_new(proc);

    block_term_type_check_push(dfl_impl_idx, tmp_left, t_lhs_t, proc);

    block_get(proc, dfl_impl_idx).push(IRStmt::Branch(IRBranch {
        cond: IRValue::Variable(t_lhs_t),
        success: rhs_idx,
        failure: not_idx,
    }));

    proc.blocks.add_edge(dfl_impl_idx, rhs_idx, ());
    proc.blocks.add_edge(dfl_impl_idx, not_idx, ());

    let t_1 = tmp_var_new(proc);
    lhs_check(&mut not_idx, tmp_left, t_1, proc);
    if fallthrough_success {
        block_get(proc, not_idx).push(IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_1),
            success: fallthrough_idx,
            failure: rhs_idx,
        }));
    } else {
        block_get(proc, not_idx).push(IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_1),
            success: rhs_idx,
            failure: fallthrough_idx,
        }));
    }

    proc.blocks.add_edge(not_idx, fallthrough_idx, ());
    proc.blocks.add_edge(not_idx, rhs_idx, ());

    block_get(proc, fallthrough_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::BOOL,
        source: IRValue::Bool(fallthrough_success),
        op: IROp::Assign,
    }));

    if tmp_left_owned {
        block_get(proc, fallthrough_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_left)]),
        }))
    }

    block_get(proc, fallthrough_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(fallthrough_idx, follow_idx, ());

    let tmp_right = tmp_var_new(proc);

    let rhs_owned = block_expr_push(
        rhs,
        &mut rhs_idx,
        IRTarget::Variable(tmp_right),
        proc,
        shared_proc,
        cfg,
    );

    let t_rhs_t = tmp_var_new(proc);
    block_term_type_check_push(rhs_idx, tmp_right, t_rhs_t, proc);

    block_get(proc, rhs_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_rhs_t),
            types: IRType::BOOL,
            source: IRValue::Variable(t_lhs_t),
            op: IROp::Or(IRValue::Variable(t_rhs_t)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_rhs_t),
            success: build_term_idx,
            failure: assign_idx,
        }),
    ]);

    proc.blocks.add_edge(rhs_idx, build_term_idx, ());
    proc.blocks.add_edge(rhs_idx, assign_idx, ());

    if !tmp_left_owned {
        block_get(proc, build_term_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(tmp_left),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_left)]),
        }));
    }

    if !rhs_owned {
        block_get(proc, build_term_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(tmp_right),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_right)]),
        }));
    }

    block_get(proc, build_term_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target,
            types: IRType::AST,
            source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
            op: IROp::NativeCall(vec![
                IRValue::String(op.to_string()),
                IRValue::Variable(tmp_left),
                IRValue::Variable(tmp_right),
            ]),
        }),
        IRStmt::Goto(follow_idx),
    ]);
    proc.blocks.add_edge(build_term_idx, follow_idx, ());

    rhs_assign(&mut assign_idx, tmp_left, tmp_right, target, proc);

    if tmp_left_owned {
        block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_left)]),
        }))
    }

    if rhs_owned {
        block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_right)]),
        }))
    }

    block_get(proc, assign_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(assign_idx, follow_idx, ());

    *block_idx = follow_idx;
}

pub fn op_obj_overload_sym(op: &str) -> Option<&'static str> {
    match op {
        "unaryMinus" => Some("minus"),
        "minus" => Some("difference"),
        "mult" => Some("product"),
        "plus" => Some("sum"),
        "mod" => Some("modulo"),
        "cartesian" => Some("cartesianProduct"),
        "intDiv" => Some("integerDivision"),
        "or" => Some("disjunction"),
        "and" => Some("conjunction"),
        "imply" => Some("implication"),
        "power" => Some("power"),
        "not" => Some("not"),
        "card" => Some("cardinality"),
        "factor" => Some("factorial"),
        "less" => Some("lessThan"),
        "div" => Some("quotient"),
        "eq" => Some("equals"),
        _ => None,
    }
}

fn block_op_sym_push<F>(
    block_idx: &mut NodeIndex,
    tmp_left: IRVar,
    tmp_right: IRVar,
    target: IRTarget,
    dfl_fn: F,
    op: &str,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) where
    F: Fn(&mut NodeIndex, IRVar, IRVar, IRTarget, &mut IRProcedure),
{
    /* // block_obj_overload_push
     *
     * <dfl_idx>:
     *  // block_term_op_push
     *
     * <assign_idx>:
     *  target := // dfl_fn()
     *  goto <follow_idx>
     *
     * <follow_idx>
     */
    let dfl_idx = proc.blocks.add_node(Vec::new());
    let mut assign_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    block_obj_overload_push(
        *block_idx,
        dfl_idx,
        follow_idx,
        false,
        target,
        tmp_left,
        ObjOverloadRhs::Var(tmp_right),
        op_obj_overload_sym(op).unwrap(),
        proc,
        shared_proc,
        cfg,
    );

    block_term_op_push(
        dfl_idx, assign_idx, follow_idx, tmp_left, false, tmp_right, false, target, op, proc,
    );

    dfl_fn(&mut assign_idx, tmp_left, tmp_right, target, proc);

    block_get(proc, assign_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(assign_idx, follow_idx, ());

    *block_idx = follow_idx;
}

pub fn block_op_plus_push(
    block_idx: &mut NodeIndex,
    target: IRTarget,
    tmp_left: IRVar,
    tmp_right: IRVar,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /* t_left_type := type_of(tmp_left);
     * t_left_obj := t_left_type == TYPE_OBJ;
     * if t_left_obj
     *  goto <obj_idx>
     * else
     *  goto <assign_idx>
     *
     * <obj_idx>:
     *  // block_obj_overload_push
     *
     * <assign_idx>:
     *  // block_term_op_push
     *
     * <assign_op_idx>:
     *  target := tmp_left + tmp_right;
     *  goto <follow_idx>
     *
     * <follow_idx>
     */
    let obj_idx = proc.blocks.add_node(Vec::new());
    let assign_idx = proc.blocks.add_node(Vec::new());
    let assign_op_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    let t_left_type = tmp_var_new(proc);
    let t_left_obj = tmp_var_new(proc);

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_left_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_left)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_left_obj),
            types: IRType::BOOL,
            source: IRValue::Variable(t_left_type),
            op: IROp::Equal(IRValue::Type(IRType::OBJECT)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_left_obj),
            success: obj_idx,
            failure: assign_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, obj_idx, ());
    proc.blocks.add_edge(*block_idx, assign_idx, ());

    block_obj_overload_push(
        obj_idx,
        assign_op_idx,
        follow_idx,
        true,
        target,
        tmp_left,
        ObjOverloadRhs::Var(tmp_right),
        "sum",
        proc,
        shared_proc,
        cfg,
    );

    block_term_op_push(
        assign_idx,
        assign_op_idx,
        follow_idx,
        tmp_left,
        false,
        tmp_right,
        false,
        target,
        "plus",
        proc,
    );

    block_get(proc, assign_op_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target,
            types: IRTypes!("plus"),
            source: IRValue::Variable(tmp_left),
            op: IROp::Plus(IRValue::Variable(tmp_right)),
        }),
        IRStmt::Goto(follow_idx),
    ]);
    proc.blocks.add_edge(assign_op_idx, follow_idx, ());

    *block_idx = follow_idx;
}

/// Emits IR for the binary operator expression `c`, writing the result into `target`.
///
/// Dispatch order per operator:
/// - Object overload via `block_obj_overload_push`
/// - Term/AST construction if either operand is a term or AST node
/// - Default primitive implementation
///
/// `and`, `or`, and `imply` use short-circuit evaluation via `block_op_rhs_deferred_push`.
pub fn block_op_push(
    c: &CSTExpressionOp,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    let tmp_left = tmp_var_new(proc);
    let tmp_left_owned = block_expr_push(
        &c.left,
        block_idx,
        IRTarget::Variable(tmp_left),
        proc,
        shared_proc,
        cfg,
    );

    let tmp_right = if !matches!(c.op, CSTOp::Imply | CSTOp::And | CSTOp::Or) {
        tmp_var_new(proc)
    } else {
        0
    };
    let tmp_right_owned = if !matches!(c.op, CSTOp::Imply | CSTOp::And | CSTOp::Or) {
        block_expr_push(
            &c.right,
            block_idx,
            IRTarget::Variable(tmp_right),
            proc,
            shared_proc,
            cfg,
        )
    } else {
        false
    };

    match c.op {
        CSTOp::Imply => {
            block_op_rhs_deferred_push(
                block_idx,
                tmp_left,
                tmp_left_owned,
                &c.right,
                true,
                |idx, tmp_left, t_target, proc| {
                    /*  t_type := type_of(tmp_left);
                     *  t_lhs_bool := t_type == TYPE_BOOL;
                     *  if t_lhs_bool
                     *   goto <not_idx>
                     *  else
                     *   goto <fail_idx>
                     *
                     * <not_idx>:
                     *  t_target := !tmp_left;
                     *
                     * <fail_idx>:
                     *  _ := exception_throw("op", "imply undefined for type");
                     *  unreachable;
                     */
                    let not_idx = proc.blocks.add_node(Vec::new());
                    let fail_idx = proc.blocks.add_node(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Ignore,
                            types: IRType::UNDEFINED,
                            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
                            op: IROp::NativeCall(vec![
                                IRValue::String("op".to_string()),
                                IRValue::String("imply undefined for type".to_string()),
                            ]),
                        }),
                        IRStmt::Unreachable,
                    ]);

                    let t_type = tmp_var_new(proc);
                    let t_lhs_bool = tmp_var_new(proc);

                    block_get(proc, *idx).extend(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_type),
                            types: IRType::TYPE,
                            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
                            op: IROp::NativeCall(vec![IRValue::Variable(tmp_left)]),
                        }),
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_lhs_bool),
                            types: IRType::BOOL,
                            source: IRValue::Variable(t_type),
                            op: IROp::Equal(IRValue::Type(IRType::BOOL)),
                        }),
                        IRStmt::Branch(IRBranch {
                            cond: IRValue::Variable(t_lhs_bool),
                            success: not_idx,
                            failure: fail_idx,
                        }),
                    ]);
                    proc.blocks.add_edge(*idx, not_idx, ());
                    proc.blocks.add_edge(*idx, fail_idx, ());

                    block_get(proc, not_idx).push(IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_target),
                        types: IRType::BOOL,
                        source: IRValue::Variable(tmp_left),
                        op: IROp::Not,
                    }));

                    *idx = not_idx;
                },
                |idx, tmp_left, tmp_right, target, proc| {
                    /* t_not := !tmp_left;
                     * target := t_not || tmp_right;
                     */
                    let t_not = tmp_var_new(proc);

                    block_get(proc, *idx).extend(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_not),
                            types: IRType::BOOL,
                            source: IRValue::Variable(tmp_left),
                            op: IROp::Not,
                        }),
                        IRStmt::Assign(IRAssign {
                            target,
                            types: IRType::BOOL,
                            source: IRValue::Variable(t_not),
                            op: IROp::Or(IRValue::Variable(tmp_right)),
                        }),
                    ]);
                },
                target,
                &c.op.to_string(),
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTOp::Or => {
            block_op_rhs_deferred_push(
                block_idx,
                tmp_left,
                tmp_left_owned,
                &c.right,
                true,
                |idx, tmp_left, t_target, proc| {
                    /*  t_type := type_of(tmp_left);
                     *  t_lhs_bool := t_type == TYPE_BOOL;
                     *  if t_lhs_bool
                     *   goto <not_idx>
                     *  else
                     *   goto <fail_idx>
                     *
                     * <not_idx>:
                     *  t_target := tmp_left;
                     *
                     * <fail_idx>:
                     *  _ := exception_throw("op", "or undefined for type");
                     *  unreachable;
                     */
                    let assign_idx = proc.blocks.add_node(Vec::new());
                    let fail_idx = proc.blocks.add_node(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Ignore,
                            types: IRType::UNDEFINED,
                            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
                            op: IROp::NativeCall(vec![
                                IRValue::String("op".to_string()),
                                IRValue::String("or undefined for type".to_string()),
                            ]),
                        }),
                        IRStmt::Unreachable,
                    ]);

                    let t_type = tmp_var_new(proc);
                    let t_lhs_bool = tmp_var_new(proc);

                    block_get(proc, *idx).extend(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_type),
                            types: IRType::TYPE,
                            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
                            op: IROp::NativeCall(vec![IRValue::Variable(tmp_left)]),
                        }),
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_lhs_bool),
                            types: IRType::BOOL,
                            source: IRValue::Variable(t_type),
                            op: IROp::Equal(IRValue::Type(IRType::BOOL)),
                        }),
                        IRStmt::Branch(IRBranch {
                            cond: IRValue::Variable(t_lhs_bool),
                            success: assign_idx,
                            failure: fail_idx,
                        }),
                    ]);
                    proc.blocks.add_edge(*idx, assign_idx, ());
                    proc.blocks.add_edge(*idx, fail_idx, ());

                    block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_target),
                        types: IRType::BOOL,
                        source: IRValue::Variable(tmp_left),
                        op: IROp::Assign,
                    }));

                    *idx = assign_idx;
                },
                |idx, tmp_left, tmp_right, target, proc| {
                    //target := tmp_left || tmp_right;
                    block_get(proc, *idx).push(IRStmt::Assign(IRAssign {
                        target,
                        types: IRType::BOOL,
                        source: IRValue::Variable(tmp_left),
                        op: IROp::Or(IRValue::Variable(tmp_right)),
                    }));
                },
                target,
                &c.op.to_string(),
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTOp::And => {
            block_op_rhs_deferred_push(
                block_idx,
                tmp_left,
                tmp_left_owned,
                &c.right,
                false,
                |idx, tmp_left, t_target, proc| {
                    /*  t_type := type_of(tmp_left);
                     *  t_lhs_bool := t_type == TYPE_BOOL;
                     *  if t_lhs_bool
                     *   goto <not_idx>
                     *  else
                     *   goto <fail_idx>
                     *
                     * <not_idx>:
                     *  t_target := tmp_left;
                     *
                     * <fail_idx>:
                     *  _ := exception_throw("op", "and undefined for type");
                     *  unreachable;
                     */
                    let assign_idx = proc.blocks.add_node(Vec::new());
                    let fail_idx = proc.blocks.add_node(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Ignore,
                            types: IRType::UNDEFINED,
                            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
                            op: IROp::NativeCall(vec![
                                IRValue::String("op".to_string()),
                                IRValue::String("and undefined for type".to_string()),
                            ]),
                        }),
                        IRStmt::Unreachable,
                    ]);

                    let t_type = tmp_var_new(proc);
                    let t_lhs_bool = tmp_var_new(proc);

                    block_get(proc, *idx).extend(vec![
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_type),
                            types: IRType::TYPE,
                            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
                            op: IROp::NativeCall(vec![IRValue::Variable(tmp_left)]),
                        }),
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_lhs_bool),
                            types: IRType::BOOL,
                            source: IRValue::Variable(t_type),
                            op: IROp::Equal(IRValue::Type(IRType::BOOL)),
                        }),
                        IRStmt::Branch(IRBranch {
                            cond: IRValue::Variable(t_lhs_bool),
                            success: assign_idx,
                            failure: fail_idx,
                        }),
                    ]);
                    proc.blocks.add_edge(*idx, assign_idx, ());
                    proc.blocks.add_edge(*idx, fail_idx, ());

                    block_get(proc, assign_idx).push(IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_target),
                        types: IRType::BOOL,
                        source: IRValue::Variable(tmp_left),
                        op: IROp::Assign,
                    }));

                    *idx = assign_idx;
                },
                |idx, tmp_left, tmp_right, target, proc| {
                    //target := tmp_left && tmp_right;
                    block_get(proc, *idx).push(IRStmt::Assign(IRAssign {
                        target,
                        types: IRType::BOOL,
                        source: IRValue::Variable(tmp_left),
                        op: IROp::And(IRValue::Variable(tmp_right)),
                    }));
                },
                target,
                &c.op.to_string(),
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTOp::Eq => {
            /* // block_obj_overload_push
             *
             * <dfl_idx>:
             *  target := tmp_left == tmp_right;
             *  goto <follow_idx>
             *
             * <follow_idx>
             */
            let dfl_idx = proc.blocks.add_node(Vec::new());
            let follow_idx = proc.blocks.add_node(Vec::new());

            block_obj_overload_push(
                *block_idx,
                dfl_idx,
                follow_idx,
                true,
                target,
                tmp_left,
                ObjOverloadRhs::Var(tmp_right),
                "equals",
                proc,
                shared_proc,
                cfg,
            );

            block_get(proc, dfl_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_left),
                    op: IROp::Equal(IRValue::Variable(tmp_right)),
                }),
                IRStmt::Goto(follow_idx),
            ]);
            proc.blocks.add_edge(dfl_idx, follow_idx, ());

            *block_idx = follow_idx;
        }
        CSTOp::Neq => {
            /* // block_obj_overload_push
             *
             * <dfl_equal_idx>:
             *  t_1 := tmp_left == tmp_right;
             *  goto <follow_equal_idx>
             *
             * <follow_equal_idx>:
             *  // block_obj_overload_push
             *
             * <dfl_not_idx>:
             *  target := !t_1;
             *  goto <follow_idx>
             *
             * <follow_idx>:
             */
            let tmp = tmp_var_new(proc);

            let dfl_equal_idx = proc.blocks.add_node(Vec::new());
            let follow_equal_idx = proc.blocks.add_node(Vec::new());
            let dfl_not_idx = proc.blocks.add_node(Vec::new());
            let follow_idx = proc.blocks.add_node(Vec::new());

            block_obj_overload_push(
                *block_idx,
                dfl_equal_idx,
                follow_equal_idx,
                true,
                IRTarget::Variable(tmp),
                tmp_left,
                ObjOverloadRhs::Var(tmp_right),
                "equals",
                proc,
                shared_proc,
                cfg,
            );

            block_get(proc, dfl_equal_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp),
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_left),
                    op: IROp::Equal(IRValue::Variable(tmp_right)),
                }),
                IRStmt::Goto(follow_equal_idx),
            ]);
            proc.blocks.add_edge(dfl_equal_idx, follow_equal_idx, ());

            block_obj_overload_push(
                follow_equal_idx,
                dfl_not_idx,
                follow_idx,
                false,
                target,
                tmp,
                ObjOverloadRhs::None,
                "not",
                proc,
                shared_proc,
                cfg,
            );

            block_get(proc, dfl_not_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp),
                    op: IROp::Not,
                }),
                IRStmt::Goto(follow_idx),
            ]);
            proc.blocks.add_edge(dfl_not_idx, follow_idx, ());

            *block_idx = follow_idx;
        }
        CSTOp::SetEq => {
            set_eq_push(
                block_idx,
                true,
                tmp_left,
                tmp_left_owned,
                tmp_right,
                tmp_right_owned,
                target,
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTOp::SetNeq => {
            set_eq_push(
                block_idx,
                false,
                tmp_left,
                tmp_left_owned,
                tmp_right,
                tmp_right_owned,
                target,
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTOp::Less => {
            /* // block_obj_overload_push
             *
             * <dfl_idx>:
             *  target := tmp_left < tmp_right;
             *  goto <follow_idx>
             *
             * <follow_idx>
             */
            let dfl_idx = proc.blocks.add_node(Vec::new());
            let follow_idx = proc.blocks.add_node(Vec::new());

            block_obj_overload_push(
                *block_idx,
                dfl_idx,
                follow_idx,
                false,
                target,
                tmp_left,
                ObjOverloadRhs::Var(tmp_right),
                "lessThan",
                proc,
                shared_proc,
                cfg,
            );

            block_get(proc, dfl_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_left),
                    op: IROp::Less(IRValue::Variable(tmp_right)),
                }),
                IRStmt::Goto(follow_idx),
            ]);
            proc.blocks.add_edge(dfl_idx, follow_idx, ());

            *block_idx = follow_idx;
        }
        CSTOp::Leq => {
            /*  // block_obj_overload_push
             *
             * <dfl_less_idx>:
             *  t_1 := tmp_left < tmp_right;
             *  goto <obj_eq_idx>
             *
             * <obj_eq_idx>:
             *  // block_obj_overload_push
             *
             * <dfl_eq_idx>:
             *  t_2 := tmp_left == tmp_right;
             *  goto <follow_idx>
             *
             * <follow_idx>:
             */
            let tmp_1 = tmp_var_new(proc);
            let tmp_2 = tmp_var_new(proc);

            let dfl_less_idx = proc.blocks.add_node(Vec::new());
            let dfl_eq_idx = proc.blocks.add_node(Vec::new());
            let obj_eq_idx = proc.blocks.add_node(Vec::new());
            let follow_idx = proc.blocks.add_node(Vec::new());

            block_obj_overload_push(
                *block_idx,
                dfl_less_idx,
                obj_eq_idx,
                false,
                IRTarget::Variable(tmp_1),
                tmp_left,
                ObjOverloadRhs::Var(tmp_right),
                "lessThan",
                proc,
                shared_proc,
                cfg,
            );

            block_get(proc, dfl_less_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp_1),
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_left),
                    op: IROp::Less(IRValue::Variable(tmp_right)),
                }),
                IRStmt::Goto(obj_eq_idx),
            ]);
            proc.blocks.add_edge(dfl_less_idx, obj_eq_idx, ());

            block_obj_overload_push(
                obj_eq_idx,
                dfl_eq_idx,
                follow_idx,
                true,
                IRTarget::Variable(tmp_2),
                tmp_left,
                ObjOverloadRhs::Var(tmp_right),
                "equals",
                proc,
                shared_proc,
                cfg,
            );

            block_get(proc, dfl_eq_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp_2),
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_left),
                    op: IROp::Equal(IRValue::Variable(tmp_right)),
                }),
                IRStmt::Goto(follow_idx),
            ]);
            proc.blocks.add_edge(dfl_eq_idx, follow_idx, ());

            block_get(proc, follow_idx).extend(vec![IRStmt::Assign(IRAssign {
                target,
                types: IRType::BOOL,
                source: IRValue::Variable(tmp_1),
                op: IROp::Or(IRValue::Variable(tmp_2)),
            })]);

            *block_idx = follow_idx;
        }
        CSTOp::Greater => {
            /* // block_obj_overload_push
             *
             * <dfl_idx>:
             *  target := tmp_right < tmp_left;
             *  goto <follow_idx>
             *
             * <follow_idx>
             */
            let dfl_idx = proc.blocks.add_node(Vec::new());
            let follow_idx = proc.blocks.add_node(Vec::new());

            block_obj_overload_push(
                *block_idx,
                dfl_idx,
                follow_idx,
                false,
                target,
                tmp_right,
                ObjOverloadRhs::Var(tmp_left),
                "lessThan",
                proc,
                shared_proc,
                cfg,
            );

            block_get(proc, dfl_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_right),
                    op: IROp::Less(IRValue::Variable(tmp_left)),
                }),
                IRStmt::Goto(follow_idx),
            ]);
            proc.blocks.add_edge(dfl_idx, follow_idx, ());

            *block_idx = follow_idx;
        }
        CSTOp::Geq => {
            /*  // block_obj_overload_push
             *
             * <dfl_less_idx>:
             *  t_1 := tmp_right < tmp_left;
             *  goto <obj_eq_idx>
             *
             * <obj_eq_idx>:
             *  // block_obj_overload_push
             *
             * <dfl_eq_idx>:
             *  t_2 := tmp_left == tmp_right;
             *  goto <follow_idx>
             *
             * <follow_idx>:
             *  target := t_1 || t_2;
             */
            let tmp_1 = tmp_var_new(proc);
            let tmp_2 = tmp_var_new(proc);

            let dfl_less_idx = proc.blocks.add_node(Vec::new());
            let dfl_eq_idx = proc.blocks.add_node(Vec::new());
            let obj_eq_idx = proc.blocks.add_node(Vec::new());
            let follow_idx = proc.blocks.add_node(Vec::new());

            block_obj_overload_push(
                *block_idx,
                dfl_less_idx,
                obj_eq_idx,
                false,
                IRTarget::Variable(tmp_1),
                tmp_right,
                ObjOverloadRhs::Var(tmp_left),
                "lessThan",
                proc,
                shared_proc,
                cfg,
            );

            block_get(proc, dfl_less_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp_1),
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_right),
                    op: IROp::Less(IRValue::Variable(tmp_left)),
                }),
                IRStmt::Goto(obj_eq_idx),
            ]);
            proc.blocks.add_edge(dfl_less_idx, obj_eq_idx, ());

            block_obj_overload_push(
                obj_eq_idx,
                dfl_eq_idx,
                follow_idx,
                true,
                IRTarget::Variable(tmp_2),
                tmp_left,
                ObjOverloadRhs::Var(tmp_right),
                "equals",
                proc,
                shared_proc,
                cfg,
            );

            block_get(proc, dfl_eq_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp_2),
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp_left),
                    op: IROp::Equal(IRValue::Variable(tmp_right)),
                }),
                IRStmt::Goto(follow_idx),
            ]);
            proc.blocks.add_edge(dfl_eq_idx, follow_idx, ());

            block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::BOOL,
                source: IRValue::Variable(tmp_1),
                op: IROp::Or(IRValue::Variable(tmp_2)),
            }));

            *block_idx = follow_idx;
        }
        CSTOp::In => {
            /* target := contains(tmp_left, tmp_right);
             */
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target,
                types: IRType::BOOL,
                source: IRValue::BuiltinProc(BuiltinProc::Contains),
                op: IROp::NativeCall(vec![
                    IRValue::Variable(tmp_right),
                    IRValue::Variable(tmp_left),
                ]),
            }));
        }
        CSTOp::NotIn => {
            /* t_1 := contains(tmp_left, tmp_right);
             * target := !t_1;
             */
            let tmp = tmp_var_new(proc);
            block_get(proc, *block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(tmp),
                    types: IRType::BOOL,
                    source: IRValue::BuiltinProc(BuiltinProc::Contains),
                    op: IROp::NativeCall(vec![
                        IRValue::Variable(tmp_right),
                        IRValue::Variable(tmp_left),
                    ]),
                }),
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRType::BOOL,
                    source: IRValue::Variable(tmp),
                    op: IROp::Not,
                }),
            ]);
        }
        CSTOp::Plus => {
            block_op_plus_push(
                block_idx,
                target,
                tmp_left,
                tmp_right,
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTOp::Minus => {
            block_op_sym_push(
                block_idx,
                tmp_left,
                tmp_right,
                target,
                |idx, tmp_left, tmp_right, target, proc| {
                    // target := tmp_left - tmp_right;
                    block_get(proc, *idx).push(IRStmt::Assign(IRAssign {
                        target,
                        types: IRTypes!("minus"),
                        source: IRValue::Variable(tmp_left),
                        op: IROp::Minus(IRValue::Variable(tmp_right)),
                    }));
                },
                &c.op.to_string(),
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTOp::Mult => {
            block_op_sym_push(
                block_idx,
                tmp_left,
                tmp_right,
                target,
                |idx, tmp_left, tmp_right, target, proc| {
                    // target := tmp_left * tmp_right;
                    block_get(proc, *idx).push(IRStmt::Assign(IRAssign {
                        target,
                        types: IRTypes!("mul"),
                        source: IRValue::Variable(tmp_left),
                        op: IROp::Mult(IRValue::Variable(tmp_right)),
                    }));
                },
                &c.op.to_string(),
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTOp::Div => {
            block_op_sym_push(
                block_idx,
                tmp_left,
                tmp_right,
                target,
                |idx, tmp_left, tmp_right, target, proc| {
                    // target := tmp_left / tmp_right;
                    block_get(proc, *idx).push(IRStmt::Assign(IRAssign {
                        target,
                        types: IRTypes!("quot"),
                        source: IRValue::Variable(tmp_left),
                        op: IROp::Divide(IRValue::Variable(tmp_right)),
                    }));
                },
                &c.op.to_string(),
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTOp::IntDiv => {
            /* // block_obj_overload_push
             *
             * <dfl_idx>:
             *  target := tmp_left \ tmp_right;
             *  goto <follow_idx>
             *
             * <follow_idx>
             */
            let dfl_idx = proc.blocks.add_node(Vec::new());
            let follow_idx = proc.blocks.add_node(Vec::new());

            block_obj_overload_push(
                *block_idx,
                dfl_idx,
                follow_idx,
                false,
                target,
                tmp_left,
                ObjOverloadRhs::Var(tmp_right),
                "integerDivision",
                proc,
                shared_proc,
                cfg,
            );

            block_get(proc, dfl_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target,
                    types: IRTypes!("quot"),
                    source: IRValue::Variable(tmp_left),
                    op: IROp::IntDivide(IRValue::Variable(tmp_right)),
                }),
                IRStmt::Goto(follow_idx),
            ]);
            proc.blocks.add_edge(dfl_idx, follow_idx, ());

            *block_idx = follow_idx;
        }
        CSTOp::Mod => {
            block_op_sym_push(
                block_idx,
                tmp_left,
                tmp_right,
                target,
                |idx, tmp_left, tmp_right, target, proc| {
                    // target := tmp_left % tmp_right;
                    block_get(proc, *idx).push(IRStmt::Assign(IRAssign {
                        target,
                        types: IRType::SET | IRType::NUMBER | IRType::DOUBLE,
                        source: IRValue::Variable(tmp_left),
                        op: IROp::Mod(IRValue::Variable(tmp_right)),
                    }));
                },
                &c.op.to_string(),
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTOp::Cartesian => {
            block_op_sym_push(
                block_idx,
                tmp_left,
                tmp_right,
                target,
                |idx, tmp_left, tmp_right, target, proc| {
                    // target := cartesian(tmp_left, tmp_right);
                    block_get(proc, *idx).push(IRStmt::Assign(IRAssign {
                        target,
                        types: IRType::SET | IRType::LIST,
                        source: IRValue::BuiltinProc(BuiltinProc::Cartesian),
                        op: IROp::NativeCall(vec![
                            IRValue::Variable(tmp_left),
                            IRValue::Variable(tmp_right),
                        ]),
                    }));
                },
                &c.op.to_string(),
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTOp::Power => {
            block_op_sym_push(
                block_idx,
                tmp_left,
                tmp_right,
                target,
                |idx, tmp_left, tmp_right, target, proc| {
                    // target := pow(tmp_left, tmp_right);
                    block_get(proc, *idx).push(IRStmt::Assign(IRAssign {
                        target,
                        types: IRType::SET
                            | IRType::NUMBER
                            | IRType::DOUBLE
                            | IRType::MATRIX
                            | IRType::VECTOR,
                        source: IRValue::BuiltinProc(BuiltinProc::Pow),
                        op: IROp::NativeCall(vec![
                            IRValue::Variable(tmp_left),
                            IRValue::Variable(tmp_right),
                        ]),
                    }));
                },
                &c.op.to_string(),
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTOp::SumMem => {
            block_set_mem_op_dfl_push(
                block_idx,
                tmp_left,
                tmp_left_owned,
                tmp_right,
                block_sum_mem_push,
                target,
                proc,
                shared_proc,
                cfg,
            );
        }
        CSTOp::ProdMem => {
            block_set_mem_op_dfl_push(
                block_idx,
                tmp_left,
                tmp_left_owned,
                tmp_right,
                block_prod_mem_push,
                target,
                proc,
                shared_proc,
                cfg,
            );
        }
    }

    /* _ := invalidate(tmp_left);
     * _ := invalidate(tmp_right);
     */
    if tmp_left_owned
        && !matches!(
            c.op,
            CSTOp::Imply
                | CSTOp::And
                | CSTOp::Or
                | CSTOp::SetEq
                | CSTOp::SetNeq
                | CSTOp::SumMem
                | CSTOp::ProdMem,
        )
    {
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_left)]),
        }));
    }
    if tmp_right_owned && !matches!(c.op, CSTOp::SetEq | CSTOp::SetNeq) {
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_right)]),
        }));
    }
}
