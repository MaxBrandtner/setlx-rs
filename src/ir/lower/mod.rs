mod assign;
mod ast;
pub mod expr;
mod iter;
mod proc;
mod stmt;
pub mod util;

use petgraph::stable_graph::{NodeIndex, StableGraph};
use std::cell::RefCell;
use std::rc::Rc;

use crate::ast::*;
use crate::builtin::*;
use crate::cli::InputOpts;
use crate::ir::def::*;
use crate::ir::dump::ir_dump;
use crate::ir::lower::ast::{block_cst_block_push, expr::block_cst_expr_push};
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::proc::procedure_vars_aggregate;
use crate::ir::lower::stmt::block_populate;
use crate::ir::lower::util::{block_get, tmp_var_new};

use proc::procedure_new;

#[derive(Default)]
pub struct IRSharedProc {
    definitions: Vec<(String, IRVar)>,
    ret_var: IRVar,
    disable_annotations: bool,
    code_lhs: usize,
    code_rhs: usize,
}

impl Clone for IRSharedProc {
    fn clone(&self) -> Self {
        IRSharedProc {
            definitions: Vec::new(),
            ret_var: 0,
            disable_annotations: self.disable_annotations,
            code_lhs: self.code_lhs,
            code_rhs: self.code_rhs,
        }
    }
}

pub trait CSTIRLower {
    fn from_cst(_: &CSTBlock, opts: &InputOpts) -> IRCfg;
    fn from_expr(_: &CSTExpression, opts: &InputOpts) -> Rc<RefCell<IRProcedure>>;
    fn from_stmt(_: &CSTBlock, opts: &InputOpts) -> Rc<RefCell<IRProcedure>>;
    fn from_ast_expr(_: &CSTExpression, opts: &InputOpts) -> Rc<RefCell<IRProcedure>>;
    fn from_ast_block(_: &CSTBlock, opts: &InputOpts) -> Rc<RefCell<IRProcedure>>;
}

impl CSTIRLower for IRCfg {
    fn from_cst(cst: &CSTBlock, opts: &InputOpts) -> IRCfg {
        let mut out = IRCfg {
            procedures: StableGraph::new(),
            main: Rc::new(RefCell::new(IRProcedure::default())),
        };

        out.main = procedure_new(
            cst,
            &CSTProcedureKind::Normal,
            &Vec::new(),
            &None,
            opts.disable_annotations,
            &mut out,
        );

        if opts.dump_ir_lower {
            ir_dump(&out, opts, "00-lower");
        }

        out
    }

    fn from_expr(expr: &CSTExpression, _opts: &InputOpts) -> Rc<RefCell<IRProcedure>> {
        /* t_ret := // expr
         * t_ret := copy(t_ret);
         * return r_ret;
         */
        let eval_proc = Rc::new(RefCell::new(IRProcedure {
            blocks: StableGraph::new(),
            start_block: NodeIndex::from(0),
            end_block: NodeIndex::from(0),
            vars: Vec::new(),
            tag: String::from("eval proc"),
        }));

        let mut eval_proc_shared = IRSharedProc::default();
        let mut eval_cfg = IRCfg {
            procedures: StableGraph::new(),
            main: eval_proc.clone(),
        };

        let mut init_idx = eval_proc.borrow_mut().blocks.add_node(Vec::new());
        let t_ret = tmp_var_new(&mut eval_proc.borrow_mut());
        let owned = block_expr_push(
            expr,
            &mut init_idx,
            IRTarget::Variable(t_ret),
            &mut eval_proc.borrow_mut(),
            &mut eval_proc_shared,
            &mut eval_cfg,
        );
        if !owned {
            block_get(&mut eval_proc.borrow_mut(), init_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_ret),
                types: IRTypes!("any"),
                source: IRValue::BuiltinProc(BuiltinProc::Copy),
                op: IROp::NativeCall(vec![IRValue::Variable(t_ret)]),
            }));
        }

        block_get(&mut eval_proc.borrow_mut(), init_idx)
            .push(IRStmt::Return(IRValue::Variable(t_ret)));
        eval_proc
    }

    fn from_stmt(cst: &CSTBlock, _opts: &InputOpts) -> Rc<RefCell<IRProcedure>> {
        /* // procedure vars aggregate
         * // stmt
         * t_ret := om;
         * goto <ret_idx>
         *
         * <ret_idx>:
         * return t_ret;
         */
        let eval_proc = Rc::new(RefCell::new(IRProcedure::from_tag("execute_proc")));

        let t_ret = tmp_var_new(&mut eval_proc.borrow_mut());

        let mut eval_proc_shared = IRSharedProc::default();

        let mut eval_cfg = IRCfg {
            procedures: StableGraph::new(),
            main: eval_proc.clone(),
        };

        let init_stmt = procedure_vars_aggregate(cst)
            .into_iter()
            .map(|i| {
                let t_ptr = tmp_var_new(&mut eval_proc.borrow_mut());
                eval_proc_shared.definitions.push((i.clone(), t_ptr));
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_ptr),
                    types: IRType::PTR,
                    source: IRValue::BuiltinProc(BuiltinProc::StackGetOrNew),
                    op: IROp::NativeCall(vec![IRValue::String(i)]),
                })
            })
            .collect::<Vec<_>>();
        let mut init_idx = eval_proc.borrow_mut().blocks.add_node(init_stmt);
        let ret_idx = eval_proc
            .borrow_mut()
            .blocks
            .add_node(vec![IRStmt::Return(IRValue::Variable(t_ret))]);
        let terminated = block_populate(
            &mut init_idx,
            cst,
            None,
            None,
            ret_idx,
            &mut eval_proc.borrow_mut(),
            &mut eval_proc_shared,
            &mut eval_cfg,
        );

        if !terminated {
            block_get(&mut eval_proc.borrow_mut(), init_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_ret),
                    types: IRType::UNDEFINED,
                    source: IRValue::Undefined,
                    op: IROp::Assign,
                }),
                IRStmt::Goto(ret_idx),
            ]);
        }

        eval_proc
    }

    fn from_ast_expr(cst: &CSTExpression, _opts: &InputOpts) -> Rc<RefCell<IRProcedure>> {
        /* t_ret := // block_cst_expr_push;
         * return t_ret;
         */
        let eval_proc = Rc::new(RefCell::new(IRProcedure::from_tag("ast_eval_proc")));

        let t_ret = tmp_var_new(&mut eval_proc.borrow_mut());

        let init_idx = eval_proc.borrow_mut().blocks.add_node(Vec::new());

        block_cst_expr_push(
            cst,
            init_idx,
            IRTarget::Variable(t_ret),
            &mut eval_proc.borrow_mut(),
        );
        block_get(&mut eval_proc.borrow_mut(), init_idx)
            .push(IRStmt::Return(IRValue::Variable(t_ret)));

        eval_proc.borrow_mut().start_block = init_idx;
        eval_proc.borrow_mut().end_block = init_idx;

        eval_proc
    }

    fn from_ast_block(cst: &CSTBlock, _opts: &InputOpts) -> Rc<RefCell<IRProcedure>> {
        /* t_ret := // block_cst_block_push;
         * return t_ret;
         */
        let eval_proc = Rc::new(RefCell::new(IRProcedure::from_tag("ast_eval_proc")));

        let t_ret = tmp_var_new(&mut eval_proc.borrow_mut());

        let init_idx = eval_proc.borrow_mut().blocks.add_node(Vec::new());

        block_cst_block_push(
            cst,
            init_idx,
            IRTarget::Variable(t_ret),
            &mut eval_proc.borrow_mut(),
        );
        block_get(&mut eval_proc.borrow_mut(), init_idx)
            .push(IRStmt::Return(IRValue::Variable(t_ret)));

        eval_proc.borrow_mut().start_block = init_idx;
        eval_proc.borrow_mut().end_block = init_idx;

        eval_proc
    }
}
