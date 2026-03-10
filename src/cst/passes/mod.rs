use crate::ast::{CSTBlock, CSTExpression};
use crate::cli::InputOpts;

mod pass_check;
mod pass_noop;
mod pass_offset;
mod pass_string;
mod unescape;

use pass_check::CheckCtx;
use pass_string::StrCtx;

pub fn cst_passes(mut cst: CSTBlock, opts: &InputOpts, src: &str) -> CSTBlock {
    let mut pass_failed = false;
    let mut err_str = String::from("");
    let mut pass_num = 1;

    pass_string::pass(&mut cst, opts, src, &mut pass_failed, &mut err_str, pass_num);
    pass_num += 1;
    pass_check::pass(&cst, opts, src, &mut pass_failed, &mut err_str, pass_num);
    pass_num += 1;
    pass_noop::pass(&mut cst, opts, pass_num);

    if pass_failed {
        panic!("{err_str}");
    } else {
        eprint!("{err_str}");
    }

    cst
}

pub fn cst_expr_passes(mut cst: CSTExpression, opts: &InputOpts, src: &str) -> CSTExpression {
    let mut pass_failed = false;
    let mut err_str = String::from("");

    pass_string::pass_expr(&mut cst, &mut pass_failed, &StrCtx::new(src, opts), &mut err_str);
    pass_check::pass_expr(&cst, &mut pass_failed, &CheckCtx::new(src, opts), &mut err_str);
    pass_noop::pass_expr(&mut cst);
    if pass_failed {
        panic!("{err_str}");
    } else {
        eprint!("{err_str}");
    }

    cst
}
