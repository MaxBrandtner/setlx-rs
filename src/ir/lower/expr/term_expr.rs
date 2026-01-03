use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_term_push(
    t: &CSTTerm,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /* t_term := term_new(t.name, t.params.len(), t.is_tterm);
     * t_i := params[0];
     * term_add(t_term, t_i);
     * // ...
     * target := t_term;
     */
    let t_term = tmp_var_new(proc);

    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_term),
        types: if t.is_tterm {
            IRType::TTERM
        } else {
            IRType::TERM
        },
        source: IRValue::BuiltinProc(BuiltinProc::TermNew),
        op: IROp::NativeCall(vec![
            IRValue::String(t.name.to_string()),
            IRValue::Number(t.params.len().into()),
            IRValue::Bool(t.is_tterm),
        ]),
    }));

    t.params.iter().for_each(|i| {
        let t_i = tmp_var_new(proc);
        let owned = block_expr_push(
            i,
            block_idx,
            IRTarget::Variable(t_i),
            proc,
            shared_proc,
            cfg,
        );

        if !owned {
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_i),
                types: IRTypes!("any"),
                source: IRValue::BuiltinProc(BuiltinProc::Copy),
                op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
            }));
        }

        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::TermAdd),
            op: IROp::NativeCall(vec![IRValue::Variable(t_term), IRValue::Variable(t_i)]),
        }));
    });

    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: if t.is_tterm {
            IRType::TTERM
        } else {
            IRType::TERM
        },
        source: IRValue::Variable(t_term),
        op: IROp::Assign,
    }));
}
