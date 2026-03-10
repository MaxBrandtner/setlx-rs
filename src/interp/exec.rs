use ariadne::{Color, Label, Report, ReportKind, Source};
use petgraph::stable_graph::NodeIndex;
use rustyline::{Config, DefaultEditor};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::panic;
use std::panic::AssertUnwindSafe;
use std::process::exit;
use std::rc::Rc;

use crate::cli::InputOpts;
use crate::interp::assign::exec_assign;
use crate::interp::debug::{DebugData, debug_ctrl};
use crate::interp::except::exception_unwind_str;
use crate::interp::get::InterpGet;
use crate::interp::heap::*;
use crate::interp::memoize::InterpMemoize;

use crate::interp::stack::InterpStack;
use crate::ir::def::*;
use crate::ir::dump::ir_dump_stmt;

fn exec_try(
    proc: Rc<RefCell<IRProcedure>>,
    mut block_idx: NodeIndex,
    fail_idx: NodeIndex,
    params: &InterpVal,
    vars: &mut [InterpVal],
    stack: &mut InterpStack,
    memo: &mut InterpMemoize,
    cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    opts: &InputOpts,
    rl: &mut DefaultEditor,
) -> NodeIndex {
    if breakpoints.try_start {
        breakpoints.step = true;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let mut immed_heap = InterpImmediateHeap::new();

        loop {
            let block: &IRBlock = &proc.borrow().blocks[block_idx];

            if breakpoints.blocks.contains(&block_idx.index()) {
                breakpoints.step = true;
            }

            if opts.debug_ir && (breakpoints.step || breakpoints.print) {
                eprintln!("<bb{}>:", block_idx.index());
            }

            for stmt in block {
                if opts.debug_ir && (breakpoints.step || breakpoints.print) {
                    let mut stmt_str = String::new();
                    ir_dump_stmt(stmt, &mut stmt_str);
                    eprint!("{stmt_str}");
                }

                if opts.debug_ir && breakpoints.step {
                    debug_ctrl(
                        vars,
                        params,
                        stack,
                        &mut immed_heap,
                        memo,
                        cstore,
                        breakpoints,
                        opts,
                        rl,
                    );
                }

                match stmt {
                    IRStmt::Annotate(lhs, rhs) => {
                        breakpoints.code_lhs = *lhs;
                        breakpoints.code_rhs = *rhs;
                        if breakpoints.print_src {
                            Report::build(
                                ReportKind::Advice,
                                (
                                    &breakpoints.srcname,
                                    breakpoints.code_lhs..breakpoints.code_rhs,
                                ),
                            )
                            .with_label(
                                Label::new((
                                    &breakpoints.srcname,
                                    breakpoints.code_lhs..breakpoints.code_rhs,
                                ))
                                .with_color(Color::Yellow),
                            )
                            .finish()
                            .eprint((&breakpoints.srcname, Source::from(&breakpoints.src)))
                            .unwrap();
                        }
                        if breakpoints.break_src {
                            debug_ctrl(
                                vars,
                                params,
                                stack,
                                &mut immed_heap,
                                memo,
                                cstore,
                                breakpoints,
                                opts,
                                rl,
                            );
                        }
                    }
                    IRStmt::Assign(a) => {
                        exec_assign(
                            a,
                            vars,
                            params,
                            stack,
                            &mut immed_heap,
                            memo,
                            cstore,
                            breakpoints,
                            opts,
                            rl,
                        );
                    }
                    IRStmt::Branch(b) => {
                        let cond = b.cond.to_bool(vars, params, breakpoints, "branch");

                        if cond {
                            block_idx = b.success;
                        } else {
                            block_idx = b.failure;
                        }

                        break;
                    }
                    IRStmt::Try(t) => {
                        block_idx = exec_try(
                            proc.clone(),
                            t.attempt,
                            t.catch,
                            params,
                            vars,
                            stack,
                            memo,
                            cstore,
                            breakpoints,
                            opts,
                            rl,
                        );
                        break;
                    }
                    IRStmt::TryEnd(next_idx) => {
                        return *next_idx;
                    }
                    IRStmt::Goto(idx) => {
                        block_idx = *idx;
                        break;
                    }
                    IRStmt::Return(_) => {
                        panic!("internal: return statement in try section");
                    }
                    IRStmt::Unreachable => {
                        panic!("interal: encountered unreachable guard");
                    }
                }
            }
        }
    }));

    match result {
        Ok(next_idx) => next_idx,
        Err(_) => fail_idx,
    }
}

pub fn exec_proc(
    proc: Rc<RefCell<IRProcedure>>,
    params: &InterpVal,
    stack: &mut InterpStack,
    memo: &mut InterpMemoize,
    cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    opts: &InputOpts,
    rl: &mut DefaultEditor,
) -> InterpVal {
    let mut block_idx = proc.borrow().start_block;

    let mut vars = vec![InterpVal::Undefined; proc.borrow().vars.len()];
    let mut immed_heap = InterpImmediateHeap::new();

    if breakpoints.call || breakpoints.procs.contains(&proc.borrow().tag) {
        breakpoints.step = true;
    }

    if opts.debug_ir && (breakpoints.step || breakpoints.print) {
        eprintln!("_{}(){{", proc.borrow().tag);
    }

    loop {
        let block: &IRBlock = &proc.borrow().blocks[block_idx];

        if breakpoints.blocks.contains(&block_idx.index()) {
            breakpoints.step = true;
        }

        if opts.debug_ir && (breakpoints.step || breakpoints.print) {
            eprintln!("<bb{}>:", block_idx.index());
        }

        for stmt in block {
            if opts.debug_ir && (breakpoints.step || breakpoints.print) {
                let mut stmt_str = String::new();
                ir_dump_stmt(stmt, &mut stmt_str);
                eprint!("{stmt_str}");
            }

            if opts.debug_ir && breakpoints.step {
                debug_ctrl(
                    &mut vars,
                    params,
                    stack,
                    &mut immed_heap,
                    memo,
                    cstore,
                    breakpoints,
                    opts,
                    rl,
                );
            }

            match stmt {
                IRStmt::Annotate(lhs, rhs) => {
                    breakpoints.code_lhs = *lhs;
                    breakpoints.code_rhs = *rhs;
                    if breakpoints.print_src {
                        Report::build(
                            ReportKind::Advice,
                            (
                                &breakpoints.srcname,
                                breakpoints.code_lhs..breakpoints.code_rhs,
                            ),
                        )
                        .with_label(
                            Label::new((
                                &breakpoints.srcname,
                                breakpoints.code_lhs..breakpoints.code_rhs,
                            ))
                            .with_color(Color::Yellow),
                        )
                        .finish()
                        .eprint((&breakpoints.srcname, Source::from(&breakpoints.src)))
                        .unwrap();
                    }
                    if breakpoints.break_src {
                        debug_ctrl(
                            &mut vars,
                            params,
                            stack,
                            &mut immed_heap,
                            memo,
                            cstore,
                            breakpoints,
                            opts,
                            rl,
                        );
                    }
                }
                IRStmt::Assign(a) => {
                    exec_assign(
                        a,
                        &mut vars,
                        params,
                        stack,
                        &mut immed_heap,
                        memo,
                        cstore,
                        breakpoints,
                        opts,
                        rl,
                    );
                }
                IRStmt::Branch(b) => {
                    let cond = b.cond.to_bool(&vars, params, breakpoints, "branch");
                    if cond {
                        block_idx = b.success;
                    } else {
                        block_idx = b.failure;
                    }

                    if breakpoints.branch {
                        breakpoints.step = true;
                    }

                    break;
                }
                IRStmt::Try(t) => {
                    block_idx = exec_try(
                        proc.clone(),
                        t.attempt,
                        t.catch,
                        params,
                        &mut vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        opts,
                        rl,
                    );
                    break;
                }
                IRStmt::TryEnd(_) => {
                    panic!("internal: encountered unmet try end statement");
                }
                IRStmt::Goto(idx) => {
                    block_idx = *idx;
                    break;
                }
                IRStmt::Return(val) => {
                    let res = val
                        .to_val(&vars, params, breakpoints, opts, &mut immed_heap)
                        .confirm()
                        .val
                        .clone()
                        .persist(&mut immed_heap);

                    if opts.debug_ir && (breakpoints.step || breakpoints.print) {
                        eprintln!("}}");
                    }

                    if breakpoints.ret {
                        breakpoints.step = true;
                    }

                    return res;
                }
                IRStmt::Unreachable => {
                    panic!("interal: encountered unreachable guard");
                }
            }
        }
    }
}

pub fn exec(cfg: IRCfg, opts: &InputOpts, src: String) {
    let main = cfg.main.clone();
    drop(cfg);

    let old_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));

    let mut stack = InterpStack::new();
    let mut memo: InterpMemoize = BTreeMap::new();
    let mut cstore = InterpClassStore::default();
    let mut breakpoints = DebugData::from_src(src, opts.srcname.clone());

    let config = Config::builder().enable_signals(true).build();
    let mut rl = DefaultEditor::with_config(config).unwrap();

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        exec_proc(
            main,
            &InterpVal::Undefined,
            &mut stack,
            &mut memo,
            &mut cstore,
            &mut breakpoints,
            opts,
            &mut rl,
        )
    }));
    panic::set_hook(old_hook);

    if let Err(e) = result {
        eprintln!(
            "{}",
            exception_unwind_str(
                e,
                &mut Vec::new(),
                &mut stack,
                &mut memo,
                &mut cstore,
                &mut breakpoints,
                opts,
                &mut rl
            )
        );
        exit(1);
    }
}
