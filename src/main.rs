mod setlx_parse {
    include!(concat!(env!("OUT_DIR"), "/grammar.rs"));
}

mod cli;
use cli::cli;
mod cst;
use cst::cst_parse;
mod util;
use util::file_read;

fn main() {
    let opts = cli();

    let input = file_read(&opts.path);
    let _cst = cst_parse(&input, &opts);
}
