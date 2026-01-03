use petgraph::stable_graph::NodeIndex;

use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;

pub fn block_get(proc: &mut IRProcedure, idx: NodeIndex) -> &mut IRBlock {
    proc.blocks.node_weight_mut(idx).unwrap()
}

pub fn stack_get(shared_proc: &mut IRSharedProc, v: &str) -> Option<IRVar> {
    for (name, param) in shared_proc.definitions.iter().rev() {
        if name == v {
            return Some(*param);
        }
    }

    None
}

pub fn stack_pop(shared_proc: &mut IRSharedProc, v: &str) {
    for (idx, (name, _)) in shared_proc.definitions.iter().rev().enumerate() {
        if name == v {
            shared_proc
                .definitions
                .remove(shared_proc.definitions.len() - 1 - idx);
            return;
        }
    }
    panic!("can't find variable on stack {v}");
}

pub fn tmp_var_new(proc: &mut IRProcedure) -> usize {
    let out = proc.vars.len();
    proc.vars.push(out);
    out
}
