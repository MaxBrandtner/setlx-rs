use crate::ast::CSTBlock;
use crate::cli::InputOpts;
use crate::setlx_parse;

mod dot;
mod dump;
use dump::cst_dump;
mod passes;
use passes::cst_passes;

pub fn cst_parse(input: &str, opts: &InputOpts) -> CSTBlock {
    let mut cst = match setlx_parse::BlockParser::new().parse(input) {
        Ok(c) => c,
        Err(e) => {
            // TODO implement proper error handling
            panic!("{:?}", e);
        }
    };

    if opts.dump_cst_parse {
        cst_dump(&cst, opts, "00-parse");
    }

    cst = cst_passes(cst, opts);

    cst
}
