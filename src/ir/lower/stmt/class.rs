use petgraph::stable_graph::{NodeIndex, StableGraph};

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::proc::proc_params_push;
use crate::ir::lower::stmt::block_populate;
use crate::ir::lower::util::{block_get, tmp_var_new};

fn constructor_new(c: &CSTClass, cfg: &mut IRCfg) -> NodeIndex {
    /* _constructor:
     * <init_block>:
     *      _ := stack_frame_add();
     *      // params
     *      _ := stack_frame_add();
     *      goto <block>
     *      // block
     *
     *  <follow_block>
     *      t := stack_frame_save();
     *      _ := stack_frame_pop();
     *      o := object_new(c.name, t);
     *      return o;
     */
    let mut constructor_proc = IRProcedure {
        blocks: StableGraph::new(),
        start_block: NodeIndex::from(0),
        end_block: NodeIndex::from(0),
        vars: Vec::new(),
    };

    let mut constructor_shared = IRSharedProc {
        definitions: Vec::new(),
        ret_var: 0,
    };

    let mut init_idx = constructor_proc
        .blocks
        .add_node(vec![IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::StackFrameAdd),
            op: IROp::NativeCall(Vec::new()),
        })]);
    constructor_proc.start_block = init_idx;

    proc_params_push(
        &mut init_idx,
        &c.params,
        &None,
        &mut constructor_proc,
        &mut constructor_shared,
        cfg,
    );
    block_get(&mut constructor_proc, init_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Ignore,
        types: IRType::UNDEFINED,
        source: IRValue::BuiltinProc(BuiltinProc::StackFrameAdd),
        op: IROp::NativeCall(Vec::new()),
    }));

    let t_stack = tmp_var_new(&mut constructor_proc);
    let t_obj = tmp_var_new(&mut constructor_proc);
    let follow_idx = constructor_proc.blocks.add_node(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_stack),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::StackFrameSave),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::StackFramePop),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_obj),
            types: IRType::OBJECT,
            source: IRValue::BuiltinProc(BuiltinProc::ObjectNew),
            op: IROp::NativeCall(vec![
                IRValue::String(c.name.clone()),
                IRValue::Variable(t_stack),
            ]),
        }),
        IRStmt::Return(IRValue::Variable(t_obj)),
    ]);

    constructor_proc.end_block = follow_idx;

    block_populate(
        &mut init_idx,
        &c.block,
        None,
        None,
        follow_idx,
        &mut constructor_proc,
        &mut constructor_shared,
        cfg,
    );
    block_get(&mut constructor_proc, init_idx).push(IRStmt::Goto(follow_idx));
    constructor_proc.blocks.add_edge(init_idx, follow_idx, ());

    cfg.procedures.add_node(constructor_proc)
}

fn static_new(c: &CSTClass, cfg: &mut IRCfg) -> NodeIndex {
    /* _static:
     * <bb1>:
     *      _ := stack_frame_add();
     *      // static block
     *      t := stack_frame_save();
     *      _ := class_static_set(c.name, t);
     *      return om;
     */
    let mut static_proc = IRProcedure {
        blocks: StableGraph::new(),
        start_block: NodeIndex::from(0),
        end_block: NodeIndex::from(0),
        vars: Vec::new(),
    };

    let mut static_shared = IRSharedProc {
        definitions: Vec::new(),
        ret_var: 0,
    };

    let mut static_init_idx = static_proc.blocks.add_node(vec![IRStmt::Assign(IRAssign {
        target: IRTarget::Ignore,
        types: IRType::UNDEFINED,
        source: IRValue::BuiltinProc(BuiltinProc::StackFrameAdd),
        op: IROp::NativeCall(Vec::new()),
    })]);

    static_proc.start_block = static_init_idx;

    if let Some(static_block) = &c.static_block {
        let static_follow_idx = static_proc.blocks.add_node(Vec::new());
        let init_terminated = block_populate(
            &mut static_init_idx,
            static_block,
            None,
            None,
            static_follow_idx,
            &mut static_proc,
            &mut static_shared,
            cfg,
        );

        if !init_terminated {
            block_get(&mut static_proc, static_init_idx).push(IRStmt::Goto(static_follow_idx));
            static_proc.blocks.add_edge(static_init_idx, static_follow_idx, ());
            static_init_idx = static_follow_idx;
        }
    }

    let t_static_stack = tmp_var_new(&mut static_proc);
    block_get(&mut static_proc, static_init_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_static_stack),
            types: IRType::LIST,
            source: IRValue::BuiltinProc(BuiltinProc::StackFrameSave),
            op: IROp::NativeCall(Vec::new()),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Ignore,
            types: IRType::UNDEFINED,
            source: IRValue::BuiltinProc(BuiltinProc::ClassStaticSet),
            op: IROp::NativeCall(vec![
                IRValue::String(c.name.clone()),
                IRValue::Variable(t_static_stack),
            ]),
        }),
        IRStmt::Return(IRValue::Undefined),
    ]);

    static_proc.end_block = static_init_idx;

    cfg.procedures.add_node(static_proc)
}

pub fn block_class_push(
    c: &CSTClass,
    block_idx: &mut NodeIndex,
    proc: &mut IRProcedure,
    cfg: &mut IRCfg,
) {
    let constructor_idx = constructor_new(c, cfg);
    let static_idx = static_new(c, cfg);

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
