#![allow(clippy::too_many_arguments)]
#![allow(clippy::mutable_key_type)]

pub mod setlx_parse {
    include!(concat!(env!("OUT_DIR"), "/grammar.rs"));
}

pub mod ast;
pub mod builtin;
mod diagnostics;
pub mod cli;
pub mod cst;
pub mod interp;
pub mod ir;
mod util;
