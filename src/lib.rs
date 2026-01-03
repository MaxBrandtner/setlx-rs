#![allow(clippy::too_many_arguments)]

pub mod setlx_parse {
    include!(concat!(env!("OUT_DIR"), "/grammar.rs"));
}

pub mod ast;
pub mod builtin;
pub mod cli;
pub mod cst;
pub mod ir;
mod util;
