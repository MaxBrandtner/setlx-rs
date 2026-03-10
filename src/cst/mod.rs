use crate::ast::{CSTBlock, CSTExpression};
use crate::cli::InputOpts;
use crate::diagnostics::report_parse_error;
use crate::setlx_parse;

mod dot;
mod dump;
use dump::cst_dump;
mod passes;
use passes::{cst_expr_passes, cst_passes};

pub fn cst_parse(input: &str, opts: &InputOpts) -> CSTBlock {
    let mut cst = match setlx_parse::BlockParser::new().parse(input) {
        Ok(c) => c,
        Err(e) => {
            let mut err_str = String::from("");
            report_parse_error(
                e,
                input,
                &opts.srcname,
                &mut err_str,
            );
            panic!("{err_str}");
        }
    };

    if opts.dump_cst_parse {
        cst_dump(&cst, opts, "00-parse");
    }

    cst = cst_passes(cst, opts, input);

    cst
}

pub fn cst_expr_parse(input: &str, opts: &InputOpts) -> CSTExpression {
    let mut cst = match setlx_parse::ExprParser::new().parse(input) {
        Ok(c) => c,
        Err(e) => {
            panic!("{:?}", e);
        }
    };

    cst = cst_expr_passes(cst, opts, input);

    cst
}
