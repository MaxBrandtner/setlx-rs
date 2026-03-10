use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::{BuiltinProc, BuiltinVar};
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::stmt::block_populate;
use crate::ir::lower::util::{block_get, stack_pop, tmp_var_new};

fn catch_block_new(
    exception_kind: u8,
    exception_var: Option<&str>,
    block: &CSTBlock,
    next_idx: NodeIndex,
    continue_idx: Option<NodeIndex>,
    break_idx: Option<NodeIndex>,
    ret_idx: NodeIndex,
    rethrow_idx: NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) -> NodeIndex {
    /* _ := stack_pop(c.exception);
     * goto <ret_idx>
     */
    let catch_ret_idx = if let Some(exception) = exception_var {
        let o = proc.blocks.add_node(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::StackPop),
                op: IROp::NativeCall(vec![IRValue::String(exception.to_string())]),
            }),
            IRStmt::Goto(ret_idx),
        ]);
        proc.blocks.add_edge(o, ret_idx, ());
        o
    } else {
        ret_idx
    };

    /* _ := stack_pop(c.exception);
     * goto <continue_idx>
     */
    let catch_continue_idx = if let Some(exception) = exception_var
        && let Some(continue_idx_val) = continue_idx
    {
        let o = proc.blocks.add_node(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::StackPop),
                op: IROp::NativeCall(vec![IRValue::String(exception.to_string())]),
            }),
            IRStmt::Goto(continue_idx_val),
        ]);
        proc.blocks.add_edge(o, continue_idx_val, ());
        Some(o)
    } else {
        continue_idx
    };

    /* _ := stack_pop(c.exception);
     * goto <break_idx>
     */
    let catch_break_idx = if let Some(exception) = exception_var
        && let Some(break_idx_val) = break_idx
    {
        let o = proc.blocks.add_node(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::StackPop),
                op: IROp::NativeCall(vec![IRValue::String(exception.to_string())]),
            }),
            IRStmt::Goto(break_idx_val),
        ]);
        proc.blocks.add_edge(o, break_idx_val, ());
        Some(o)
    } else {
        break_idx
    };

    let t_exception = if let Some(exception) = exception_var {
        let t = tmp_var_new(proc);
        shared_proc.definitions.push((exception.to_string(), t));
        t
    } else {
        0
    };

    let catch_main_block = proc.blocks.add_node(Vec::new());
    let mut catch_main_changed_block = catch_main_block;
    let catch_main_terminated = block_populate(
        &mut catch_main_changed_block,
        block,
        catch_continue_idx,
        catch_break_idx,
        catch_ret_idx,
        proc,
        shared_proc,
        cfg,
    );

    if !catch_main_terminated {
        /* _ := stack_pop(c.exception);
         * goto <next_idx>
         */
        let catch_next_idx = if let Some(exception) = exception_var {
            let o = proc.blocks.add_node(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::StackPop),
                    op: IROp::NativeCall(vec![IRValue::String(exception.to_string())]),
                }),
                IRStmt::Goto(next_idx),
            ]);
            proc.blocks.add_edge(o, next_idx, ());
            o
        } else {
            next_idx
        };
        block_get(proc, catch_main_changed_block).push(IRStmt::Goto(catch_next_idx));
        proc.blocks
            .add_edge(catch_main_changed_block, catch_next_idx, ());
    }

    if let Some(exception) = exception_var {
        stack_pop(shared_proc, exception);
    }

    /* t_exception := stack_add(c.exception);
     * *t_exception := exception_val;
     * _ := exception_reset();
     * goto <catch_main_block>
     */
    let catch_block = if let Some(exception) = exception_var {
        let o = proc.blocks.add_node(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_exception),
                types: IRType::PTR,
                source: IRValue::BuiltinProc(BuiltinProc::StackAdd),
                op: IROp::NativeCall(vec![IRValue::String(exception.to_string())]),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Deref(t_exception),
                types: IRTypes!("any"),
                source: IRValue::BuiltinVar(BuiltinVar::ExceptionVal),
                op: IROp::Assign,
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::ExceptionReset),
                op: IROp::NativeCall(Vec::new()),
            }),
            IRStmt::Goto(catch_main_block),
        ]);
        proc.blocks.add_edge(o, catch_main_block, ());
        o
    } else {
        let o = proc.blocks.add_node(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Ignore,
                types: IRType::UNDEFINED,
                source: IRValue::BuiltinProc(BuiltinProc::ExceptionReset),
                op: IROp::NativeCall(Vec::new()),
            }),
            IRStmt::Goto(catch_main_block),
        ]);
        proc.blocks.add_edge(o, catch_main_block, ());
        o
    };

    /* tmp := EXCEPTION_KIND == 1
     * if tmp
     *   goto <catch_block>
     * else
     *   goto <rethrow_idx>
     */
    let t_except = tmp_var_new(proc);
    let o = proc.blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_except),
            types: IRType::BOOL,
            source: IRValue::BuiltinVar(BuiltinVar::ExceptionKind),
            op: IROp::Equal(IRValue::Number(exception_kind.into())),
        }),
        IRStmt::Branch(IRBranch {
            cond: IRValue::Variable(t_except),
            success: catch_block,
            failure: rethrow_idx,
        }),
    ]);
    proc.blocks.add_edge(o, catch_block, ());
    proc.blocks.add_edge(o, rethrow_idx, ());
    o
}

/// Emits IR for a try-catch statement, advancing `block_idx` past the statement.
///
/// The try body is wrapped in a try region. Catch branches are matched against
/// the exception kind: `catch` handles user exceptions, `catchLng` handles
/// internal exceptions, and `finally` handles both. Unmatched exceptions are
/// rethrown. The exception value is pushed onto the stack under the catch
/// variable name for the duration of the catch body.
pub fn block_try_push(
    t: &CSTTryCatch,
    block_idx: &mut NodeIndex,
    continue_idx: Option<NodeIndex>,
    break_idx: Option<NodeIndex>,
    ret_idx: NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    let next_idx = proc.blocks.add_node(Vec::new());
    let try_ret_idx = proc
        .blocks
        .add_node(vec![IRStmt::TryEnd(ret_idx)]);
    proc.blocks.add_edge(try_ret_idx, ret_idx, ());
    let try_continue_idx = if let Some(continue_idx_val) = continue_idx {
        let o = proc
            .blocks
            .add_node(vec![IRStmt::TryEnd(continue_idx_val)]);
        proc.blocks.add_edge(o, continue_idx_val, ());
        Some(o)
    } else {
        None
    };
    let try_break_idx = if let Some(break_idx_val) = break_idx {
        let o = proc
            .blocks
            .add_node(vec![IRStmt::TryEnd(break_idx_val)]);
        proc.blocks.add_edge(o, break_idx_val, ());
        Some(o)
    } else {
        None
    };
    let main_idx = proc.blocks.add_node(Vec::new());
    let mut main_changed_idx = main_idx;
    let main_terminated = block_populate(
        &mut main_changed_idx,
        &t.try_branch,
        try_continue_idx,
        try_break_idx,
        try_ret_idx,
        proc,
        shared_proc,
        cfg,
    );

    if !main_terminated {
        block_get(proc, main_changed_idx).push(IRStmt::TryEnd(next_idx));
        proc.blocks.add_edge(main_changed_idx, next_idx, ());
    }

    let cst_catch_lng = t
        .catch_branches
        .iter()
        .find(|&x| matches!(x.kind, CSTCatchKind::Lng) || matches!(x.kind, CSTCatchKind::Final));
    let cst_catch_usr = t
        .catch_branches
        .iter()
        .find(|&x| matches!(x.kind, CSTCatchKind::Usr) || matches!(x.kind, CSTCatchKind::Final));

    /* <rethrow_idx>:
     * _ := throw(ExceptionKind, ExceptionVal);
     * unreachable;
     */
    let rethrow_idx = proc.blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Throw),
            op: IROp::NativeCall(vec![
                IRValue::BuiltinVar(BuiltinVar::ExceptionKind),
                IRValue::BuiltinVar(BuiltinVar::ExceptionVal),
            ]),
        }),
        IRStmt::Unreachable,
    ]);

    let catch_usr_idx = if let Some(c) = cst_catch_usr {
        catch_block_new(
            1,
            Some(&c.exception),
            &c.block,
            next_idx,
            continue_idx,
            break_idx,
            ret_idx,
            rethrow_idx,
            proc,
            shared_proc,
            cfg,
        )
    } else {
        rethrow_idx
    };

    let catch_lng_idx = if let Some(c) = cst_catch_usr
        && cst_catch_usr != cst_catch_lng
    {
        catch_block_new(
            0,
            Some(&c.exception),
            &c.block,
            next_idx,
            continue_idx,
            break_idx,
            ret_idx,
            rethrow_idx,
            proc,
            shared_proc,
            cfg,
        )
    } else {
        catch_usr_idx
    };

    /* try
     *    <main_idx>
     * catch
     *    <catch_lng_idx>
     */
    block_get(proc, *block_idx).push(IRStmt::Try(IRTry {
        attempt: main_idx,
        catch: catch_lng_idx,
    }));

    *block_idx = next_idx;
}

/// Emits IR for a check statement, advancing `block_idx` past the statement.
///
/// The check body is wrapped in a try region that catches backtrack exceptions.
/// If a backtrack is caught the after-backtrack block is executed, otherwise
/// control falls through normally.
pub fn block_check_push(
    c: &CSTCheck,
    block_idx: &mut NodeIndex,
    continue_idx: Option<NodeIndex>,
    break_idx: Option<NodeIndex>,
    ret_idx: NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    let next_idx = proc.blocks.add_node(Vec::new());
    let try_ret_idx = proc
        .blocks
        .add_node(vec![IRStmt::TryEnd(ret_idx)]);
    proc.blocks.add_edge(try_ret_idx, ret_idx, ());
    let try_continue_idx = if let Some(continue_idx_val) = continue_idx {
        let o = proc
            .blocks
            .add_node(vec![IRStmt::TryEnd(continue_idx_val)]);
        proc.blocks.add_edge(o, continue_idx_val, ());
        Some(o)
    } else {
        None
    };
    let try_break_idx = if let Some(break_idx_val) = break_idx {
        let o = proc
            .blocks
            .add_node(vec![IRStmt::TryEnd(break_idx_val)]);
        proc.blocks.add_edge(o, break_idx_val, ());
        Some(o)
    } else {
        None
    };

    let main_idx = proc.blocks.add_node(Vec::new());
    let mut main_changed_idx = main_idx;

    let main_terminated = block_populate(
        &mut main_changed_idx,
        &c.block,
        try_continue_idx,
        try_break_idx,
        try_ret_idx,
        proc,
        shared_proc,
        cfg,
    );

    if !main_terminated {
        block_get(proc, main_changed_idx).push(IRStmt::TryEnd(next_idx));
        proc.blocks.add_edge(main_changed_idx, next_idx, ());
    }

    /* <rethrow_idx>:
     * _ := throw(ExceptionVal);
     * unreachable;
     */
    let rethrow_idx = proc.blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::Throw),
            op: IROp::NativeCall(vec![IRValue::BuiltinVar(BuiltinVar::ExceptionVal)]),
        }),
        IRStmt::Unreachable,
    ]);

    let catch_idx = catch_block_new(
        2,
        None,
        &c.after_backtrack,
        next_idx,
        continue_idx,
        break_idx,
        ret_idx,
        rethrow_idx,
        proc,
        shared_proc,
        cfg,
    );

    /* try
     *    <main_idx>
     * catch
     *    <catch_lng_idx>
     */
    block_get(proc, *block_idx).push(IRStmt::Try(IRTry {
        attempt: main_idx,
        catch: catch_idx,
    }));

    *block_idx = next_idx;
}
