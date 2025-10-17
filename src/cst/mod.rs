use crate::cli::InputOpts;
use crate::setlx_parse;
use crate::util::debug_file_create;
use setlx_rs::ast::CSTBlock;
use std::io::Write;

mod dot;
use dot::cst_dump_dot;

pub fn cst_parse(input: &str, opts: &InputOpts) -> CSTBlock {
    let cst = match setlx_parse::BlockParser::new().parse(input) {
        Ok(c) => c,
        Err(e) => {
            // TODO implement proper error handling
            panic!("{:?}", e);
        }
    };

    if opts.dump_cst_struct || opts.dump_cst_all || opts.dump_all {
        let mut file = debug_file_create(format!("{}-cst-struct.dump", &opts.stem));
        writeln!(&mut file, "{:?}", cst).unwrap();
    }

    if opts.dump_cst_dot || opts.dump_cst_all || opts.dump_all {
        cst_dump_dot(&cst, &opts.stem);
    }

    cst
}
