use petgraph::stable_graph::{NodeIndex, StableGraph};
use std::cell::RefCell;
use std::rc::Rc;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::proc::{proc_params_push, proc_vars_push};
use crate::ir::lower::stmt::block_populate;
use crate::ir::lower::util::{block_get, tmp_var_new};

fn constructor_new(
    c: &CSTClass,
    shared_proc: &IRSharedProc,
    cfg: &mut IRCfg,
) -> Rc<RefCell<IRProcedure>> {
    /* _constructor:
     * <init_block>:
     *  /* source section {lhs} {rhs} */
     *  o := object_new(c.name);
     *  o_addr := &o;
     *  t_this := stack_alias("this", o_addr, true);
     *  _ := stack_frame_add();
     *  // procedure vars aggregate
     *  // params
     *  // block
     *
     * <follow_block>
     *  t := stack_frame_save();
     *  _ := stack_pop("this");
     *  o := *o_addr;
     *  _ := object_add_image(o, t);
     *  return o;
     */
    let constructor_proc = Rc::new(RefCell::new(IRProcedure {
        blocks: StableGraph::new(),
        start_block: NodeIndex::from(0),
        end_block: NodeIndex::from(0),
        vars: Vec::new(),
        tag: String::from(""),
    }));

    let mut constructor_shared = shared_proc.clone();

    let mut init_idx = constructor_proc.borrow_mut().blocks.add_node(vec![]);

    if !constructor_shared.disable_annotations {
        block_get(&mut constructor_proc.borrow_mut(), init_idx).push(IRStmt::Annotate(
            constructor_shared.code_lhs,
            constructor_shared.code_rhs,
        ));
    }

    let t_obj = tmp_var_new(&mut constructor_proc.borrow_mut());
    let t_obj_addr = tmp_var_new(&mut constructor_proc.borrow_mut());
    let t_this = tmp_var_new(&mut constructor_proc.borrow_mut());

    block_get(&mut constructor_proc.borrow_mut(), init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_obj),
            types: IRType::OBJECT,
            source: IRValue::BuiltinProc(BuiltinProc::ObjectNew),
            op: IROp::NativeCall(vec![IRValue::String(c.name.clone())]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_obj_addr),
            types: IRType::PTR,
            source: IRValue::Variable(t_obj),
            op: IROp::PtrAddress,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_this),
            types: IRType::PTR,
            source: IRValue::BuiltinProc(BuiltinProc::StackAlias),
            op: IROp::NativeCall(vec![
                IRValue::String(String::from("this")),
                IRValue::Variable(t_obj_addr),
                IRValue::Bool(true),
            ]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::StackFrameAdd),
            op: IROp::NativeCall(vec![]),
        }),
    ]);
    constructor_proc.borrow_mut().start_block = init_idx;

    proc_vars_push(
        &mut init_idx,
        &c.block,
        &mut constructor_proc.borrow_mut(),
        &mut constructor_shared,
    );

    proc_params_push(
        &mut init_idx,
        &c.params,
        &None,
        false /* constructor isn't a closure */,
        &mut constructor_proc.borrow_mut(),
        &mut constructor_shared,
        cfg,
    );

    let t_stack = tmp_var_new(&mut constructor_proc.borrow_mut());
    let follow_idx = constructor_proc.borrow_mut().blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_stack),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::StackFrameSave),
            op: IROp::NativeCall(vec![]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::StackPop),
            op: IROp::NativeCall(vec![IRValue::String(String::from("this"))]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_obj),
            types: IRTypes!("any"),
            source: IRValue::Variable(t_obj_addr),
            op: IROp::PtrDeref,
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ObjectAddImage),
            op: IROp::NativeCall(vec![IRValue::Variable(t_obj), IRValue::Variable(t_stack)]),
        }),
        IRStmt::Return(IRValue::Variable(t_obj)),
    ]);

    constructor_proc.borrow_mut().end_block = follow_idx;

    block_populate(
        &mut init_idx,
        &c.block,
        None,
        None,
        follow_idx,
        &mut constructor_proc.borrow_mut(),
        &mut constructor_shared,
        cfg,
    );
    block_get(&mut constructor_proc.borrow_mut(), init_idx).push(IRStmt::Goto(follow_idx));
    constructor_proc
        .borrow_mut()
        .blocks
        .add_edge(init_idx, follow_idx, ());

    let constructor_idx = cfg.procedures.add_node(constructor_proc.clone());
    constructor_proc.borrow_mut().tag = format!("proc{}", constructor_idx.index());
    constructor_proc
}

fn static_new(
    c: &CSTClass,
    shared_proc: &IRSharedProc,
    cfg: &mut IRCfg,
) -> Rc<RefCell<IRProcedure>> {
    /* _static:
     * <bb1>:
     *  /* source section {lhs} {rhs} */
     *  _ := stack_frame_add();
     *  // proc_vars_push
     *  // static block
     *  t := stack_frame_save();
     *  return t;
     */
    let static_proc = Rc::new(RefCell::new(IRProcedure {
        blocks: StableGraph::new(),
        start_block: NodeIndex::from(0),
        end_block: NodeIndex::from(0),
        vars: Vec::new(),
        tag: String::from(""),
    }));

    let mut static_shared = shared_proc.clone();

    let mut static_init_idx = static_proc.borrow_mut().blocks.add_node(vec![]);

    if !static_shared.disable_annotations {
        block_get(&mut static_proc.borrow_mut(), static_init_idx).push(IRStmt::Annotate(
            static_shared.code_lhs,
            static_shared.code_rhs,
        ));
    }
    block_get(&mut static_proc.borrow_mut(), static_init_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Ignore,
        types: IRType::UNDEFINED,
        source: IRValue::BuiltinProc(BuiltinProc::StackFrameAdd),
        op: IROp::NativeCall(Vec::new()),
    }));
    static_proc.borrow_mut().start_block = static_init_idx;

    if let Some(static_block) = &c.static_block {
        proc_vars_push(
            &mut static_init_idx,
            static_block,
            &mut static_proc.borrow_mut(),
            &mut static_shared,
        );

        let static_follow_idx = static_proc.borrow_mut().blocks.add_node(Vec::new());
        let init_terminated = block_populate(
            &mut static_init_idx,
            static_block,
            None,
            None,
            static_follow_idx,
            &mut static_proc.borrow_mut(),
            &mut static_shared,
            cfg,
        );

        if !init_terminated {
            block_get(&mut static_proc.borrow_mut(), static_init_idx)
                .push(IRStmt::Goto(static_follow_idx));
            static_proc
                .borrow_mut()
                .blocks
                .add_edge(static_init_idx, static_follow_idx, ());
            static_init_idx = static_follow_idx;
        }
    }

    let t_static_stack = tmp_var_new(&mut static_proc.borrow_mut());
    block_get(&mut static_proc.borrow_mut(), static_init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_static_stack),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::StackFrameSave),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Return(IRValue::Variable(t_static_stack)),
    ]);

    static_proc.borrow_mut().end_block = static_init_idx;

    let static_idx = cfg.procedures.add_node(static_proc.clone());
    static_proc.borrow_mut().tag = format!("proc{}", static_idx.index());
    static_proc
}

/// Emits IR to register the class `c` in the class store.
///
/// Compiles a static procedure from the class's static block and a constructor
/// procedure from the class body, then emits a `class_add` call to register
/// both under the class name at runtime.
pub fn block_class_push(
    c: &CSTClass,
    block_idx: &mut NodeIndex,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    let constructor_idx = constructor_new(c, shared_proc, cfg);
    let static_idx = static_new(c, shared_proc, cfg);

    // _ := class_add(c.name, _static, _constructor);
    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Ignore,
        types: IRType::UNDEFINED,
        source: IRValue::BuiltinProc(BuiltinProc::ClassAdd),
        op: IROp::NativeCall(vec![
            IRValue::String(c.name.clone()),
            IRValue::Procedure(static_idx),
            IRValue::Procedure(constructor_idx),
        ]),
    }));
}
