use ariadne::ReportKind;

use crate::ast::*;
use crate::cli::InputOpts;
use crate::cst::dump::cst_dump;
use crate::cst::passes::pass_offset;
use crate::cst::passes::unescape::unescape;
use crate::diagnostics::{parse_err_add_offset, report, report_parse_error};
use crate::setlx_parse;

pub struct StrCtx<'a> {
    pub src: &'a str,
    pub srcname: &'a str,
    pub lhs: usize,
    pub rhs: usize,
    pub warn_invalid_backslash: bool,
}

impl<'a> StrCtx<'a> {
    pub fn new(src: &'a str, opts: &'a InputOpts) -> Self {
        StrCtx {
            src,
            srcname: &opts.srcname,
            lhs: 0,
            rhs: 0,
            warn_invalid_backslash: opts.warn_invalid_backslash,
        }
    }

    fn set_pos(&self, lhs: usize, rhs: usize) -> Self {
        StrCtx {
            src: self.src,
            srcname: self.srcname,
            lhs,
            rhs,
            warn_invalid_backslash: self.warn_invalid_backslash,
        }
    }
}

fn pass_param(p: &mut CSTParam, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    p.default
        .iter_mut()
        .for_each(|i| pass_expr(i, pass_failed, ctx, err_str));
}

fn pass_class(c: &mut CSTClass, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    c.params
        .iter_mut()
        .for_each(|i| pass_param(i, pass_failed, ctx, err_str));
    pass_block(&mut c.block, pass_failed, ctx, err_str);
    c.static_block
        .iter_mut()
        .for_each(|i| pass_block(i, pass_failed, ctx, err_str));
}

fn pass_if_branch(i: &mut CSTIfBranch, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    pass_expr(&mut i.condition, pass_failed, ctx, err_str);
    pass_block(&mut i.block, pass_failed, ctx, err_str);
}

fn pass_if(i: &mut CSTIf, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    i.branches
        .iter_mut()
        .for_each(|i| pass_if_branch(i, pass_failed, ctx, err_str));
    i.alternative
        .iter_mut()
        .for_each(|i| pass_block(i, pass_failed, ctx, err_str));
}

fn pass_match_branch(
    m: &mut CSTMatchBranch,
    pass_failed: &mut bool,
    ctx: &StrCtx,
    err_str: &mut String,
) {
    match m {
        CSTMatchBranch::Case(c) => {
            c.expressions
                .iter_mut()
                .for_each(|i| pass_expr(i, pass_failed, ctx, err_str));
            c.condition
                .iter_mut()
                .for_each(|i| pass_expr(i, pass_failed, ctx, err_str));
            pass_block(&mut c.statements, pass_failed, ctx, err_str);
        }
        CSTMatchBranch::Regex(r) => {
            pass_expr(&mut r.pattern, pass_failed, ctx, err_str);
            r.pattern_out
                .iter_mut()
                .for_each(|i| pass_expr(i, pass_failed, ctx, err_str));
            r.condition
                .iter_mut()
                .for_each(|i| pass_expr(i, pass_failed, ctx, err_str));
            pass_block(&mut r.statements, pass_failed, ctx, err_str);
        }
    }
}

fn pass_match(m: &mut CSTMatch, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    pass_expr(&mut m.expression, pass_failed, ctx, err_str);
    m.branches
        .iter_mut()
        .for_each(|i| pass_match_branch(i, pass_failed, ctx, err_str));
    pass_block(&mut m.default, pass_failed, ctx, err_str);
}

fn pass_scan(s: &mut CSTScan, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    pass_expr(&mut s.expression, pass_failed, ctx, err_str);
    s.branches
        .iter_mut()
        .for_each(|i| pass_match_branch(i, pass_failed, ctx, err_str));
}

fn pass_iter_param(
    i: &mut CSTIterParam,
    pass_failed: &mut bool,
    ctx: &StrCtx,
    err_str: &mut String,
) {
    pass_expr(&mut i.variable, pass_failed, ctx, err_str);
    pass_expr(&mut i.collection, pass_failed, ctx, err_str);
}

fn pass_for(f: &mut CSTFor, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    f.params
        .iter_mut()
        .for_each(|i| pass_iter_param(i, pass_failed, ctx, err_str));
    f.condition
        .iter_mut()
        .for_each(|i| pass_expr(&mut *i, pass_failed, ctx, err_str));
    pass_block(&mut f.block, pass_failed, ctx, err_str);
}

fn pass_while(w: &mut CSTWhile, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    pass_expr(&mut w.condition, pass_failed, ctx, err_str);
    pass_block(&mut w.block, pass_failed, ctx, err_str);
}

fn pass_catch(c: &mut CSTCatch, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    pass_block(&mut c.block, pass_failed, ctx, err_str);
}

fn pass_try(t: &mut CSTTryCatch, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    pass_block(&mut t.try_branch, pass_failed, ctx, err_str);
    t.catch_branches
        .iter_mut()
        .for_each(|i| pass_catch(i, pass_failed, ctx, err_str));
}

fn pass_check(c: &mut CSTCheck, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    pass_block(&mut c.block, pass_failed, ctx, err_str);
    pass_block(&mut c.after_backtrack, pass_failed, ctx, err_str);
}

fn pass_return(r: &mut CSTReturn, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    r.val
        .iter_mut()
        .for_each(|i| pass_expr(i, pass_failed, ctx, err_str));
}

fn pass_assign(a: &mut CSTAssign, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    pass_expr(&mut a.assign, pass_failed, ctx, err_str);
    pass_stmt(&mut a.expr, pass_failed, ctx, err_str);
}

fn pass_assign_mod(
    a: &mut CSTAssignMod,
    pass_failed: &mut bool,
    ctx: &StrCtx,
    err_str: &mut String,
) {
    pass_expr(&mut a.assign, pass_failed, ctx, err_str);
    pass_expr(&mut a.expr, pass_failed, ctx, err_str);
}

fn pass_lambda(l: &mut CSTLambda, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    pass_collection(&mut l.params, pass_failed, ctx, err_str);
    pass_expr(&mut l.expr, pass_failed, ctx, err_str);
}

fn pass_op(o: &mut CSTExpressionOp, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    pass_expr(&mut o.left, pass_failed, ctx, err_str);
    pass_expr(&mut o.right, pass_failed, ctx, err_str);
}

fn pass_unary_op(
    o: &mut CSTExpressionUnaryOp,
    pass_failed: &mut bool,
    ctx: &StrCtx,
    err_str: &mut String,
) {
    pass_expr(&mut o.expr, pass_failed, ctx, err_str);
}

fn pass_proc(p: &mut CSTProcedure, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    p.params
        .iter_mut()
        .for_each(|i| pass_param(i, pass_failed, ctx, err_str));
    pass_block(&mut p.block, pass_failed, ctx, err_str);
}

fn pass_call(c: &mut CSTProcedureCall, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    c.params
        .iter_mut()
        .for_each(|i| pass_expr(i, pass_failed, ctx, err_str));
    c.rest_param
        .iter_mut()
        .for_each(|i| pass_expr(&mut *i, pass_failed, ctx, err_str));
}

fn pass_term(t: &mut CSTTerm, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    t.params
        .iter_mut()
        .for_each(|i| pass_expr(i, pass_failed, ctx, err_str));
}

fn pass_accessible(
    a: &mut CSTAccessible,
    pass_failed: &mut bool,
    ctx: &StrCtx,
    err_str: &mut String,
) {
    pass_expr(&mut a.head, pass_failed, ctx, err_str);
    a.body
        .iter_mut()
        .for_each(|i| pass_expr(i, pass_failed, ctx, err_str));
}

fn pass_range(r: &mut CSTRange, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    r.left
        .iter_mut()
        .for_each(|i| pass_expr(&mut *i, pass_failed, ctx, err_str));
    r.right
        .iter_mut()
        .for_each(|i| pass_expr(&mut *i, pass_failed, ctx, err_str));
}

fn pass_set(s: &mut CSTSet, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    s.range
        .iter_mut()
        .for_each(|i| pass_range(i, pass_failed, ctx, err_str));
    s.expressions
        .iter_mut()
        .for_each(|i| pass_expr(i, pass_failed, ctx, err_str));
    s.rest
        .iter_mut()
        .for_each(|i| pass_expr(i, pass_failed, ctx, err_str));
}

fn pass_comprehension(
    c: &mut CSTComprehension,
    pass_failed: &mut bool,
    ctx: &StrCtx,
    err_str: &mut String,
) {
    pass_expr(&mut c.expression, pass_failed, ctx, err_str);
    c.condition
        .iter_mut()
        .for_each(|i| pass_expr(&mut *i, pass_failed, ctx, err_str));
    c.iterators
        .iter_mut()
        .for_each(|i| pass_iter_param(i, pass_failed, ctx, err_str));
}

fn pass_collection(
    c: &mut CSTCollection,
    pass_failed: &mut bool,
    ctx: &StrCtx,
    err_str: &mut String,
) {
    match c {
        CSTCollection::Set(s) => pass_set(s, pass_failed, ctx, err_str),
        CSTCollection::List(s) => pass_set(s, pass_failed, ctx, err_str),
        CSTCollection::SetComprehension(s) => pass_comprehension(s, pass_failed, ctx, err_str),
        CSTCollection::ListComprehension(s) => pass_comprehension(s, pass_failed, ctx, err_str),
    }
}

fn pass_matrix(
    m: &mut [Vec<CSTExpression>],
    pass_failed: &mut bool,
    ctx: &StrCtx,
    err_str: &mut String,
) {
    m.iter_mut().for_each(|i| {
        i.iter_mut()
            .for_each(|j| pass_expr(j, pass_failed, ctx, err_str))
    });
}

fn pass_vector(
    m: &mut [CSTExpression],
    pass_failed: &mut bool,
    ctx: &StrCtx,
    err_str: &mut String,
) {
    m.iter_mut()
        .for_each(|i| pass_expr(i, pass_failed, ctx, err_str));
}

fn pass_quant(q: &mut CSTQuantifier, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    q.iterators
        .iter_mut()
        .for_each(|i| pass_iter_param(i, pass_failed, ctx, err_str));
    pass_expr(&mut q.condition, pass_failed, ctx, err_str);
}

pub fn pass_expr(
    e: &mut CSTExpression,
    pass_failed: &mut bool,
    ictx: &StrCtx,
    err_str: &mut String,
) {
    let ctx = ictx.set_pos(e.lhs, e.rhs);

    match &mut e.kind {
        CSTExpressionKind::Lambda(l) => pass_lambda(l, pass_failed, &ctx, err_str),
        CSTExpressionKind::Op(o) => pass_op(o, pass_failed, &ctx, err_str),
        CSTExpressionKind::UnaryOp(o) => pass_unary_op(o, pass_failed, &ctx, err_str),
        CSTExpressionKind::Procedure(p) => pass_proc(p, pass_failed, &ctx, err_str),
        CSTExpressionKind::Call(c) => pass_call(c, pass_failed, &ctx, err_str),
        CSTExpressionKind::Term(t) => pass_term(t, pass_failed, &ctx, err_str),
        CSTExpressionKind::Accessible(a) => pass_accessible(a, pass_failed, &ctx, err_str),
        CSTExpressionKind::Collection(c) => pass_collection(c, pass_failed, &ctx, err_str),
        CSTExpressionKind::Matrix(m) => pass_matrix(m, pass_failed, &ctx, err_str),
        CSTExpressionKind::Vector(v) => pass_vector(v, pass_failed, &ctx, err_str),
        CSTExpressionKind::Quantifier(q) => pass_quant(q, pass_failed, &ctx, err_str),
        CSTExpressionKind::String(s) => {
            fn split_on_dollar(input: &str) -> Result<Vec<(usize, usize, String)>, usize> {
                enum State {
                    Normal,
                    Inside,
                }

                let mut state = State::Normal;
                let mut result: Vec<(usize, usize, String)> = Vec::new();
                let mut current = String::new();
                let mut sgmt_start: usize = 0;
                let mut c_len: usize = 0;

                for (idx, c) in input.char_indices() {
                    match state {
                        State::Normal => match c {
                            '$' => {
                                if current.ends_with('\\') {
                                    current.pop();
                                    current.push('$');
                                } else {
                                    result.push((sgmt_start, idx, current));
                                    current = String::new();
                                    c_len = c.len_utf8();
                                    sgmt_start = idx + c_len;
                                    state = State::Inside;
                                }
                            }
                            _ => current.push(c),
                        },
                        State::Inside => match c {
                            '$' => {
                                if current.ends_with('\\') {
                                    current.pop();
                                    current.push('$');
                                } else {
                                    result.push((sgmt_start, idx, current));
                                    current = String::new();
                                    c_len = c.len_utf8();
                                    sgmt_start = idx + c_len;
                                    state = State::Normal;
                                }
                            }
                            _ => current.push(c),
                        },
                    }
                }

                if matches!(state, State::Inside) {
                    return Err(sgmt_start - c_len);
                }

                if !current.is_empty() {
                    result.push((sgmt_start, input.len(), current));
                }

                if result.is_empty() {
                    let len = input.len();
                    result.push((len, len, String::from("")));
                }

                Ok(result)
            }

            let v = match split_on_dollar(&unescape(&s[1..s.len() - 1], &ctx, err_str)) {
                Ok(v) => v,
                Err(rhs) => {
                    report(
                        ReportKind::Error,
                        "parse error",
                        "missing closing '$'",
                        ctx.lhs,
                        rhs,
                        ctx.src,
                        ctx.srcname,
                        err_str,
                    );
                    *pass_failed = true;
                    Vec::new()
                }
            };

            let mut out = CSTExpression {
                lhs: ctx.lhs,
                rhs: ctx.rhs,
                kind: CSTExpressionKind::Om,
            };

            for (idx, i) in v.into_iter().enumerate() {
                if idx == 0 {
                    out = CSTExpression {
                        lhs: i.0,
                        rhs: i.1,
                        kind: CSTExpressionKind::Literal(i.2),
                    }
                } else if idx % 2 == 0 {
                    out = CSTExpression {
                        lhs: out.lhs,
                        rhs: i.1,
                        kind: CSTExpressionKind::Op(CSTExpressionOp {
                            op: CSTOp::Plus,
                            left: Box::new(out),
                            right: Box::new(CSTExpression {
                                lhs: i.0,
                                rhs: i.1,
                                kind: CSTExpressionKind::Literal(i.2),
                            }),
                        }),
                    };
                } else {
                    let expr = match setlx_parse::ExprParser::new().parse(&i.2) {
                        Ok(mut e) => {
                            pass_offset::pass_expr(&mut e, i.0 + 1);
                            pass_expr(&mut e, pass_failed, &ctx, err_str);
                            e
                        }
                        Err(mut e) => {
                            parse_err_add_offset(&mut e, i.0 + 1);
                            report_parse_error(e, &i.2, ctx.srcname, err_str);
                            *pass_failed = true;
                            CSTExpression {
                                lhs: i.0,
                                rhs: i.1,
                                kind: CSTExpressionKind::Om,
                            }
                        }
                    };
                    out = CSTExpression {
                        lhs: i.0,
                        rhs: i.1,
                        kind: CSTExpressionKind::Op(CSTExpressionOp {
                            op: CSTOp::Plus,
                            left: Box::new(out),
                            right: Box::new(CSTExpression {
                                lhs: expr.lhs,
                                rhs: expr.rhs,
                                kind: CSTExpressionKind::Serialize(Box::new(expr)),
                            }),
                        }),
                    };
                }
            }

            *e = out;
        }
        CSTExpressionKind::Literal(l) => {
            fn unescape_quotes(s: &str) -> String {
                let mut out = String::with_capacity(s.len());
                let mut chars = s.chars().peekable();

                while let Some(c) = chars.next() {
                    if c == '\'' && matches!(chars.peek(), Some('\'')) {
                        let _ = chars.next();
                    }
                    out.push(c);
                }

                out
            }
            *l = unescape_quotes(&l[1..l.len() - 1]);
        }
        _ => (),
    }
}

fn pass_stmt(s: &mut CSTStatement, pass_failed: &mut bool, ictx: &StrCtx, err_str: &mut String) {
    let ctx = ictx.set_pos(s.lhs, s.rhs);

    match &mut s.kind {
        CSTStatementKind::Class(c) => pass_class(c, pass_failed, &ctx, err_str),
        CSTStatementKind::If(i) => pass_if(i, pass_failed, &ctx, err_str),
        CSTStatementKind::Switch(i) => pass_if(i, pass_failed, &ctx, err_str),
        CSTStatementKind::Match(m) => pass_match(m, pass_failed, &ctx, err_str),
        CSTStatementKind::Scan(s) => pass_scan(s, pass_failed, &ctx, err_str),
        CSTStatementKind::For(f) => pass_for(f, pass_failed, &ctx, err_str),
        CSTStatementKind::DoWhile(w) => pass_while(w, pass_failed, &ctx, err_str),
        CSTStatementKind::While(w) => pass_while(w, pass_failed, &ctx, err_str),
        CSTStatementKind::TryCatch(t) => pass_try(t, pass_failed, &ctx, err_str),
        CSTStatementKind::Check(c) => pass_check(c, pass_failed, &ctx, err_str),
        CSTStatementKind::Return(r) => pass_return(r, pass_failed, &ctx, err_str),
        CSTStatementKind::Assign(a) => pass_assign(a, pass_failed, &ctx, err_str),
        CSTStatementKind::AssignMod(a) => pass_assign_mod(a, pass_failed, &ctx, err_str),
        CSTStatementKind::Expression(e) => pass_expr(e, pass_failed, &ctx, err_str),
        _ => (),
    }
}

fn pass_block(cst: &mut CSTBlock, pass_failed: &mut bool, ctx: &StrCtx, err_str: &mut String) {
    cst.iter_mut()
        .for_each(|i| pass_stmt(i, pass_failed, ctx, err_str));
}

pub fn pass(
    cst: &mut CSTBlock,
    opts: &InputOpts,
    src: &str,
    pass_failed: &mut bool,
    err_str: &mut String,
    pass_num: u64,
) {
    /* - literal formatting
     * - convert strings to literals
     */
    let ctx = StrCtx::new(src, opts);

    pass_block(cst, pass_failed, &ctx, err_str);

    if opts.dump_cst_pass_string {
        cst_dump(cst, opts, &format!("{pass_num}-string"));
    }
}
