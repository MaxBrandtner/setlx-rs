use petgraph::stable_graph::NodeIndex;
use std::cell::RefCell;
use std::rc::Rc;

use crate::IRTypes;
use crate::builtin::*;
use crate::interp::heap::InterpVal;
use crate::interp::stack::{InterpStackEntry, InterpStackVar};
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::op_expr::{ObjOverloadRhs, block_obj_overload_push};
use crate::ir::lower::util::{block_get, tmp_var_new};

fn print_params_push(block_idx: &mut NodeIndex, t_target: IRVar, proc: &mut IRProcedure) {
    /* t_i := 0;
     * t_len := amount(params);
     * goto <check_idx>:
     *
     * <check_idx>:
     * t_check := t_i < t_len;
     * if t_check
     *  goto <loop_idx>
     * else
     *  goto <follow_idx>
     *
     * <loop_idx>:
     * t_pp := params[t_i];
     * t_p := *t_pp;
     * t_s := serialize(t_p);
     * t_target_new := t_target + t_s;
     * _ := invalidate(t_target);
     * _ := invalidate(t_s);
     * t_target := t_target_new;
     * t_i_new := t_i + 1;
     * _ := invalidate(t_i);
     * t_i := t_i_new;
     * goto <check_idx>
     *
     * <follow_idx>
     *  _ := invalidate(t_i);
     *  _ := invalidate(t_len);
     */
    let check_idx = proc.blocks.add_node(Vec::new());
    let loop_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    let t_i = tmp_var_new(proc);
    let t_len = tmp_var_new(proc);
    let t_check = tmp_var_new(proc);
    let t_pp = tmp_var_new(proc);
    let t_p = tmp_var_new(proc);
    let t_s = tmp_var_new(proc);
    let t_target_new = tmp_var_new(proc);
    let t_i_new = tmp_var_new(proc);

    block_get(proc, *block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRType::NUMBER,
            source: IRValue::Number(0.into()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::BuiltinVar(BuiltinVar::Params)]),
        }),
        IRStmt::Goto(check_idx),
    ]);

    proc.blocks.add_edge(*block_idx, check_idx, ());

    block_get(proc, check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_i),
            op: IROp::Less(IRValue::Variable(t_len)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_check),
            success: loop_idx,
            failure: follow_idx,
        }),
    ]);

    proc.blocks.add_edge(check_idx, loop_idx, ());
    proc.blocks.add_edge(check_idx, follow_idx, ());

    block_get(proc, loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_pp),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Variable(t_i)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_pp),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_s),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Serialize),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_target_new),
            types: IRType::STRING,
            source: IRValue::Variable(t_target),
            op: IROp::Plus(IRValue::Variable(t_s)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_target)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_s)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_target),
            types: IRType::STRING,
            source: IRValue::Variable(t_target_new),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_new),
            types: IRType::UNDEFINED,
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
            types: IRType::STRING,
            source: IRValue::Variable(t_i_new),
            op: IROp::Assign,
        }),
        IRStmt::Goto(check_idx),
    ]);

    proc.blocks.add_edge(loop_idx, check_idx, ());

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
            op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
        }),
    ]);

    *block_idx = follow_idx;
}

fn n_print_stub_new(is_stderr: bool) -> Rc<RefCell<IRProcedure>> {
    /*
     * <init_idx>:
     *  t_out := "";
     *  t_out := // print_params_push
     *  _ := print_stdout(t_out);
     *  _ := invalidate(t_out);
     *  return om;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag(if is_stderr {
        "nPrintErr"
    } else {
        "nPrint"
    })));

    let mut init_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    proc.borrow_mut().start_block = init_idx;

    let t_out = tmp_var_new(&mut proc.borrow_mut());

    block_get(&mut proc.borrow_mut(), init_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_out),
        types: IRType::STRING,
        source: IRValue::String(String::from("")),
        op: IROp::Assign,
    }));

    print_params_push(&mut init_idx, t_out, &mut proc.borrow_mut());
    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(if is_stderr {
                BuiltinProc::PrintStderr
            } else {
                BuiltinProc::PrintStdout
            }),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Return(IRValue::Undefined),
    ]);

    proc.borrow_mut().end_block = init_idx;

    proc
}

fn print_stub_new(is_stderr: bool) -> Rc<RefCell<IRProcedure>> {
    /*
     * <init_idx>:
     *  t_out := "";
     *  t_out := // print_params_push
     *  t_out_new := t_out + "\n";
     *  _ := invalidate(t_out);
     *  _ := print_stdout(t_out_new);
     *  _ := invalidate(t_out_new);
     *  return om;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag(if is_stderr {
        "printErr"
    } else {
        "print"
    })));

    let mut init_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    proc.borrow_mut().start_block = init_idx;

    let t_out = tmp_var_new(&mut proc.borrow_mut());
    let t_out_new = tmp_var_new(&mut proc.borrow_mut());

    block_get(&mut proc.borrow_mut(), init_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_out),
        types: IRType::STRING,
        source: IRValue::String(String::from("")),
        op: IROp::Assign,
    }));

    print_params_push(&mut init_idx, t_out, &mut proc.borrow_mut());
    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out_new),
            types: IRType::STRING,
            source: IRValue::Variable(t_out),
            op: IROp::Plus(IRValue::String("\n".to_string())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::PrintStdout),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out_new)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out_new)]),
        }),
        IRStmt::Return(IRValue::Undefined),
    ]);

    proc.borrow_mut().end_block = init_idx;

    proc
}

fn abort_stub_new() -> Rc<RefCell<IRProcedure>> {
    /*
     * <init_idx>:
     *  t_out := "";
     *  t_out := // print_params_push
     *  t_out_new := "abort: " + t_out;
     *  _ := invalidate(t_out);
     *  _ := throw(3, t_out_new);
     *  unreachable;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("abort")));

    let mut init_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    proc.borrow_mut().start_block = init_idx;

    let t_out = tmp_var_new(&mut proc.borrow_mut());
    let t_out_new = tmp_var_new(&mut proc.borrow_mut());

    block_get(&mut proc.borrow_mut(), init_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_out),
        types: IRType::STRING,
        source: IRValue::String(String::from("")),
        op: IROp::Assign,
    }));

    print_params_push(&mut init_idx, t_out, &mut proc.borrow_mut());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out_new),
            types: IRType::STRING,
            source: IRValue::String(String::from("abort: ")),
            op: IROp::Plus(IRValue::Variable(t_out)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Throw),
            op: IROp::NativeCall(vec![
                IRValue::Number(3.into()),
                IRValue::Variable(t_out_new),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    proc.borrow_mut().end_block = init_idx;

    proc
}

fn assert_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  if t_p1
     *   goto <end_idx>
     *  else
     *   goto <throw_prep_idx>
     *
     * <throw_prep_idx>:
     *  t_p2_addr := params[1];
     *  t_p2 := *t_p2_addr;
     *  t_p2_type := type_of(t_p2);
     *  t_p2_type_str := t_p2_type == TYPE_STRING;
     *  if t_p_2_type_str
     *   goto <str_idx>
     *  else
     *   goto <literal_idx>
     *
     * <str_idx>:
     *  t_msg := "Assertion failed: \"" + t_p2;
     *  t_msg_new := t_msg + "\"";
     *  _ := invalidate(t_msg);
     *  t_msg := t_msg_new;
     *  goto <throw_idx>
     *
     * <literal_idx>:
     *  t_msg := "Assertion failed: " + t_p2;
     *  goto <throw_idx>
     *
     * <throw_idx>:
     *  _ := throw(3, t_msg);
     *  unreachable;
     *
     * <end_idx>:
     *  return om;
     */

    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("assert")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p2_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p2 = tmp_var_new(&mut proc.borrow_mut());
    let t_msg = tmp_var_new(&mut proc.borrow_mut());
    let t_msg_new = tmp_var_new(&mut proc.borrow_mut());
    let t_p2_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p2_type_str = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let end_idx = proc
        .borrow_mut()
        .blocks
        .add_node(vec![IRStmt::Return(IRValue::Undefined)]);
    let throw_prep_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let str_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let literal_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let throw_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1),
            success: end_idx,
            failure: throw_prep_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, end_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(init_idx, throw_prep_idx, ());

    block_get(&mut proc.borrow_mut(), throw_prep_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p2_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p2)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2_type_str),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p2_type),
            op: IROp::Equal(IRValue::Type(IRType::STRING)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p2_type_str),
            success: str_idx,
            failure: literal_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(throw_prep_idx, str_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(throw_prep_idx, literal_idx, ());

    block_get(&mut proc.borrow_mut(), str_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_msg),
            types: IRType::STRING,
            source: IRValue::String("Assertion failed: \"".to_string()),
            op: IROp::Plus(IRValue::Variable(t_p2)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_msg_new),
            types: IRType::STRING,
            source: IRValue::Variable(t_msg),
            op: IROp::Plus(IRValue::String("\"".to_string())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_msg)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_msg),
            types: IRType::STRING,
            source: IRValue::Variable(t_msg_new),
            op: IROp::Assign,
        }),
        IRStmt::Goto(throw_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(str_idx, throw_idx, ());

    block_get(&mut proc.borrow_mut(), literal_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_msg),
            types: IRType::STRING,
            source: IRValue::String("Assertion failed: ".to_string()),
            op: IROp::Plus(IRValue::Variable(t_p2)),
        }),
        IRStmt::Goto(throw_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(literal_idx, throw_idx, ());

    block_get(&mut proc.borrow_mut(), throw_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Throw),
            op: IROp::NativeCall(vec![IRValue::Number(3.into()), IRValue::Variable(t_msg)]),
        }),
        IRStmt::Unreachable,
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = end_idx;

    proc
}

fn first_stub_new(tag: &str) -> Rc<RefCell<IRProcedure>> {
    /* t_p1_addr := params[0];
     * t_p1 := *t_p1_addr;
     * t_p1_type := type_of(t_p1);
     * t_p1_set := t_p1_type == TYPE_SET;
     * if t_p1_set
     *  goto <arb_idx>
     * else
     *  goto <arr_idx>
     *
     * <arb_idx>:
     *  t_out := set_borrow(t_p1, true);
     *  _ := mark_persist(t_out);
     *  t_out := copy(t_out);
     *  goto <ret_idx>
     *
     * <arr_idx>:
     *  t_amount := amount(t_p1);
     *  t_amount_zero := t_amount == 0;
     *  _ := invalidate(t_amount);
     *  if t_amount_zero
     *   goto <out_om_idx>
     *  else
     *   goto <arr_acc_idx>
     *
     * <out_om_idx>:
     *  t_out := om;
     *  goto <ret_idx>
     *
     * <arr_acc_idx>:
     *  t_out := t_p1[0];
     *  t_out := copy(t_out);
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  return t_out;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag(tag)));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_set = tmp_var_new(&mut proc.borrow_mut());
    let t_out = tmp_var_new(&mut proc.borrow_mut());
    let t_amount = tmp_var_new(&mut proc.borrow_mut());
    let t_amount_zero = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let arb_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let arr_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let out_om_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let arr_acc_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_set),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::SET)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_set),
            success: arb_idx,
            failure: arr_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, arb_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, arr_idx, ());

    block_get(&mut proc.borrow_mut(), arb_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::SetBorrow),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1), IRValue::Bool(true)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::MarkPersist),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(arb_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), arr_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_amount),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
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
            failure: arr_acc_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(arr_idx, out_om_idx, ());
    proc.borrow_mut().blocks.add_edge(arr_idx, arr_acc_idx, ());

    block_get(&mut proc.borrow_mut(), out_om_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(out_om_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), arr_acc_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(arr_acc_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_out)));

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn last_stub_new(tag: &str) -> Rc<RefCell<IRProcedure>> {
    /* t_p1_addr := params[0];
     * t_p1 := *t_p1_addr;
     * t_p1_type := type_of(t_p1);
     * t_p1_set := t_p1_type == TYPE_SET;
     * if t_p1_set
     *  goto <arb_idx>
     * else
     *  goto <arr_idx>
     *
     * <arb_idx>:
     *  t_out := set_borrow(t_p1, false);
     *  _ := mark_persist(t_out);
     *  t_out := copy(t_out);
     *  goto <ret_idx>
     *
     * <arr_idx>:
     *  t_amount := amount(t_p1);
     *  t_amount_zero := t_amount == 0;
     *  if t_amount_zero
     *   goto <out_om_idx>
     *  else
     *   goto <arr_acc_idx>
     *
     * <out_om_idx>:
     *  := invalidate(t_amount);
     *  t_out := om;
     *  goto <ret_idx>
     *
     * <arr_acc_idx>:
     *  t_amount_sub_one := t_amount - 1;
     *  _ := invalidate(t_amount);
     *  t_out := t_p1[t_amount_sub_one];
     *  _ := invalidate(t_amount_sub_one);
     *  t_out := copy(t_out);
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  return t_out;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag(tag)));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_set = tmp_var_new(&mut proc.borrow_mut());
    let t_out = tmp_var_new(&mut proc.borrow_mut());
    let t_amount = tmp_var_new(&mut proc.borrow_mut());
    let t_amount_zero = tmp_var_new(&mut proc.borrow_mut());
    let t_amount_sub_one = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let arb_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let arr_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let out_om_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let arr_acc_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_set),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::SET)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_set),
            success: arb_idx,
            failure: arr_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, arb_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, arr_idx, ());

    block_get(&mut proc.borrow_mut(), arb_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::SetBorrow),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1), IRValue::Bool(false)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::MarkPersist),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(arb_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), arr_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_amount),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_amount_zero),
            types: IRType::BOOL,
            source: IRValue::Variable(t_amount),
            op: IROp::Equal(IRValue::Number(0.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_amount_zero),
            success: out_om_idx,
            failure: arr_acc_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(arr_idx, out_om_idx, ());
    proc.borrow_mut().blocks.add_edge(arr_idx, arr_acc_idx, ());

    block_get(&mut proc.borrow_mut(), out_om_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_amount)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(out_om_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), arr_acc_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_amount_sub_one),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_amount),
            op: IROp::Minus(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_amount)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1),
            op: IROp::AccessArray(IRValue::Variable(t_amount_sub_one)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_amount_sub_one)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(arr_acc_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_out)));

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn fromb_stub_new() -> Rc<RefCell<IRProcedure>> {
    /*  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_type_set := t_p1_type == TYPE_SET;
     *  if t_p1_type_set
     *   goto <set_idx>
     *  else
     *   goto <list_idx>
     *
     * <set_idx>:
     *  t_out := set_take(t_p1, true);
     *  goto <ret_idx>
     *
     * <list_idx>:
     *  t_len := amount(t_p1);
     *  t_slice := slice(t_p1, 1, t_len);
     *  _ := invalidate(t_len);
     *  t_new := copy(t_slice);
     *  t_out := t_p1[0];
     *  t_out := copy(t_out);
     *  _ := invalidate(t_p1);
     *  *t_p1_addr := t_new;
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  return t_out;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("fromB")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type_set = tmp_var_new(&mut proc.borrow_mut());
    let t_len = tmp_var_new(&mut proc.borrow_mut());
    let t_slice = tmp_var_new(&mut proc.borrow_mut());
    let t_new = tmp_var_new(&mut proc.borrow_mut());
    let t_out = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let set_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let list_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type_set),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::SET)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_type_set),
            success: set_idx,
            failure: list_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, set_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, list_idx, ());

    block_get(&mut proc.borrow_mut(), set_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::SetTake),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1), IRValue::Bool(true)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(set_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), list_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_slice),
            types: IRType::STRING | IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_p1),
                IRValue::Number(1.into()),
                IRValue::Variable(t_len),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_new),
            types: IRType::STRING | IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_slice)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Deref(t_p1_addr),
            types: IRType::STRING | IRType::LIST,
            source: IRValue::Variable(t_new),
            op: IROp::Assign,
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(list_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_out)));

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

fn get_stub_new(tag: &str) -> Rc<RefCell<IRProcedure>> {
    /* t_p_len := amount(params);
     * t_p_arg := t_p_len == 1;
     * if t_p_arg
     *  goto <arg_idx>
     * else
     *  goto <narg_idx>
     *
     * <arg_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_ret := read_line_stdin(t_p1);
     *  goto <ret_idx>
     *
     * <narg_idx>:
     *  t_p1 := "";
     *  t_ret := read_line_stdin(t_p1);
     *  _ := invalidate(t_p1);
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  return t_ret;
     */

    let proc = Rc::new(RefCell::new(IRProcedure::from_tag(tag)));

    let t_p_len = tmp_var_new(&mut proc.borrow_mut());
    let t_p_arg = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let arg_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let narg_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::BuiltinVar(BuiltinVar::Params)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p_arg),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p_len),
            op: IROp::Equal(IRValue::Number(1.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p_arg),
            success: arg_idx,
            failure: narg_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, arg_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, narg_idx, ());

    block_get(&mut proc.borrow_mut(), arg_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::ReadLineStdin),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(arg_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), narg_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRType::STRING,
            source: IRValue::String("".to_string()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::ReadLineStdin),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(narg_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_ret)));

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn read_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* t_p_len := amount(params);
     * t_p_arg := t_p_len == 1;
     * if t_p_arg
     *  goto <arg_idx>
     * else
     *  goto <narg_idx>
     *
     * <arg_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_ret := read_line_stdin(t_p1);
     *  goto <ret_idx>
     *
     * <narg_idx>:
     *  t_p1 := "";
     *  t_ret := read_line_stdin(t_p1);
     *  _ := invalidate(t_p1);
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  t_ret_new := parse_int(t_ret);
     *  t_ret_new_om := t_ret_new == om;
     *  if t_ret_new_om
     *   goto <double_idx>
     *  else
     *   goto <ret_assign_idx>
     *
     * <double_idx>:
     *  t_ret_new := parse_float(t_ret);
     *  t_ret_new_om := t_ret_new == om;
     *  if t_ret_new_om
     *   goto <ret_fin_idx>
     *  else
     *   goto <ret_assign_idx>
     *
     * <ret_assign_idx>:
     *  _ := invalidate(t_ret);
     *  t_ret := t_ret_new;
     *  goto <ret_fin_idx>
     *
     * <ret_fin_idx>:
     *  return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("read")));

    let t_p_len = tmp_var_new(&mut proc.borrow_mut());
    let t_p_arg = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());
    let t_ret_new = tmp_var_new(&mut proc.borrow_mut());
    let t_ret_new_om = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let arg_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let narg_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let double_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_assign_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_fin_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::BuiltinVar(BuiltinVar::Params)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p_arg),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p_len),
            op: IROp::Equal(IRValue::Number(1.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p_arg),
            success: arg_idx,
            failure: narg_idx,
        }),
    ]);
    proc.borrow_mut().blocks.add_edge(init_idx, arg_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, narg_idx, ());

    block_get(&mut proc.borrow_mut(), arg_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::ReadLineStdin),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(arg_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), narg_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRType::STRING,
            source: IRValue::String("".to_string()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::ReadLineStdin),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(narg_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret_new),
            types: IRType::NUMBER | IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ParseInt),
            op: IROp::NativeCall(vec![IRValue::Variable(t_ret)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret_new_om),
            types: IRType::BOOL,
            source: IRValue::Variable(t_ret_new),
            op: IROp::Equal(IRValue::Undefined),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_ret_new_om),
            success: double_idx,
            failure: ret_assign_idx,
        }),
    ]);
    proc.borrow_mut().blocks.add_edge(ret_idx, double_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(ret_idx, ret_assign_idx, ());

    block_get(&mut proc.borrow_mut(), double_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret_new),
            types: IRType::DOUBLE | IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ParseFloat),
            op: IROp::NativeCall(vec![IRValue::Variable(t_ret)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret_new_om),
            types: IRType::BOOL,
            source: IRValue::Variable(t_ret_new),
            op: IROp::Equal(IRValue::Undefined),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_ret_new_om),
            success: ret_fin_idx,
            failure: ret_assign_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(double_idx, ret_fin_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(double_idx, ret_assign_idx, ());

    block_get(&mut proc.borrow_mut(), ret_assign_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_ret)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_ret_new),
            op: IROp::Assign,
        }),
        IRStmt::Goto(ret_fin_idx),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(ret_assign_idx, ret_fin_idx, ());

    block_get(&mut proc.borrow_mut(), ret_fin_idx).push(IRStmt::Return(IRValue::Variable(t_ret)));

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_fin_idx;

    proc
}

fn throw_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* t_p1_addr := params[0];
     * t_p1 := *t_p1_addr;
     * t_owned := copy(t_p1);
     * _ := throw(1, t_owned);
     * // unreachable
     */

    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("throw")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_owned = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_owned),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Throw),
            op: IROp::NativeCall(vec![IRValue::Number(1.into()), IRValue::Variable(t_owned)]),
        }),
        IRStmt::Unreachable,
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

fn eval_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* t_p1_addr := params[0];
     * t_p1 := *t_p1_addr;
     * t_ret := eval(t_p1);
     * return t_ret;
     */

    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("eval")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Eval),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

fn eval_term_stub_new() -> Rc<RefCell<IRProcedure>> {
    /*  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_ast := t_p1_type == TYPE_AST;
     *  if t_p1_ast
     *   goto <eval_idx>
     *  else
     *   goto <clone_idx>
     *
     * <eval_idx>:
     *  t_ret := eval_term(t_p1);
     *  goto <ret_idx>
     *
     * <clone_idx>:
     *  t_ret := copy(t_p1);
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  return t_ret;
     */

    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("evalTerm")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_ast = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let eval_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let clone_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    let init_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_ast),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::AST)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_ast),
            success: eval_idx,
            failure: clone_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, eval_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, clone_idx, ());

    block_get(&mut proc.borrow_mut(), eval_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::EvalTerm),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(eval_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), clone_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(clone_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_ret)));

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn execute_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* t_p1_addr := params[0];
     * t_p1 := *t_p1_addr;
     * t_ret := execute(t_p1);
     * return t_ret;
     */

    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("execute")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Execute),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

fn pow_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* t_p1_addr := params[0];
     * t_p1 := *t_p1_addr;
     * t_p1_type := type_of(t_p1);
     * t_check := t_p1_type == TYPE_SET;
     * if t_check
     *  goto <ret_idx>
     * else
     *  goto <throw_idx>
     *
     * <throw_idx>:
     * _ := throw(1, "pow undefined for type");
     * unreachable;
     *
     * <ret_idx>:
     * t_ret := pow(2, t_p1);
     * return t_ret;
     */

    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("pow")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_check = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let throw_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Throw),
            op: IROp::NativeCall(vec![
                IRValue::Number(1.into()),
                IRValue::String("pow undefined for type".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    let ret_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::SET,
            source: IRValue::BuiltinProc(BuiltinProc::Pow),
            op: IROp::NativeCall(vec![IRValue::Number(2.into()), IRValue::Variable(t_p1)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    let init_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::SET)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_check),
            success: ret_idx,
            failure: throw_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, ret_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, throw_idx, ());

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn join_stub_new() -> Rc<RefCell<IRProcedure>> {
    /*  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_clue_addr := params[1];
     *  t_clue_raw := *t_clue_addr;
     *  t_clue := serialize(t_clue_raw);
     *  t_iter := iter_new(t_p1);
     *  t_ret := "";
     *  t_init_iter := true;
     *  goto <cond_idx>:
     *
     * <cond_idx>:
     *  t_i := om;
     *  t_i_addr := &t_i;
     *  t_check := iter_next(t_iter, t_i_addr);
     *  if t_check
     *   goto <add_idx>
     *  else
     *   goto <ret_idx>
     *
     * <add_idx>:
     *  if t_init_iter
     *   goto <add_init_idx>
     *  else
     *   goto <add_clue_idx>
     *
     * <add_init_idx>:
     *  t_init_iter := false;
     *  goto <add_i_idx>
     *
     * <add_i_idx>:
     *  t_ret := t_ret + t_i;
     *  goto <cond_idx>
     *
     * <add_clue_idx>:
     *  t_ret := t_ret + t_clue;
     *  goto <add_i_idx>
     *
     * <ret_idx>:
     *  return t_ret;
     */

    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("join")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_clue_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_clue_raw = tmp_var_new(&mut proc.borrow_mut());
    let t_clue = tmp_var_new(&mut proc.borrow_mut());
    let t_iter = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());
    let t_init_iter = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let cond_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let add_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let add_init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let add_clue_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let add_i_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_clue_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_clue_raw),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_clue_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_clue),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Serialize),
            op: IROp::NativeCall(vec![IRValue::Variable(t_clue_raw)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_iter),
            types: IRType::ITERATOR,
            source: IRValue::BuiltinProc(BuiltinProc::IterNew),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::String("".to_string()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_init_iter),
            types: IRType::BOOL,
            source: IRValue::Bool(true),
            op: IROp::Assign,
        }),
        IRStmt::Goto(cond_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, cond_idx, ());

    let t_i = tmp_var_new(&mut proc.borrow_mut());
    let t_i_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_check = tmp_var_new(&mut proc.borrow_mut());

    block_get(&mut proc.borrow_mut(), cond_idx).extend(vec![
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
            target: IRTarget::Variable(t_check),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::IterNext),
            op: IROp::NativeCall(vec![IRValue::Variable(t_iter), IRValue::Variable(t_i_addr)]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_check),
            success: add_idx,
            failure: ret_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(cond_idx, add_idx, ());
    proc.borrow_mut().blocks.add_edge(cond_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), add_idx).push(IRStmt::Branch(IRBranch {
        cond: IRValue::Variable(t_init_iter),
        success: add_init_idx,
        failure: add_clue_idx,
    }));

    proc.borrow_mut().blocks.add_edge(add_idx, add_init_idx, ());
    proc.borrow_mut().blocks.add_edge(add_idx, add_clue_idx, ());

    block_get(&mut proc.borrow_mut(), add_init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_init_iter),
            types: IRType::BOOL,
            source: IRValue::Bool(false),
            op: IROp::Assign,
        }),
        IRStmt::Goto(add_i_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(add_init_idx, add_i_idx, ());

    block_get(&mut proc.borrow_mut(), add_i_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::Variable(t_ret),
            op: IROp::Plus(IRValue::Variable(t_i)),
        }),
        IRStmt::Goto(cond_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(add_i_idx, cond_idx, ());

    block_get(&mut proc.borrow_mut(), add_clue_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::Variable(t_ret),
            op: IROp::Plus(IRValue::Variable(t_clue)),
        }),
        IRStmt::Goto(add_i_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(add_clue_idx, add_i_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_ret)));

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn args_stub_new() -> Rc<RefCell<IRProcedure>> {
    /*  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_type_p1 := type_of(t_p1);
     *  t_check_1 := t_type_p1 == TYPE_AST;
     *  t_check_2 := t_type_p1 == TYPE_TERM;
     *  t_check := t_check_1 || t_check_2;
     *  t_check_3 := t_type_p1 == TYPE_TTERM;
     *  t_check := t_check || t_check_3;
     *  if t_check
     *   goto <prep_idx>
     *  else
     *   goto <throw_idx>
     *
     * <throw_idx>
     *  _ := throw(1, "parameter is not a term");
     *  unreachable;
     *
     * <prep_idx>
     *  t_len := amount(t_p1);
     *  t_ret := list_new();
     *  t_i := 0;
     *  goto <cond_idx>
     *
     * <cond_idx>
     *  t_check := t_i < t_len;
     *  if t_check
     *   goto <loop_idx>
     *  else
     *   goto <ret_idx>
     *
     * <loop_idx>
     *  t_i_new := t_i + 1;
     *  _ := invalidate(t_i);
     *  t_i := t_i_new;
     *  t_n := t_p1[t_i];
     *  t_n_om := t_n == om;
     *  if t_n_om
     *   goto <t_n_nil_idx>
     *  else
     *   goto <t_n_copy_idx>
     *
     * <t_n_nil_idx>:
     *  t_n := "nil";
     *  goto <push_idx>
     *
     * <t_n_copy_idx>:
     *  t_n := copy(t_n);
     *  goto <push_idx>
     *
     *  _ := list_push(t_ret, t_n);
     *  goto <cond_idx>
     *
     * <ret_idx>
     *  _ := invalidate(t_len);
     *  _ := invalidate(t_i);
     *  return t_ret;
     */

    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("args")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_type_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_check_1 = tmp_var_new(&mut proc.borrow_mut());
    let t_check_2 = tmp_var_new(&mut proc.borrow_mut());
    let t_check_3 = tmp_var_new(&mut proc.borrow_mut());
    let t_check = tmp_var_new(&mut proc.borrow_mut());
    let t_n_om = tmp_var_new(&mut proc.borrow_mut());
    let t_i_new = tmp_var_new(&mut proc.borrow_mut());
    let t_n = tmp_var_new(&mut proc.borrow_mut());
    let t_len = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());
    let t_i = tmp_var_new(&mut proc.borrow_mut());

    let t_n_nil_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let t_n_copy_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let push_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let throw_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let prep_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let cond_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let loop_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    let init_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type_p1),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_check_1),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type_p1),
            op: IROp::Equal(IRValue::Type(IRType::AST)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_check_2),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type_p1),
            op: IROp::Equal(IRValue::Type(IRType::TERM)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_check_1),
            op: IROp::Or(IRValue::Variable(t_check_2)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_check_3),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type_p1),
            op: IROp::Equal(IRValue::Type(IRType::TTERM)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_check),
            op: IROp::Or(IRValue::Variable(t_check_3)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_check),
            success: prep_idx,
            failure: throw_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, prep_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, throw_idx, ());

    block_get(&mut proc.borrow_mut(), throw_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Throw),
            op: IROp::NativeCall(vec![
                IRValue::Number(1.into()),
                IRValue::String(String::from("parameter is not a term")),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    block_get(&mut proc.borrow_mut(), prep_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRType::NUMBER,
            source: IRValue::Number(0.into()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(cond_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(prep_idx, cond_idx, ());

    block_get(&mut proc.borrow_mut(), cond_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_i),
            op: IROp::Less(IRValue::Variable(t_len)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_check),
            success: loop_idx,
            failure: ret_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(cond_idx, loop_idx, ());
    proc.borrow_mut().blocks.add_edge(cond_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), loop_idx).extend(vec![
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
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_n),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1),
            op: IROp::AccessArray(IRValue::Variable(t_i)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_n_om),
            types: IRType::BOOL,
            source: IRValue::Variable(t_n),
            op: IROp::Equal(IRValue::Undefined),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_n_om),
            success: t_n_nil_idx,
            failure: t_n_copy_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(loop_idx, t_n_nil_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(loop_idx, t_n_copy_idx, ());

    block_get(&mut proc.borrow_mut(), t_n_nil_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_n),
            types: IRTypes!("any"),
            source: IRValue::String(String::from("nil")),
            op: IROp::Assign,
        }),
        IRStmt::Goto(push_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(t_n_nil_idx, push_idx, ());

    block_get(&mut proc.borrow_mut(), t_n_copy_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_n),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_n)]),
        }),
        IRStmt::Goto(push_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(t_n_copy_idx, push_idx, ());

    block_get(&mut proc.borrow_mut(), push_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ListPush),
            op: IROp::NativeCall(vec![IRValue::Variable(t_ret), IRValue::Variable(t_n)]),
        }),
        IRStmt::Goto(cond_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(push_idx, cond_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).extend(vec![
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
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn fct_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_ast := t_p1_type == TYPE_AST;
     *  if t_p1_ast
     *   goto <ast_idx>
     *  else
     *   goto <term_dispatch_idx>
     *
     * <ast_idx>:
     *  t_ast_tag := t_p1[0];
     *  t_tag := ast_tterm_tag_get(t_ast_tag);
     *  t_ret := "@@@" + t_tag;
     *  _ := invalidate(t_tag);
     *  goto <ret_idx>;
     *
     * <term_dispatch_idx>:
     *  t_p1_tterm := t_p1_type == TYPE_TTERM;
     *  if t_p1_tterm
     *   goto <ret_tt_init_idx>
     *  else
     *   goto <ret_t_init_idx>
     *
     * <ret_tt_init_idx>:
     *  t_ret := "@@@";
     *  goto <check_idx>
     *
     * <ret_t_init_idx>:
     *  t_ret := "";
     *  goto <check_idx>
     *
     * <check_idx>
     *  t_p1_term := t_p1_type == TYPE_TERM;
     *  t_p1_tl := t_p1_tterm || t_pl_term;
     *  if t_p1_tl
     *   goto <term_idx>
     *  else
     *   goto <throw_idx>
     *
     * <term_idx>:
     *  t_tag := t_p1[0];
     *  t_copy := copy(t_tag);
     *  t_ret_new := t_ret + t_copy;
     *  _ := invalidate(t_copy);
     *  _ := invalidate(t_ret);
     *  t_ret := t_ret_new;
     *  goto <ret_idx>;
     *
     * <ret_idx>:
     *  return t_ret;
     *
     * <throw_idx>:
     *  _ := throw(1, "Operand is not a term");
     *  unreachable;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("fct")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_ast = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_tterm = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_term = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_tl = tmp_var_new(&mut proc.borrow_mut());
    let t_ast_tag = tmp_var_new(&mut proc.borrow_mut());
    let t_tag = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());
    let t_copy = tmp_var_new(&mut proc.borrow_mut());
    let t_ret_new = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ast_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let term_dispatch_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_tt_init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_t_init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let check_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let term_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let throw_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    // <init_idx>
    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_ast),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::AST)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_ast),
            success: ast_idx,
            failure: term_dispatch_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, ast_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(init_idx, term_dispatch_idx, ());

    block_get(&mut proc.borrow_mut(), ast_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ast_tag),
            types: IRType::STRING,
            source: IRValue::Variable(t_p1),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_tag),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::AstTTermTagGet),
            op: IROp::NativeCall(vec![IRValue::Variable(t_ast_tag)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::String("@@@".into()),
            op: IROp::Plus(IRValue::Variable(t_tag)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_tag)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(ast_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), term_dispatch_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_tterm),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::TTERM)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_tterm),
            success: ret_tt_init_idx,
            failure: ret_t_init_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(term_dispatch_idx, ret_tt_init_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(term_dispatch_idx, ret_t_init_idx, ());

    block_get(&mut proc.borrow_mut(), ret_tt_init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::String("@@@".into()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(check_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(ret_tt_init_idx, check_idx, ());

    block_get(&mut proc.borrow_mut(), ret_t_init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::String("".into()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(check_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(ret_t_init_idx, check_idx, ());

    block_get(&mut proc.borrow_mut(), check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_term),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::TERM)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_tl),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_tterm),
            op: IROp::Or(IRValue::Variable(t_p1_term)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_tl),
            success: term_idx,
            failure: throw_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(check_idx, term_idx, ());
    proc.borrow_mut().blocks.add_edge(check_idx, throw_idx, ());

    block_get(&mut proc.borrow_mut(), term_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_tag),
            types: IRType::STRING,
            source: IRValue::Variable(t_p1),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_copy),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_tag)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret_new),
            types: IRType::STRING,
            source: IRValue::Variable(t_ret),
            op: IROp::Plus(IRValue::Variable(t_copy)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_copy)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_ret)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::Variable(t_ret_new),
            op: IROp::Assign,
        }),
        IRStmt::Goto(ret_idx),
    ]);

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_ret)));

    block_get(&mut proc.borrow_mut(), throw_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Throw),
            op: IROp::NativeCall(vec![
                IRValue::Number(1.into()),
                IRValue::String("Operand is not a term".into()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn make_term_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_fct_addr := params[0];
     *  t_fct := *t_fct_addr;
     *  t_args_addr := params[1];
     *  t_args := *t_args_addr;
     *  t_fct_amount := amount(t_fct);
     *  t_amount := amount(t_args);
     *  t_i := 0;
     *  goto <args_cnt_cond_idx>
     *
     * <args_cnt_cond_idx>:
     *  t_cond_amount := t_i < t_fct_amount;
     *  t_cond_four := t_i < 4;
     *  t_cond := t_cond_amount && t_cond_four;
     *  if t_cond
     *   goto <args_check_idx>
     *  else
     *   goto <args_cnt_assign_idx>
     *
     * <args_check_idx>:
     *  t_entry_addr := t_fct_addr[t_i];
     *  t_entry := *t_entry_addr;
     *  t_entry_at := t_entry == "@";
     *  if t_entry_at
     *   goto <args_incr_idx>
     *  else
     *   goto <args_cnt_assign_idx>
     *
     * <args_incr_idx>:
     *  t_i_new := t_i + 1;
     *  _ := invalidate(t_i);
     *  t_i := t_i_new;
     *  goto <args_cnt_cond_idx>
     *
     * <args_cnt_assign_idx>:
     *  t_at_count_zero := t_i == 0;
     *  t_at_count_three := t_i == 3;
     *  _ := invalidate(t_i);
     *  if t_at_count_three
     *   goto <ast_check_idx>
     *  else
     *   goto <term_check_idx>:
     *
     * <ast_check_idx>:
     *  t_slice := slice(t_fct, 3, t_fct_amount);
     *  t_tag := ast_tag_get(t_slice, t_amount);
     *  t_tag_om := t_tag == om;
     *  if t_tag_om
     *   goto <tterm_new_idx>
     *  else
     *   goto <ast_new_idx>
     *
     * <tterm_new_idx>:
     *  t_ret := term_new(t_slice, t_amount, true);
     *  goto <params_loop_idx>
     *
     * <ast_new_idx>:
     *  t_ret := ast_node_new_sized(t_tag, t_amount);
     *  _ := invalidate(t_tag);
     *  goto <params_loop_idx>
     *
     * <term_check_idx>:
     *  if t_at_count_zero
     *   goto <term_zero_idx>
     *  else
     *   goto <term_new_idx>
     *
     * <term_zero_idx>:
     *  t_ret := term_new(t_fct, t_amount, false);
     *  goto <params_loop_idx>
     *
     * <term_new_idx>:
     *  t_slice := slice(t_fct, 1, t_fct_amount);
     *  t_ret := term_new(t_slice, t_amount, false);
     *  goto <params_loop_idx>
     *
     * <params_loop_idx>:
     *  t_i := 0;
     *  t_ret_addr := &t_ret;
     *  goto <params_cond_idx>
     *
     * <params_cond_idx>:
     *  t_check := t_i < t_amount;
     *  if t_check
     *   goto <param_assign_idx>
     *  else
     *   goto <follow_idx>
     *
     * <param_assign_idx>:
     *  t_i_new := t_i + 1;
     *  t_entry := t_ret_addr[t_i_new];
     *  t_arg := t_args[t_i];
     *  t_insert := copy(t_arg);
     *  *t_entry := t_insert;
     *  _ := invalidate(t_i);
     *  t_i := t_i_new;
     *  goto <params_cond_idx>
     *
     * <follow_idx>:
     *  _ := invalidate(t_fct_amount);;
     *  _ := invalidate(t_amount);;
     *  _ := invalidate(t_i);
     *  return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("makeTerm")));

    let t_fct_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_fct = tmp_var_new(&mut proc.borrow_mut());
    let t_args_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_args = tmp_var_new(&mut proc.borrow_mut());
    let t_fct_amount = tmp_var_new(&mut proc.borrow_mut());
    let t_amount = tmp_var_new(&mut proc.borrow_mut());
    let t_i = tmp_var_new(&mut proc.borrow_mut());
    let t_i_new = tmp_var_new(&mut proc.borrow_mut());
    let t_cond_amount = tmp_var_new(&mut proc.borrow_mut());
    let t_cond_four = tmp_var_new(&mut proc.borrow_mut());
    let t_cond = tmp_var_new(&mut proc.borrow_mut());
    let t_entry_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_entry = tmp_var_new(&mut proc.borrow_mut());
    let t_entry_at = tmp_var_new(&mut proc.borrow_mut());
    let t_at_count_zero = tmp_var_new(&mut proc.borrow_mut());
    let t_at_count_three = tmp_var_new(&mut proc.borrow_mut());
    let t_slice = tmp_var_new(&mut proc.borrow_mut());
    let t_tag = tmp_var_new(&mut proc.borrow_mut());
    let t_tag_om = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());
    let t_ret_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_check = tmp_var_new(&mut proc.borrow_mut());
    let t_arg = tmp_var_new(&mut proc.borrow_mut());
    let t_insert = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let args_cnt_cond_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let args_check_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let args_incr_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let args_cnt_assign_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ast_check_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let tterm_new_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ast_new_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let term_check_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let term_zero_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let term_new_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let params_loop_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let params_cond_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let param_assign_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let follow_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_fct_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_fct),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_fct_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_args_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_args),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_args_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_fct_amount),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_fct)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_amount),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_args)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRType::NUMBER,
            source: IRValue::Number(0.into()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(args_cnt_cond_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(init_idx, args_cnt_cond_idx, ());

    block_get(&mut proc.borrow_mut(), args_cnt_cond_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cond_amount),
            types: IRType::BOOL,
            source: IRValue::Variable(t_i),
            op: IROp::Less(IRValue::Variable(t_fct_amount)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cond_four),
            types: IRType::BOOL,
            source: IRValue::Variable(t_i),
            op: IROp::Less(IRValue::Number(4.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cond),
            types: IRType::BOOL,
            source: IRValue::Variable(t_cond_amount),
            op: IROp::And(IRValue::Variable(t_cond_four)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cond),
            success: args_check_idx,
            failure: args_cnt_assign_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(args_cnt_cond_idx, args_check_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(args_cnt_cond_idx, args_cnt_assign_idx, ());

    block_get(&mut proc.borrow_mut(), args_check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_entry_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_fct_addr),
            op: IROp::AccessArray(IRValue::Variable(t_i)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_entry),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_entry_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_entry_at),
            types: IRType::BOOL,
            source: IRValue::Variable(t_entry),
            op: IROp::Equal(IRValue::String("@".to_string())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_entry_at),
            success: args_incr_idx,
            failure: args_cnt_assign_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(args_check_idx, args_incr_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(args_check_idx, args_cnt_assign_idx, ());

    block_get(&mut proc.borrow_mut(), args_incr_idx).extend(vec![
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
        IRStmt::Goto(args_cnt_cond_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(args_incr_idx, args_cnt_cond_idx, ());

    block_get(&mut proc.borrow_mut(), args_cnt_assign_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_at_count_zero),
            types: IRType::BOOL,
            source: IRValue::Variable(t_i),
            op: IROp::Equal(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_at_count_three),
            types: IRType::BOOL,
            source: IRValue::Variable(t_i),
            op: IROp::Equal(IRValue::Number(3.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_at_count_three),
            success: ast_check_idx,
            failure: term_check_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(args_cnt_assign_idx, ast_check_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(args_cnt_assign_idx, term_check_idx, ());

    block_get(&mut proc.borrow_mut(), ast_check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_slice),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_fct),
                IRValue::Number(3.into()),
                IRValue::Variable(t_fct_amount),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_tag),
            types: IRType::STRING | IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::AstTagGet),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_slice),
                IRValue::Variable(t_amount),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_tag_om),
            types: IRType::BOOL,
            source: IRValue::Variable(t_tag),
            op: IROp::Equal(IRValue::Undefined),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_tag_om),
            success: tterm_new_idx,
            failure: ast_new_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(ast_check_idx, tterm_new_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(ast_check_idx, ast_new_idx, ());

    block_get(&mut proc.borrow_mut(), tterm_new_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::TTERM,
            source: IRValue::BuiltinProc(BuiltinProc::TermNew),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_slice),
                IRValue::Variable(t_amount),
                IRValue::Bool(true),
            ]),
        }),
        IRStmt::Goto(params_loop_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(tterm_new_idx, params_loop_idx, ());

    block_get(&mut proc.borrow_mut(), ast_new_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::AST,
            source: IRValue::BuiltinProc(BuiltinProc::AstNodeNewSized),
            op: IROp::NativeCall(vec![IRValue::Variable(t_tag), IRValue::Variable(t_amount)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_tag)]),
        }),
        IRStmt::Goto(params_loop_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(ast_new_idx, params_loop_idx, ());

    block_get(&mut proc.borrow_mut(), term_check_idx).push(IRStmt::Branch(IRBranch {
        cond: IRValue::Variable(t_at_count_zero),
        success: term_zero_idx,
        failure: term_new_idx,
    }));

    proc.borrow_mut()
        .blocks
        .add_edge(term_check_idx, term_zero_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(term_check_idx, term_new_idx, ());

    block_get(&mut proc.borrow_mut(), term_zero_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::TermNew),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_fct),
                IRValue::Variable(t_amount),
                IRValue::Bool(false),
            ]),
        }),
        IRStmt::Goto(params_loop_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(term_zero_idx, params_loop_idx, ());

    block_get(&mut proc.borrow_mut(), term_new_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_slice),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_fct),
                IRValue::Number(1.into()),
                IRValue::Variable(t_fct_amount),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::TERM,
            source: IRValue::BuiltinProc(BuiltinProc::TermNew),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_slice),
                IRValue::Variable(t_amount),
                IRValue::Bool(false),
            ]),
        }),
        IRStmt::Goto(params_loop_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(term_new_idx, params_loop_idx, ());

    block_get(&mut proc.borrow_mut(), params_loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRType::NUMBER,
            source: IRValue::Number(0.into()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_ret),
            op: IROp::PtrAddress,
        }),
        IRStmt::Goto(params_cond_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(params_loop_idx, params_cond_idx, ());

    block_get(&mut proc.borrow_mut(), params_cond_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_i),
            op: IROp::Less(IRValue::Variable(t_amount)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_check),
            success: param_assign_idx,
            failure: follow_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(params_cond_idx, param_assign_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(params_cond_idx, follow_idx, ());

    block_get(&mut proc.borrow_mut(), param_assign_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_new),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_i),
            op: IROp::Plus(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_entry_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_ret_addr),
            op: IROp::AccessArray(IRValue::Variable(t_i_new)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_arg),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_args),
            op: IROp::AccessArray(IRValue::Variable(t_i)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_insert),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_arg)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Deref(t_entry_addr),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_insert),
            op: IROp::Assign,
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
        IRStmt::Goto(params_cond_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(param_assign_idx, params_cond_idx, ());

    block_get(&mut proc.borrow_mut(), follow_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_fct_amount)]),
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
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = follow_idx;

    proc
}

fn matches_stub_new() -> Rc<RefCell<IRProcedure>> {
    /*  t_regex_addr := params[1];
     *  t_regex := *t_regex_addr;
     *  t_input_addr := params[0];
     *  t_input := *t_input_addr;
     *  t_reg := regex_compile(t_regex, 0);
     *  t_params_len := amount(params);
     *  t_params_len_check := t_params_len == 3;
     *  if t_params_len_check
     *   goto <groups_check_idx>
     *  else
     *   goto <matched_idx>
     *
     * <groups_check_idx>:
     *  t_group_check_addr := params[2];
     *  t_group_check := *t_groups_check_addr;
     *  if t_group_check
     *   goto <groups_idx>
     *  else
     *   goto <matched_idx>
    *
     * <groups_idx>:
     *  t_matched := false;
     *  t_matched_addr := &t_matched;
     *  t_ret := regex_match_groups(t_input, t_reg, t_matched_addr);
     *  goto <ret_idx>
     *
     * <matched_idx>
     *  t_ret := regex_match(t_input, t_reg);
     *  goto <ret_idx>

     *
     * <ret_idx>:
     *  return t_ret;
     */

    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("matches")));

    let t_regex_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_regex = tmp_var_new(&mut proc.borrow_mut());
    let t_input_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_input = tmp_var_new(&mut proc.borrow_mut());
    let t_reg = tmp_var_new(&mut proc.borrow_mut());
    let t_params_len = tmp_var_new(&mut proc.borrow_mut());
    let t_params_len_check = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());
    let t_matched = tmp_var_new(&mut proc.borrow_mut());
    let t_matched_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_group_check_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_group_check = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let groups_check_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let groups_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let matched_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc
        .borrow_mut()
        .blocks
        .add_node(vec![IRStmt::Return(IRValue::Variable(t_ret))]);

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_regex_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_regex),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_regex_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_input_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_reg),
            types: IRType::NATIVE_REGEX,
            source: IRValue::BuiltinProc(BuiltinProc::RegexCompile),
            op: IROp::NativeCall(vec![IRValue::Variable(t_regex), IRValue::Number(0.into())]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_params_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::BuiltinVar(BuiltinVar::Params)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_params_len_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_params_len),
            op: IROp::Equal(IRValue::Number(3.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_params_len_check),
            success: groups_check_idx,
            failure: matched_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(init_idx, groups_check_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, matched_idx, ());

    block_get(&mut proc.borrow_mut(), groups_check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_group_check_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(2.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_group_check),
            types: IRType::PTR,
            source: IRValue::Variable(t_group_check_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_group_check),
            success: groups_idx,
            failure: matched_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(groups_check_idx, groups_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(groups_check_idx, matched_idx, ());

    block_get(&mut proc.borrow_mut(), groups_idx).extend(vec![
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
            target: IRTarget::Variable(t_ret),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::RegexMatchGroups),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input),
                IRValue::Variable(t_reg),
                IRValue::Variable(t_matched_addr),
            ]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(groups_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), matched_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::RegexMatch),
            op: IROp::NativeCall(vec![IRValue::Variable(t_input), IRValue::Variable(t_reg)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(matched_idx, ret_idx, ());

    proc
}

fn replace_stub_new() -> Rc<RefCell<IRProcedure>> {
    /*  t_input_addr := params[0];
     *  t_input := *t_input_addr;
     *  t_regex_addr := params[1];
     *  t_regex := *t_regex_addr;
     *  t_replace_addr := params[2];
     *  t_replace := *t_replace_addr;
     *  t_reg := regex_compile(t_regex, 0);
     *  t_out := "";
     *  t_input_len := amount(t_input);
     *  t_input_slice := slice(t_input, 0, t_input_len);
     *  goto <loop_idx>
     *
     * <loop_idx>:
     *  t_offset := om;
     *  t_offset_addr := &t_offset;
     *  t_len := om;
     *  t_len_addr := &t_len;
     *  t_matched := regex_match_len(t_input_slice, t_regex, t_len_addr, t_offset_addr);
     *  if t_matched
     *   goto <append_idx>
     *  else
     *   goto <end_idx>
     *
     * <append_idx>:
     *  t_pre_slice := slice(t_input_slice, 0, t_offset);
     *  t_out_new := t_out + t_pre_slice;
     *  _ := invalidate(t_out);
     *  t_out := t_out_new + t_replace;
     *  _ := invalidate(t_out_new);
     *  t_pos := t_offset + t_len;
     *  t_input_slice := slice(t_input_slice, t_pos, t_input_len);
     *  _ := invalidate(t_pos);
     *  _ := invalidate(t_offset);
     *  _ := invalidate(t_len);
     *  goto <loop_idx>
     *
     * <end_idx>:
     *  t_pre_slice := slice(t_input_slice, 0, t_input_len);
     *  t_out_new := t_out + t_pre_slice;
     *  _ := invalidate(t_out);
     *  return t_out_new;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("replace")));

    let t_input_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_input = tmp_var_new(&mut proc.borrow_mut());
    let t_regex_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_regex = tmp_var_new(&mut proc.borrow_mut());
    let t_replace_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_replace = tmp_var_new(&mut proc.borrow_mut());
    let t_reg = tmp_var_new(&mut proc.borrow_mut());
    let t_out = tmp_var_new(&mut proc.borrow_mut());
    let t_input_len = tmp_var_new(&mut proc.borrow_mut());
    let t_input_slice = tmp_var_new(&mut proc.borrow_mut());

    let t_offset = tmp_var_new(&mut proc.borrow_mut());
    let t_offset_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_len = tmp_var_new(&mut proc.borrow_mut());
    let t_len_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_matched = tmp_var_new(&mut proc.borrow_mut());

    let t_pre_slice = tmp_var_new(&mut proc.borrow_mut());
    let t_out_new = tmp_var_new(&mut proc.borrow_mut());
    let t_pos = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let loop_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let append_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let end_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_input_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_regex_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_regex),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_regex_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_replace_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(2.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_replace),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_replace_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_reg),
            types: IRType::NATIVE_REGEX,
            source: IRValue::BuiltinProc(BuiltinProc::RegexCompile),
            op: IROp::NativeCall(vec![IRValue::Variable(t_regex), IRValue::Number(0.into())]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRType::STRING,
            source: IRValue::String("".into()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_input)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input_slice),
            types: IRType::STRING | IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input),
                IRValue::Number(0.into()),
                IRValue::Variable(t_input_len),
            ]),
        }),
        IRStmt::Goto(loop_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, loop_idx, ());

    block_get(&mut proc.borrow_mut(), loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_offset),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_offset_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_offset),
            op: IROp::PtrAddress,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_len),
            op: IROp::PtrAddress,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_matched),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::RegexMatchLen),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input_slice),
                IRValue::Variable(t_reg),
                IRValue::Variable(t_len_addr),
                IRValue::Variable(t_offset_addr),
            ]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_matched),
            success: append_idx,
            failure: end_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(loop_idx, append_idx, ());
    proc.borrow_mut().blocks.add_edge(loop_idx, end_idx, ());

    block_get(&mut proc.borrow_mut(), append_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_pre_slice),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input_slice),
                IRValue::Number(0.into()),
                IRValue::Variable(t_offset),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out_new),
            types: IRType::STRING,
            source: IRValue::Variable(t_out),
            op: IROp::Plus(IRValue::Variable(t_pre_slice)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRType::STRING,
            source: IRValue::Variable(t_out_new),
            op: IROp::Plus(IRValue::Variable(t_replace)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out_new)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_pos),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_offset),
            op: IROp::Plus(IRValue::Variable(t_len)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input_slice),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input_slice),
                IRValue::Variable(t_pos),
                IRValue::Variable(t_input_len),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_pos)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_offset)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
        }),
        IRStmt::Goto(loop_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(append_idx, loop_idx, ());

    block_get(&mut proc.borrow_mut(), end_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_pre_slice),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input_slice),
                IRValue::Number(0.into()),
                IRValue::Variable(t_input_len),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out_new),
            types: IRType::STRING,
            source: IRValue::Variable(t_out),
            op: IROp::Plus(IRValue::Variable(t_pre_slice)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Return(IRValue::Variable(t_out_new)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = end_idx;

    proc
}

fn replace_first_stub_new() -> Rc<RefCell<IRProcedure>> {
    /*  t_input_addr := params[0];
     *  t_input := *t_input_addr;
     *  t_regex_addr := params[1];
     *  t_regex := *t_regex_addr;
     *  t_replace_addr := params[2];
     *  t_replace := *t_replace_addr;
     *  t_reg := regex_compile(t_regex, 0);
     *  t_out := "";
     *  t_input_len := amount(t_input);
     *  t_input_slice := slice(t_input, 0, t_input_len);
     *  goto <cond_idx>
     *
     * <cond_idx>:
     *  t_offset := om;
     *  t_offset_addr := &t_offset;
     *  t_len := om;
     *  t_len_addr := &t_len;
     *  t_matched := regex_match_len(t_input_slice, t_regex, t_len_addr, t_offset_addr);
     *  if t_matched
     *   goto <append_idx>
     *  else
     *   goto <end_idx>
     *
     * <append_idx>:
     *  t_pre_slice := slice(t_input_slice, 0, t_offset);
     *  t_out_new := t_out + t_pre_slice;
     *  _ := invalidate(t_out);
     *  t_out := t_out_new + t_replace;
     *  _ := invalidate(t_out_new);
     *  t_pos := t_offset + t_len;
     *  t_input_slice := slice(t_input_slice, t_pos, t_input_len);
     *  _ := invalidate(t_pos);
     *  _ := invalidate(t_offset);
     *  _ := invalidate(t_len);
     *  goto <end_idx>
     *
     * <end_idx>:
     *  t_pre_slice := slice(t_input_slice, 0, t_input_len);
     *  t_out_new := t_out + t_pre_slice;
     *  _ := invalidate(t_out);
     *  return t_out_new;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("replaceFirst")));

    let t_input_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_input = tmp_var_new(&mut proc.borrow_mut());
    let t_regex_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_regex = tmp_var_new(&mut proc.borrow_mut());
    let t_replace_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_replace = tmp_var_new(&mut proc.borrow_mut());
    let t_reg = tmp_var_new(&mut proc.borrow_mut());
    let t_out = tmp_var_new(&mut proc.borrow_mut());
    let t_input_len = tmp_var_new(&mut proc.borrow_mut());
    let t_input_slice = tmp_var_new(&mut proc.borrow_mut());

    let t_offset = tmp_var_new(&mut proc.borrow_mut());
    let t_offset_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_len = tmp_var_new(&mut proc.borrow_mut());
    let t_len_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_matched = tmp_var_new(&mut proc.borrow_mut());

    let t_pre_slice = tmp_var_new(&mut proc.borrow_mut());
    let t_out_new = tmp_var_new(&mut proc.borrow_mut());
    let t_pos = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let cond_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let append_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let end_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_input_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_regex_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_regex),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_regex_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_replace_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(2.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_replace),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_replace_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_reg),
            types: IRType::NATIVE_REGEX,
            source: IRValue::BuiltinProc(BuiltinProc::RegexCompile),
            op: IROp::NativeCall(vec![IRValue::Variable(t_regex), IRValue::Number(0.into())]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRType::STRING,
            source: IRValue::String("".into()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_input)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input_slice),
            types: IRType::STRING | IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input),
                IRValue::Number(0.into()),
                IRValue::Variable(t_input_len),
            ]),
        }),
        IRStmt::Goto(cond_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, cond_idx, ());

    block_get(&mut proc.borrow_mut(), cond_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_offset),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_offset_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_offset),
            op: IROp::PtrAddress,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_len),
            op: IROp::PtrAddress,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_matched),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::RegexMatchLen),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input_slice),
                IRValue::Variable(t_reg),
                IRValue::Variable(t_len_addr),
                IRValue::Variable(t_offset_addr),
            ]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_matched),
            success: append_idx,
            failure: end_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(cond_idx, append_idx, ());
    proc.borrow_mut().blocks.add_edge(cond_idx, end_idx, ());

    block_get(&mut proc.borrow_mut(), append_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_pre_slice),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input_slice),
                IRValue::Number(0.into()),
                IRValue::Variable(t_offset),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out_new),
            types: IRType::STRING,
            source: IRValue::Variable(t_out),
            op: IROp::Plus(IRValue::Variable(t_pre_slice)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRType::STRING,
            source: IRValue::Variable(t_out_new),
            op: IROp::Plus(IRValue::Variable(t_replace)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out_new)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_pos),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_offset),
            op: IROp::Plus(IRValue::Variable(t_len)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input_slice),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input_slice),
                IRValue::Variable(t_pos),
                IRValue::Variable(t_input_len),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_pos)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_offset)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
        }),
        IRStmt::Goto(end_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(append_idx, end_idx, ());

    block_get(&mut proc.borrow_mut(), end_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_pre_slice),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input_slice),
                IRValue::Number(0.into()),
                IRValue::Variable(t_input_len),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out_new),
            types: IRType::STRING,
            source: IRValue::Variable(t_out),
            op: IROp::Plus(IRValue::Variable(t_pre_slice)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Return(IRValue::Variable(t_out_new)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = end_idx;

    proc
}

fn split_stub_new() -> Rc<RefCell<IRProcedure>> {
    /*  t_input_addr := params[0];
     *  t_input := *t_input_addr;
     *  t_regex_addr := params[1];
     *  t_regex := *t_regex_addr;
     *  t_reg := regex_compile(t_regex, 0);
     *  t_out := list_new(0);
     *  t_input_len := amount(t_input);
     *  t_input_slice := slice(t_input, 0, t_input_len);
     *  goto <loop_idx>
     *
     * <loop_idx>:
     *  t_offset := om;
     *  t_offset_addr := &t_offset;
     *  t_len := om;
     *  t_len_addr := &t_len;
     *  t_matched := regex_match_len(t_input_slice, t_regex, t_len_addr, t_offset_addr);
     *  if t_matched
     *   goto <append_idx>
     *  else
     *   goto <end_idx>
     *
     * <append_idx>:
     *  t_pre_slice := slice(t_input_slice, 0, t_offset);
     *  t_insert := copy(t_pre_slice);
     *  _ := list_push(t_out, t_insert);
     *  t_pos := t_offset + t_len;
     *  t_input_slice := slice(t_input_slice, t_pos, t_input_len);
     *  _ := invalidate(t_pos);
     *  _ := invalidate(t_offset);
     *  _ := invalidate(t_len);
     *  goto <loop_idx>
     *
     * <end_idx>:
     *  t_pre_slice := slice(t_input_slice, 0, t_input_len);
     *  t_insert := copy(t_pre_slice);
     *  _ := list_push(t_out, t_insert);
     *  return t_out;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("split")));

    let t_input_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_input = tmp_var_new(&mut proc.borrow_mut());
    let t_regex_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_regex = tmp_var_new(&mut proc.borrow_mut());
    let t_reg = tmp_var_new(&mut proc.borrow_mut());
    let t_out = tmp_var_new(&mut proc.borrow_mut());
    let t_input_len = tmp_var_new(&mut proc.borrow_mut());
    let t_input_slice = tmp_var_new(&mut proc.borrow_mut());

    let t_offset = tmp_var_new(&mut proc.borrow_mut());
    let t_offset_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_len = tmp_var_new(&mut proc.borrow_mut());
    let t_len_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_matched = tmp_var_new(&mut proc.borrow_mut());

    let t_pre_slice = tmp_var_new(&mut proc.borrow_mut());
    let t_insert = tmp_var_new(&mut proc.borrow_mut());
    let t_pos = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let loop_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let append_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let end_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_input_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_regex_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_regex),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_regex_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_reg),
            types: IRType::NATIVE_REGEX,
            source: IRValue::BuiltinProc(BuiltinProc::RegexCompile),
            op: IROp::NativeCall(vec![IRValue::Variable(t_regex), IRValue::Number(0.into())]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(vec![IRValue::Number(0.into())]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_input)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input_slice),
            types: IRType::STRING | IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input),
                IRValue::Number(0.into()),
                IRValue::Variable(t_input_len),
            ]),
        }),
        IRStmt::Goto(loop_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, loop_idx, ());

    block_get(&mut proc.borrow_mut(), loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_offset),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_offset_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_offset),
            op: IROp::PtrAddress,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_len),
            op: IROp::PtrAddress,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_matched),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::RegexMatchLen),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input_slice),
                IRValue::Variable(t_reg),
                IRValue::Variable(t_len_addr),
                IRValue::Variable(t_offset_addr),
            ]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_matched),
            success: append_idx,
            failure: end_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(loop_idx, append_idx, ());
    proc.borrow_mut().blocks.add_edge(loop_idx, end_idx, ());

    block_get(&mut proc.borrow_mut(), append_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_pre_slice),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input_slice),
                IRValue::Number(0.into()),
                IRValue::Variable(t_offset),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_insert),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_pre_slice)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ListPush),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out), IRValue::Variable(t_insert)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_pos),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_offset),
            op: IROp::Plus(IRValue::Variable(t_len)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_input_slice),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input_slice),
                IRValue::Variable(t_pos),
                IRValue::Variable(t_input_len),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_pos)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_offset)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
        }),
        IRStmt::Goto(loop_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(append_idx, loop_idx, ());

    block_get(&mut proc.borrow_mut(), end_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_pre_slice),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Slice),
            op: IROp::NativeCall(vec![
                IRValue::Variable(t_input_slice),
                IRValue::Number(0.into()),
                IRValue::Variable(t_input_len),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_insert),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_pre_slice)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ListPush),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out), IRValue::Variable(t_insert)]),
        }),
        IRStmt::Return(IRValue::Variable(t_out)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = end_idx;

    proc
}

fn from_stub_new(tag: &str) -> Rc<RefCell<IRProcedure>> {
    /*  t_in_addr := params[0];
     *  t_in := *t_in_addr;
     *  t_out := pop(t_in);
     *  return t_out;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag(tag)));

    let t_in_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_in = tmp_var_new(&mut proc.borrow_mut());
    let t_out = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_in_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_in),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_in_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRType::STRING | IRType::LIST | IRType::SET,
            source: IRValue::BuiltinProc(BuiltinProc::Pop),
            op: IROp::NativeCall(vec![IRValue::Variable(t_in)]),
        }),
        IRStmt::Return(IRValue::Variable(t_out)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

pub fn max_stub_new(tag: &str, comp_max: bool) -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_out := om;
     *  t_iter := iter_new(t_p1);
     *  goto <cond_idx>:
     *
     * <cond_idx>:
     *  t_i := om;
     *  t_i_addr := &t_i;
     *  t_cond := iter_next(t_iter, t_i_addr);
     *  if t_cond
     *   goto <loop_idx>
     *  else
     *   goto <ret_idx>
     *
     * <loop_idx>:
     *  t_out_om := t_out == om;
     *  if t_out_om
     *   goto <assign_idx>
     *  else
     *   goto <check_idx>
     *
     * <assign_idx>:
     *  _ := invalidate(t_out);
     *  t_out := copy(t_i);
     *  goto <cond_idx>
     *
     * <check_idx>:
     *  // if comp_max {
     *  t_out_less := t_out < t_i;
     *  // } else {
     *  t_out_less := t_i < t_out;
     *  // }
     *  if t_out_less
     *   goto <assign_idx>
     *  else
     *   goto <cond_idx>
     *
     * <ret_idx>:
     *  return t_out;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag(tag)));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_out = tmp_var_new(&mut proc.borrow_mut());
    let t_iter = tmp_var_new(&mut proc.borrow_mut());
    let t_i = tmp_var_new(&mut proc.borrow_mut());
    let t_i_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_cond = tmp_var_new(&mut proc.borrow_mut());
    let t_out_om = tmp_var_new(&mut proc.borrow_mut());
    let t_out_less = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let cond_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let loop_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let assign_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let check_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc
        .borrow_mut()
        .blocks
        .add_node(vec![IRStmt::Return(IRValue::Variable(t_out))]);

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRType::UNDEFINED,
            source: IRValue::Undefined,
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_iter),
            types: IRType::ITERATOR,
            source: IRValue::BuiltinProc(BuiltinProc::IterNew),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Goto(cond_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, cond_idx, ());

    block_get(&mut proc.borrow_mut(), cond_idx).extend(vec![
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
            success: loop_idx,
            failure: ret_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(cond_idx, loop_idx, ());
    proc.borrow_mut().blocks.add_edge(cond_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out_om),
            types: IRType::BOOL,
            source: IRValue::Variable(t_out),
            op: IROp::Equal(IRValue::Undefined),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_out_om),
            success: assign_idx,
            failure: check_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(loop_idx, assign_idx, ());
    proc.borrow_mut().blocks.add_edge(loop_idx, check_idx, ());

    block_get(&mut proc.borrow_mut(), assign_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Goto(cond_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(assign_idx, cond_idx, ());

    block_get(&mut proc.borrow_mut(), check_idx).extend(vec![
        if comp_max {
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_out_less),
                types: IRType::BOOL,
                source: IRValue::Variable(t_out),
                op: IROp::Less(IRValue::Variable(t_i)),
            })
        } else {
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_out_less),
                types: IRType::BOOL,
                source: IRValue::Variable(t_i),
                op: IROp::Less(IRValue::Variable(t_out)),
            })
        },
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_out_less),
            success: assign_idx,
            failure: cond_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(check_idx, assign_idx, ());
    proc.borrow_mut().blocks.add_edge(check_idx, cond_idx, ());

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn load_library_stub_new() -> Rc<RefCell<IRProcedure>> {
    /*  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_name := t_p1 + ".stlx";
     *  t_file := open_at(LIBRARY_PATH, t_name, 0x01);
     *  _ := invalidate(t_name);
     *  t_str := read_all(t_file);
     *  _ := invalidate(t_file);
     *  _ := execute(t_str);
     *  _ := invalidate(t_str);
     *  return true;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("loadLibrary")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_name = tmp_var_new(&mut proc.borrow_mut());
    let t_file = tmp_var_new(&mut proc.borrow_mut());
    let t_str = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_name),
            types: IRType::STRING,
            source: IRValue::Variable(t_p1),
            op: IROp::Plus(IRValue::String(".stlx".to_string())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_file),
            types: IRType::FILE,
            source: IRValue::BuiltinProc(BuiltinProc::OpenAt),
            op: IROp::NativeCall(vec![
                IRValue::BuiltinVar(BuiltinVar::LibraryPath),
                IRValue::Variable(t_name),
                IRValue::Number(1.into()),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_name)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_str),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::ReadAll),
            op: IROp::NativeCall(vec![IRValue::Variable(t_file)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_file)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Execute),
            op: IROp::NativeCall(vec![IRValue::Variable(t_str)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_str)]),
        }),
        IRStmt::Return(IRValue::Bool(true)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

fn load_stub_new() -> Rc<RefCell<IRProcedure>> {
    /*  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_file := open_at(., t_p1, 0x01);
     *  t_str := read_all(t_file);
     *  _ := invalidate(t_file);
     *  _ := execute(t_str);
     *  _ := invalidate(t_str);
     *  return true;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("load")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_file = tmp_var_new(&mut proc.borrow_mut());
    let t_str = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_file),
            types: IRType::FILE,
            source: IRValue::BuiltinProc(BuiltinProc::OpenAt),
            op: IROp::NativeCall(vec![
                IRValue::String(".".to_string()),
                IRValue::Variable(t_p1),
                IRValue::Number(1.into()),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_str),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::ReadAll),
            op: IROp::NativeCall(vec![IRValue::Variable(t_file)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_file)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Execute),
            op: IROp::NativeCall(vec![IRValue::Variable(t_str)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_str)]),
        }),
        IRStmt::Return(IRValue::Bool(true)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

fn parse_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* t_p1_addr := params[0];
     * t_p1 := *t_p1_addr;
     * t_ret := parse_ast(t_p1);
     * return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("parse")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::AST,
            source: IRValue::BuiltinProc(BuiltinProc::ParseAst),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

fn parse_statements_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* t_p1_addr := params[0];
     * t_p1 := *t_p1_addr;
     * t_ret := parse_ast_block(t_p1);
     * return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("parseStatements")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::AST,
            source: IRValue::BuiltinProc(BuiltinProc::ParseAstBlock),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

fn is_term_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_ast := t_p1_type == TYPE_AST;
     *  t_p1_term := t_p1_type == TYPE_TERM;
     *  t_p1_tterm := t_p1_type == TYPE_TTERM;
     *  t_ret := t_p1_ast || t_p1_term;
     *  t_ret := t_ret || t_p1_tterm;
     *  return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("isTerm")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_ast = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_term = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_tterm = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_ast),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::AST)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_term),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::TERM)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_tterm),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::TTERM)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_ast),
            op: IROp::Or(IRValue::Variable(t_p1_term)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::BOOL,
            source: IRValue::Variable(t_ret),
            op: IROp::Or(IRValue::Variable(t_p1_tterm)),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

fn is_variable_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_ret := false;
     *  t_p1_ast := t_p1_type == TYPE_AST;
     *  if t_p1_ast
     *   goto <ast_idx>
     *  else
     *   goto <ret_idx>
     *
     * <ast_idx>:
     *  t_tag := t_p1[0];
     *  t_ret := t_tag == "var";
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("isVariable")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_ast = tmp_var_new(&mut proc.borrow_mut());
    let t_tag = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ast_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc
        .borrow_mut()
        .blocks
        .add_node(vec![IRStmt::Return(IRValue::Variable(t_ret))]);

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::BOOL,
            source: IRValue::Bool(false),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_ast),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::AST)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_ast),
            success: ast_idx,
            failure: ret_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, ast_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), ast_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_tag),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::BOOL,
            source: IRValue::Variable(t_tag),
            op: IROp::Equal(IRValue::String("var".into())),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(ast_idx, ret_idx, ());

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn is_type_stub_new(tag: &str, t: IRType) -> Rc<RefCell<IRProcedure>> {
    /* t_p1_addr := params[0];
     * t_p1 := *t_p1_addr;
     * t_p1_type := type_of(t_p1);
     * t_ret := t_p1_type == TYPE_STRING;
     * return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag(tag)));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(t)),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

fn is_number_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* t_p1_addr := params[0];
     * t_p1 := *t_p1_addr;
     * t_p1_type := type_of(t_p1);
     * t_n := t_p1_type == TYPE_NUMBER;
     * t_d := t_p1_type == TYPE_DOUBLE;
     * t_ret := t_n || t_d;
     * return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("isNumber")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_n = tmp_var_new(&mut proc.borrow_mut());
    let t_d = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_n),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_d),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::DOUBLE)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::BOOL,
            source: IRValue::Variable(t_d),
            op: IROp::Or(IRValue::Variable(t_n)),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

fn log10_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_int := t_p1_type == TYPE_NUMBER;
     *  t_p1_float := t_p1_type == TYPE_DOUBLE;
     *  t_p1_num := t_p1_int || t_p1_float;
     *  if t_p1_num
     *   goto <assign_idx>
     *  else
     *   goto <fail_idx>
     *
     * <assign_idx>:
     *  t_ln := ln(t_p1_n);
     *  t_ln_t := ln(10);
     *  t_ret := t_ln / t_ln_t;
     *  _ := invalidate(t_ln);
     *  _ := invalidate(t_ln_t);
     *  goto <ret_idx>
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "expected number or float");
     *  unreachable;
     *
     * <ret_idx>:
     *  return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("log10")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_int = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_float = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_num = tmp_var_new(&mut proc.borrow_mut());
    let t_ln = tmp_var_new(&mut proc.borrow_mut());
    let t_ln_t = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let assign_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_int),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_float),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::DOUBLE)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_num),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_int),
            op: IROp::Or(IRValue::Variable(t_p1_float)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_num),
            success: assign_idx,
            failure: fail_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, assign_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), assign_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ln),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Ln),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ln_t),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Ln),
            op: IROp::NativeCall(vec![IRValue::Number(10.into())]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::DOUBLE,
            source: IRValue::Variable(t_ln),
            op: IROp::Divide(IRValue::Variable(t_ln_t)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_ln)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_ln_t)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(assign_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".to_string()),
                IRValue::String("expected number or float".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_ret)));

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn log1p_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_int := t_p1_type == TYPE_NUMBER;
     *  t_p1_float := t_p1_type == TYPE_DOUBLE;
     *  t_p1_num := t_p1_int || t_p1_float;
     *  if t_p1_num
     *   goto <assign_idx>
     *  else
     *   goto <fail_idx>
     *
     * <assign_idx>:
     *  t_p1_n := t_p1 + 1;
     *  t_ret := ln(t_p1_n);
     *  _ := invalidate(t_p1_n);
     *  goto <ret_idx>
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "expected number or float");
     *  unreachable;
     *
     * <ret_idx>:
     *  return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("log1p")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_int = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_float = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_num = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_n = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let assign_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_int),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_float),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::DOUBLE)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_num),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_int),
            op: IROp::Or(IRValue::Variable(t_p1_float)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_num),
            success: assign_idx,
            failure: fail_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, assign_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), assign_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_n),
            types: IRTypes!("plus"),
            source: IRValue::Variable(t_p1),
            op: IROp::Plus(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Ln),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1_n)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1_n)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(assign_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".to_string()),
                IRValue::String("expected number or float".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_ret)));

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn ln_exp_stub_new(is_exp: bool) -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  // block_obj_overload_push
     *
     * <dfl_idx>:
     *  t_p1_type := type_of(t_p1);
     *  t_p1_int := t_p1_type == TYPE_NUMBER;
     *  t_p1_float := t_p1_type == TYPE_DOUBLE;
     *  t_p1_num := t_p1_int || t_p1_float;
     *  if t_p1_num
     *   goto <assign_idx>
     *  else
     *   goto <fail_idx>
     *
     * <assign_idx>:
     *  t_ret := ln(t_p1);
     *  goto <ret_idx>
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "expected number, float, or object");
     *  unreachable;
     *
     * <ret_idx>:
     *  return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag(if is_exp {
        "exp"
    } else {
        "log"
    })));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_int = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_float = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_num = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let dfl_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let assign_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
    ]);

    block_obj_overload_push(
        init_idx,
        dfl_idx,
        ret_idx,
        false,
        IRTarget::Variable(t_ret),
        t_p1,
        ObjOverloadRhs::None,
        if is_exp { "f_exp" } else { "f_log" },
        &mut proc.borrow_mut(),
        &mut IRSharedProc::default(),
        &mut IRCfg::from_proc(proc.clone()),
    );

    block_get(&mut proc.borrow_mut(), dfl_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_int),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_float),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::DOUBLE)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_num),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_int),
            op: IROp::Or(IRValue::Variable(t_p1_float)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_num),
            success: assign_idx,
            failure: fail_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(dfl_idx, assign_idx, ());
    proc.borrow_mut().blocks.add_edge(dfl_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), assign_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::DOUBLE,
            source: IRValue::BuiltinProc(if is_exp {
                BuiltinProc::Exp
            } else {
                BuiltinProc::Ln
            }),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(assign_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".to_string()),
                IRValue::String("expected number, float, or object".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_ret)));

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn sqrt_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_int := t_p1_type == TYPE_NUMBER;
     *  t_p1_float := t_p1_type == TYPE_DOUBLE;
     *  t_p1_num := t_p1_int || t_p1_float;
     *  if t_p1_num
     *   goto <assign_idx>
     *  else
     *   goto <fail_idx>
     *
     * <assign_idx>:
     *  t_ret := sqrt(t_p1);
     *  goto <ret_idx>
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "expected number, float, or object");
     *  unreachable;
     *
     * <ret_idx>:
     *  return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("sqrt")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_int = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_float = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_num = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let assign_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_int),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_float),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::DOUBLE)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_num),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_int),
            op: IROp::Or(IRValue::Variable(t_p1_float)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_num),
            success: assign_idx,
            failure: fail_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, assign_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), assign_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::DOUBLE,
            source: IRValue::BuiltinProc(BuiltinProc::Sqrt),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(assign_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".to_string()),
                IRValue::String("expected number, float, or object".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_ret)));

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

pub fn domain_range_stub_new(is_range: bool) -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_set := t_p1_type == TYPE_SET;
     *  if t_p1_set
     *   goto <iter_idx>
     *  else
     *   goto <fail_idx>
     *
     * <iter_idx>:
     *  t_set := set_new();
     *  t_iter := iter_new(t_p1);
     *  goto <iter_cond_idx>
     *
     * <iter_cond_idx>:
     *  t_i := om;
     *  t_i_addr := &t_i;
     *  t_cond := iter_next(t_iter, t_i_addr);
     *  if t_cond
     *   goto <check_list_idx>
     *  else
     *   goto <ret_idx>
     *
     * <check_list_idx>:
     *  t_i_type := type_of(t_i);
     *  t_i_list := t_i_type == TYPE_LIST;
     *  if t_i_list
     *   goto <check_len_idx>
     *  else
     *   goto <iter_cond_idx>
     *
     * <check_len_idx>:
     *  t_i_len := amount(t_i);
     *  t_i_len_eq := t_i_len == 2;
     *  _ := invalidate(t_i_len);
     *  if t_i_len_eq
     *   goto <set_insert_idx>
     *  else
     *   goto <iter_cond_idx>
     *
     * <set_insert_idx>:
     *  t_j := t_i[0];
     *  t_insert := copy(t_i);
     *  _ := set_insert(t_set, t_insert);
     *  goto <iter_cond_idx>
     *
     * <ret_idx>:
     *  return t_set;
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "operand is not a set");
     *  unreachable;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag(if is_range {
        "range"
    } else {
        "domain"
    })));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_set = tmp_var_new(&mut proc.borrow_mut());

    let t_set = tmp_var_new(&mut proc.borrow_mut());
    let t_iter = tmp_var_new(&mut proc.borrow_mut());

    let t_i = tmp_var_new(&mut proc.borrow_mut());
    let t_j = tmp_var_new(&mut proc.borrow_mut());
    let t_i_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_cond = tmp_var_new(&mut proc.borrow_mut());

    let t_i_type = tmp_var_new(&mut proc.borrow_mut());
    let t_i_list = tmp_var_new(&mut proc.borrow_mut());

    let t_i_len = tmp_var_new(&mut proc.borrow_mut());
    let t_i_len_eq = tmp_var_new(&mut proc.borrow_mut());

    let t_insert = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let iter_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let iter_cond_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let check_list_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let check_len_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let set_insert_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_set),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::SET)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_set),
            success: iter_idx,
            failure: fail_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, iter_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), iter_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_set),
            types: IRType::SET,
            source: IRValue::BuiltinProc(BuiltinProc::SetNew),
            op: IROp::NativeCall(vec![]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_iter),
            types: IRType::ITERATOR,
            source: IRValue::BuiltinProc(BuiltinProc::IterNew),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Goto(iter_cond_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(iter_idx, iter_cond_idx, ());

    block_get(&mut proc.borrow_mut(), iter_cond_idx).extend(vec![
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
            success: check_list_idx,
            failure: ret_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(iter_cond_idx, check_list_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(iter_cond_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), check_list_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_list),
            types: IRType::BOOL,
            source: IRValue::Variable(t_i_type),
            op: IROp::Equal(IRValue::Type(IRType::LIST)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_i_list),
            success: check_len_idx,
            failure: iter_cond_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(check_list_idx, check_len_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(check_list_idx, iter_cond_idx, ());

    block_get(&mut proc.borrow_mut(), check_len_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_len_eq),
            types: IRType::BOOL,
            source: IRValue::Variable(t_i_len),
            op: IROp::Equal(IRValue::Number(2.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i_len)]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_i_len_eq),
            success: set_insert_idx,
            failure: iter_cond_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(check_len_idx, set_insert_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(check_len_idx, iter_cond_idx, ());

    block_get(&mut proc.borrow_mut(), set_insert_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_j),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_i),
            op: IROp::AccessArray(IRValue::Number(if is_range { 1.into() } else { 0.into() })),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_insert),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_j)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::SetInsert),
            op: IROp::NativeCall(vec![IRValue::Variable(t_set), IRValue::Variable(t_insert)]),
        }),
        IRStmt::Goto(iter_cond_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(set_insert_idx, iter_cond_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_set)));

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".to_string()),
                IRValue::String("operand is not a set".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

pub fn sleep_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_int := t_p1_type == TYPE_NUMBER;
     *  if t_p1_int
     *   goto <check_idx>
     *  else
     *   goto <fail_idx>
     *
     * <check_idx>:
     *  t_np := t_p1 < 1;
     *  if t_np
     *   goto <fail_idx>
     *  else
     *   goto <sleep_idx>
     *
     * <sleep_idx>:
     *  _ := sleep(t_p1);
     *  return om;
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "sleep is not a positive integer");
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("sleep")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_int = tmp_var_new(&mut proc.borrow_mut());
    let t_np = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let check_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let sleep_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_int),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_int),
            success: check_idx,
            failure: fail_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, check_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_np),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1),
            op: IROp::Less(IRValue::Number(1.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_np),
            success: fail_idx,
            failure: sleep_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(check_idx, fail_idx, ());
    proc.borrow_mut().blocks.add_edge(check_idx, sleep_idx, ());

    block_get(&mut proc.borrow_mut(), sleep_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Sleep),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Return(IRValue::Undefined),
    ]);

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".to_string()),
                IRValue::String("sleep is not a positive integer".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = sleep_idx;

    proc
}

pub fn reverse_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_list := t_p1_type == TYPE_LIST;
     *  if t_p1_list
     *   goto <list_new_idx>
     *  else
     *   goto <str_check_idx>
     *
     * <list_new_idx>:
     *  t_ret := list_new();
     *  t_len := amount(t_p1);
     *  t_i := t_len;
     *  goto <list_cond_idx>
     *
     * <list_cond_idx>:
     *  t_cond := 0 < t_i;
     *  if t_cond
     *   goto <list_loop_idx>
     *  else
     *   goto <list_inv_idx>
     *
     * <list_loop_idx>:
     *  t_i_new := t_i - 1;
     *  _ := invalidate(t_i);
     *  t_i := t_i_new;
     *  t_obj := t_p1[t_i];
     *  t_obj := copy(t_obj);
     *  _ := list_push(t_ret, t_obj);
     *  goto <list_cond_idx>
     *
     * <list_inv_idx>:
     *  _ := invalidate(t_i);
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  return t_ret;
     *
     * <str_check_idx>:
     *  t_p1_string := t_p1_type == TYPE_STING;
     *  if t_p1_string
     *   goto <str_new_idx>
     *  else
     *   goto <fail_idx>
     *
     * <str_new_idx>:
     *  t_ret := "";
     *  t_len := amount(t_p1);
     *  t_i := t_len;
     *  goto <str_cond_idx>
     *
     * <str_cond_idx>:
     *  t_cond := 0 < t_i;
     *  if t_cond
     *   goto <str_loop_idx>
     *  else
     *   goto <str_inv_idx>
     *
     * <str_loop_idx>:
     *  t_i_new := t_i - 1;
     *  _ := invalidate(t_i);
     *  t_i := t_i_new;
     *  t_obj := t_p1[t_i];
     *  t_ret_new := t_ret + t_obj;
     *  _ := invalidate(t_ret);
     *  r_ret := t_ret_new;
     *  goto <str_cond_idx>
     *
     * <str_inv_idx>:
     *  _ := invalidate(t_i);
     *  goto <ret_idx>
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "reverse only supports strings or lists");
     *  unreachable;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("reverse")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_list = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_string = tmp_var_new(&mut proc.borrow_mut());

    let t_ret = tmp_var_new(&mut proc.borrow_mut());
    let t_len = tmp_var_new(&mut proc.borrow_mut());
    let t_i = tmp_var_new(&mut proc.borrow_mut());
    let t_cond = tmp_var_new(&mut proc.borrow_mut());
    let t_i_new = tmp_var_new(&mut proc.borrow_mut());
    let t_obj = tmp_var_new(&mut proc.borrow_mut());

    let t_ret_new = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let list_new_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let list_cond_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let list_loop_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let list_inv_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let str_check_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let str_new_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let str_cond_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let str_loop_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let str_inv_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_list),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::LIST)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_list),
            success: list_new_idx,
            failure: str_check_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(init_idx, list_new_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(init_idx, str_check_idx, ());

    block_get(&mut proc.borrow_mut(), list_new_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ListNew),
            op: IROp::NativeCall(vec![]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_len),
            op: IROp::Assign,
        }),
        IRStmt::Goto(list_cond_idx),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(list_new_idx, list_cond_idx, ());

    block_get(&mut proc.borrow_mut(), list_cond_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cond),
            types: IRType::BOOL,
            source: IRValue::Number(0.into()),
            op: IROp::Less(IRValue::Variable(t_i)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cond),
            success: list_loop_idx,
            failure: list_inv_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(list_cond_idx, list_loop_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(list_cond_idx, list_inv_idx, ());

    block_get(&mut proc.borrow_mut(), list_loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_new),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_i),
            op: IROp::Minus(IRValue::Number(1.into())),
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
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_obj),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1),
            op: IROp::AccessArray(IRValue::Variable(t_i)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_obj),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_obj)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ListPush),
            op: IROp::NativeCall(vec![IRValue::Variable(t_ret), IRValue::Variable(t_obj)]),
        }),
        IRStmt::Goto(list_cond_idx),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(list_loop_idx, list_cond_idx, ());

    block_get(&mut proc.borrow_mut(), list_inv_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(list_inv_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), str_check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_string),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::STRING)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_string),
            success: str_new_idx,
            failure: fail_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(str_check_idx, str_new_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(str_check_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), str_new_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::String(String::new()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_len),
            op: IROp::Assign,
        }),
        IRStmt::Goto(str_cond_idx),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(str_new_idx, str_cond_idx, ());

    block_get(&mut proc.borrow_mut(), str_cond_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cond),
            types: IRType::BOOL,
            source: IRValue::Number(0.into()),
            op: IROp::Less(IRValue::Variable(t_i)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cond),
            success: str_loop_idx,
            failure: str_inv_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(str_cond_idx, str_loop_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(str_cond_idx, str_inv_idx, ());

    block_get(&mut proc.borrow_mut(), str_loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_new),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_i),
            op: IROp::Minus(IRValue::Number(1.into())),
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
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_obj),
            types: IRType::STRING,
            source: IRValue::Variable(t_p1),
            op: IROp::AccessArray(IRValue::Variable(t_i)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret_new),
            types: IRType::STRING,
            source: IRValue::Variable(t_ret),
            op: IROp::Plus(IRValue::Variable(t_obj)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_ret)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::Variable(t_ret_new),
            op: IROp::Assign,
        }),
        IRStmt::Goto(str_cond_idx),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(str_loop_idx, str_cond_idx, ());

    block_get(&mut proc.borrow_mut(), str_inv_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(str_inv_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_ret)));

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".to_string()),
                IRValue::String("reverse only supports strings or lists".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn os_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "Windows",
        "macos" => "Mac OS X",
        "ios" => "iOS",
        "android" => "Android",
        "linux" => "Linux",
        "freebsd" => "FreeBSD",
        "dragonfly" => "DragonFly BSD",
        "netbsd" => "NetBSD",
        "openbsd" => "OpenBSD",
        "solaris" => "Solaris",
        "haiku" => "Haiku",
        "redox" => "Redox",
        other => other,
    }
}
pub fn get_os_id_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  return os_name;
     *
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("getOsID")));

    let init_idx = proc
        .borrow_mut()
        .blocks
        .add_node(vec![IRStmt::Return(IRValue::String(os_name().to_string()))]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

pub fn math_const_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_str := t_p1_type == TYPE_STRING;
     *  if t_p1_str
     *   goto <check_pi_idx>
     *  else
     *   goto <fail_idx>
     *
     * <check_pi_idx>:
     *  t_p1_eq := t_p1 == "pi";
     *  if t_p1_eq
     *   goto <pi_idx>
     *  else
     *   goto <check_e_idx>
     *
     * <pi_idx>:
     *  t_ret := pi;
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  return t_ret;
     *
     * <check_e_idx>:
     *  t_p1_eq := t_p1 == "e";
     *  if t_p1_eq
     *   goto <e_idx>
     *  else
     *   goto <check_inf_idx>
     *
     * <e_idx>:
     *  t_ret := e;
     *  goto <ret_idx>
     *
     * <check_inf_idx>:
     *  t_p1_eq_low := t_p1 == "infinity";
     *  t_p1_eq_cap := t_p1 == "Infinity";
     *  t_p1_eq := t_p1_eq_low || t_p1_eq_cap;
     *  if t_p1_eq
     *   goto <inf_idx>
     *  else
     *   goto <fail_idx>
     *
     * <inf_idx>:
     *  t_ret := infinity;
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "unknown math constant");
     *  unreachable;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("mathConst")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_str = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_eq_low = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_eq_cap = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_eq = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let check_pi_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let pi_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let check_e_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let e_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let check_inf_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let inf_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_str),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::STRING)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_str),
            success: check_pi_idx,
            failure: fail_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(init_idx, check_pi_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), check_pi_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_eq),
            types: IRType::BOOL,
            source: IRValue::String("pi".to_string()),
            op: IROp::Equal(IRValue::Variable(t_p1)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_eq),
            success: pi_idx,
            failure: check_e_idx,
        }),
    ]);
    proc.borrow_mut().blocks.add_edge(check_pi_idx, pi_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(check_pi_idx, check_e_idx, ());

    block_get(&mut proc.borrow_mut(), pi_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::DOUBLE,
            source: IRValue::Double(std::f64::consts::PI),
            op: IROp::Assign,
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(pi_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), check_e_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_eq),
            types: IRType::BOOL,
            source: IRValue::String("e".to_string()),
            op: IROp::Equal(IRValue::Variable(t_p1)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_eq),
            success: e_idx,
            failure: check_inf_idx,
        }),
    ]);
    proc.borrow_mut().blocks.add_edge(check_e_idx, e_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(check_e_idx, check_inf_idx, ());

    block_get(&mut proc.borrow_mut(), e_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::DOUBLE,
            source: IRValue::Double(std::f64::consts::E),
            op: IROp::Assign,
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(e_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), check_inf_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_eq_low),
            types: IRType::BOOL,
            source: IRValue::String("infinity".to_string()),
            op: IROp::Equal(IRValue::Variable(t_p1)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_eq_cap),
            types: IRType::BOOL,
            source: IRValue::String("Infinity".to_string()),
            op: IROp::Equal(IRValue::Variable(t_p1)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_eq),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_eq_low),
            op: IROp::Or(IRValue::Variable(t_p1_eq_cap)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_eq),
            success: inf_idx,
            failure: fail_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(check_inf_idx, inf_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(check_inf_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), inf_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::DOUBLE,
            source: IRValue::Double(f64::INFINITY),
            op: IROp::Assign,
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(inf_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_ret)));

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".to_string()),
                IRValue::String("unknown math constant".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

pub fn now_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_ret := unix_epoch();
     *  return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("now")));

    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::UnixEpoch),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

#[derive(Clone, Copy, Display, Debug, EnumString)]
#[strum(serialize_all = "camelCase")]
enum FloatOp {
    Round,
    Floor,
    Ceil,
}

fn float_op_stub_new(op: FloatOp) -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_int := t_p1_type == TYPE_NUMBER;
     *  t_p1_d := t_p1_type == TYPE_DOUBLE;
     *  t_p1_num := t_p1_int || t_p1_d;
     *  if t_p1_num
     *   goto <num_check_idx>
     *  else
     *   goto <fail_idx>
     *
     * <num_check_idx>:
     *  if t_p1_int
     *   goto <int_idx>
     *  else
     *   goto <float_idx>
     *
     * <int_idx>:
     *  t_ret := copy(t_p1);
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  return t_ret;
     *
     * <float_idx>:
     *  t_ret := round(t_p1);
     *  goto <ret_idx>
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "round unsupported for type");
     *  unreachable;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag(&op.to_string())));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_int = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_d = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_num = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let num_check_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let int_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let float_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_int),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_d),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::DOUBLE)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_num),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_int),
            op: IROp::Or(IRValue::Variable(t_p1_d)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_num),
            success: num_check_idx,
            failure: fail_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(init_idx, num_check_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), num_check_idx).push(IRStmt::Branch(IRBranch {
        cond: IRValue::Variable(t_p1_int),
        success: int_idx,
        failure: float_idx,
    }));
    proc.borrow_mut()
        .blocks
        .add_edge(num_check_idx, int_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(num_check_idx, float_idx, ());

    block_get(&mut proc.borrow_mut(), int_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(int_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), float_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::DOUBLE,
            source: IRValue::BuiltinProc(match op {
                FloatOp::Round => BuiltinProc::Round,
                FloatOp::Floor => BuiltinProc::Floor,
                FloatOp::Ceil => BuiltinProc::Ceil,
            }),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(float_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_ret)));

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".to_string()),
                IRValue::String("round unsupported for type".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn abs_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_int := t_p1_type == TYPE_NUMBER;
     *  t_p1_d := t_p1_type == TYPE_DOUBLE;
     *  t_p1_num := t_p1_int || t_p1_d;
     *  if t_p1_num
     *   goto <neg_check_idx>
     *  else
     *   goto <check_str_idx>
     *
     * <neg_check_idx>:
     *  t_p1_neg := t_p1 < 0;
     *  if t_p1_neg
     *   goto <inv_idx>
     *  else
     *   goto <int_idx>
     *
     * <inv_idx>:
     *  t_ret := 0 - t_p1;
     *  goto <ret_idx>
     *
     * <int_idx>:
     *  t_ret := copy(t_p1);
     *  goto <ret_idx>
     *
     * <check_str_idx>:
     *  t_p1_str := t_p1_type == TYPE_STRING;
     *  if t_p1_str
     *   goto <check_str_len_idx>
     *  else
     *   goto <fail_idx>
     *
     * <check_str_len_idx>:
     *  t_p1_len := amount(t_p1);
     *  t_p1_unary := t_p1_len == 1;
     *  _ := invalidate(t_p1_len);
     *  if t_p1_unary
     *   goto <abs_str_idx>
     *  else
     *   goto <fail_idx>
     *
     * <abs_str_idx>:
     *  t_ret := str_val(t_p1);
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  return t_ret;
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "abs unsupported for type");
     *  unreachable;
     */

    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("abs")));

    let mut proc_ref = proc.borrow_mut();

    let t_p1_addr = tmp_var_new(&mut proc_ref);
    let t_p1 = tmp_var_new(&mut proc_ref);
    let t_p1_type = tmp_var_new(&mut proc_ref);
    let t_p1_int = tmp_var_new(&mut proc_ref);
    let t_p1_d = tmp_var_new(&mut proc_ref);
    let t_p1_num = tmp_var_new(&mut proc_ref);
    let t_p1_neg = tmp_var_new(&mut proc_ref);
    let t_p1_len = tmp_var_new(&mut proc_ref);
    let t_ret = tmp_var_new(&mut proc_ref);

    let init_idx = proc_ref.blocks.add_node(Vec::new());
    let neg_check_idx = proc_ref.blocks.add_node(Vec::new());
    let inv_idx = proc_ref.blocks.add_node(Vec::new());
    let int_idx = proc_ref.blocks.add_node(Vec::new());
    let check_str_idx = proc_ref.blocks.add_node(Vec::new());
    let check_str_len_idx = proc_ref.blocks.add_node(Vec::new());
    let abs_str_idx = proc_ref.blocks.add_node(Vec::new());
    let ret_idx = proc_ref.blocks.add_node(Vec::new());
    let fail_idx = proc_ref.blocks.add_node(Vec::new());

    block_get(&mut proc_ref, init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_int),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_d),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::DOUBLE)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_num),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_int),
            op: IROp::Or(IRValue::Variable(t_p1_d)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_num),
            success: neg_check_idx,
            failure: check_str_idx,
        }),
    ]);

    proc_ref.blocks.add_edge(init_idx, neg_check_idx, ());
    proc_ref.blocks.add_edge(init_idx, check_str_idx, ());

    block_get(&mut proc_ref, neg_check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_neg),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1),
            op: IROp::Less(IRValue::Number(0.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_neg),
            success: inv_idx,
            failure: int_idx,
        }),
    ]);

    proc_ref.blocks.add_edge(neg_check_idx, inv_idx, ());
    proc_ref.blocks.add_edge(neg_check_idx, int_idx, ());

    block_get(&mut proc_ref, inv_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::DOUBLE,
            source: IRValue::Number(0.into()),
            op: IROp::Minus(IRValue::Variable(t_p1)),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc_ref.blocks.add_edge(inv_idx, ret_idx, ());

    block_get(&mut proc_ref, int_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc_ref.blocks.add_edge(int_idx, ret_idx, ());

    block_get(&mut proc_ref, check_str_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::STRING)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_type),
            success: check_str_len_idx,
            failure: fail_idx,
        }),
    ]);

    proc_ref
        .blocks
        .add_edge(check_str_idx, check_str_len_idx, ());
    proc_ref.blocks.add_edge(check_str_idx, fail_idx, ());

    block_get(&mut proc_ref, check_str_len_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_num),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_len),
            op: IROp::Equal(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1_len)]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_num),
            success: abs_str_idx,
            failure: fail_idx,
        }),
    ]);

    proc_ref.blocks.add_edge(check_str_len_idx, abs_str_idx, ());
    proc_ref.blocks.add_edge(check_str_len_idx, fail_idx, ());

    block_get(&mut proc_ref, abs_str_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::StrVal),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc_ref.blocks.add_edge(abs_str_idx, ret_idx, ());

    block_get(&mut proc_ref, ret_idx).push(IRStmt::Return(IRValue::Variable(t_ret)));

    block_get(&mut proc_ref, fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".to_string()),
                IRValue::String("abs unsupported for type".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    proc_ref.start_block = init_idx;
    proc_ref.end_block = ret_idx;

    drop(proc_ref);
    proc
}

fn is_infinite_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_float := t_p1_type == TYPE_FLOAT;
     *  if t_p1_float
     *   goto <is_infinite_idx>
     *  else
     *   goto <false_idx>
     *
     * <is_infinite_idx>:
     *  t_ret := t_p1 == infinite;
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  return t_ret;
     *
     * <false_idx>:
     *  t_ret := false;
     *  goto <ret_idx>
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("isInfinite")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_float = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let is_infinite_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let false_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_float),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::DOUBLE)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_float),
            success: is_infinite_idx,
            failure: false_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(init_idx, is_infinite_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, false_idx, ());

    block_get(&mut proc.borrow_mut(), is_infinite_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1),
            op: IROp::Equal(IRValue::Double(f64::INFINITY)),
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(is_infinite_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), false_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::BOOL,
            source: IRValue::Bool(false),
            op: IROp::Assign,
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(false_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).push(IRStmt::Return(IRValue::Variable(t_ret)));

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn char_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_int := t_p1_type == TYPE_NUMBER;
     *  if t_p1_int
     *   goto <range_idx>
     *  else
     *   goto <fail_idx>
     *
     * <range_idx>:
     *  t_cond := t_p1 < 256;
     *  if t_cond
     *   goto <cast_idx>
     *  else
     *   goto <fail_idx>
     *
     * <cast_idx>:
     *  t_ret := to_char(t_p1);
     *  return t_ret;
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "char only supports integers from 0 to 255");
     *  unreachable;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("char")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_int = tmp_var_new(&mut proc.borrow_mut());
    let t_cond = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let range_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let cast_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_int),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_int),
            success: range_idx,
            failure: fail_idx,
        }),
    ]);
    proc.borrow_mut().blocks.add_edge(init_idx, range_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), range_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cond),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1),
            op: IROp::Less(IRValue::Number(256.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cond),
            success: cast_idx,
            failure: fail_idx,
        }),
    ]);
    proc.borrow_mut().blocks.add_edge(range_idx, cast_idx, ());
    proc.borrow_mut().blocks.add_edge(range_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), cast_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::ToChar),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".to_string()),
                IRValue::String("char only supports integers from 0 to 255".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = cast_idx;

    proc
}

fn run_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_str := t_p1_type == TYPE_STRING;
     *  if t_p1_str
     *   goto <num_check_idx>
     *  else
     *   goto <fail_idx>
     *
     * <float_idx>:
     *  t_ret := cmd(t_p1);
     *  return t_ret;
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "round unsupported for type");
     *  unreachable;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("run")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_str = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let float_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_str),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::STRING)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_str),
            success: float_idx,
            failure: fail_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, float_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), float_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Cmd),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".to_string()),
                IRValue::String("run unsupported for type".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = float_idx;

    proc
}

fn random_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_int := t_p1_type == TYPE_NUMBER;
     *  t_p1_float := t_p1_type == TYPE_DOUBLE;
     *  t_p1_num := t_p1_int || t_p1_float;
     *  if t_p1_num
     *   goto <rnd_idx>
     *  else
     *   goto <fail_idx>
     *
     * <rnd_idx>:
     *  t_rng := rnd_float();
     *  t_ret := t_p1 * t_rng;
     *  return t_ret;
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "random unsupported for type");
     *  unreachable;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("random")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_int = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_float = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_num = tmp_var_new(&mut proc.borrow_mut());
    let t_rng = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let rnd_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_int),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_float),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::DOUBLE)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_num),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_int),
            op: IROp::Or(IRValue::Variable(t_p1_float)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_num),
            success: rnd_idx,
            failure: fail_idx,
        }),
    ]);

    proc.borrow_mut().blocks.add_edge(init_idx, rnd_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), rnd_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_rng),
            types: IRType::DOUBLE,
            source: IRValue::BuiltinProc(BuiltinProc::RndFloat),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::DOUBLE,
            source: IRValue::Variable(t_p1),
            op: IROp::Mult(IRValue::Variable(t_rng)),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".to_string()),
                IRValue::String("random unsupported for type".to_string()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = rnd_idx;

    proc
}

#[derive(Clone, Copy, Display, Debug, EnumString)]
#[strum(serialize_all = "lowercase")]
enum NumOp {
    Sin,
    Cos,
    Tan,
    SinH,
    CosH,
    TanH,
    Ulp,
}

fn num_op_stub_new(op: NumOp) -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_int := t_p1_type == TYPE_NUMBER;
     *  t_p1_d := t_p1_type == TYPE_DOUBLE;
     *  t_p1_num := t_p1_int || t_p1_d;
     *  if t_p1_num
     *   goto <num_check_idx>
     *  else
     *   goto <fail_idx>
     *
     * <float_idx>:
     *  t_ret := sin(t_p1);
     *  return t_ret;
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "round unsupported for type");
     *  unreachable;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag(&op.to_string())));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_int = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_d = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_num = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let float_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_int),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_d),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::DOUBLE)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_num),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_int),
            op: IROp::Or(IRValue::Variable(t_p1_d)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_num),
            success: float_idx,
            failure: fail_idx,
        }),
    ]);
    proc.borrow_mut().blocks.add_edge(init_idx, float_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), float_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::DOUBLE,
            source: IRValue::BuiltinProc(match op {
                NumOp::Sin => BuiltinProc::Sin,
                NumOp::Cos => BuiltinProc::Cos,
                NumOp::Tan => BuiltinProc::Tan,
                NumOp::SinH => BuiltinProc::SinH,
                NumOp::CosH => BuiltinProc::CosH,
                NumOp::TanH => BuiltinProc::TanH,
                NumOp::Ulp => BuiltinProc::Ulp,
            }),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".to_string()),
                IRValue::String(format!("{op} unsupported for type")),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = float_idx;

    proc
}

pub fn str_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_ret := serialize(t_p1);
     *  return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("str")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Serialize),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

pub fn compare_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p2_addr := params[1];
     *  t_p2 := *t_p2_addr;
     *  t_ret := cmp(t_p1, t_p2);
     *  return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("compare")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p2_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p2 = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p2_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Cmp),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1), IRValue::Variable(t_p2)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

pub fn ask_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p2_addr := params[1];
     *  t_p2 := *t_p2_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_str := t_p1_type == TYPE_STRING;
     *  t_p2_type := type_of(t_p2);
     *  t_p2_list := t_p2_type == TYPE_LIST;
     *  t_check := t_p1_str && t_p2_list;
     *  if t_check
     *   goto <list_check_idx>
     *  else
     *   goto <fail_idx>
     *
     * <list_check_idx>:
     *  t_p1_nl := t_p1 + "\n";
     *  _ := print_stdout(t_p1_nl);
     *  _ := invalidate(t_p1_nl);
     *  t_len := amount(t_p2);
     *  t_len_zero := t_len == 0;
     *  if t_len_zero
     *   goto <fail_idx>
     *  else
     *   goto <iter_opts_init_idx>
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "invalid parameters");
     *  unreachable;
     *
     * <iter_opts_init_idx>:
     *  t_i := 0;
     *  goto <iter_opts_idx>
     *
     * <iter_opts_idx>:
     *  t_i_cond := t_i < t_len;
     *  if t_i_cond
     *   goto <iter_opts_loop_idx>
     *  else
     *   goto <check_one_idx>
     *
     * <iter_opts_loop_idx>:
     *  t_j := t_p2[t_i];
     *  _ := invalidate(t_i);
     *  t_i := t_i_new;
     *  t_i_s := serialize(t_i);
     *  t_j_s := serialize(t_j);
     *  t_s := t_i_s + ") ";
     *  t_s_new := t_s + t_j_s;
     *  _ := invalidate(t_s);
     *  t_s := t_s_new + "\n";
     *  _ := print_stdout(t_s);
     *  _ := invalidate(t_i_s);
     *  _ := invalidate(t_j_s);
     *  _ := invalidate(t_s);
     *  t_i_new := t_i + 1;
     *  goto <iter_opts_idx>
     *
     * <check_one_idx>:
     *  _ := invalidate(t_i);
     *  t_len_one := t_len == 1;
     *  if t_len_one
     *   goto <one_idx>
     *  else
     *   goto <selection_idx>
     *
     * <one_idx>:
     *  t_j := t_p2[0];
     *  t_j_s := serialize(t_j);
     *  t_stdin := "[Enter] to confim " + t_j_s;
     *  t_ret := read_stdin(t_j_s);
     *  _ := invalidate(t_ret);
     *  t_ret := t_j_s;
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  _ := invalidate(t_len);
     *  return t_ret;
     *
     * <selection_idx>:
     *  t_len_s := serialize(t_len);
     *  t_stdin := "Please enter a number between 1 and " + t_len_s;
     *  t_stdin_new := t_stdin + ": ";
     *  t_out := read_stdin(t_stdin_new);
     *  _ := invalidate(t_len_s);
     *  _ := invalidate(t_stdin);
     *  _ := invalidate(t_stdin_new);
     *  t_num := parse_int(t_out);
     *  t_num_om := t_num == om;
     *  if t_num_om
     *   goto <selection_idx>
     *  else
     *   goto <check_num_idx>
     *
     * <check_num_idx>:
     *  t_num_pos := 0 < t_num;
     *  t_len_exc := t_len < t_num;
     *  t_len_nexc := !t_len_exc;
     *  t_len_check := t_num_pos && t_len_nexc;
     *  if t_len_check
     *   goto <ser_idx>
     *  else
     *   goto <inv_sel_idx>
     *
     * <ser_idx>:
     *  t_num_m := t_num - 1;
     *  t_j := t_p2[t_num_m];
     *  _ := invalidate(t_num_m);
     *  _ := invalidate(t_num);
     *  t_ret := serialize(t_j);
     *  goto <ret_idx>
     *
     * <inv_sel_idx>:
     *  _ := invalidate(t_num);
     *  goto <selection_idx>
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("ask")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p2_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p2 = tmp_var_new(&mut proc.borrow_mut());

    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_str = tmp_var_new(&mut proc.borrow_mut());
    let t_p2_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p2_list = tmp_var_new(&mut proc.borrow_mut());
    let t_check = tmp_var_new(&mut proc.borrow_mut());

    let t_p1_nl = tmp_var_new(&mut proc.borrow_mut());
    let t_len = tmp_var_new(&mut proc.borrow_mut());
    let t_len_zero = tmp_var_new(&mut proc.borrow_mut());

    let t_i = tmp_var_new(&mut proc.borrow_mut());
    let t_i_cond = tmp_var_new(&mut proc.borrow_mut());
    let t_i_new = tmp_var_new(&mut proc.borrow_mut());

    let t_i_s = tmp_var_new(&mut proc.borrow_mut());
    let t_j = tmp_var_new(&mut proc.borrow_mut());
    let t_j_s = tmp_var_new(&mut proc.borrow_mut());
    let t_s = tmp_var_new(&mut proc.borrow_mut());
    let t_s_new = tmp_var_new(&mut proc.borrow_mut());

    let t_len_one = tmp_var_new(&mut proc.borrow_mut());

    let t_stdin = tmp_var_new(&mut proc.borrow_mut());
    let t_stdin_new = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let t_len_s = tmp_var_new(&mut proc.borrow_mut());
    let t_out = tmp_var_new(&mut proc.borrow_mut());
    let t_num = tmp_var_new(&mut proc.borrow_mut());
    let t_num_m = tmp_var_new(&mut proc.borrow_mut());
    let t_num_om = tmp_var_new(&mut proc.borrow_mut());

    let t_num_pos = tmp_var_new(&mut proc.borrow_mut());
    let t_len_exc = tmp_var_new(&mut proc.borrow_mut());
    let t_len_nexc = tmp_var_new(&mut proc.borrow_mut());
    let t_len_check = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let list_check_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let iter_opts_init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let iter_opts_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let iter_opts_loop_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let check_one_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let one_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let selection_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let check_num_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ser_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let inv_sel_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p2_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_str),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::STRING)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p2)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2_list),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p2_type),
            op: IROp::Equal(IRValue::Type(IRType::LIST)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_str),
            op: IROp::And(IRValue::Variable(t_p2_list)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_check),
            success: list_check_idx,
            failure: fail_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(init_idx, list_check_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".into()),
                IRValue::String("invalid parameters".into()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    block_get(&mut proc.borrow_mut(), list_check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_nl),
            types: IRType::STRING,
            source: IRValue::Variable(t_p1),
            op: IROp::Plus(IRValue::String("\n".to_string())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::PrintStdout),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1_nl)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1_nl)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p2)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len_zero),
            types: IRType::BOOL,
            source: IRValue::Variable(t_len),
            op: IROp::Equal(IRValue::Number(0.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_len_zero),
            success: fail_idx,
            failure: iter_opts_init_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(list_check_idx, fail_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(list_check_idx, iter_opts_init_idx, ());

    block_get(&mut proc.borrow_mut(), iter_opts_init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i),
            types: IRType::NUMBER,
            source: IRValue::Number(0.into()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(iter_opts_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(iter_opts_init_idx, iter_opts_idx, ());

    block_get(&mut proc.borrow_mut(), iter_opts_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_cond),
            types: IRType::BOOL,
            source: IRValue::Variable(t_i),
            op: IROp::Less(IRValue::Variable(t_len)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_i_cond),
            success: iter_opts_loop_idx,
            failure: check_one_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(iter_opts_idx, iter_opts_loop_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(iter_opts_idx, check_one_idx, ());

    block_get(&mut proc.borrow_mut(), iter_opts_loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_j),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p2),
            op: IROp::AccessArray(IRValue::Variable(t_i)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_j_s),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Serialize),
            op: IROp::NativeCall(vec![IRValue::Variable(t_j)]),
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
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_s),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Serialize),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_s),
            types: IRType::STRING,
            source: IRValue::Variable(t_i_s),
            op: IROp::Plus(IRValue::String(") ".into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_s_new),
            types: IRType::STRING,
            source: IRValue::Variable(t_s),
            op: IROp::Plus(IRValue::Variable(t_j_s)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_s)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_s),
            types: IRType::STRING,
            source: IRValue::Variable(t_s_new),
            op: IROp::Plus(IRValue::String("\n".into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::PrintStdout),
            op: IROp::NativeCall(vec![IRValue::Variable(t_s)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i_s)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_j_s)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_s)]),
        }),
        IRStmt::Goto(iter_opts_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(iter_opts_loop_idx, iter_opts_idx, ());

    block_get(&mut proc.borrow_mut(), check_one_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len_one),
            types: IRType::BOOL,
            source: IRValue::Variable(t_len),
            op: IROp::Equal(IRValue::Number(1.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_len_one),
            success: one_idx,
            failure: selection_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(check_one_idx, one_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(check_one_idx, selection_idx, ());

    block_get(&mut proc.borrow_mut(), one_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_j),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p2),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_j_s),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Serialize),
            op: IROp::NativeCall(vec![IRValue::Variable(t_j)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_stdin),
            types: IRType::STRING,
            source: IRValue::String("[Enter] to confirm ".into()),
            op: IROp::Plus(IRValue::Variable(t_j_s)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::ReadLineStdin),
            op: IROp::NativeCall(vec![IRValue::Variable(t_stdin)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_ret)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::Variable(t_j_s),
            op: IROp::Assign,
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(one_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), selection_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len_s),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Serialize),
            op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_stdin),
            types: IRType::STRING,
            source: IRValue::String("Please enter a number between 1 and ".to_string()),
            op: IROp::Plus(IRValue::Variable(t_len_s)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_stdin_new),
            types: IRType::STRING,
            source: IRValue::Variable(t_stdin),
            op: IROp::Plus(IRValue::String(":".to_string())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::ReadLineStdin),
            op: IROp::NativeCall(vec![IRValue::Variable(t_stdin_new)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_len_s)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_stdin)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_stdin_new)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_num),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::ParseInt),
            op: IROp::NativeCall(vec![IRValue::Variable(t_out)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_num_om),
            types: IRType::BOOL,
            source: IRValue::Variable(t_num),
            op: IROp::Equal(IRValue::Undefined),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_num_om),
            success: selection_idx,
            failure: check_num_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(selection_idx, selection_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(selection_idx, check_num_idx, ());

    block_get(&mut proc.borrow_mut(), check_num_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_num_pos),
            types: IRType::BOOL,
            source: IRValue::Number(0.into()),
            op: IROp::Less(IRValue::Variable(t_num)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len_exc),
            types: IRType::BOOL,
            source: IRValue::Variable(t_len),
            op: IROp::Less(IRValue::Variable(t_num)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len_nexc),
            types: IRType::BOOL,
            source: IRValue::Variable(t_len_exc),
            op: IROp::Not,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_num_pos),
            op: IROp::And(IRValue::Variable(t_len_nexc)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_len_check),
            success: ser_idx,
            failure: inv_sel_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(check_num_idx, ser_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(check_num_idx, inv_sel_idx, ());

    block_get(&mut proc.borrow_mut(), ser_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_num_m),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_num),
            op: IROp::Minus(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_j),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p2),
            op: IROp::AccessArray(IRValue::Variable(t_num_m)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_num_m)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_num)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Serialize),
            op: IROp::NativeCall(vec![IRValue::Variable(t_j)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(ser_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), inv_sel_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_num)]),
        }),
        IRStmt::Goto(selection_idx),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(inv_sel_idx, selection_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

pub fn signum_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_int := t_p1_type == TYPE_NUMBER;
     *  t_p1_float := t_p1_type == TYPE_DOUBLE;
     *  t_p1_num := t_p1_int || t_p1_float;
     *  if t_p1_num
     *   goto <check_lz_idx>
     *  else
     *   goto <fail_idx>
     *
     * <check_lz_idx>:
     *  t_p1_lz := t_p1 < 0;
     *  if t_p1_lz
     *   goto <ret_neg_idx>
     *  else
     *   goto <check_z_idx>
     *
     * <ret_neg_idx>:
     *  t_ret := -1;
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  return t_ret;
     *
     * <check_z_idx>:
     *  t_p1_z := t_p1 == 0;
     *  if t_p1_z
     *   goto <ret_z_idx>
     *  else
     *   goto <ret_p_idx>
     *
     * <ret_z_idx>:
     *  t_ret := 0;
     *  goto <ret_idx>
     *
     * <ret_p_idx>:
     *  t_ret := 1;
     *  goto <ret_idx>
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "signum only supports numbers");
     *  unreachable;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("signum")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_int = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_float = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_num = tmp_var_new(&mut proc.borrow_mut());

    let t_p1_lz = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_z = tmp_var_new(&mut proc.borrow_mut());

    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let check_lz_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_neg_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let check_z_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_z_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_p_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_int),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_float),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::DOUBLE)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_num),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_int),
            op: IROp::Or(IRValue::Variable(t_p1_float)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_num),
            success: check_lz_idx,
            failure: fail_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(init_idx, check_lz_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), check_lz_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_lz),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1),
            op: IROp::Less(IRValue::Number(0.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_lz),
            success: ret_neg_idx,
            failure: check_z_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(check_lz_idx, ret_neg_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(check_lz_idx, check_z_idx, ());

    block_get(&mut proc.borrow_mut(), ret_neg_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::NUMBER,
            source: IRValue::Number((-1).into()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(ret_neg_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), check_z_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_z),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1),
            op: IROp::Equal(IRValue::Number(0.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_z),
            success: ret_z_idx,
            failure: ret_p_idx,
        }),
    ]);

    proc.borrow_mut()
        .blocks
        .add_edge(check_z_idx, ret_z_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(check_z_idx, ret_p_idx, ());

    block_get(&mut proc.borrow_mut(), ret_z_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::NUMBER,
            source: IRValue::Number(0.into()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(ret_z_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), ret_p_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::NUMBER,
            source: IRValue::Number(1.into()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(ret_idx),
    ]);

    proc.borrow_mut().blocks.add_edge(ret_p_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx)
        .extend(vec![IRStmt::Return(IRValue::Variable(t_ret))]);

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".into()),
                IRValue::String("signum only supports numbers".into()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

pub fn int_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_s := serialize(t_p1);
     *  t_ret := parse_int(t_s);
     *  _ := invalidate(t_s);
     *  return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("int")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_s = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_s),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Serialize),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::ParseInt),
            op: IROp::NativeCall(vec![IRValue::Variable(t_s)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_s)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

pub fn double_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_s := serialize(t_p1);
     *  t_ret := parse_float(t_s);
     *  _ := invalidate(t_s);
     *  return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("double")));

    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_s = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_s),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Serialize),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::DOUBLE,
            source: IRValue::BuiltinProc(BuiltinProc::ParseFloat),
            op: IROp::NativeCall(vec![IRValue::Variable(t_s)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_s)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    proc
}

fn logo_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_cnt := 0;
     *  goto <loop_idx>
     *
     * <loop_idx>:
     *  t_out := read_line_stdin("Insert USD-Coin: ");
     *  t_out_penny := t_out == "Penny";
     *  if t_out_penny
     *   goto <print_penny_idx>
     *  else
     *   goto <check_nickel_idx>
     *
     * <print_penny_idx>:
     *  _ := print_stdout("  ...cheap bastard...\n");
     *  t_ret := 27926;
     *  goto <print_small_idx>
     *
     * <print_small_idx>:
     *  t_str := include_str("src/logo_small.txt");
     *  t_iter := iter_new(t_str);
     *  goto <print_iter_idx>
     *
     * <print_iter_idx>:
     *  t_i := om;
     *  t_i_addr := &t_i;
     *  t_cond := iter_next(t_iter, t_i_addr);
     *  if t_cond
     *   goto <print_idx>
     *  else
     *   goto <inv_str_idx>
     *
     * <print_idx>:
     *  _ := print_stdout(t_i);
     *  _ := sleep(10);
     *  goto <print_iter_idx>
     *
     * <inv_str_idx>:
     *  _ := print_stdout("Please come again.\n");
     *  _ := invalidate(t_str);
     *  goto <ret_idx>
     *
     * <ret_idx>:
     *  _ := invalidate(t_cnt);
     *  return t_ret;
     *
     * <check_nickel_idx>:
     *  t_out_nickel := t_out == "Nickel";
     *  if t_out_nickel
     *   goto <nickel_idx>
     *  else
     *   goto <check_dime_idx>
     *
     * <nickel_idx>:
     *  t_ret := 13183;
     *  _ := print_stdout("  ...well... never mind...\n");
     *  goto <print_small_idx>
     *
     * <check_dime_idx>:
     *  t_out_dime := t_out == "Dime";
     *  if t_out_dime
     *   goto <dime_idx>
     *  else
     *   goto <check_quarter_idx>
     *
     * <dime_idx>:
     *  t_ret := 28903;
     *  _ := print_stdout("  ...you can do one better...\n");
     *  goto <print_big_idx>
     *
     * <print_big_idx>:
     *  t_str := include_str("src/logo_big.txt");
     *  t_iter := iter_new(t_str);
     *  goto <print_iter_idx>
     *
     * <check_quarter_idx>:
     *  t_out_quarter := t_out == "Quarter";
     *  if t_out_quarter
     *   goto <quarter_idx>
     *  else
     *   goto <cnt_check_idx>
     *
     * <quarter_idx>:
     *  t_ret := 20898;
     *  _ := print_stdout("Thank you!\n");
     *  goto <print_big_idx>
     *
     * <cnt_check_idx>:
     *  t_cnt_check := t_cnt < 3;
     *  if t_cnt_check
     *   goto <inc_idx>
     *  else
     *   goto <fail_idx>
     *
     * <inc_idx>:
     *  t_cnt_new := t_cnt + 1;
     *  _ := invalidate(t_cnt);
     *  t_cnt := t_cnt_new;
     *  goto <loop_idx>
     *
     * <fail_idx>:
     *  _ := print_stdout("Too bad... here's a `penny' for your thoughts.");
     *  t_ret := 0;
     *  goto <ret_idx>
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("logo")));

    let t_cnt = tmp_var_new(&mut proc.borrow_mut());
    let t_cnt_new = tmp_var_new(&mut proc.borrow_mut());
    let t_cnt_check = tmp_var_new(&mut proc.borrow_mut());

    let t_out = tmp_var_new(&mut proc.borrow_mut());
    let t_out_penny = tmp_var_new(&mut proc.borrow_mut());
    let t_out_nickel = tmp_var_new(&mut proc.borrow_mut());
    let t_out_dime = tmp_var_new(&mut proc.borrow_mut());
    let t_out_quarter = tmp_var_new(&mut proc.borrow_mut());

    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let t_str = tmp_var_new(&mut proc.borrow_mut());
    let t_iter = tmp_var_new(&mut proc.borrow_mut());
    let t_i = tmp_var_new(&mut proc.borrow_mut());
    let t_i_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_cond = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let loop_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let print_penny_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let print_small_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let print_big_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let print_iter_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let print_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let inv_str_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    let check_nickel_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let nickel_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let check_dime_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let dime_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let check_quarter_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let quarter_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let cnt_check_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let inc_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cnt),
            types: IRType::NUMBER,
            source: IRValue::Number(0.into()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(loop_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(init_idx, loop_idx, ());

    block_get(&mut proc.borrow_mut(), loop_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::ReadLineStdin),
            op: IROp::NativeCall(vec![IRValue::String("Insert USD-Coin: ".into())]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out_penny),
            types: IRType::BOOL,
            source: IRValue::Variable(t_out),
            op: IROp::Equal(IRValue::String("Penny".into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_out_penny),
            success: print_penny_idx,
            failure: check_nickel_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(loop_idx, print_penny_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(loop_idx, check_nickel_idx, ());

    block_get(&mut proc.borrow_mut(), print_penny_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::PrintStdout),
            op: IROp::NativeCall(vec![IRValue::String("  ...cheap bastard...\n".into())]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::NUMBER,
            source: IRValue::Number(27926.into()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(print_small_idx),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(print_penny_idx, print_small_idx, ());

    block_get(&mut proc.borrow_mut(), check_nickel_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out_nickel),
            types: IRType::BOOL,
            source: IRValue::Variable(t_out),
            op: IROp::Equal(IRValue::String("Nickel".into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_out_nickel),
            success: nickel_idx,
            failure: check_dime_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(check_nickel_idx, nickel_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(check_nickel_idx, check_dime_idx, ());

    block_get(&mut proc.borrow_mut(), nickel_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::NUMBER,
            source: IRValue::Number(13183.into()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::PrintStdout),
            op: IROp::NativeCall(vec![IRValue::String("  ...well... never mind...\n".into())]),
        }),
        IRStmt::Goto(print_small_idx),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(nickel_idx, print_small_idx, ());

    block_get(&mut proc.borrow_mut(), check_dime_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out_dime),
            types: IRType::BOOL,
            source: IRValue::Variable(t_out),
            op: IROp::Equal(IRValue::String("Dime".into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_out_dime),
            success: dime_idx,
            failure: check_quarter_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(check_dime_idx, dime_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(check_dime_idx, check_quarter_idx, ());

    block_get(&mut proc.borrow_mut(), dime_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::NUMBER,
            source: IRValue::Number(28903.into()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::PrintStdout),
            op: IROp::NativeCall(vec![IRValue::String(
                "  ...you can do one better...\n".into(),
            )]),
        }),
        IRStmt::Goto(print_big_idx),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(dime_idx, print_big_idx, ());

    block_get(&mut proc.borrow_mut(), check_quarter_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out_quarter),
            types: IRType::BOOL,
            source: IRValue::Variable(t_out),
            op: IROp::Equal(IRValue::String("Quarter".into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_out_quarter),
            success: quarter_idx,
            failure: cnt_check_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(check_quarter_idx, quarter_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(check_quarter_idx, cnt_check_idx, ());

    block_get(&mut proc.borrow_mut(), quarter_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::NUMBER,
            source: IRValue::Number(20898.into()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::PrintStdout),
            op: IROp::NativeCall(vec![IRValue::String("Thank you!\n".into())]),
        }),
        IRStmt::Goto(print_big_idx),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(quarter_idx, print_big_idx, ());

    block_get(&mut proc.borrow_mut(), print_small_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_str),
            types: IRType::STRING,
            source: IRValue::String(include_str!("../logo_small.txt").to_string()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_iter),
            types: IRType::ITERATOR,
            source: IRValue::BuiltinProc(BuiltinProc::IterNew),
            op: IROp::NativeCall(vec![IRValue::Variable(t_str)]),
        }),
        IRStmt::Goto(print_iter_idx),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(print_small_idx, print_iter_idx, ());

    block_get(&mut proc.borrow_mut(), print_big_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_str),
            types: IRType::STRING,
            source: IRValue::String(include_str!("../logo_big.txt").to_string()),
            op: IROp::Assign,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_iter),
            types: IRType::ITERATOR,
            source: IRValue::BuiltinProc(BuiltinProc::IterNew),
            op: IROp::NativeCall(vec![IRValue::Variable(t_str)]),
        }),
        IRStmt::Goto(print_iter_idx),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(print_big_idx, print_iter_idx, ());

    block_get(&mut proc.borrow_mut(), print_iter_idx).extend(vec![
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
            success: print_idx,
            failure: inv_str_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(print_iter_idx, print_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(print_iter_idx, inv_str_idx, ());

    block_get(&mut proc.borrow_mut(), print_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::PrintStdout),
            op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Sleep),
            op: IROp::NativeCall(vec![IRValue::Number(10.into())]),
        }),
        IRStmt::Goto(print_iter_idx),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(print_idx, print_iter_idx, ());

    block_get(&mut proc.borrow_mut(), inv_str_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::PrintStdout),
            op: IROp::NativeCall(vec![IRValue::String("Please come again.\n".into())]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_str)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(inv_str_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), cnt_check_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cnt_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_cnt),
            op: IROp::Less(IRValue::Number(2.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_cnt_check),
            success: inc_idx,
            failure: fail_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(cnt_check_idx, inc_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(cnt_check_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), inc_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cnt_new),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_cnt),
            op: IROp::Plus(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_cnt)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_cnt),
            types: IRType::NUMBER,
            source: IRValue::Variable(t_cnt_new),
            op: IROp::Assign,
        }),
        IRStmt::Goto(loop_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(inc_idx, loop_idx, ());

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::PrintStdout),
            op: IROp::NativeCall(vec![IRValue::String(
                "Too bad... here's a `penny' for your thoughts.\n".into(),
            )]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::NUMBER,
            source: IRValue::Number(0.into()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(fail_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), ret_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_cnt)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn rnd_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* <init_idx>:
     *  t_p_len := amount(params);
     *  t_p1_addr := params[0];
     *  t_p1 := *t_p1_addr;
     *  t_p_len_two := t_p_len == 2;
     *  if t_p_len_two
     *   goto <rnd_quot_idx>
     *  else
     *   goto <rnd_idx>
     *
     * <rnd_quot_idx>:
     *  t_p2_addr := params[1];
     *  t_p2 := *t_p2_addr;
     *  t_p1_type := type_of(t_p1);
     *  t_p1_int := t_p1_type == TYPE_NUMBER;
     *  t_p1_float := t_p1_type == TYPE_DOUBLE;
     *  t_p1_num := t_p1_int || t_p1_float;
     *  t_p2_type := type_of(t_p2);
     *  t_p2_int := t_p2_type == TYPE_NUMBER;
     *  t_p2_check := t_p1_num && t_p2_int;
     *  if t_p2_check
     *   goto <rnd_quot_geq_idx>
     *  else
     *   goto <fail_idx>
     *
     * <rnd_quot_geq_idx>:
     *  t_p2_check := 1 < t_p2;
     *  if t_p2_check
     *   goto <rnd_quot_calc_idx>
     *  else
     *   goto <fail_idx>
     *
     * <rnd_quot_calc_idx>:
     *  t_rnd := rnd_float();
     *  t_state := t_p1 * t_rnd;
     *  t_rnd := rnd_float();
     *  t_rhs := t_state / t_rnd;
     *  t_ret := t_p2 * t_rhs;
     *  goto <ret_idx>.
     *
     * <ret_idx>:
     *  return t_ret;
     *
     * <fail_idx>:
     *  _ := exception_throw("predefined procedure", "unsupported input for rnd");
     *  unreachable;
     *
     * <rnd_idx>:
     *  t_p1_type := type_of(t_p1);
     *  t_p1_int := t_p1_type == TYPE_NUMBER;
     *  t_p1_float := t_p1_type == TYPE_DOUBLE;
     *  t_p1_num := t_p1_int || t_p1_float;
     *  if t_p1_num
     *   goto <rnd_calc_idx>
     *  else
     *   goto <rnd_check_acc_idx>
     *
     * <rnd_calc_idx>:
     *  t_rnd := rnd_float();
     *  t_ret := t_p1 * t_rnd;
     *  goto <ret_idx>
     *
     * <rnd_check_acc_idx>:
     *  t_p1_set := t_p1_type == TYPE_SET;
     *  t_p1_list := t_p1_type == TYPE_LIST;
     *  t_p1_str := t_p1_type == TYPE_STRING;
     *  t_p1_acc := t_p1_list || t_p1_str;
     *  t_p1_acc := t_p1_acc || t_p1_set;
     *  if t_p1_acc
     *   goto <rnd_acc_idx>
     *  else
     *   goto <fail_idx>
     *
     * <rnd_acc_idx>:
     *  t_rnd := rnd_float();
     *  t_len := amount(t_p1);
     *  t_len_f := t_rnd * t_len;
     *  _ := invalidate(t_len);
     *  t_idx := floor(t_len_f);
     *  t_ret := t_p1[t_idx];
     *  t_ret := copy(t_ret);
     *  goto <ret_idx>
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("rnd")));

    let t_p_len = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p1 = tmp_var_new(&mut proc.borrow_mut());
    let t_p_len_two = tmp_var_new(&mut proc.borrow_mut());
    let t_p2_addr = tmp_var_new(&mut proc.borrow_mut());
    let t_p2 = tmp_var_new(&mut proc.borrow_mut());

    let t_p1_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_int = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_float = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_num = tmp_var_new(&mut proc.borrow_mut());
    let t_p2_type = tmp_var_new(&mut proc.borrow_mut());
    let t_p2_int = tmp_var_new(&mut proc.borrow_mut());
    let t_p2_check = tmp_var_new(&mut proc.borrow_mut());

    let t_rnd = tmp_var_new(&mut proc.borrow_mut());
    let t_state = tmp_var_new(&mut proc.borrow_mut());
    let t_rhs = tmp_var_new(&mut proc.borrow_mut());
    let t_ret = tmp_var_new(&mut proc.borrow_mut());

    let t_p1_set = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_list = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_str = tmp_var_new(&mut proc.borrow_mut());
    let t_p1_acc = tmp_var_new(&mut proc.borrow_mut());
    let t_len = tmp_var_new(&mut proc.borrow_mut());
    let t_len_f = tmp_var_new(&mut proc.borrow_mut());
    let t_idx = tmp_var_new(&mut proc.borrow_mut());

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let rnd_quot_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let rnd_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let rnd_quot_geq_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let rnd_quot_calc_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let rnd_calc_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let rnd_check_acc_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let rnd_acc_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let ret_idx = proc.borrow_mut().blocks.add_node(Vec::new());
    let fail_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::BuiltinVar(BuiltinVar::Params)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRType::UNDEFINED,
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p_len_two),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p_len),
            op: IROp::Equal(IRValue::Number(2.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p_len_two),
            success: rnd_quot_idx,
            failure: rnd_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(init_idx, rnd_quot_idx, ());
    proc.borrow_mut().blocks.add_edge(init_idx, rnd_idx, ());

    block_get(&mut proc.borrow_mut(), rnd_quot_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2),
            types: IRType::UNDEFINED,
            source: IRValue::Variable(t_p2_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_int),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_float),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::DOUBLE)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_num),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_int),
            op: IROp::Or(IRValue::Variable(t_p1_float)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2_type),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p2)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2_int),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p2_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2_check),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_num),
            op: IROp::And(IRValue::Variable(t_p2_int)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p2_check),
            success: rnd_quot_geq_idx,
            failure: fail_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(rnd_quot_idx, rnd_quot_geq_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(rnd_quot_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), rnd_quot_geq_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p2_check),
            types: IRType::BOOL,
            source: IRValue::Number(1.into()),
            op: IROp::Less(IRValue::Variable(t_p2)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p2_check),
            success: rnd_quot_calc_idx,
            failure: fail_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(rnd_quot_geq_idx, rnd_quot_calc_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(rnd_quot_geq_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), rnd_quot_calc_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_rnd),
            types: IRType::DOUBLE,
            source: IRValue::BuiltinProc(BuiltinProc::RndFloat),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_state),
            types: IRType::DOUBLE,
            source: IRValue::Variable(t_p1),
            op: IROp::Mult(IRValue::Variable(t_rnd)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_rnd),
            types: IRType::DOUBLE,
            source: IRValue::BuiltinProc(BuiltinProc::RndFloat),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_rhs),
            types: IRType::DOUBLE,
            source: IRValue::Variable(t_state),
            op: IROp::Divide(IRValue::Variable(t_rnd)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::DOUBLE,
            source: IRValue::Variable(t_p2),
            op: IROp::Mult(IRValue::Variable(t_rhs)),
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(rnd_quot_calc_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), rnd_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_type),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_int),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::NUMBER)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_float),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::DOUBLE)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_num),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_int),
            op: IROp::Or(IRValue::Variable(t_p1_float)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_num),
            success: rnd_calc_idx,
            failure: rnd_check_acc_idx,
        }),
    ]);
    proc.borrow_mut().blocks.add_edge(rnd_idx, rnd_calc_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(rnd_idx, rnd_check_acc_idx, ());

    block_get(&mut proc.borrow_mut(), rnd_calc_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_rnd),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::RndFloat),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::DOUBLE,
            source: IRValue::Variable(t_p1),
            op: IROp::Mult(IRValue::Variable(t_rnd)),
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(rnd_calc_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), rnd_check_acc_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_set),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::SET)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_list),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::LIST)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_str),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_type),
            op: IROp::Equal(IRValue::Type(IRType::STRING)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_acc),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_list),
            op: IROp::Or(IRValue::Variable(t_p1_str)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_acc),
            types: IRType::BOOL,
            source: IRValue::Variable(t_p1_acc),
            op: IROp::Or(IRValue::Variable(t_p1_set)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_p1_acc),
            success: rnd_acc_idx,
            failure: fail_idx,
        }),
    ]);
    proc.borrow_mut()
        .blocks
        .add_edge(rnd_check_acc_idx, rnd_acc_idx, ());
    proc.borrow_mut()
        .blocks
        .add_edge(rnd_check_acc_idx, fail_idx, ());

    block_get(&mut proc.borrow_mut(), rnd_acc_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_rnd),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::RndFloat),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Amount),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_len_f),
            types: IRType::DOUBLE,
            source: IRValue::Variable(t_rnd),
            op: IROp::Mult(IRValue::Variable(t_len)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_len)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_idx),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::Floor),
            op: IROp::NativeCall(vec![IRValue::Variable(t_len_f)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::UNDEFINED,
            source: IRValue::Variable(t_p1),
            op: IROp::AccessArray(IRValue::Variable(t_idx)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_idx)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_ret)]),
        }),
        IRStmt::Goto(ret_idx),
    ]);
    proc.borrow_mut().blocks.add_edge(rnd_acc_idx, ret_idx, ());

    block_get(&mut proc.borrow_mut(), fail_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ExceptionThrow),
            op: IROp::NativeCall(vec![
                IRValue::String("predefined procedure".into()),
                IRValue::String("unsupported input for rnd".into()),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    block_get(&mut proc.borrow_mut(), ret_idx)
        .extend(vec![IRStmt::Return(IRValue::Variable(t_ret))]);

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = ret_idx;

    proc
}

fn clear_cache_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* t_p1_addr := params[0];
     * t_p1 := *t_p1_addr;
     * _ := cache_clear(t_p1);
     * return om;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("clearCache")));
    let mut proc_ref = proc.borrow_mut();

    let block_idx = proc_ref.blocks.add_node(Vec::new());
    proc_ref.start_block = block_idx;
    proc_ref.end_block = block_idx;

    let t_p1_addr = tmp_var_new(&mut proc_ref);
    let t_p1 = tmp_var_new(&mut proc_ref);

    block_get(&mut proc_ref, block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_p1),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_p1_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::CacheClear),
            op: IROp::NativeCall(vec![IRValue::Variable(t_p1)]),
        }),
        IRStmt::Return(IRValue::Undefined),
    ]);

    drop(proc_ref);
    proc
}

fn read_file_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* t_file_addr := params[0];
     * t_file := *t_file_addr;
     * t_fd := open_at(".", t_file, 0x01);
     * t_out := read_all_list(t_fd);
     * _ := invalidate(t_fd);
     * return t_out;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("readFile")));
    let mut proc_ref = proc.borrow_mut();

    let block_idx = proc_ref.blocks.add_node(Vec::new());
    proc_ref.start_block = block_idx;
    proc_ref.end_block = block_idx;

    let t_file_addr = tmp_var_new(&mut proc_ref);
    let t_file = tmp_var_new(&mut proc_ref);
    let t_fd = tmp_var_new(&mut proc_ref);
    let t_ret = tmp_var_new(&mut proc_ref);

    block_get(&mut proc_ref, block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_file_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_file),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_file_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_fd),
            types: IRType::FILE,
            source: IRValue::BuiltinProc(BuiltinProc::OpenAt),
            op: IROp::NativeCall(vec![
                IRValue::String(".".to_string()),
                IRValue::Variable(t_file),
                IRValue::Number(0x01.into()),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::ReadAllList),
            op: IROp::NativeCall(vec![IRValue::Variable(t_fd)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_fd)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    drop(proc_ref);
    proc
}

fn write_file_stub_new() -> Rc<RefCell<IRProcedure>> {
    /*  t_file_addr := params[0];
     *  t_file := *t_file_addr;
     *  t_content_addr := params[1];
     *  t_content := *t_content_addr;
     *  t_content_type := type_of(t_content);
     *  t_content_str := t_content_type == TYPE_STRING;
     *  if t_content_str
     *   goto <copy_idx>
     *  else
     *   goto <init_iter_idx>
     *
     * <copy_idx>:
     *  t_final := copy(t_content);
     *  goto <write_idx>
     *
     * <init_iter_idx>
     *  t_iter := iter_new(t_content);
     *  t_final := "";
     *  goto <iter_idx>
     *
     * <iter_idx>:
     *  t_i_addr := &t_i;
     *  t_concat := iter_next(t_iter, t_i_addr);
     *  if t_concat
     *   goto <concat_idx>
     *  else
     *   goto <write_idx>
     *
     * <concat_idx>:
     *  t_final_new := t_final + t_i;
     *  _ := invalidate(t_final);
     *  t_final := t_final_new + "\n";
     *  _ := invalidate(t_final_new);
     *  goto <iter_idx>
     *
     * <write_idx>:
     *  t_fd := open_at(".", t_file, 0x0a);
     *  _ := write(t_fd, t_final);
     *  _ := invalidate(t_fd);
     *  _ := invalidate(t_final);
     *  return om;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("writeFile")));
    let mut proc_ref = proc.borrow_mut();

    let t_file_addr = tmp_var_new(&mut proc_ref);
    let t_file = tmp_var_new(&mut proc_ref);
    let t_content_addr = tmp_var_new(&mut proc_ref);
    let t_content = tmp_var_new(&mut proc_ref);
    let t_content_type = tmp_var_new(&mut proc_ref);
    let t_content_str = tmp_var_new(&mut proc_ref);
    let t_iter = tmp_var_new(&mut proc_ref);
    let t_final = tmp_var_new(&mut proc_ref);
    let t_i = tmp_var_new(&mut proc_ref);
    let t_i_addr = tmp_var_new(&mut proc_ref);
    let t_concat = tmp_var_new(&mut proc_ref);
    let t_final_new = tmp_var_new(&mut proc_ref);
    let t_fd = tmp_var_new(&mut proc_ref);

    let init_idx = proc_ref.blocks.add_node(Vec::new());
    let copy_idx = proc_ref.blocks.add_node(Vec::new());
    let init_iter_idx = proc_ref.blocks.add_node(Vec::new());
    let iter_idx = proc_ref.blocks.add_node(Vec::new());
    let concat_idx = proc_ref.blocks.add_node(Vec::new());
    let write_idx = proc_ref.blocks.add_node(Vec::new());

    proc_ref.start_block = init_idx;
    proc_ref.end_block = write_idx;

    block_get(&mut proc_ref, init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_file_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_file),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_file_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_content_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(1.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_content),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_content_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_content_type),
            types: IRType::NUMBER,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_content)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_content_str),
            types: IRType::BOOL,
            source: IRValue::Variable(t_content_type),
            op: IROp::Equal(IRValue::Type(IRType::STRING)),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_content_str),
            success: copy_idx,
            failure: init_iter_idx,
        }),
    ]);
    proc_ref.blocks.add_edge(init_idx, copy_idx, ());
    proc_ref.blocks.add_edge(init_idx, init_iter_idx, ());

    block_get(&mut proc_ref, copy_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_final),
            types: IRType::STRING,
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_content)]),
        }),
        IRStmt::Goto(write_idx),
    ]);
    proc_ref.blocks.add_edge(copy_idx, write_idx, ());

    block_get(&mut proc_ref, init_iter_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_iter),
            types: IRType::ITERATOR,
            source: IRValue::BuiltinProc(BuiltinProc::IterNew),
            op: IROp::NativeCall(vec![IRValue::Variable(t_content)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_final),
            types: IRType::STRING,
            source: IRValue::String(String::new()),
            op: IROp::Assign,
        }),
        IRStmt::Goto(iter_idx),
    ]);
    proc_ref.blocks.add_edge(init_iter_idx, iter_idx, ());

    block_get(&mut proc_ref, iter_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_i_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_i),
            op: IROp::PtrAddress,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_concat),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::IterNext),
            op: IROp::NativeCall(vec![IRValue::Variable(t_iter), IRValue::Variable(t_i_addr)]),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_concat),
            success: concat_idx,
            failure: write_idx,
        }),
    ]);
    proc_ref.blocks.add_edge(iter_idx, concat_idx, ());
    proc_ref.blocks.add_edge(iter_idx, write_idx, ());

    block_get(&mut proc_ref, concat_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_final_new),
            types: IRType::STRING,
            source: IRValue::Variable(t_final),
            op: IROp::Plus(IRValue::Variable(t_i)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_final)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_final),
            types: IRType::STRING,
            source: IRValue::Variable(t_final_new),
            op: IROp::Plus(IRValue::String("\n".to_string())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_final_new)]),
        }),
        IRStmt::Goto(iter_idx),
    ]);
    proc_ref.blocks.add_edge(concat_idx, iter_idx, ());

    block_get(&mut proc_ref, write_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_fd),
            types: IRType::FILE,
            source: IRValue::BuiltinProc(BuiltinProc::OpenAt),
            op: IROp::NativeCall(vec![
                IRValue::String(".".to_string()),
                IRValue::Variable(t_file),
                IRValue::Number(0x0a.into()),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Write),
            op: IROp::NativeCall(vec![IRValue::Variable(t_fd), IRValue::Variable(t_final)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_fd)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_final)]),
        }),
        IRStmt::Return(IRValue::Undefined),
    ]);

    drop(proc_ref);
    proc
}

fn delete_file_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* t_file_addr := params[0];
     * t_file := *t_file_addr;
     * _ := delete(t_file);
     * return om;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("deleteFile")));
    let mut proc_ref = proc.borrow_mut();

    let block_idx = proc_ref.blocks.add_node(Vec::new());
    proc_ref.start_block = block_idx;
    proc_ref.end_block = block_idx;

    let t_file_addr = tmp_var_new(&mut proc_ref);
    let t_file = tmp_var_new(&mut proc_ref);

    block_get(&mut proc_ref, block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_file_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_file),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_file_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Delete),
            op: IROp::NativeCall(vec![IRValue::Variable(t_file)]),
        }),
        IRStmt::Return(IRValue::Undefined),
    ]);

    drop(proc_ref);
    proc
}

fn is_prime_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* t_num_addr := params[0];
     * t_num := *t_num_addr;
     * t_ret := is_prime(t_num);
     * return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("isPrime")));
    let mut proc_ref = proc.borrow_mut();

    let t_num_addr = tmp_var_new(&mut proc_ref);
    let t_num = tmp_var_new(&mut proc_ref);
    let t_ret = tmp_var_new(&mut proc_ref);

    let block_idx = proc_ref.blocks.add_node(Vec::new());
    proc_ref.start_block = block_idx;
    proc_ref.end_block = block_idx;

    block_get(&mut proc_ref, block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_num_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_num),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_num_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::IsPrime),
            op: IROp::NativeCall(vec![IRValue::Variable(t_num)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    drop(proc_ref);
    proc
}

fn is_probable_prime_stub_new() -> Rc<RefCell<IRProcedure>> {
    /* t_num_addr := params[0];
     * t_num := *t_num_addr;
     * t_ret := is_probable_prime(t_num);
     * return t_ret;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("isProbablePrime")));
    let mut proc_ref = proc.borrow_mut();

    let t_num_addr = tmp_var_new(&mut proc_ref);
    let t_num = tmp_var_new(&mut proc_ref);
    let t_ret = tmp_var_new(&mut proc_ref);

    let block_idx = proc_ref.blocks.add_node(Vec::new());
    proc_ref.start_block = block_idx;
    proc_ref.end_block = block_idx;

    block_get(&mut proc_ref, block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_num_addr),
            types: IRType::PTR,
            source: IRValue::BuiltinVar(BuiltinVar::Params),
            op: IROp::AccessArray(IRValue::Number(0.into())),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_num),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_num_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_ret),
            types: IRType::BOOL,
            source: IRValue::BuiltinProc(BuiltinProc::IsProbablePrime),
            op: IROp::NativeCall(vec![IRValue::Variable(t_num)]),
        }),
        IRStmt::Return(IRValue::Variable(t_ret)),
    ]);

    drop(proc_ref);
    proc
}

fn reset_random_stub_new() -> Rc<RefCell<IRProcedure>> {
    /*
     * <init_idx>:
     *  return om;
     */
    let proc = Rc::new(RefCell::new(IRProcedure::from_tag("resetRandom")));

    let init_idx = proc.borrow_mut().blocks.add_node(Vec::new());

    proc.borrow_mut().start_block = init_idx;
    proc.borrow_mut().end_block = init_idx;

    block_get(&mut proc.borrow_mut(), init_idx).extend(vec![IRStmt::Return(IRValue::Undefined)]);

    proc
}

pub fn stubs_init() -> Vec<InterpStackEntry> {
    vec![
        InterpStackEntry::Variable(InterpStackVar {
            var: "abort".to_string(),
            val: Box::new(InterpVal::Procedure(abort_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "abs".to_string(),
            val: Box::new(InterpVal::Procedure(abs_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "arb".to_string(),
            val: Box::new(InterpVal::Procedure(first_stub_new("arb"))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "args".to_string(),
            val: Box::new(InterpVal::Procedure(args_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "ask".to_string(),
            val: Box::new(InterpVal::Procedure(ask_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "assert".to_string(),
            val: Box::new(InterpVal::Procedure(assert_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "ceil".to_string(),
            val: Box::new(InterpVal::Procedure(float_op_stub_new(FloatOp::Ceil))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "char".to_string(),
            val: Box::new(InterpVal::Procedure(char_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "clearCache".to_string(),
            val: Box::new(InterpVal::Procedure(clear_cache_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "compare".to_string(),
            val: Box::new(InterpVal::Procedure(compare_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "cos".to_string(),
            val: Box::new(InterpVal::Procedure(num_op_stub_new(NumOp::Cos))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "cosh".to_string(),
            val: Box::new(InterpVal::Procedure(num_op_stub_new(NumOp::CosH))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "deleteFile".to_string(),
            val: Box::new(InterpVal::Procedure(delete_file_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "domain".to_string(),
            val: Box::new(InterpVal::Procedure(domain_range_stub_new(false))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "double".to_string(),
            val: Box::new(InterpVal::Procedure(double_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "eval".to_string(),
            val: Box::new(InterpVal::Procedure(eval_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "evalTerm".to_string(),
            val: Box::new(InterpVal::Procedure(eval_term_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "execute".to_string(),
            val: Box::new(InterpVal::Procedure(execute_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "exp".to_string(),
            val: Box::new(InterpVal::Procedure(ln_exp_stub_new(true))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "fct".to_string(),
            val: Box::new(InterpVal::Procedure(fct_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "first".to_string(),
            val: Box::new(InterpVal::Procedure(first_stub_new("first"))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "floor".to_string(),
            val: Box::new(InterpVal::Procedure(float_op_stub_new(FloatOp::Floor))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "from".to_string(),
            val: Box::new(InterpVal::Procedure(from_stub_new("from"))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "fromB".to_string(),
            val: Box::new(InterpVal::Procedure(fromb_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "fromE".to_string(),
            val: Box::new(InterpVal::Procedure(from_stub_new("fromE"))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "get".to_string(),
            val: Box::new(InterpVal::Procedure(get_stub_new("get"))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "getOsID".to_string(),
            val: Box::new(InterpVal::Procedure(get_os_id_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "int".to_string(),
            val: Box::new(InterpVal::Procedure(int_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "isBoolean".to_string(),
            val: Box::new(InterpVal::Procedure(is_type_stub_new(
                "isString",
                IRType::BOOL,
            ))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "isDouble".to_string(),
            val: Box::new(InterpVal::Procedure(is_type_stub_new(
                "isDouble",
                IRType::DOUBLE,
            ))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "isInfinite".to_string(),
            val: Box::new(InterpVal::Procedure(is_infinite_stub_new())),
        }),
        // TODO isMap isError
        InterpStackEntry::Variable(InterpStackVar {
            var: "isInteger".to_string(),
            val: Box::new(InterpVal::Procedure(is_type_stub_new(
                "isInteger",
                IRType::NUMBER,
            ))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "isNumber".to_string(),
            val: Box::new(InterpVal::Procedure(is_number_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "isList".to_string(),
            val: Box::new(InterpVal::Procedure(is_type_stub_new(
                "isList",
                IRType::LIST,
            ))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "isObject".to_string(),
            val: Box::new(InterpVal::Procedure(is_type_stub_new(
                "isString",
                IRType::OBJECT,
            ))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "isPrime".to_string(),
            val: Box::new(InterpVal::Procedure(is_prime_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "isProbablePrime".to_string(),
            val: Box::new(InterpVal::Procedure(is_probable_prime_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "isSet".to_string(),
            val: Box::new(InterpVal::Procedure(is_type_stub_new("isSet", IRType::SET))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "isString".to_string(),
            val: Box::new(InterpVal::Procedure(is_type_stub_new(
                "isString",
                IRType::STRING,
            ))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "isTerm".to_string(),
            val: Box::new(InterpVal::Procedure(is_term_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "isVariable".to_string(),
            val: Box::new(InterpVal::Procedure(is_variable_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "join".to_string(),
            val: Box::new(InterpVal::Procedure(join_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "last".to_string(),
            val: Box::new(InterpVal::Procedure(last_stub_new("last"))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "load".to_string(),
            val: Box::new(InterpVal::Procedure(load_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "loadLibrary".to_string(),
            val: Box::new(InterpVal::Procedure(load_library_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "log".to_string(),
            val: Box::new(InterpVal::Procedure(ln_exp_stub_new(false))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "log10".to_string(),
            val: Box::new(InterpVal::Procedure(log10_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "log1p".to_string(),
            val: Box::new(InterpVal::Procedure(log1p_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "logo".to_string(),
            val: Box::new(InterpVal::Procedure(logo_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "makeTerm".to_string(),
            val: Box::new(InterpVal::Procedure(make_term_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "matches".to_string(),
            val: Box::new(InterpVal::Procedure(matches_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "mathConst".to_string(),
            val: Box::new(InterpVal::Procedure(math_const_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "max".to_string(),
            val: Box::new(InterpVal::Procedure(max_stub_new("max", true))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "min".to_string(),
            val: Box::new(InterpVal::Procedure(max_stub_new("min", false))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "nPrint".to_string(),
            val: Box::new(InterpVal::Procedure(n_print_stub_new(false))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "nPrintErr".to_string(),
            val: Box::new(InterpVal::Procedure(n_print_stub_new(true))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "now".to_string(),
            val: Box::new(InterpVal::Procedure(now_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "parse".to_string(),
            val: Box::new(InterpVal::Procedure(parse_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "parseStatements".to_string(),
            val: Box::new(InterpVal::Procedure(parse_statements_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "pow".to_string(),
            val: Box::new(InterpVal::Procedure(pow_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "print".to_string(),
            val: Box::new(InterpVal::Procedure(print_stub_new(false))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "printErr".to_string(),
            val: Box::new(InterpVal::Procedure(print_stub_new(true))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "raise".to_string(),
            val: Box::new(InterpVal::Procedure(throw_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "random".to_string(),
            val: Box::new(InterpVal::Procedure(random_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "range".to_string(),
            val: Box::new(InterpVal::Procedure(domain_range_stub_new(true))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "read".to_string(),
            val: Box::new(InterpVal::Procedure(read_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "readFile".to_string(),
            val: Box::new(InterpVal::Procedure(read_file_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "replace".to_string(),
            val: Box::new(InterpVal::Procedure(replace_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "replaceFirst".to_string(),
            val: Box::new(InterpVal::Procedure(replace_first_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "resetRandom".to_string(),
            val: Box::new(InterpVal::Procedure(reset_random_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "reverse".to_string(),
            val: Box::new(InterpVal::Procedure(reverse_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "rnd".to_string(),
            val: Box::new(InterpVal::Procedure(rnd_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "round".to_string(),
            val: Box::new(InterpVal::Procedure(float_op_stub_new(FloatOp::Round))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "run".to_string(),
            val: Box::new(InterpVal::Procedure(run_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "signum".to_string(),
            val: Box::new(InterpVal::Procedure(signum_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "sin".to_string(),
            val: Box::new(InterpVal::Procedure(num_op_stub_new(NumOp::Sin))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "sinh".to_string(),
            val: Box::new(InterpVal::Procedure(num_op_stub_new(NumOp::SinH))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "sleep".to_string(),
            val: Box::new(InterpVal::Procedure(sleep_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "split".to_string(),
            val: Box::new(InterpVal::Procedure(split_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "sqrt".to_string(),
            val: Box::new(InterpVal::Procedure(sqrt_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "str".to_string(),
            val: Box::new(InterpVal::Procedure(str_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "tan".to_string(),
            val: Box::new(InterpVal::Procedure(num_op_stub_new(NumOp::Tan))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "tanh".to_string(),
            val: Box::new(InterpVal::Procedure(num_op_stub_new(NumOp::TanH))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "throw".to_string(),
            val: Box::new(InterpVal::Procedure(throw_stub_new())),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "ulp".to_string(),
            val: Box::new(InterpVal::Procedure(num_op_stub_new(NumOp::Ulp))),
        }),
        InterpStackEntry::Variable(InterpStackVar {
            var: "writeFile".to_string(),
            val: Box::new(InterpVal::Procedure(write_file_stub_new())),
        }),
    ]
}
