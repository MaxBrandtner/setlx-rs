mod assign;
mod catch;
mod class;
mod for_stmt;
mod if_stmt;
mod match_stmt;
mod scan_stmt;
mod while_stmt;

use petgraph::stable_graph::NodeIndex;

use assign::{block_assign_mod_push, block_assign_push};
use catch::{block_check_push, block_try_push};
use class::block_class_push;
use for_stmt::block_for_push;
use if_stmt::block_if_push;
use match_stmt::block_match_push;
use scan_stmt::block_scan_push;
use while_stmt::{block_do_while_push, block_while_push};

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

/// Compiles a sequence of CST statements into IR, appending to the current block.
/// Returns `true` if the follow block was terminated (e.g. by a `return`,
/// `break`, or `continue`), `false` if control falls through normally.
///
/// - `block_idx` — the current block being written to; advanced forward as
///   new blocks are created for control flow
/// - `cst` — the sequence of CST statements to compile
/// - `continue_idx` — the block to jump to on a `continue` statement, or
///   `None` if not inside a loop
/// - `break_idx` — the block to jump to on a `break` statement, or `None`
///   if not inside a loop
/// - `ret_idx` — the block to jump to on a `return` statement
pub fn block_populate(
    block_idx: &mut NodeIndex,
    cst: &[CSTStatement],
    continue_idx: Option<NodeIndex>,
    break_idx: Option<NodeIndex>,
    ret_idx: NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) -> bool /* follow block terminated */ {
    let lhs_old = shared_proc.code_lhs;
    let rhs_old = shared_proc.code_rhs;

    for stmt in cst {

        if !shared_proc.disable_annotations {
            block_get(proc, *block_idx).push(IRStmt::Annotate(stmt.lhs, stmt.rhs));
        }

        shared_proc.code_lhs = stmt.lhs;
        shared_proc.code_rhs = stmt.rhs;

        match &stmt.kind {
            CSTStatementKind::Class(c) => {
                block_class_push(c, block_idx, proc, shared_proc, cfg);
            }
            CSTStatementKind::Match(m) => {
                block_match_push(
                    m,
                    block_idx,
                    continue_idx,
                    break_idx,
                    ret_idx,
                    proc,
                    shared_proc,
                    cfg,
                );
            }
            CSTStatementKind::Scan(s) => {
                block_scan_push(
                    s,
                    block_idx,
                    continue_idx,
                    break_idx,
                    ret_idx,
                    proc,
                    shared_proc,
                    cfg,
                );
            }
            CSTStatementKind::Exit => {
                /* //native call
                 * _ := exit(0);
                 * unreachable;
                 */
                block_get(proc, *block_idx).extend(vec![
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Ignore,
                        types: IRType::UNDEFINED,
                        source: IRValue::BuiltinProc(BuiltinProc::Exit),
                        op: IROp::NativeCall(Vec::new()),
                    }),
                    IRStmt::Unreachable,
                ]);
                return true;
            }
            CSTStatementKind::Return(expr) => {
                let ret_var = shared_proc.ret_var;
                if let Some(expr_val) = &expr.val {
                    let is_owned = block_expr_push(
                        expr_val,
                        block_idx,
                        IRTarget::Variable(ret_var),
                        proc,
                        shared_proc,
                        cfg,
                    );

                    if !is_owned {
                        // ret := copy(ret);
                        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                            target: IRTarget::Variable(ret_var),
                            types: IRTypes!("any"),
                            source: IRValue::BuiltinProc(BuiltinProc::Copy),
                            op: IROp::NativeCall(vec![IRValue::Variable(ret_var)]),
                        }));
                    }

                    block_get(proc, *block_idx).push(
                        IRStmt::Assign(IRAssign {
                            target: IRTarget::Ignore,
                            types: IRType::UNDEFINED,
                            source: IRValue::BuiltinProc(BuiltinProc::MarkPersist),
                            op: IROp::NativeCall(vec![IRValue::Variable(ret_var)]),
                        })
                    );
                } else {
                    // ret := om;
                    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                        target: IRTarget::Variable(ret_var),
                        types: IRType::UNDEFINED,
                        source: IRValue::Undefined,
                        op: IROp::Assign,
                    }));
                }

                // goto <bb{ret_block}>;
                block_get(proc, *block_idx).push(IRStmt::Goto(ret_idx));
                proc.blocks.add_edge(*block_idx, ret_idx, ());
                return true;
            }
            CSTStatementKind::If(c) | CSTStatementKind::Switch(c) => {
                block_if_push(
                    c,
                    block_idx,
                    continue_idx,
                    break_idx,
                    ret_idx,
                    proc,
                    shared_proc,
                    cfg,
                );
            }
            CSTStatementKind::For(f) => {
                block_for_push(f, block_idx, ret_idx, proc, shared_proc, cfg);
            }
            CSTStatementKind::While(w) => {
                block_while_push(
                    w,
                    block_idx,
                    ret_idx,
                    proc,
                    shared_proc,
                    cfg,
                );
            }
            CSTStatementKind::DoWhile(w) => {
                block_do_while_push(
                    w,
                    block_idx,
                    ret_idx,
                    proc,
                    shared_proc,
                    cfg,
                );
                break;
            }
            CSTStatementKind::TryCatch(t) => {
                block_try_push(
                    t,
                    block_idx,
                    continue_idx,
                    break_idx,
                    ret_idx,
                    proc,
                    shared_proc,
                    cfg,
                );
            }
            CSTStatementKind::Check(c) => {
                block_check_push(
                    c,
                    block_idx,
                    continue_idx,
                    break_idx,
                    ret_idx,
                    proc,
                    shared_proc,
                    cfg,
                );
            }
            CSTStatementKind::Assign(_) => {
                block_assign_push(stmt, block_idx, proc, shared_proc, cfg);
            }
            CSTStatementKind::AssignMod(a) => {
                block_assign_mod_push(a, block_idx, proc, shared_proc, cfg);
            }
            CSTStatementKind::Expression(e) => {
                let v = tmp_var_new(proc);
                let owned = block_expr_push(e, block_idx, IRTarget::Variable(v), proc, shared_proc, cfg);
                if owned {
                    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                        target: IRTarget::Ignore,
                        types: IRType::UNDEFINED,
                        source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                        op: IROp::NativeCall(vec![IRValue::Variable(v)]),
                    }));
                }
            }
            CSTStatementKind::Backtrack => {
                /* _ := throw(2, "");
                 * unreachable;
                 */
                block_get(proc, *block_idx).extend(vec![
                    IRStmt::Assign(IRAssign {
                        target: IRTarget::Ignore,
                        types: IRType::UNDEFINED,
                        source: IRValue::BuiltinProc(BuiltinProc::Throw),
                        op: IROp::NativeCall(vec![
                            IRValue::Number(2.into()),
                            IRValue::String("".to_string()),
                        ]),
                    }),
                    IRStmt::Unreachable,
                ]);
                return true;
            }
            CSTStatementKind::Continue => {
                block_get(proc, *block_idx).push(IRStmt::Goto(continue_idx.unwrap()));
                proc.blocks.add_edge(*block_idx, continue_idx.unwrap(), ());
                return true;
            }
            CSTStatementKind::Break => {
                block_get(proc, *block_idx).push(IRStmt::Goto(break_idx.unwrap()));
                proc.blocks.add_edge(*block_idx, break_idx.unwrap(), ());
                return true;
            }
        }
    }

    if !shared_proc.disable_annotations {
        block_get(proc, *block_idx).push(IRStmt::Annotate(lhs_old, rhs_old));
    }

    shared_proc.code_lhs = lhs_old;
    shared_proc.code_rhs = rhs_old;

    false
}
