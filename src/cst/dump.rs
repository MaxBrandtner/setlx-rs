use dot::render;
use std::io::Write;

use crate::ast::CSTBlock;
use crate::cli::InputOpts;
use crate::cst::dot::CSTGraph;
use crate::util::file::debug_file_create;

pub fn cst_dump(cst: &CSTBlock, opts: &InputOpts, pass_name: &str) {
    let mut file = debug_file_create(format!("{}-cst-{pass_name}.dump", &opts.stem));
    writeln!(&mut file, "{:?}", cst).unwrap();

    let graph = CSTGraph::new(cst);
    let mut file = debug_file_create(format!("{}-cst-{pass_name}.dot", &opts.stem));
    render(&graph, &mut file).unwrap();
}
