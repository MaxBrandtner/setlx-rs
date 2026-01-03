use petgraph::stable_graph::NodeIndex;

use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::util::{block_get, stack_get, tmp_var_new};

pub fn block_var_push(
    c: &str,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
) {
    if let Some(tmp) = stack_get(shared_proc, c) {
        // target := *tmp;
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target,
            types: IRTypes!("any"),
            source: IRValue::Variable(tmp),
            op: IROp::PtrDeref,
        }));
    } else {
        /* t_1 := stack_get_assert(c);
         * target := *t_1;
         */
        let tmp = tmp_var_new(proc);
        block_get(proc, *block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(tmp),
                types: IRType::PTR,
                source: IRValue::BuiltinProc(BuiltinProc::StackGetAssert),
                op: IROp::NativeCall(vec![IRValue::String(c.to_string())]),
            }),
            IRStmt::Assign(IRAssign {
                target,
                types: IRTypes!("any"),
                source: IRValue::Variable(tmp),
                op: IROp::PtrDeref,
            }),
        ]);
    }
}
