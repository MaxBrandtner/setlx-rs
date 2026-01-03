#![allow(clippy::too_many_arguments)]

mod setlx_parse {
    include!(concat!(env!("OUT_DIR"), "/grammar.rs"));
}

mod ast;
mod builtin;
mod cli;
use cli::cli;
mod cst;
use cst::cst_parse;
mod ir;
use ir::def::*;
use ir::lower::CSTIRLower;
mod util;
use util::file::file_read;

fn main() {
    let opts = cli();

    let input = file_read(&opts.path);
    let cst = cst_parse(&input, &opts);
    let _ir = IRCfg::from_cst(&cst, &opts);
}
