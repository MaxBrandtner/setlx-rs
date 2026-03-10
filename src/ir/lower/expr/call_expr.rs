use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::proc::{call_params_invalidate_push, call_params_push};
use crate::ir::lower::util::{block_get, stack_get, tmp_var_new};

pub fn block_call_push(
    c: &CSTProcedureCall,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    let t_params = tmp_var_new(proc);
    let t_proc = if let Some(t_3) = stack_get(shared_proc, &c.name) {
        // t_proc := *t_3;
        let t_proc = tmp_var_new(proc);

        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_proc),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_3),
            op: IROp::PtrDeref,
        }));

        t_proc
    } else {
        /* t_3 := stack_get_or_new(c.name);
         * t_proc := *t_3;
         */
        let t_3 = tmp_var_new(proc);
        let t_proc = tmp_var_new(proc);

        block_get(proc, *block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_3),
                types: IRType::PTR,
                source: IRValue::BuiltinProc(BuiltinProc::StackGetOrNew),
                op: IROp::NativeCall(vec![IRValue::String(c.name.to_string())]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_proc),
                types: IRTypes!("any"),
                source: IRValue::Variable(t_3),
                op: IROp::PtrDeref,
            }),
        ]);

        t_proc
    };

    let inv_vars = call_params_push(
        c,
        block_idx,
        IRTarget::Variable(t_params),
        t_proc,
        proc,
        shared_proc,
        cfg,
    );

    let call_idx = proc.blocks.add_node(Vec::new());
    let landing_pad_idx = proc.blocks.add_node(Vec::new());
    let follow_idx = proc.blocks.add_node(Vec::new());

    block_get(proc, *block_idx).push(IRStmt::Try(IRTry {
        attempt: call_idx,
        catch: landing_pad_idx,
    }));

    proc.blocks.add_edge(*block_idx, call_idx, ());
    proc.blocks.add_edge(*block_idx, landing_pad_idx, ());

    let t_tmp = tmp_var_new(proc);
    // t_tmp := t_proc(t_params);
    block_get(proc, call_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_tmp),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_proc),
            op: IROp::Call(t_params),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::MarkPersist),
            op: IROp::NativeCall(vec![IRValue::Variable(t_tmp)]),
        }),
        IRStmt::TryEnd(follow_idx),
    ]);

    proc.blocks.add_edge(call_idx, follow_idx, ());

    call_params_invalidate_push(landing_pad_idx, &inv_vars, proc);
    call_params_invalidate_push(follow_idx, &inv_vars, proc);

    /* <landing_pad_idx>:
     *  _ := invalidate(t_params);
     *  _ := rethrow();
     *  unreachable;
     */
    block_get(proc, landing_pad_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_params)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Rethrow),
            op: IROp::NativeCall(vec![]),
        }),
        IRStmt::Unreachable,
    ]);

    /* _ := invalidate(t_params);
     * _ := mark_immed(t_tmp);
     * target := t_tmp;
     */
    block_get(proc, follow_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_params)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::MarkImmed),
            op: IROp::NativeCall(vec![IRValue::Variable(t_tmp)]),
        }),
        IRStmt::Assign(IRAssign {
            target,
            types: IRTypes!("any"),
            source: IRValue::Variable(t_tmp),
            op: IROp::Assign,
        }),
    ]);

    *block_idx = follow_idx
}
