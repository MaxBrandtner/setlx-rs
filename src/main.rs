#![allow(clippy::mutable_key_type)]
#![allow(clippy::too_many_arguments)]

use gag::BufferRedirect;
use pretty_assertions::assert_eq;
use std::fs;
use std::io::Read;

mod setlx_parse {
    include!(concat!(env!("OUT_DIR"), "/grammar.rs"));
}

mod ast;
mod builtin;
mod cli;
use cli::cli;
mod cst;
use cst::cst_parse;
mod diagnostics;
mod interp;
use interp::exec::exec;
mod ir;
use ir::def::*;
use ir::lower::CSTIRLower;
mod util;
use util::file::file_read;

fn main() {
    let opts = cli();

    let input = if !opts.shell {
        file_read(&opts.path)
    } else {
        include_str!("shell.stlx").to_string()
    };

    let cst = cst_parse(&input, &opts);
    let ir = IRCfg::from_cst(&cst, &opts);

    if opts.dry_run {
        return;
    }

    if let Some(path) = &opts.diff_stdout {
        let reference = fs::read_to_string(path).unwrap();
        let mut buf = BufferRedirect::stdout().unwrap();
        exec(ir, &opts, input);

        let mut output = String::new();
        buf.read_to_string(&mut output).unwrap();

        assert_eq!(output, reference);
    } else {
        exec(ir, &opts, input);
    }
}
