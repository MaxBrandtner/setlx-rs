use petgraph::stable_graph::NodeIndex;

use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::op_expr::block_op_plus_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_set_mem_op_dfl_push(
    block_idx: &mut NodeIndex,
    tmp_left: IRVar,
    tmp_left_owned: bool,
    tmp_right: IRVar,
    op_fn: fn(&mut NodeIndex, IRVar, IRTarget, &mut IRProcedure, &mut IRSharedProc, &mut IRCfg),
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /* t_len := amount(tmp_right);
     * t_len_zero := t_len == 0;
     * if t_len_zero
     *  goto <assign_idx>
     * else
     *  goto <prod_mem_idx>
     *
     * <assign_idx>:
     *  target := tmp_left;
     *  goto <follow_idx>
     *
     * <prod_mem_idx>:
     *  _ := invalidate(tmp_left);
     * target := // block_prod_mem_push
     * goto <follow_idx>
     *
     * <follow_idx>:
     */
    let t_len = tmp_var_new(proc);
    let t_len_zero = tmp_var_new(proc);

    let assign_idx = proc.blocks.add_node(Vec::new());
    let mut prod_mem_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_right)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len_zero),
            types: IRType::BOOL,
            source: IRValue::Variable(t_len),
            op: IROp::Equal(IRValue::Number(0.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_len_zero),
            success: assign_idx,
            failure: prod_mem_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, assign_idx, ());
    proc.blocks.add_edge(*block_idx, prod_mem_idx, ());

    block_get(proc, assign_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target,
            types: IRTypes!("any"),
            source: IRValue::Variable(tmp_left),
            op: IROp::Assign,
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(assign_idx, follow_idx, ());

    if tmp_left_owned {
        block_get(proc, prod_mem_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(tmp_left)]),
        }));
    }

    op_fn(&mut prod_mem_idx, tmp_right, target, proc, shared_proc, cfg);

    block_get(proc, prod_mem_idx).push(IRStmt::Goto(follow_idx));
    proc.blocks.add_edge(prod_mem_idx, follow_idx, ());

    *block_idx = follow_idx;
}

pub fn block_sum_mem_push(
    block_idx: &mut NodeIndex,
    t_set: IRVar,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /*  t_amount := amount(t_set);
     *  t_amount_zero := t_amount == 0;
     *  if t_amount_zero
     *   goto <out_om_idx>
     *  else
     *   goto <t_check_idx>
     *
     * <out_om_idx>:
     *  t_out := om;
     *  goto <follow_idx>
     *
     * <t_check_idx>:
     *  t_set_type := type_of(t_set);
     *  t_set_type_set := t_set_type == TYPE_SET;
     *  if t_set_type_set:
     *   goto <set_get_zero_idx>
     *  else
     *   goto <arr_get_zero_idx>
     *
     * <set_get_zero_idx>:
     *  t_set_zero := set_borrow(t_set, true);
     *  _ := mark_persist(t_set_zero);
     *  goto <set_set_idx>
     *
     * <arr_get_zero_idx>:
     *  t_set_zero := t_set[0];
     *  goto <set_set_idx>
     *
     * <set_set_idx>:
     *  t_set_zero_type := type_of(t_set_zero);
     *  t_set_zero_type_set := t_set_zero_type == TYPE_SET;
     *  t_set_iter := iter_new(t_set);
     *  t_i := om;
     *  t_i_addr := &t_i;
     *  _ := iter_next(t_set_iter, t_i_addr); // skip first entry
     *  if t_set_zero_type_set
     *   goto <set_union_idx>
     *  else
     *   goto <mult_idx>
     *
     * <set_union_idx>:
     *  t_out := copy(t_set_zero);
     *  goto <set_iter_idx>
     *
     * <set_iter_idx>:
     *  t_i_cond := iter_next(t_set_iter, t_i_addr);
     *  if t_i_cond
     *   goto <set_iter_loop_idx>
     *  else
     *   goto <follow_idx>
     *
     * <set_iter_loop_idx>:
     *  t_i_iter := iter_new(t_i);
     *  goto <set_iter_loop_iter_idx>
     *
     * <set_iter_loop_iter_idx>:
     *  t_j := om;
     *  t_j_addr := &t_j;
     *  t_j_cond := iter_next(t_i_iter, t_j_addr);
     *  if t_j_cond
     *   goto <set_iter_loop_loop_idx>
     *  else
     *   goto <set_iter_idx>
     *
     * <set_iter_loop_loop_idx>:
     *  t_j_copy := copy(t_j);
     *  _ := set_insert(t_out, t_j_copy);
     *  goto <set_iter_loop_idx>
     *
     * <mult_idx>:
     *  t_out := copy(t_set_zero);
     *  goto <mult_loop_idx>
     *
     * <mult_loop_idx>:
     *  t_cond := iter_next(t_set_iter, t_i_addr);
     *  if t_cond
     *   goto <mult_loop_mult_idx>
     *  else
     *   goto <follow_idx>
     *
     * <mult_loop_mult_idx>:
     *  t_out_new := // block_op_plus_push (t_out, t_i)
     *  _ := invalidate(t_out);
     *  t_out := t_out_new;
     *  goto <mult_loop_idx>
     *
     * <follow_idx>:
     *  target := t_out;
     */
    let out_om_idx = proc.blocks.add_node(Vec::new());
    let set_check_idx = proc.blocks.add_node(Vec::new());
    let set_get_zero_idx = proc.blocks.add_node(Vec::new());
    let arr_get_zero_idx = proc.blocks.add_node(Vec::new());
    let set_set_idx = proc.blocks.add_node(Vec::new());
    let set_union_idx = proc.blocks.add_node(Vec::new());
    let set_iter_idx = proc.blocks.add_node(Vec::new());
    let set_iter_loop_idx = proc.blocks.add_node(Vec::new());
    let set_iter_loop_iter_idx = proc.blocks.add_node(Vec::new());
    let set_iter_loop_loop_idx = proc.blocks.add_node(Vec::new());
    let mult_idx = proc.blocks.add_node(Vec::new());
    let mult_loop_idx = proc.blocks.add_node(Vec::new());
    let mut mult_loop_mult_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    let t_amount = tmp_var_new(proc);
    let t_amount_zero = tmp_var_new(proc);
    let t_set_type = tmp_var_new(proc);
    let t_set_type_set = tmp_var_new(proc);
    let t_set_zero = tmp_var_new(proc);
    let t_set_zero_type = tmp_var_new(proc);
    let t_set_zero_type_set = tmp_var_new(proc);
    let t_set_iter = tmp_var_new(proc);
    let t_i = tmp_var_new(proc);
    let t_i_addr = tmp_var_new(proc);
    let t_out = tmp_var_new(proc);
    let t_i_iter = tmp_var_new(proc);
    let t_j = tmp_var_new(proc);
    let t_j_addr = tmp_var_new(proc);
    let t_j_cond = tmp_var_new(proc);
    let t_j_copy = tmp_var_new(proc);
    let t_i_cond = tmp_var_new(proc);
    let t_out_new = tmp_var_new(proc);
    let t_cond = tmp_var_new(proc);

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_amount),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_amount_zero),
            types: IRType::BOOL,
            source: IRValue::Variable(t_amount),
            op: IROp::Equal(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_amount)]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_amount_zero),
            success: out_om_idx,
            failure: set_check_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, out_om_idx, ());
    proc.blocks.add_edge(*block_idx, set_check_idx, ());

    block_get(proc, out_om_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(out_om_idx, follow_idx, ());

    block_get(proc, set_check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set_type_set),
            types: IRType::BOOL,
            source: IRValue::Variable(t_set_type),
            op: IROp::Equal(IRValue::Type(IRType::SET)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_set_type_set),
            success: set_get_zero_idx,
            failure: arr_get_zero_idx,
        }),
    ]);

    proc.blocks.add_edge(set_check_idx, set_get_zero_idx, ());
    proc.blocks.add_edge(set_check_idx, arr_get_zero_idx, ());

    block_get(proc, set_get_zero_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set_zero),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::SetBorrow),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set), IRValue::Bool(true)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::MarkPersist),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set_zero)]),
        }),
        IRStmt::Goto(set_set_idx),
    ]);

    proc.blocks.add_edge(set_get_zero_idx, set_set_idx, ());

    block_get(proc, arr_get_zero_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set_zero),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_set),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Goto(set_set_idx),
    ]);

    proc.blocks.add_edge(arr_get_zero_idx, set_set_idx, ());

    block_get(proc, set_set_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set_zero_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set_zero)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set_zero_type_set),
            types: IRType::BOOL,
            source: IRValue::Variable(t_set_zero_type),
            op: IROp::Equal(IRValue::Type(IRType::SET)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set_iter),
            types: IRType::ITERATOR,
            source: IRValue::BuiltinProc(BuiltinProc::IterNew),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set)]),
        }),
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
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::IterNext),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_set_iter),
                IRValue::Variable(t_i_addr),
            ]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_set_zero_type_set),
            success: set_union_idx,
            failure: mult_idx,
        }),
    ]);

    proc.blocks.add_edge(set_set_idx, set_union_idx, ());
    proc.blocks.add_edge(set_set_idx, mult_idx, ());

    block_get(proc, set_union_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set_zero)]),
        }),
        IRStmt::Goto(set_iter_idx),
    ]);

    proc.blocks.add_edge(set_union_idx, set_iter_idx, ());

    block_get(proc, set_iter_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_cond),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::IterNext),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_set_iter),
                IRValue::Variable(t_i_addr),
            ]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_i_cond),
            success: set_iter_loop_idx,
            failure: follow_idx,
        }),
    ]);

    proc.blocks.add_edge(set_iter_idx, set_iter_loop_idx, ());
    proc.blocks.add_edge(set_iter_idx, follow_idx, ());

    block_get(proc, set_iter_loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_iter),
            types: IRType::ITERATOR,
            source: IRValue::BuiltinProc(BuiltinProc::IterNew),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Goto(set_iter_loop_iter_idx),
    ]);

    proc.blocks
        .add_edge(set_iter_loop_idx, set_iter_loop_iter_idx, ());

    block_get(proc, set_iter_loop_iter_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_j),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_j_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_j),
            op: IROp::PtrAddress,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_j_cond),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::IterNext),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_i_iter),
                IRValue::Variable(t_j_addr),
            ]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_j_cond),
            success: set_iter_loop_loop_idx,
            failure: set_iter_idx,
        }),
    ]);

    proc.blocks
        .add_edge(set_iter_loop_idx, set_iter_loop_loop_idx, ());
    proc.blocks.add_edge(set_iter_loop_idx, set_iter_idx, ());

    block_get(proc, set_iter_loop_loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_j_copy),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_j)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::SetInsert),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out), IRValue::Variable(t_j_copy)]),
        }),
        IRStmt::Goto(set_iter_loop_iter_idx),
    ]);

    proc.blocks
        .add_edge(set_iter_loop_loop_idx, set_iter_loop_iter_idx, ());

    block_get(proc, mult_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set_zero)]),
        }),
        IRStmt::Goto(mult_loop_idx),
    ]);

    proc.blocks.add_edge(mult_idx, mult_loop_idx, ());

    block_get(proc, mult_loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cond),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::IterNext),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_set_iter),
                IRValue::Variable(t_i_addr),
            ]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cond),
            success: mult_loop_mult_idx,
            failure: follow_idx,
        }),
    ]);

    proc.blocks.add_edge(mult_loop_idx, mult_loop_mult_idx, ());
    proc.blocks.add_edge(mult_loop_idx, follow_idx, ());

    block_op_plus_push(
        &mut mult_loop_mult_idx,
        IRTarget::Variable(t_out_new),
        t_out,
        t_i,
        proc,
        shared_proc,
        cfg,
    );

    block_get(proc, mult_loop_mult_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("plus"),
            source: IRValue::Variable(t_out_new),
            op: IROp::Assign,
        }),
        IRStmt::Goto(mult_loop_idx),
    ]);

    proc.blocks.add_edge(mult_loop_mult_idx, mult_loop_idx, ());

    block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRTypes!("any"),
        source: IRValue::Variable(t_out),
        op: IROp::Assign,
    }));

    *block_idx = follow_idx;
}

pub fn block_prod_mem_push(
    block_idx: &mut NodeIndex,
    t_set: IRVar,
    target: IRTarget,
    proc: &mut IRProcedure,
    _: &mut IRSharedProc,
    _: &mut IRCfg,
) {
    /*  t_amount := amount(t_set);
     *  t_amount_zero := t_amount == 0;
     *  _ := invalidate(t_amount);
     *  if t_amount_zero
     *   goto <out_om_idx>
     *  else
     *   goto <set_check_idx>
     *
     * <out_om_idx>:
     *  t_out := om;
     *  goto <follow_idx>
     *
     * <set_check_idx>
     *  t_set_type := type_of(t_set);
     *  t_set_type_set := t_set_type == TYPE_SET;
     *  if t_set_type_set:
     *   goto <set_get_zero_idx>
     *  else
     *   goto <arr_get_zero_idx>
     *
     * <set_get_zero_idx>:
     *  t_set_zero := set_borrow(t_set, true);
     *  _ := mark_persist(t_set_zero);
     *  goto <set_set_idx>
     *
     * <arr_get_zero_idx>:
     *  t_set_zero := t_set[0];
     *  goto <set_set_idx>
     *
     * <set_set_idx>:
     *  t_set_zero_type := type_of(t_set_zero);
     *  t_set_zero_type_set := t_set_zero_type == TYPE_SET;
     *  t_set_iter := iter_new(t_set);
     *  t_i := om;
     *  t_i_addr := &t_i;
     *  _ := iter_next(t_set_iter, t_i_addr); // skip first entry
     *  if t_set_zero_type_set
     *   goto <set_union_idx>
     *  else
     *   goto <mult_idx>
     *
     * <set_union_idx>:
     *  t_out := copy(t_set_zero);
     *  goto <set_iter_idx>
     *
     * <set_iter_idx>:
     *  t_i_cond := iter_next(t_set_iter, t_i_addr);
     *  if t_i_cond
     *   goto <set_iter_loop_idx>
     *  else
     *   goto <follow_idx>
     *
     * <set_iter_loop_idx>:
     *  t_out_iter := iter_new(t_out);
     *  t_out_new := set_new();
     *  goto <set_iter_loop_iter_idx>
     *
     * <set_iter_loop_iter_idx>:
     *  t_j := om;
     *  t_j_addr := &t_j;
     *  t_j_cond := iter_next(t_out_iter, t_j_addr);
     *  if t_j_cond
     *   goto <set_iter_loop_loop_idx>
     *  else
     *   goto <set_iter_assign_idx>
     *
     * <set_iter_loop_loop_idx>:
     *  t_cont := contains(t_i, t_j);
     *  if t_cont
     *   goto <set_iter_loop_insert_idx>
     *  else
     *   goto <set_iter_loop_iter_idx>
     *
     * <set_iter_loop_insert_idx>:
     *  t_j_copy := copy(t_j);
     *  _ := set_insert(t_out_new, t_j_copy);
     *  goto <set_iter_loop_iter_idx>
     *
     * <set_iter_assign_idx>:
     *  _ := invalidate(t_out);
     *  t_out := t_out_new;
     *  goto <set_iter_idx>
     *
     * <mult_idx>:
     *  t_out := copy(t_set_zero);
     *  goto <mult_loop_idx>
     *
     * <mult_loop_idx>:
     *  t_cond := iter_next(t_set_iter, t_i_addr);
     *  if t_cond
     *   goto <mult_loop_mult_idx>
     *  else
     *   goto <follow_idx>
     *
     * <mult_loop_mult_idx>:
     *  t_out_new := t_out * t_i;
     *  _ := invalidate(t_out);
     *  t_out := t_out_new;
     *  goto <mult_loop_idx>
     *
     * <follow_idx>:
     *  target := t_out;
     */
    let out_om_idx = proc.blocks.add_node(Vec::new());
    let set_check_idx = proc.blocks.add_node(Vec::new());
    let set_get_zero_idx = proc.blocks.add_node(Vec::new());
    let arr_get_zero_idx = proc.blocks.add_node(Vec::new());
    let set_set_idx = proc.blocks.add_node(Vec::new());
    let set_union_idx = proc.blocks.add_node(Vec::new());
    let set_iter_idx = proc.blocks.add_node(Vec::new());
    let set_iter_loop_idx = proc.blocks.add_node(Vec::new());
    let set_iter_loop_iter_idx = proc.blocks.add_node(Vec::new());
    let set_iter_loop_loop_idx = proc.blocks.add_node(Vec::new());
    let set_iter_loop_insert_idx = proc.blocks.add_node(Vec::new());
    let set_iter_assign_idx = proc.blocks.add_node(Vec::new());
    let mult_idx = proc.blocks.add_node(Vec::new());
    let mult_loop_idx = proc.blocks.add_node(Vec::new());
    let mult_loop_mult_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    let t_set_type = tmp_var_new(proc);
    let t_amount = tmp_var_new(proc);
    let t_amount_zero = tmp_var_new(proc);
    let t_set_type_set = tmp_var_new(proc);
    let t_set_zero = tmp_var_new(proc);
    let t_set_zero_type = tmp_var_new(proc);
    let t_set_zero_type_set = tmp_var_new(proc);
    let t_set_iter = tmp_var_new(proc);
    let t_i = tmp_var_new(proc);
    let t_i_addr = tmp_var_new(proc);
    let t_out = tmp_var_new(proc);
    let t_i_cond = tmp_var_new(proc);
    let t_out_iter = tmp_var_new(proc);
    let t_out_new = tmp_var_new(proc);
    let t_j = tmp_var_new(proc);
    let t_j_addr = tmp_var_new(proc);
    let t_j_cond = tmp_var_new(proc);
    let t_cont = tmp_var_new(proc);
    let t_j_copy = tmp_var_new(proc);
    let t_cond = tmp_var_new(proc);

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_amount),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_amount_zero),
            types: IRType::BOOL,
            source: IRValue::Variable(t_amount),
            op: IROp::Equal(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_amount)]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_amount_zero),
            success: out_om_idx,
            failure: set_check_idx,
        }),
    ]);

    proc.blocks.add_edge(*block_idx, out_om_idx, ());
    proc.blocks.add_edge(*block_idx, set_check_idx, ());

    block_get(proc, out_om_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(out_om_idx, follow_idx, ());

    block_get(proc, set_check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set_type_set),
            types: IRType::BOOL,
            source: IRValue::Variable(t_set_type),
            op: IROp::Equal(IRValue::Type(IRType::SET)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_set_type_set),
            success: set_get_zero_idx,
            failure: arr_get_zero_idx,
        }),
    ]);

    proc.blocks.add_edge(set_check_idx, set_get_zero_idx, ());
    proc.blocks.add_edge(set_check_idx, arr_get_zero_idx, ());

    block_get(proc, set_get_zero_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set_zero),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::SetBorrow),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set), IRValue::Bool(true)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::MarkPersist),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set_zero)]),
        }),
        IRStmt::Goto(set_set_idx),
    ]);

    proc.blocks.add_edge(set_get_zero_idx, set_set_idx, ());

    block_get(proc, arr_get_zero_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set_zero),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_set),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Goto(set_set_idx),
    ]);

    proc.blocks.add_edge(arr_get_zero_idx, set_set_idx, ());

    block_get(proc, set_set_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set_zero_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set_zero)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set_zero_type_set),
            types: IRType::BOOL,
            source: IRValue::Variable(t_set_zero_type),
            op: IROp::Equal(IRValue::Type(IRType::SET)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set_iter),
            types: IRType::ITERATOR,
            source: IRValue::BuiltinProc(BuiltinProc::IterNew),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set)]),
        }),
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
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::IterNext),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_set_iter),
                IRValue::Variable(t_i_addr),
            ]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_set_zero_type_set),
            success: set_union_idx,
            failure: mult_idx,
        }),
    ]);

    proc.blocks.add_edge(set_set_idx, set_union_idx, ());
    proc.blocks.add_edge(set_set_idx, mult_idx, ());

    block_get(proc, set_union_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set_zero)]),
        }),
        IRStmt::Goto(set_iter_idx),
    ]);

    proc.blocks.add_edge(set_union_idx, set_iter_idx, ());

    block_get(proc, set_iter_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_cond),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::IterNext),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_set_iter),
                IRValue::Variable(t_i_addr),
            ]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_i_cond),
            success: set_iter_loop_idx,
            failure: follow_idx,
        }),
    ]);

    proc.blocks.add_edge(set_iter_idx, set_iter_loop_idx, ());
    proc.blocks.add_edge(set_iter_idx, follow_idx, ());

    block_get(proc, set_iter_loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out_iter),
            types: IRType::ITERATOR,
            source: IRValue::BuiltinProc(BuiltinProc::IterNew),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out_new),
            types: IRType::SET,
            source: IRValue::BuiltinProc(BuiltinProc::SetNew),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Goto(set_iter_loop_iter_idx),
    ]);

    proc.blocks
        .add_edge(set_iter_loop_idx, set_iter_loop_iter_idx, ());

    block_get(proc, set_iter_loop_iter_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_j),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_j_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_j),
            op: IROp::PtrAddress,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_j_cond),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::IterNext),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_out_iter),
                IRValue::Variable(t_j_addr),
            ]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_j_cond),
            success: set_iter_loop_loop_idx,
            failure: set_iter_assign_idx,
        }),
    ]);

    proc.blocks
        .add_edge(set_iter_loop_iter_idx, set_iter_loop_loop_idx, ());
    proc.blocks
        .add_edge(set_iter_loop_iter_idx, set_iter_assign_idx, ());

    block_get(proc, set_iter_loop_loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cont),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::Contains),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i), IRValue::Variable(t_j)]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cont),
            success: set_iter_loop_insert_idx,
            failure: set_iter_loop_iter_idx,
        }),
    ]);

    proc.blocks
        .add_edge(set_iter_loop_loop_idx, set_iter_loop_insert_idx, ());
    proc.blocks
        .add_edge(set_iter_loop_loop_idx, set_iter_loop_iter_idx, ());

    block_get(proc, set_iter_loop_insert_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_j_copy),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_j)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::SetInsert),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_out_new),
                IRValue::Variable(t_j_copy),
            ]),
        }),
        IRStmt::Goto(set_iter_loop_iter_idx),
    ]);

    proc.blocks
        .add_edge(set_iter_loop_insert_idx, set_iter_loop_iter_idx, ());

    block_get(proc, set_iter_assign_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_out_new),
            op: IROp::Assign,
        }),
        IRStmt::Goto(set_iter_idx),
    ]);

    proc.blocks.add_edge(set_iter_assign_idx, set_iter_idx, ());

    block_get(proc, mult_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set_zero)]),
        }),
        IRStmt::Goto(mult_loop_idx),
    ]);

    proc.blocks.add_edge(mult_idx, mult_loop_idx, ());

    block_get(proc, mult_loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cond),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::IterNext),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_set_iter),
                IRValue::Variable(t_i_addr),
            ]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cond),
            success: mult_loop_mult_idx,
            failure: follow_idx,
        }),
    ]);

    proc.blocks.add_edge(mult_loop_idx, mult_loop_mult_idx, ());
    proc.blocks.add_edge(mult_loop_idx, follow_idx, ());

    block_get(proc, mult_loop_mult_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out_new),
            types: IRTypes!("mul"),
            source: IRValue::Variable(t_out),
            op: IROp::Mult(IRValue::Variable(t_i)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("mul"),
            source: IRValue::Variable(t_out_new),
            op: IROp::Assign,
        }),
        IRStmt::Goto(mult_loop_idx),
    ]);

    proc.blocks.add_edge(mult_loop_mult_idx, mult_loop_idx, ());

    block_get(proc, follow_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRTypes!("any"),
        source: IRValue::Variable(t_out),
        op: IROp::Assign,
    }));

    *block_idx = follow_idx;
}
