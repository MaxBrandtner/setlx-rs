use crate::ast::CSTBlock;
use crate::cli::InputOpts;

mod pass_check;
mod pass_noop;
mod pass_string;

pub fn cst_passes(mut cst: CSTBlock, opts: &InputOpts) -> CSTBlock {
    let mut pass_num = 1;

    cst = pass_string::pass(cst, opts, pass_num);
    pass_num += 1;
    cst = pass_check::pass(cst, opts, pass_num);
    pass_num += 1;
    cst = pass_noop::pass(cst, opts, pass_num);

    cst
}
