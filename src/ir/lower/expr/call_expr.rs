use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::proc::call_params_push;
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

    let inv_vars = call_params_push(
        c,
        block_idx,
        IRTarget::Variable(t_params),
        proc,
        shared_proc,
        cfg,
    );

    if let Some(t_3) = stack_get(shared_proc, &c.name) {
        // target := t_3(params);
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target,
            types: IRTypes!("any"),
            source: IRValue::Variable(t_3),
            op: IROp::Call(t_params),
        }));
    } else {
        /* t_2 := stack_get_assert(name);
         * t_3 := *t_2;
         * target := t_3(params);
         */
        let t_2 = tmp_var_new(proc);
        let t_3 = tmp_var_new(proc);
        block_get(proc, *block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_2),
                types: IRType::PTR,
                source: IRValue::BuiltinProc(BuiltinProc::StackGetAssert),
                op: IROp::NativeCall(vec![IRValue::String(c.name.clone())]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_3),
                types: IRTypes!("any"),
                source: IRValue::Variable(t_2),
                op: IROp::PtrDeref,
            }),
            IRStmt::Assign(IRAssign {
                target,
                types: IRTypes!("any"),
                source: IRValue::Variable(t_3),
                op: IROp::Call(t_params),
            }),
        ]);
    }

    inv_vars.iter().for_each(|i| {
        // _ := invalidate(i);
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(*i)]),
        }));
    });

    // _ := invalidate(t_params);
    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Ignore,
        types: IRType::UNDEFINED,
        source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
        op: IROp::NativeCall(vec![IRValue::Variable(t_params)]),
    }));
}
