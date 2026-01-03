mod assign;
mod ast;
mod expr;
mod iter;
mod proc;
mod stmt;
mod util;

use petgraph::stable_graph::{NodeIndex, StableGraph};

use crate::ast::*;
use crate::cli::InputOpts;
use crate::ir::def::*;
use crate::ir::dump::ir_dump;

use proc::procedure_new;

#[derive(Default)]
pub struct IRSharedProc {
    definitions: Vec<(String, IRVar)>,
    ret_var: IRVar,
}

pub trait CSTIRLower {
    fn from_cst(_: &CSTBlock, opts: &InputOpts) -> IRCfg;
}

impl CSTIRLower for IRCfg {
    fn from_cst(cst: &CSTBlock, opts: &InputOpts) -> IRCfg {
        let mut out = IRCfg {
            procedures: StableGraph::new(),
            main: NodeIndex::from(0),
            n_cached: 0,
        };

        out.main = procedure_new(cst, &CSTProcedureKind::Normal, &Vec::new(), &None, &mut out);

        if opts.dump_ir_lower {
            ir_dump(&out, opts, "00-lower");
        }

        out
    }
}
