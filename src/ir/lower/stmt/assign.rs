use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::*;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::assign::assign_parse;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn block_assign_push(
    stmt: &CSTStatement,
    block_idx: &mut NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    let source_cst: &CSTExpression;
    let mut targets_cst: Vec<&CSTExpression> = Vec::new();

    let mut current_assign: &CSTStatement = stmt;
    loop {
        match current_assign {
            CSTStatement::Assign(d) => {
                targets_cst.push(&d.assign);
                current_assign = &d.expr;
            }
            CSTStatement::Expression(e) => {
                source_cst = e;
                break;
            }
            _ => {
                panic!("CSTAssign expr must be CSTAssign or CSTExpression");
            }
        }
    }

    let tmp = tmp_var_new(proc);
    //tmp := expr;
    let is_owned = block_expr_push(
        source_cst,
        block_idx,
        IRTarget::Variable(tmp),
        proc,
        shared_proc,
        cfg,
    );

    for i in targets_cst.iter().rev() {
        assign_parse(block_idx, tmp, is_owned, None, false, i, proc, shared_proc, cfg);
    }
}

pub fn block_assign_mod_push(
    a: &CSTAssignMod,
    block_idx: &mut NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    // t_expr := expr;
    let t_expr = tmp_var_new(proc);
    let expr_owned = block_expr_push(
        &a.expr,
        block_idx,
        IRTarget::Variable(t_expr),
        proc,
        shared_proc,
        cfg,
    );
    // t_assign := assign;
    let t_assign = tmp_var_new(proc);
    let assign_owned = block_expr_push(
        &a.assign,
        block_idx,
        IRTarget::Variable(t_expr),
        proc,
        shared_proc,
        cfg,
    );

    let t_out = tmp_var_new(proc);
    match a.kind {
        CSTAssignModKind::PlusEq => block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_assign),
            op: IROp::Plus(IRValue::Variable(t_expr)),
        })),
        CSTAssignModKind::MinusEq => block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_assign),
            op: IROp::Minus(IRValue::Variable(t_expr)),
        })),
        CSTAssignModKind::MultEq => block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_assign),
            op: IROp::Mult(IRValue::Variable(t_expr)),
        })),
        CSTAssignModKind::DivEq => block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_assign),
            op: IROp::Divide(IRValue::Variable(t_expr)),
        })),
        CSTAssignModKind::IntDivEq => block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_assign),
            op: IROp::IntDivide(IRValue::Variable(t_expr)),
        })),
        CSTAssignModKind::ModEq => block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_out),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_assign),
            op: IROp::Mod(IRValue::Variable(t_expr)),
        })),
    }

    assign_parse(
        block_idx,
        t_out,
        true,
        None,
        false,
        &a.assign,
        proc,
        shared_proc,
        cfg,
    );

    if expr_owned {
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_expr)]),
        }));
    }

    if assign_owned {
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
            op: IROp::NativeCall(vec![IRValue::Variable(t_assign)]),
        }));
    }
}
