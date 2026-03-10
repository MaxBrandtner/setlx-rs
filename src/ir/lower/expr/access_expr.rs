use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::proc::{call_params_invalidate_push, call_params_push};
use crate::ir::lower::util::{block_get, tmp_var_new};

/// Normalizes the index in `tmp` for zero-based access into `t_expr`.
///
/// # Arguments
///
/// * `tmp` - the index variable to normalize, mutated in place (mutable borrow)
/// * `t_expr` - the collection used to resolve negative indices (borrowed)
/// * `allow_zero` - if true, index `0` is passed through without error
pub fn block_arr_access_push(
    block_idx: &mut NodeIndex,
    tmp: IRVar,
    t_expr: IRVar,
    allow_zero: bool,
    proc: &mut IRProcedure,
) {
    /*  t_check_zero := tmp == 0;
     *  if t_check_zero
     *   goto <throw_idx>
     *  else
     *   goto <assign_idx>
     *
     * <throw_idx>:
     *  _ := throw(1, "index '0' is invalid");
     *  unreachable
     *
     * <assign_idx>
     *  t_check_neg := tmp < 0;
     *  if t_check_neg
     *   goto <amount_idx>
     *  else
     *   goto <decr_idx>
     *
     * <amount_idx>:
     *  t_amount := amount(t_expr);
     *  tmp_new := t_amount + tmp;
     *  _ := invalidate(t_amount);
     *  _ := invalidate(tmp);
     *  tmp := tmp_new;
     *  t_check_neq := tmp < 0;
     *  if t_check_neq
     *   goto <set_zero_idx>
     *  else
     *   goto <follow_idx>
     *
     * <set_zero_idx>:
     *  _ := invalidate(tmp);
     *  tmp := 0;
     *  goto <follow_idx>
     *
     * <decr_idx>:
     *  tmp_new := tmp - 1;
     *  _ := invalidate(tmp);
     *  tmp := tmp_new;
     *  goto <follow_idx>
     *
     * <follow_idx>:
     */
    let t_check_zero = tmp_var_new(proc);
    let tmp_new = tmp_var_new(proc);

    let throw_idx = proc.blocks.add_node(Vec::new());
    let assign_idx = proc.blocks.add_node(Vec::new());
    let amount_idx = proc.blocks.add_node(Vec::new());
    let set_zero_idx = proc.blocks.add_node(Vec::new());
    let decr_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_check_zero),
            types: IRType::BOOL,
            source: IRValue::Variable(tmp),
            op: IROp::Equal(IRValue::Number(0.into())),
        }),
        if allow_zero {
            IRStmt::Goto(assign_idx)
        } else {
            IRStmt::Branch(IRBranch {
                cond: IRValue::Variable(t_check_zero),
                success: throw_idx,
                failure: assign_idx,
            })
        },
    ]);

    if !allow_zero {
        proc.blocks.add_edge(*block_idx, throw_idx, ());
    }
    proc.blocks.add_edge(*block_idx, assign_idx, ());

    block_get(proc, throw_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Throw),
            op: IROp::NativeCall(vec![
                IRValue::Number(1.into()),
                IRValue::String(String::from("index '0' is invalid")),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    let t_check_neq = tmp_var_new(proc);

    block_get(proc, assign_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_check_neq),
            types: IRType::BOOL,
            source: IRValue::Variable(tmp),
            op: IROp::Less(IRValue::Number(0.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_check_neq),
            success: amount_idx,
            failure: decr_idx,
        }),
    ]);

    proc.blocks.add_edge(assign_idx, amount_idx, ());
    proc.blocks.add_edge(assign_idx, decr_idx, ());

    let t_amount = tmp_var_new(proc);

    block_get(proc, amount_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_amount),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(tmp_new),
            types: IRType::NUMBER | IRType::DOUBLE,
            source: IRValue::Variable(t_amount),
            op: IROp::Plus(IRValue::Variable(tmp)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_amount)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(tmp),
            types: IRType::NUMBER | IRType::DOUBLE,
            source: IRValue::Variable(tmp_new),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_check_neq),
            types: IRType::BOOL,
            source: IRValue::Variable(tmp),
            op: IROp::Less(IRValue::Number(0.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_check_neq),
            success: set_zero_idx,
            failure: follow_idx,
        }),
    ]);

    proc.blocks.add_edge(amount_idx, set_zero_idx, ());
    proc.blocks.add_edge(amount_idx, follow_idx, ());

    block_get(proc, set_zero_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(tmp),
            types: IRType::NUMBER,
            source: IRValue::Number(0.into()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(set_zero_idx, follow_idx, ());

    block_get(proc, decr_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(tmp_new),
            types: IRType::NUMBER | IRType::DOUBLE,
            source: IRValue::Variable(tmp),
            op: IROp::Minus(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(tmp),
            types: IRType::NUMBER | IRType::DOUBLE,
            source: IRValue::Variable(tmp_new),
            op: IROp::Assign,
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(decr_idx, follow_idx, ());

    *block_idx = follow_idx
}

/// A new stack frame is established for the duration of the call. The object
/// is aliased as `this` and each of its members is aliased by name into the
/// frame, all marked as cross-frame, making them accessible to the callee and
/// any procedures it invokes. The procedure is then called with the provided
/// parameters. On return the frame is torn down, removing all aliases. If an
/// exception is raised the frame is torn down before rethrowing.
///
/// # Arguments
///
/// * `t_obj_addr` - pointer to the object whose members and `this` are aliased into the frame
/// * `t_proc` - the method procedure to call
/// * `t_params` - the parameters to pass to the call
pub fn block_obj_call_impl_push(
    block_idx: &mut NodeIndex,
    t_obj_addr: IRVar,
    t_proc: IRVar,
    t_params: IRVar,
    target: IRTarget,
    proc: &mut IRProcedure,
) {
    /*  _ := stack_frame_add();
     *  try
     *   goto <stack_iter_new_idx>
     *  catch
     *   goto <rethrow_idx>
     *
     * <stack_iter_new_idx>:
     *   _ := stack_alias("this", t_obj_addr, true);
     *   t_obj := *t_obj_addr;
     *   t_iter := object_iter_new(t_obj);
     *   goto <stack_iter_idx>
     *
     * <stack_iter_idx>:
     *  t_key := om;
     *  t_key_addr := &t_key;
     *  t_val_ptr := om;
     *  t_val_ptr_addr := &t_val_ptr;
     *  t_cond := object_iter_next(t_iter, t_key_addr, t_val_ptr_addr);
     *  if t_cond
     *   goto <stack_add_idx>
     *  else
     *   goto <call_idx>
     *
     * <stack_add_idx>:
     *  _ := stack_alias(t_key, t_val_ptr, true);
     *  goto <stack_iter_idx>
     *
     * <call_idx>:
     *  t_out := t_proc(t_params);
     *  _ := mark_persist(t_out);
     *  try_end <target_idx>
     *
     * <target_idx>
     *  _ := stack_frame_pop();
     *  _ := mark_immed(t_out);
     *  target := t_out;
     *
     * <rethrow_idx>:
     *   _ := stack_frame_pop();
     *   _ := rethrow();
     *   unreachable;
     */
    let t_iter = tmp_var_new(proc);
    let t_key = tmp_var_new(proc);
    let t_key_addr = tmp_var_new(proc);
    let t_val_ptr = tmp_var_new(proc);
    let t_val_ptr_addr = tmp_var_new(proc);
    let t_obj = tmp_var_new(proc);
    let t_cond = tmp_var_new(proc);
    let t_out = tmp_var_new(proc);

    let stack_iter_new_idx = proc.blocks.add_node(Vec::new());
    let stack_iter_idx = proc.blocks.add_node(Vec::new());
    let stack_add_idx = proc.blocks.add_node(Vec::new());
    let call_idx = proc.blocks.add_node(Vec::new());
    let target_idx = proc.blocks.add_node(Vec::new());
    let rethrow_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::StackFrameAdd),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Try(IRTry {
            attempt: stack_iter_new_idx,
            catch: rethrow_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, stack_iter_new_idx, ());
    proc.blocks.add_edge(*block_idx, stack_iter_new_idx, ());

    block_get(proc, stack_iter_new_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::StackAlias),
            op: IROp::NativeCall(vec![
                IRValue::String("this".to_string()),
                IRValue::Variable(t_obj_addr),
                IRValue::Bool(true),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_obj),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_obj_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_iter),
            types: IRType::OBJ_ITER,
            source: IRValue::BuiltinProc(BuiltinProc::ObjectIterNew),
            op: IROp::NativeCall(vec![IRValue::Variable(t_obj)]),
        }),
        IRStmt::Goto(stack_iter_idx),
    ]);

    proc.blocks.add_edge(stack_iter_new_idx, stack_iter_idx, ());

    block_get(proc, stack_iter_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_key),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_key_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_key),
            op: IROp::PtrAddress,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_val_ptr),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_val_ptr_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_val_ptr),
            op: IROp::PtrAddress,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cond),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::ObjectIterNext),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_iter),
                IRValue::Variable(t_key_addr),
                IRValue::Variable(t_val_ptr_addr),
            ]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cond),
            success: stack_add_idx,
            failure: call_idx,
        }),
    ]);

    proc.blocks.add_edge(stack_iter_idx, stack_add_idx, ());
    proc.blocks.add_edge(stack_iter_idx, call_idx, ());

    block_get(proc, stack_add_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::StackAlias),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_key),
                IRValue::Variable(t_val_ptr),
                IRValue::Bool(true),
            ]),
        }),
        IRStmt::Goto(stack_iter_idx),
    ]);

    proc.blocks.add_edge(stack_add_idx, stack_iter_idx, ());

    block_get(proc, call_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_proc),
            op: IROp::Call(t_params),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::MarkPersist),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::TryEnd(target_idx),
    ]);

    proc.blocks.add_edge(call_idx, target_idx, ());

    block_get(proc, target_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::StackFramePop),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::MarkImmed),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target,
            types: IRTypes!("any"),
            source: IRValue::Variable(t_out),
            op: IROp::Assign,
        }),
    ]);

    block_get(proc, rethrow_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::StackFramePop),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Rethrow),
            op: IROp::NativeCall(Vec::new()),
        }),
    ]);

    *block_idx = target_idx;
}

fn block_obj_call_push(
    c: &CSTProcedureCall,
    block_idx: &mut NodeIndex,
    t_head: IRVar,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) -> IRVar /* owned */ {
    /*  t_head_val := *t_head;
     *  t_ptr := object_get(t_head_val, c.name);
     *  t_ptr_om := t_ptr == om;
     *  if t_ptr_om
     *   goto <except_idx>
     *  else
     *   goto <frame_idx>
     *
     * <except_idx>:
     *  _ := exception_throw("object call", "object doesn't contain {c.name}");
     *  unreachable;
     *
     * <frame_idx>:
     *  t_proc := *t_ptr;
     *  t_params := // call_params_push
     *  t_var := // block_obj_call_impl_push
     *  _ := invalidate(t_params);
     *  t_head := &t_var;
     */
    let t_head_val = tmp_var_new(proc);
    let t_ptr = tmp_var_new(proc);
    let t_ptr_om = tmp_var_new(proc);
    let t_params = tmp_var_new(proc);
    let t_proc = tmp_var_new(proc);
    let t_var = tmp_var_new(proc);

    let except_idx = proc.blocks.add_node(Vec::new());
    let mut frame_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_head_val),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_head),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ptr),
            types: IRType::PTR | IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ObjectGet),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_head_val),
                IRValue::String(c.name.to_string()),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ptr_om),
            types: IRType::BOOL,
            source: IRValue::Variable(t_ptr),
            op: IROp::Equal(IRValue::Undefined),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_ptr_om),
            success: except_idx,
            failure: frame_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, except_idx, ());
    proc.blocks.add_edge(*block_idx, frame_idx, ());

    block_get(proc, except_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("object call".to_string()),
                IRValue::String(format!("object doesn't contain {}", c.name)),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    block_get(proc, frame_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_proc),
        types: IRTypes!("any"),
        source: IRValue::Variable(t_ptr),
        op: IROp::PtrDeref,
    }));

    let inv_vars = call_params_push(
        c,
        &mut frame_idx,
        IRTarget::Variable(t_params),
        t_proc,
        proc,
        shared_proc,
        cfg,
    );

    block_obj_call_impl_push(
        &mut frame_idx,
        t_head,
        t_proc,
        t_params,
        IRTarget::Variable(t_var),
        proc,
    );

    call_params_invalidate_push(frame_idx, &inv_vars, proc);

    block_get(proc, frame_idx).extend(vec![
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

    *block_idx = frame_idx;

    t_var
}

/// Supports member access (`.name`), method calls (`.name(...)`), numeric
/// indexing (`[i]`), range slicing (`[i..j]`), and set tag lookup (`{tag}`).
/// Returns the owned temporary that must be invalidated by the caller if the
/// result is owned, or `None` if the result is a borrowed reference into an
/// existing stack or heap slot.
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

    let mut t_owned = if head_owned { Some(t_head_var) } else { None };

    let t_head = tmp_var_new(proc);
    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_head),
        types: IRType::PTR,
        source: IRValue::Variable(t_head_var),
        op: IROp::PtrAddress,
    }));

    for i in &a.body {
        let t_owned_new;

        match &i.kind {
            CSTExpressionKind::Variable(v) => {
                /* t_head_val := *t_head;
                 * t_head := object_get_or_new(t_head, v);
                 */
                let t_head_val = tmp_var_new(proc);
                block_get(proc, *block_idx).extend(vec![
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_head_val),
                        types: IRTypes!("any"),
                        source: IRValue::Variable(t_head),
                        op: IROp::PtrDeref,
                    }),
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(t_head),
                        types: IRType::PTR,
                        source: IRValue::BuiltinProc(BuiltinProc::ObjectGetOrNew),
                        op: IROp::NativeCall(vec![
                            IRValue::Variable(t_head_val),
                            IRValue::String(v.to_string()),
                        ]),
                    }),
                ]);

                t_owned_new = t_owned;
            }
            CSTExpressionKind::Call(c) => {
                t_owned_new = Some(block_obj_call_push(
                    c,
                    block_idx,
                    t_head,
                    proc,
                    shared_proc,
                    cfg,
                ));
            }
            CSTExpressionKind::Collection(c) => match c {
                CSTCollection::Set(s) => {
                    /* t_var := //expr
                     * t_head_val := *t_head;
                     * t_val := set_get_tag_all(t_head_val, t_var)
                     * t_head := &t_val;
                     * _ := invalidate(t_var);
                     */
                    let t_head_val = tmp_var_new(proc);
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
                            target: IRTarget::Variable(t_head_val),
                            types: IRType::PTR,
                            source: IRValue::Variable(t_head),
                            op: IROp::PtrDeref,
                        }),
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_val),
                            types: IRType::SET,
                            source: IRValue::BuiltinProc(BuiltinProc::SetGetTagAll),
                            op: IROp::NativeCall(vec![
                                IRValue::Variable(t_head_val),
                                IRValue::Variable(t_var),
                            ]),
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
                         * // block arr access push
                         * // block arr access push
                         * t_right := t_right + 1;
                         * t_out = slice(t_head, t_left, t_right);
                         * _ := invalidate(t_left);
                         * _ := invalidate(t_right);
                         * t_head := &t_out;
                         */
                        let t_left = tmp_var_new(proc);
                        let t_right = tmp_var_new(proc);

                        if let Some(l_expr) = &range.left {
                            let lhs_owned = block_expr_push(
                                l_expr,
                                block_idx,
                                IRTarget::Variable(t_left),
                                proc,
                                shared_proc,
                                cfg,
                            );
                            if !lhs_owned {
                                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                                    target: IRTarget::Variable(t_left),
                                    types: IRTypes!("any"),
                                    source: IRValue::BuiltinProc(BuiltinProc::Copy),
                                    op: IROp::NativeCall(vec![IRValue::Variable(t_left)]),
                                }));
                            }
                        } else {
                            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_left),
                                types: IRType::NUMBER,
                                source: IRValue::Number(1.into()),
                                op: IROp::Assign,
                            }));
                        }

                        if let Some(r_expr) = &range.right {
                            let rhs_owned = block_expr_push(
                                r_expr,
                                block_idx,
                                IRTarget::Variable(t_right),
                                proc,
                                shared_proc,
                                cfg,
                            );
                            if !rhs_owned {
                                block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                                    target: IRTarget::Variable(t_right),
                                    types: IRTypes!("any"),
                                    source: IRValue::BuiltinProc(BuiltinProc::Copy),
                                    op: IROp::NativeCall(vec![IRValue::Variable(t_right)]),
                                }));
                            }
                        } else {
                            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_right),
                                types: IRType::NUMBER,
                                source: IRValue::Number((-1).into()),
                                op: IROp::Assign,
                            }));
                        }

                        let t_head_val = tmp_var_new(proc);

                        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_head_val),
                            types: IRTypes!("any"),
                            source: IRValue::Variable(t_head),
                            op: IROp::PtrDeref,
                        }));

                        block_arr_access_push(block_idx, t_left, t_head_val, false, proc);
                        block_arr_access_push(block_idx, t_right, t_head_val, true, proc);

                        let t_right_new = tmp_var_new(proc);
                        let t_out = tmp_var_new(proc);

                        block_get(proc, *block_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_right_new),
                                types: IRType::NUMBER | IRType::DOUBLE,
                                source: IRValue::Variable(t_right),
                                op: IROp::Plus(IRValue::Number(1.into())),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Ignore,
                                types: IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                op: IROp::NativeCall(vec![IRValue::Variable(t_right)]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_out),
                                types: IRType::STRING | IRType::LIST,
                                source: IRValue::BuiltinProc(BuiltinProc::Slice),
                                op: IROp::NativeCall(vec![
                                    IRValue::Variable(t_head_val),
                                    IRValue::Variable(t_left),
                                    IRValue::Variable(t_right_new),
                                ]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Ignore,
                                types: IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                op: IROp::NativeCall(vec![IRValue::Variable(t_right_new)]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Ignore,
                                types: IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                op: IROp::NativeCall(vec![IRValue::Variable(t_left)]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head),
                                types: IRType::PTR,
                                source: IRValue::Variable(t_out),
                                op: IROp::PtrAddress,
                            }),
                        ]);

                        t_owned_new = t_owned;
                    } else if l.expressions.len() != 1 {
                        /*  t_expr_list := list_new();
                         *  // for i in l.expressions {
                         *   t_expr := // i;
                         *   _ := list_push(t_expr_list, t_expr);
                         *  // }
                         *  t_head_val := *t_head;
                         *  t_entry := set_get_tag(t_head_val, t_expr_list);
                         *  t_entry_om := t_entry == om;
                         *  if t_entry_om
                         *   goto <dfl_idx>
                         *  else
                         *   goto <assign_idx>
                         *
                         * <dfl_idx>:
                         *  t_head_val := om;
                         *  t_head := &t_head_val;
                         *  goto <follow_idx>
                         *
                         * <assign_idx>:
                         *  t_head := t_entry;
                         *  goto <follow_idx>
                         *
                         * <follow_idx>:
                         *  _ := invalidate(t_expr_list);
                         */
                        let dfl_idx = proc.blocks.add_node(Vec::new());
                        let assign_idx = proc.blocks.add_node(Vec::new());
                        let follow_idx = proc.blocks.add_node(Vec::new());

                        let t_expr_list = tmp_var_new(proc);
                        let t_head_val = tmp_var_new(proc);
                        let t_entry = tmp_var_new(proc);
                        let t_entry_om = tmp_var_new(proc);

                        block_get(proc, *block_idx).extend(vec![IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(t_expr_list),
                            types: IRType::LIST,
                            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                            op: IROp::NativeCall(vec![]),
                        })]);

                        for expr in &l.expressions {
                            let t_expr = tmp_var_new(proc);
                            let expr_owned = block_expr_push(
                                expr,
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
                                    IRValue::Variable(t_expr_list),
                                    IRValue::Variable(t_expr),
                                ]),
                            }));
                        }

                        block_get(proc, *block_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head_val),
                                types: IRTypes!("any"),
                                source: IRValue::Variable(t_head),
                                op: IROp::PtrDeref,
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_entry),
                                types: IRTypes!("any"),
                                source: IRValue::BuiltinProc(BuiltinProc::SetGetTag),
                                op: IROp::NativeCall(vec![
                                    IRValue::Variable(t_head_val),
                                    IRValue::Variable(t_expr_list),
                                ]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_entry_om),
                                types: IRType::BOOL,
                                source: IRValue::Variable(t_entry),
                                op: IROp::Equal(IRValue::Undefined),
                            }),
                            IRStmt::Branch(IRBranch {
                                cond: IRValue::Variable(t_entry_om),
                                success: dfl_idx,
                                failure: assign_idx,
                            }),
                        ]);

                        proc.blocks.add_edge(*block_idx, dfl_idx, ());
                        proc.blocks.add_edge(*block_idx, assign_idx, ());

                        block_get(proc, dfl_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head_val),
                                types: IRType::UNDEFINED,
                                source: IRValue::Undefined,
                                op: IROp::Assign,
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head),
                                types: IRType::PTR,
                                source: IRValue::Variable(t_head_val),
                                op: IROp::PtrAddress,
                            }),
                            IRStmt::Goto(follow_idx),
                        ]);

                        proc.blocks.add_edge(dfl_idx, follow_idx, ());

                        block_get(proc, assign_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head),
                                types: IRType::PTR,
                                source: IRValue::Variable(t_entry),
                                op: IROp::Assign,
                            }),
                            IRStmt::Goto(follow_idx),
                        ]);

                        proc.blocks.add_edge(assign_idx, follow_idx, ());

                        block_get(proc, follow_idx).extend(vec![IRStmt::Assign(IRAssign {
                            target: IRTarget::Ignore,
                            types: IRType::UNDEFINED,
                            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                            op: IROp::NativeCall(vec![IRValue::Variable(t_expr_list)]),
                        })]);

                        *block_idx = follow_idx;

                        t_owned_new = t_owned;
                    } else {
                        /*  t_expr = //expr
                         *  t_expr_type := type_of(t_expr);
                         *  t_expr_is_int := t_expr_type == TYPE_NUM;
                         *  t_expr_is_float := t_expr_type == TYPE_DOUBLE;
                         *  t_expr_is_num := t_expr_is_int || t_expr_is_float;
                         *  t_head_val := *t_head;
                         *  t_head_type := type_of(t_head_val);
                         *  t_head_type_set := t_head_type == TYPE_SET;
                         *  t_head_type_nset := !t_head_type_set;
                         *  t_check := t_expr_is_num && t_head_type_nset;
                         *  if t_check
                         *   goto <access_check_idx>
                         *  else
                         *   goto <set_idx>
                         *
                         * <access_check_idx>:
                         *  t_expr_new = t_expr - 1;
                         *  t_head_val_amount := amount(t_head_val);
                         *  t_acc_check := t_expr_new < t_head_val_amount;
                         *  _ := invalidate(t_head_val_amount);
                         *  if t_acc_check
                         *   goto <access_idx>
                         *  else
                         *   goto <det_dfl_acc_idx>
                         *
                         * <access_idx>:
                         *  t_head = t_head[t_expr_new];
                         *  _ := invalidate(t_expr_new);
                         *  goto <follow_idx>
                         *
                         * <det_dfl_acc_idx>:
                         *  _ := invalidate(t_expr_new);
                         *  goto <det_dfl_idx>
                         *
                         * <det_dfl_idx>:
                         *  t_head_val := om;
                         *  t_head := &t_head_val;
                         *  goto <follow_idx>
                         *
                         * <set_idx>:
                         *  t_entry := set_get_tag(t_head_val, t_expr);
                         *  t_entry_null := t_entry == om;
                         *  if t_entry_null
                         *   goto <det_dfl_idx>
                         *  else
                         *   goto <assign_entry_idx>
                         *
                         * <assign_entry_idx>:
                         *  t_head := t_entry;
                         *  goto <follow_idx>
                         *
                         * <follow_idx>:
                         *  // if expr_owned {
                         *   _ := invalidate(t_expr);
                         *  // }
                         */
                        let t_expr = tmp_var_new(proc);
                        let expr_owned = block_expr_push(
                            &l.expressions[0],
                            block_idx,
                            IRTarget::Variable(t_expr),
                            proc,
                            shared_proc,
                            cfg,
                        );

                        let t_expr_type = tmp_var_new(proc);
                        let t_expr_is_int = tmp_var_new(proc);
                        let t_expr_is_float = tmp_var_new(proc);
                        let t_expr_is_num = tmp_var_new(proc);
                        let t_expr_new = tmp_var_new(proc);
                        let t_head_val = tmp_var_new(proc);
                        let t_head_type = tmp_var_new(proc);
                        let t_head_type_set = tmp_var_new(proc);
                        let t_head_type_nset = tmp_var_new(proc);
                        let t_check = tmp_var_new(proc);

                        let access_check_idx = proc.blocks.add_node(Vec::new());
                        let access_idx = proc.blocks.add_node(Vec::new());
                        let det_dfl_acc_idx = proc.blocks.add_node(vec![]);
                        let det_dfl_idx = proc.blocks.add_node(Vec::new());
                        let set_idx = proc.blocks.add_node(Vec::new());
                        let assign_entry_idx = proc.blocks.add_node(Vec::new());
                        let follow_idx = proc.blocks.add_node(Vec::new());

                        block_get(proc, *block_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_expr_type),
                                types: IRType::TYPE,
                                source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
                                op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_expr_is_int),
                                types: IRType::BOOL,
                                source: IRValue::Variable(t_expr_type),
                                op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_expr_is_float),
                                types: IRType::BOOL,
                                source: IRValue::Variable(t_expr_type),
                                op: IROp::Equal(IRValue::Type(IRType::DOUBLE)),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_expr_is_num),
                                types: IRType::BOOL,
                                source: IRValue::Variable(t_expr_is_int),
                                op: IROp::Or(IRValue::Variable(t_expr_is_float)),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head_val),
                                types: IRTypes!("any"),
                                source: IRValue::Variable(t_head),
                                op: IROp::PtrDeref,
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head_type),
                                types: IRType::TYPE,
                                source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
                                op: IROp::NativeCall(vec![IRValue::Variable(t_head_val)]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head_type_set),
                                types: IRType::BOOL,
                                source: IRValue::Variable(t_head_type),
                                op: IROp::Equal(IRValue::Type(IRType::SET)),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head_type_nset),
                                types: IRType::BOOL,
                                source: IRValue::Variable(t_head_type_set),
                                op: IROp::Not,
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_check),
                                types: IRType::BOOL,
                                source: IRValue::Variable(t_expr_is_num),
                                op: IROp::And(IRValue::Variable(t_head_type_nset)),
                            }),
                            IRStmt::Branch(IRBranch {
                                cond: IRValue::Variable(t_check),
                                success: access_check_idx,
                                failure: set_idx,
                            }),
                        ]);

                        proc.blocks.add_edge(*block_idx, access_check_idx, ());
                        proc.blocks.add_edge(*block_idx, set_idx, ());

                        let t_acc_check = tmp_var_new(proc);
                        let t_head_val_amount = tmp_var_new(proc);

                        block_get(proc, access_check_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_expr_new),
                                types: IRType::NUMBER | IRType::DOUBLE,
                                source: IRValue::Variable(t_expr),
                                op: IROp::Minus(IRValue::Number(1.into())),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head_val_amount),
                                types: IRType::NUMBER,
                                source: IRValue::BuiltinProc(BuiltinProc::Amount),
                                op: IROp::NativeCall(vec![IRValue::Variable(t_head_val)]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_acc_check),
                                types: IRType::BOOL,
                                source: IRValue::Variable(t_expr_new),
                                op: IROp::Less(IRValue::Variable(t_head_val_amount)),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Ignore,
                                types: IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                op: IROp::NativeCall(vec![IRValue::Variable(t_head_val_amount)]),
                            }),
                            IRStmt::Branch(IRBranch {
                                cond: IRValue::Variable(t_acc_check),
                                success: access_idx,
                                failure: det_dfl_acc_idx,
                            }),
                        ]);

                        proc.blocks.add_edge(access_check_idx, access_idx, ());
                        proc.blocks.add_edge(access_check_idx, det_dfl_acc_idx, ());

                        block_get(proc, access_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head),
                                types: IRType::PTR,
                                source: IRValue::Variable(t_head),
                                op: IROp::AccessArray(IRValue::Variable(t_expr_new)),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Ignore,
                                types: IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                op: IROp::NativeCall(vec![IRValue::Variable(t_expr_new)]),
                            }),
                            IRStmt::Goto(follow_idx),
                        ]);

                        proc.blocks.add_edge(access_idx, follow_idx, ());

                        block_get(proc, det_dfl_acc_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Ignore,
                                types: IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                op: IROp::NativeCall(vec![IRValue::Variable(t_expr_new)]),
                            }),
                            IRStmt::Goto(det_dfl_idx),
                        ]);

                        proc.blocks.add_edge(det_dfl_acc_idx, det_dfl_idx, ());

                        block_get(proc, det_dfl_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head_val),
                                types: IRType::UNDEFINED,
                                source: IRValue::Undefined,
                                op: IROp::Assign,
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head),
                                types: IRType::PTR,
                                source: IRValue::Variable(t_head_val),
                                op: IROp::PtrAddress,
                            }),
                            IRStmt::Goto(follow_idx),
                        ]);

                        proc.blocks.add_edge(det_dfl_idx, follow_idx, ());

                        let t_entry = tmp_var_new(proc);
                        let t_entry_is_om = tmp_var_new(proc);

                        block_get(proc, set_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_entry),
                                types: IRType::PTR | IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::SetGetTag),
                                op: IROp::NativeCall(vec![
                                    IRValue::Variable(t_head_val),
                                    IRValue::Variable(t_expr),
                                ]),
                            }),
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_entry_is_om),
                                types: IRType::BOOL,
                                source: IRValue::Variable(t_entry),
                                op: IROp::Equal(IRValue::Undefined),
                            }),
                            IRStmt::Branch(IRBranch {
                                cond: IRValue::Variable(t_entry_is_om),
                                success: det_dfl_idx,
                                failure: assign_entry_idx,
                            }),
                        ]);

                        proc.blocks.add_edge(set_idx, det_dfl_idx, ());
                        proc.blocks.add_edge(set_idx, assign_entry_idx, ());

                        block_get(proc, assign_entry_idx).extend(vec![
                            IRStmt::Assign(IRAssign {
                                target: IRTarget::Variable(t_head),
                                types: IRType::PTR,
                                source: IRValue::Variable(t_entry),
                                op: IROp::Assign,
                            }),
                            IRStmt::Goto(follow_idx),
                        ]);

                        proc.blocks.add_edge(assign_entry_idx, follow_idx, ());

                        if expr_owned {
                            block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
                                target: IRTarget::Ignore,
                                types: IRType::UNDEFINED,
                                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                                op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
                            }));
                        }

                        *block_idx = follow_idx;

                        t_owned_new = t_owned;
                    }
                }
                _ => panic!("accessible collection should be list or set"),
            },
            _ => unreachable!(),
        }

        if let Some(t_o) = t_owned
            && t_owned_new != t_owned
        {
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                op: IROp::NativeCall(vec![IRValue::Variable(t_o)]),
            }));
        }

        t_owned = t_owned_new;
    }

    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::PTR,
        source: IRValue::Variable(t_head),
        op: IROp::Assign,
    }));

    t_owned
}

/// Emits IR to evaluate the access chain `a` and write the dereferenced value into `target`.
///
/// Delegates to `block_access_ref_push` to walk the chain, then dereferences
/// the resulting pointer. Returns true if the result is owned and must be
/// invalidated by the caller after use.
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
