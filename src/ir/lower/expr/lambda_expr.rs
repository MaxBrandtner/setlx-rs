use petgraph::stable_graph::{NodeIndex, StableGraph};

use crate::ast::*;
use crate::builtin::{BuiltinProc, BuiltinVar};
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::ast::expr::block_cst_expr_push;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_lambda_push(
    expr: &CSTExpression,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    cfg: &mut IRCfg,
) {
    let l = if let CSTExpression::Lambda(l) = expr {
        l
    } else {
        unreachable!();
    };
    let mut lambda_proc = IRProcedure {
        start_block: NodeIndex::from(0),
        end_block: NodeIndex::from(0),
        blocks: StableGraph::new(),
        vars: Vec::new(),
    };

    // _ := stack_frame_add();
    let mut lambda_idx = lambda_proc.blocks.add_node(vec![IRStmt::Assign(IRAssign {
        target: IRTarget::Ignore,
        types: IRType::UNDEFINED,
        source: IRValue::BuiltinProc(BuiltinProc::StackFrameAdd),
        op: IROp::NativeCall(Vec::new()),
    })]);
    lambda_proc.start_block = lambda_idx;

    if l.is_closure {
        /* t_stack := params[0];
         * _ := stack_frame_restore(t_stack);
         */
        let t_stack = tmp_var_new(proc);
        block_get(&mut lambda_proc, lambda_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_stack),
                types: IRType::PTR,
                source: IRValue::BuiltinVar(BuiltinVar::Params),
                op: IROp::AccessArray(IRValue::Number(0.into())),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::StackFrameRestore),
                op: IROp::NativeCall(vec![IRValue::Variable(t_stack)]),
            }),
        ]);
    }

    let params_offset = if l.is_closure { 1 } else { 0 };

    let vars = match &l.params {
        CSTCollection::List(s) => &s.expressions,
        _ => panic!("lambda params must be a list of variables"),
    };

    let mut lambda_shared_proc = IRSharedProc {
        definitions: Vec::new(),
        ret_var: 0,
    };

    for (i, _) in vars.iter().enumerate() {
        /* t_param = param[i + params_offset];
         * t_var = stack_add("var");
         * *t_var = *t_param;
         */
        let var_name = match &vars[i] {
            CSTExpression::Variable(v) => v.clone(),
            _ => panic!("lambda param must be variable"),
        };

        let t_param = tmp_var_new(&mut lambda_proc);
        let t_var = tmp_var_new(&mut lambda_proc);

        lambda_shared_proc
            .definitions
            .push((var_name.clone(), t_var));

        block_get(&mut lambda_proc, lambda_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_param),
                types: IRType::PTR,
                source: IRValue::BuiltinVar(BuiltinVar::Params),
                op: IROp::AccessArray(IRValue::Number((i + params_offset).into())),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_var),
                types: IRType::PTR,
                source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
                op: IROp::NativeCall(vec![IRValue::String(var_name)]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Deref(t_var),
                types: IRTypes!("any"),
                source: IRValue::Variable(t_param),
                op: IROp::PtrDeref,
            }),
        ]);
    }

    let lambda_target = tmp_var_new(&mut lambda_proc);
    let lambda_target_owned = block_expr_push(
        &l.expr,
        &mut lambda_idx,
        IRTarget::Variable(lambda_target),
        &mut lambda_proc,
        &mut lambda_shared_proc,
        cfg,
    );

    if lambda_target_owned {
        block_get(&mut lambda_proc, lambda_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(lambda_target),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(lambda_target)]),
        }));
    }

    /* _ := stack_frame_pop();
     * return target;
     */
    block_get(&mut lambda_proc, lambda_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::StackFramePop),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Return(IRValue::Variable(lambda_target)),
    ]);

    lambda_proc.end_block = lambda_idx;

    let lambda_proc_idx = cfg.procedures.add_node(lambda_proc);

    if l.is_closure {
        /* t_info := // cst expr
         * t_stack := stack_copy();
         * target := closure_new(lambda_proc_idx, t_stack, t_info);
         */
        let t_info = tmp_var_new(proc);
        block_cst_expr_push(expr, *block_idx, IRTarget::Variable(t_info), proc);
        let t_stack = tmp_var_new(proc);
        block_get(proc, *block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_stack),
                types: IRType::LIST,
                source: IRValue::BuiltinProc(BuiltinProc::StackCopy),
                op: IROp::NativeCall(Vec::new()),
            }),
            IRStmt::Assign(IRAssign {
                target,
                types: IRType::CLOSURE,
                source: IRValue::BuiltinProc(BuiltinProc::ClosureNew),
                op: IROp::NativeCall(vec![
                    IRValue::Procedure(lambda_proc_idx),
                    IRValue::Variable(t_stack),
                    IRValue::Variable(t_info),
                ]),
            }),
        ]);
    } else {
        /* t_info := // cst expr
         * target := procedure_new(lambda_proc_idx, t_info);
         */
        let t_info = tmp_var_new(proc);
        block_cst_expr_push(expr, *block_idx, IRTarget::Variable(t_info), proc);
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target,
            types: IRType::PROCEDURE,
            source: IRValue::BuiltinProc(BuiltinProc::ProcedureNew),
            op: IROp::NativeCall(vec![
                IRValue::Procedure(lambda_proc_idx),
                IRValue::Variable(t_info),
            ]),
        }));
    }
}
