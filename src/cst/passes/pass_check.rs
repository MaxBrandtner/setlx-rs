use ariadne::ReportKind;
use bitflags::bitflags;

use crate::ast::*;
use crate::cli::InputOpts;
use crate::cst::dump::cst_dump;
use crate::diagnostics::report;
use crate::ir::lower::expr::term_expr::tterm_ast_tag_get;

bitflags! {
    #[derive(Clone, Copy, Default)]
    struct CheckCtxFlags: u32 {
        const IS_LOOP                          = 1 << 0;
        const IS_ASSIGN_TARGET                 = 1 << 1;
        const IS_ASSIGN_IMPL_TARGET            = 1 << 2;
        const IS_ASSIGN_ACCESSIBLE_BODY_TARGET = 1 << 3;
        const IS_ACCESSIBLE_BODY               = 1 << 4;
    }
}

pub struct CheckCtx<'a> {
    src: &'a str,
    srcname: &'a str,
    lhs: usize,
    rhs: usize,
    warn_unresolved_tterm: bool,
    warn_unreachable_code: bool,
    flags: CheckCtxFlags,
}

impl<'a> CheckCtx<'a> {
    pub fn new(src: &'a str, opts: &'a InputOpts) -> Self {
        CheckCtx {
            src,
            srcname: &opts.srcname,
            lhs: 0,
            rhs: 0,
            warn_unresolved_tterm: opts.warn_unresolved_tterm,
            warn_unreachable_code: opts.warn_unreachable_code,
            flags: CheckCtxFlags::default(),
        }
    }

    fn reset_flags(&self) -> Self {
        CheckCtx {
            src: self.src,
            srcname: self.srcname,
            lhs: self.lhs,
            rhs: self.rhs,
            warn_unresolved_tterm: self.warn_unresolved_tterm,
            warn_unreachable_code: self.warn_unreachable_code,
            flags: CheckCtxFlags::default(),
        }
    }

    fn clear_flags(&self, flags: CheckCtxFlags) -> Self {
        CheckCtx {
            src: self.src,
            srcname: self.srcname,
            lhs: self.lhs,
            rhs: self.rhs,
            warn_unresolved_tterm: self.warn_unresolved_tterm,
            warn_unreachable_code: self.warn_unreachable_code,
            flags: self.flags & !flags,
        }
    }

    fn set_flags(&self, flags: CheckCtxFlags) -> Self {
        CheckCtx {
            src: self.src,
            srcname: self.srcname,
            lhs: self.lhs,
            rhs: self.rhs,
            warn_unresolved_tterm: self.warn_unresolved_tterm,
            warn_unreachable_code: self.warn_unreachable_code,
            flags: self.flags | flags,
        }
    }

    fn set_pos(&self, lhs: usize, rhs: usize) -> Self {
        CheckCtx {
            src: self.src,
            srcname: self.srcname,
            lhs,
            rhs,
            warn_unresolved_tterm: self.warn_unresolved_tterm,
            warn_unreachable_code: self.warn_unreachable_code,
            flags: self.flags,
        }
    }
}

fn pass_param(p: &CSTParam, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    if let Some(expr) = &p.default {
        pass_expr(expr, pass_failed, &ctx.reset_flags(), err_str);
    }
}

fn pass_class(c: &CSTClass, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    c.params
        .iter()
        .for_each(|i| pass_param(i, pass_failed, ctx, err_str));
    pass_block(&c.block, pass_failed, &ctx.reset_flags(), err_str);
    if let Some(s_block) = &c.static_block {
        pass_block(s_block, pass_failed, &ctx.reset_flags(), err_str);
    }
}

fn pass_if_branch(i: &CSTIfBranch, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    pass_expr(&i.condition, pass_failed, &ctx.reset_flags(), err_str);
    pass_block(&i.block, pass_failed, ctx, err_str);
}

fn pass_if(i: &CSTIf, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    i.branches
        .iter()
        .for_each(|i| pass_if_branch(i, pass_failed, ctx, err_str));
    if let Some(alt) = &i.alternative {
        pass_block(alt, pass_failed, ctx, err_str);
    }
}

fn pass_match_branch(
    m: &CSTMatchBranch,
    pass_failed: &mut bool,
    ctx: &CheckCtx,
    err_str: &mut String,
) {
    match m {
        CSTMatchBranch::Case(c) => {
            c.expressions.iter().for_each(|i| {
                pass_expr(
                    i,
                    pass_failed,
                    &ctx.set_flags(CheckCtxFlags::IS_ASSIGN_IMPL_TARGET),
                    err_str,
                )
            });
            if let Some(cond) = &c.condition {
                pass_expr(cond, pass_failed, ctx, err_str);
            }

            pass_block(&c.statements, pass_failed, ctx, err_str);
        }
        CSTMatchBranch::Regex(r) => {
            pass_expr(&r.pattern, pass_failed, ctx, err_str);
            if let Some(pattern_out) = &r.pattern_out {
                pass_expr(
                    pattern_out,
                    pass_failed,
                    &ctx.set_flags(
                        CheckCtxFlags::IS_ASSIGN_TARGET | CheckCtxFlags::IS_ASSIGN_IMPL_TARGET,
                    ),
                    err_str,
                );
            }
            if let Some(cond) = &r.condition {
                pass_expr(cond, pass_failed, ctx, err_str);
            }
            pass_block(&r.statements, pass_failed, ctx, err_str);
        }
    }
}

fn pass_match(m: &CSTMatch, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    pass_expr(&m.expression, pass_failed, ctx, err_str);
    m.branches
        .iter()
        .for_each(|i| pass_match_branch(i, pass_failed, ctx, err_str));
    pass_block(&m.default, pass_failed, ctx, err_str);
}

fn pass_scan(s: &CSTScan, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    pass_expr(&s.expression, pass_failed, ctx, err_str);
    s.branches
        .iter()
        .for_each(|i| pass_match_branch(i, pass_failed, ctx, err_str));
}

fn pass_iter_param(i: &CSTIterParam, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    if !matches!(
        i.variable.kind,
        CSTExpressionKind::Variable(_)
            | CSTExpressionKind::Collection(_)
            | CSTExpressionKind::Ignore
    ) {
        report(
            ReportKind::Error,
            "invalid expression",
            "Expected variable, list, _",
            ctx.lhs,
            ctx.rhs,
            ctx.src,
            ctx.srcname,
            err_str,
        );
        *pass_failed = true;
    }
    pass_expr(
        &i.variable,
        pass_failed,
        &ctx.set_flags(CheckCtxFlags::IS_ASSIGN_TARGET | CheckCtxFlags::IS_ASSIGN_IMPL_TARGET),
        err_str,
    );
    pass_expr(&i.collection, pass_failed, ctx, err_str);
}

fn pass_for(f: &CSTFor, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    f.params
        .iter()
        .for_each(|i| pass_iter_param(i, pass_failed, ctx, err_str));
    if let Some(cond) = &f.condition {
        pass_expr(cond, pass_failed, ctx, err_str);
    }

    pass_block(
        &f.block,
        pass_failed,
        &ctx.set_flags(CheckCtxFlags::IS_LOOP),
        err_str,
    );
}

fn pass_while(w: &CSTWhile, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    pass_expr(&w.condition, pass_failed, ctx, err_str);
    pass_block(
        &w.block,
        pass_failed,
        &ctx.set_flags(CheckCtxFlags::IS_LOOP),
        err_str,
    );
}

fn pass_try(t: &CSTTryCatch, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    pass_block(&t.try_branch, pass_failed, ctx, err_str);
    t.catch_branches
        .iter()
        .for_each(|i| pass_block(&i.block, pass_failed, ctx, err_str));
}

fn pass_check(c: &CSTCheck, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    pass_block(&c.block, pass_failed, ctx, err_str);
    pass_block(&c.after_backtrack, pass_failed, ctx, err_str);
}

fn pass_return(r: &CSTReturn, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    if let Some(val) = &r.val {
        pass_expr(val, pass_failed, ctx, err_str);
    }
}

fn pass_assign(a: &CSTAssign, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    pass_expr(
        &a.assign,
        pass_failed,
        &ctx.set_flags(CheckCtxFlags::IS_ASSIGN_TARGET | CheckCtxFlags::IS_ASSIGN_IMPL_TARGET),
        err_str,
    );
    pass_stmt(&a.expr, pass_failed, ctx, err_str);
}

fn pass_assign_mod(a: &CSTAssignMod, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    pass_expr(
        &a.assign,
        pass_failed,
        &ctx.set_flags(CheckCtxFlags::IS_ASSIGN_TARGET | CheckCtxFlags::IS_ASSIGN_IMPL_TARGET),
        err_str,
    );
    pass_expr(&a.expr, pass_failed, ctx, err_str);
}

fn pass_lambda(l: &CSTLambda, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    match &l.params {
        CSTCollection::List(ls) => {
            if ls.range.is_some() {
                report(
                    ReportKind::Error,
                    "invalid expression",
                    "Encountered unexpected range",
                    ctx.lhs,
                    ctx.rhs,
                    ctx.src,
                    ctx.srcname,
                    err_str,
                );
                *pass_failed = true;
            }

            if ls.rest.is_some() {
                report(
                    ReportKind::Error,
                    "invalid expression",
                    "Encountered unexpected list condition",
                    ctx.lhs,
                    ctx.rhs,
                    ctx.src,
                    ctx.srcname,
                    err_str,
                );
                *pass_failed = true;
            }

            for i in &ls.expressions {
                if !matches!(i.kind, CSTExpressionKind::Variable(_)) {
                    report(
                        ReportKind::Error,
                        "invalid expression",
                        "expected variable as lambda parameter",
                        ctx.lhs,
                        ctx.rhs,
                        ctx.src,
                        ctx.srcname,
                        err_str,
                    );
                    *pass_failed = true;
                }
            }
        }
        _ => {
            report(
                ReportKind::Error,
                "invalid expression",
                "expected list of lambda parameters",
                ctx.lhs,
                ctx.rhs,
                ctx.src,
                ctx.srcname,
                err_str,
            );
            *pass_failed = true;
        }
    }

    pass_collection(&l.params, pass_failed, ctx, err_str);
    pass_expr(&l.expr, pass_failed, ctx, err_str);
}

fn pass_op(o: &CSTExpressionOp, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    pass_expr(&o.left, pass_failed, ctx, err_str);
    pass_expr(&o.right, pass_failed, ctx, err_str);
}

fn pass_unary_op(
    o: &CSTExpressionUnaryOp,
    pass_failed: &mut bool,
    ctx: &CheckCtx,
    err_str: &mut String,
) {
    pass_expr(&o.expr, pass_failed, ctx, err_str);
}

fn pass_proc(p: &CSTProcedure, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    p.params
        .iter()
        .for_each(|i| pass_param(i, pass_failed, ctx, err_str));
    pass_block(&p.block, pass_failed, &ctx.reset_flags(), err_str);
}

fn pass_call(c: &CSTProcedureCall, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    c.params
        .iter()
        .for_each(|i| pass_expr(i, pass_failed, ctx, err_str));
    if let Some(rest) = &c.rest_param {
        pass_expr(rest, pass_failed, ctx, err_str);
    }
}

fn pass_term(t: &CSTTerm, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    if ctx.warn_unresolved_tterm
        && t.is_tterm
        && tterm_ast_tag_get(&t.name, t.params.len()).is_none()
    {
        report(
            ReportKind::Warning,
            "unresolved tterm",
            "tterm doesn't resolve to AST node",
            ctx.lhs,
            ctx.rhs,
            ctx.src,
            ctx.srcname,
            err_str,
        );
    }

    t.params
        .iter()
        .for_each(|i| pass_expr(i, pass_failed, &ctx.reset_flags(), err_str));
}

fn pass_accessible(
    a: &CSTAccessible,
    pass_failed: &mut bool,
    ctx: &CheckCtx,
    err_str: &mut String,
) {
    pass_expr(&a.head, pass_failed, ctx, err_str);
    a.body.iter().for_each(|i| {
        pass_expr(
            i,
            pass_failed,
            &ctx.clear_flags(
                CheckCtxFlags::IS_ASSIGN_TARGET | CheckCtxFlags::IS_ASSIGN_IMPL_TARGET,
            )
            .set_flags(
                CheckCtxFlags::IS_ACCESSIBLE_BODY
                    | if ctx.flags.contains(CheckCtxFlags::IS_ASSIGN_IMPL_TARGET) {
                        CheckCtxFlags::IS_ASSIGN_ACCESSIBLE_BODY_TARGET
                    } else {
                        CheckCtxFlags::default()
                    },
            ),
            err_str,
        )
    });
}

fn pass_range(r: &CSTRange, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    if let Some(lhs) = &r.left {
        pass_expr(lhs, pass_failed, &ctx.reset_flags(), err_str);
    }

    if let Some(rhs) = &r.right {
        pass_expr(rhs, pass_failed, &ctx.reset_flags(), err_str);
    }
}

fn pass_set(s: &CSTSet, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    if ctx.flags.contains(CheckCtxFlags::IS_ASSIGN_IMPL_TARGET) && s.range.is_some() {
        report(
            ReportKind::Error,
            "invalid expression",
            "ranges are not supported assign targets",
            ctx.lhs,
            ctx.rhs,
            ctx.src,
            ctx.srcname,
            err_str,
        );

        *pass_failed = true;
    }

    if ctx.flags.contains(CheckCtxFlags::IS_ASSIGN_TARGET) && s.rest.is_some() {
        report(
            ReportKind::Error,
            "invalid expression",
            "set conditions are not supported for assign targets",
            ctx.lhs,
            ctx.rhs,
            ctx.src,
            ctx.srcname,
            err_str,
        );

        *pass_failed = true;
    }

    if let Some(range) = &s.range {
        pass_range(range, pass_failed, &ctx.reset_flags(), err_str);
    }

    s.expressions.iter().for_each(|i| {
        if ctx.flags.contains(CheckCtxFlags::IS_ASSIGN_TARGET)
            && !matches!(
                i.kind,
                CSTExpressionKind::Variable(_)
                    | CSTExpressionKind::Accessible(_)
                    | CSTExpressionKind::Collection(_)
                    | CSTExpressionKind::Ignore
            )
        {
            report(
                ReportKind::Error,
                "invalid expression",
                "Expected variable, accessible, list, _",
                ctx.lhs,
                ctx.rhs,
                ctx.src,
                ctx.srcname,
                err_str,
            );

            *pass_failed = true;
        }

        pass_expr(i, pass_failed, &ctx.reset_flags(), err_str);
    });

    if let Some(rest) = &s.rest {
        pass_expr(rest, pass_failed, &ctx.reset_flags(), err_str);
    }
}

fn pass_comprehension(
    c: &CSTComprehension,
    pass_failed: &mut bool,
    ctx: &CheckCtx,
    err_str: &mut String,
) {
    pass_expr(&c.expression, pass_failed, &ctx.reset_flags(), err_str);
    c.iterators
        .iter()
        .for_each(|i| pass_iter_param(i, pass_failed, &ctx.reset_flags(), err_str));
    if let Some(cond) = &c.condition {
        pass_expr(cond, pass_failed, &ctx.reset_flags(), err_str);
    }
}

fn pass_collection(
    c: &CSTCollection,
    pass_failed: &mut bool,
    ctx: &CheckCtx,
    err_str: &mut String,
) {
    match c {
        CSTCollection::Set(s) => {
            if ctx.flags.contains(CheckCtxFlags::IS_ASSIGN_TARGET) {
                report(
                    ReportKind::Error,
                    "invalid expression",
                    "sets are not supported assign targets",
                    ctx.lhs,
                    ctx.rhs,
                    ctx.src,
                    ctx.srcname,
                    err_str,
                );
                *pass_failed = true;
            }

            if ctx
                .flags
                .contains(CheckCtxFlags::IS_ASSIGN_ACCESSIBLE_BODY_TARGET)
            {
                report(
                    ReportKind::Error,
                    "invalid expression",
                    "accessible body sets are not supported assignment targets",
                    ctx.lhs,
                    ctx.rhs,
                    ctx.src,
                    ctx.srcname,
                    err_str,
                );
                *pass_failed = true;
            }

            if ctx.flags.contains(CheckCtxFlags::IS_ACCESSIBLE_BODY) {
                if s.range.is_some() {
                    report(
                        ReportKind::Error,
                        "invalid expression",
                        "ranges are not supported for accessible body elements",
                        ctx.lhs,
                        ctx.rhs,
                        ctx.src,
                        ctx.srcname,
                        err_str,
                    );
                    *pass_failed = true;
                }

                if s.rest.is_some() {
                    report(
                        ReportKind::Error,
                        "invalid expression",
                        "accessible body elements must not contain conditions",
                        ctx.lhs,
                        ctx.rhs,
                        ctx.src,
                        ctx.srcname,
                        err_str,
                    );
                    *pass_failed = true;
                }

                if s.expressions.len() != 1 {
                    report(
                        ReportKind::Error,
                        "invalid expression",
                        "accessible body elements must be singleton lists",
                        ctx.lhs,
                        ctx.rhs,
                        ctx.src,
                        ctx.srcname,
                        err_str,
                    );
                    *pass_failed = true;
                }
            }

            pass_set(s, pass_failed, ctx, err_str);
        }
        CSTCollection::List(s) => {
            if ctx.flags.contains(CheckCtxFlags::IS_ACCESSIBLE_BODY) {
                if s.rest.is_some() {
                    report(
                        ReportKind::Error,
                        "invalid expression",
                        "accessible body elements must not contain conditions",
                        ctx.lhs,
                        ctx.rhs,
                        ctx.src,
                        ctx.srcname,
                        err_str,
                    );
                    *pass_failed = true;
                }
                if s.range.is_some() && !s.expressions.is_empty() {
                    report(
                        ReportKind::Error,
                        "invalid expression",
                        "accessible body element list with ranges must not contain additonal elements",
                        ctx.lhs,
                        ctx.rhs,
                        ctx.src,
                        ctx.srcname,
                        err_str,
                    );
                    *pass_failed = true;
                }
            }

            pass_set(s, pass_failed, ctx, err_str);
        }
        CSTCollection::ListComprehension(s) | CSTCollection::SetComprehension(s) => {
            if ctx.flags.contains(CheckCtxFlags::IS_ASSIGN_IMPL_TARGET) {
                report(
                    ReportKind::Error,
                    "invalid expression",
                    "assign targets are not supported for comprehensions",
                    ctx.lhs,
                    ctx.rhs,
                    ctx.src,
                    ctx.srcname,
                    err_str,
                );
                *pass_failed = true;
            }
            pass_comprehension(s, pass_failed, &ctx.reset_flags(), err_str);
        }
    }
}

fn pass_matrix(
    m: &[Vec<CSTExpression>],
    pass_failed: &mut bool,
    ctx: &CheckCtx,
    err_str: &mut String,
) {
    let m_len = if !m.is_empty() { m[0].len() } else { 0 };

    m.iter().for_each(|i| {
        if i.len() != m_len {
            report(
                ReportKind::Error,
                "invalid expression",
                "all matrix rows must be of equal length",
                ctx.lhs,
                ctx.rhs,
                ctx.src,
                ctx.srcname,
                err_str,
            );
            *pass_failed = true;
        }
        i.iter()
            .for_each(|j| pass_expr(j, pass_failed, &ctx.reset_flags(), err_str));
    });
}

fn pass_vector(m: &[CSTExpression], pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    m.iter()
        .for_each(|i| pass_expr(i, pass_failed, &ctx.reset_flags(), err_str));
}

fn pass_quant(q: &CSTQuantifier, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    q.iterators
        .iter()
        .for_each(|i| pass_iter_param(i, pass_failed, &ctx.reset_flags(), err_str));
    pass_expr(&q.condition, pass_failed, &ctx.reset_flags(), err_str);
}

pub fn pass_expr(e: &CSTExpression, pass_failed: &mut bool, ictx: &CheckCtx, err_str: &mut String) {
    let ctx = ictx.set_pos(e.lhs, e.rhs);

    if ctx.flags.contains(CheckCtxFlags::IS_ASSIGN_IMPL_TARGET)
        && !matches!(
            e.kind,
            CSTExpressionKind::Accessible(_)
                | CSTExpressionKind::Collection(_)
                | CSTExpressionKind::Variable(_)
                | CSTExpressionKind::Term(_)
                | CSTExpressionKind::Op(_)
                | CSTExpressionKind::UnaryOp(_)
                | CSTExpressionKind::Literal(_)
                | CSTExpressionKind::Bool(_)
                | CSTExpressionKind::Number(_)
                | CSTExpressionKind::Call(_)
                | CSTExpressionKind::Ignore
                | CSTExpressionKind::Om,
        )
    {
        report(
            ReportKind::Error,
            "invalid assignment",
            "cannot assign to expression",
            e.lhs,
            e.rhs,
            ctx.src,
            ctx.srcname,
            err_str,
        );
        *pass_failed = true;
    }

    match &e.kind {
        CSTExpressionKind::Lambda(l) => pass_lambda(l, pass_failed, &ctx, err_str),
        CSTExpressionKind::Op(o) => {
            if ctx.flags.contains(CheckCtxFlags::IS_ASSIGN_TARGET) {
                report(
                    ReportKind::Error,
                    "invalid assignment",
                    "op can't be regular assign target",
                    e.lhs,
                    e.rhs,
                    ctx.src,
                    ctx.srcname,
                    err_str,
                );
                *pass_failed = true;
            }
            pass_op(o, pass_failed, &ctx, err_str);
        }
        CSTExpressionKind::UnaryOp(o) => {
            if ctx.flags.contains(CheckCtxFlags::IS_ASSIGN_TARGET) {
                report(
                    ReportKind::Error,
                    "invalid assignment",
                    "op can't be regular assign target",
                    e.lhs,
                    e.rhs,
                    ctx.src,
                    ctx.srcname,
                    err_str,
                );
                *pass_failed = true;
            }

            pass_unary_op(o, pass_failed, &ctx, err_str);
        }
        CSTExpressionKind::Procedure(p) => pass_proc(p, pass_failed, &ctx, err_str),
        CSTExpressionKind::Call(c) => {
            if ctx.flags.contains(CheckCtxFlags::IS_ASSIGN_TARGET) {
                report(
                    ReportKind::Error,
                    "invalid assignment",
                    "term can't be regular assign target",
                    e.lhs,
                    e.rhs,
                    ctx.src,
                    ctx.srcname,
                    err_str,
                );

                *pass_failed = true;
            }

            pass_call(c, pass_failed, &ctx, err_str);
        }
        CSTExpressionKind::Term(t) => {
            if ctx.flags.contains(CheckCtxFlags::IS_ASSIGN_TARGET) {
                report(
                    ReportKind::Error,
                    "invalid assignment",
                    "term can't be regular assign target",
                    e.lhs,
                    e.rhs,
                    ctx.src,
                    ctx.srcname,
                    err_str,
                );
                *pass_failed = true;
            }

            pass_term(t, pass_failed, &ctx, err_str);
        }
        CSTExpressionKind::Accessible(a) => pass_accessible(a, pass_failed, &ctx, err_str),
        CSTExpressionKind::Collection(c) => pass_collection(c, pass_failed, &ctx, err_str),
        CSTExpressionKind::Matrix(m) => pass_matrix(m, pass_failed, &ctx, err_str),
        CSTExpressionKind::Vector(v) => pass_vector(v, pass_failed, &ctx, err_str),
        CSTExpressionKind::Quantifier(q) => pass_quant(q, pass_failed, &ctx, err_str),
        CSTExpressionKind::Literal(_)
        | CSTExpressionKind::Bool(_)
        | CSTExpressionKind::Number(_) => {
            if ctx.flags.contains(CheckCtxFlags::IS_ASSIGN_TARGET) {
                report(
                    ReportKind::Error,
                    "invalid assignment",
                    "expressions can't be regular assign target",
                    e.lhs,
                    e.rhs,
                    ctx.src,
                    ctx.srcname,
                    err_str,
                );
                *pass_failed = true;
            }
        }
        _ => (),
    }
}

fn pass_stmt(s: &CSTStatement, pass_failed: &mut bool, ictx: &CheckCtx, err_str: &mut String) {
    let ctx = ictx.set_pos(s.lhs, s.rhs);

    match &s.kind {
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
        CSTStatementKind::Expression(e) => pass_expr(e, pass_failed, &ctx.reset_flags(), err_str),
        CSTStatementKind::Break => {
            if !ctx.flags.contains(CheckCtxFlags::IS_LOOP) {
                report(
                    ReportKind::Error,
                    "invalid statement",
                    "encountered break statement outside of loop",
                    s.lhs,
                    s.rhs,
                    ctx.src,
                    ctx.srcname,
                    err_str,
                );
                *pass_failed = true;
            }
        }
        CSTStatementKind::Continue => {
            if !ctx.flags.contains(CheckCtxFlags::IS_LOOP) {
                report(
                    ReportKind::Error,
                    "invalid statement",
                    "encountered continue statement outside of loop",
                    s.lhs,
                    s.rhs,
                    ctx.src,
                    ctx.srcname,
                    err_str,
                );
                *pass_failed = true;
            }
        }
        _ => (),
    }
}

fn pass_block(cst: &CSTBlock, pass_failed: &mut bool, ctx: &CheckCtx, err_str: &mut String) {
    let mut prior_ret = false;
    cst.iter().for_each(|i| {
        if prior_ret && ctx.warn_unreachable_code {
            report(
                ReportKind::Warning,
                "unreachable code",
                "encountered statements in block after a terminating statement",
                i.lhs,
                i.rhs,
                ctx.src,
                ctx.srcname,
                err_str,
            );
        }

        prior_ret = matches!(
            i.kind,
            CSTStatementKind::Return(_)
                | CSTStatementKind::Break
                | CSTStatementKind::Continue
                | CSTStatementKind::Backtrack
        );

        pass_stmt(i, pass_failed, ctx, err_str);
    });
}
pub fn pass(
    cst: &CSTBlock,
    opts: &InputOpts,
    src: &str,
    pass_failed: &mut bool,
    err_str: &mut String,
    pass_num: u64,
) {
    /* - check lambda params
     * - check assignment targets are ignore, variables, or list
     * - check iterator variables are lists, variables or ignore
     * - check accessible body sets are singletons, and lists are ranges or singletons
     * - check break, continue only in for, while, doWhile
     * - check unreachable code
     *     - catch branches
     *     - duplicate match case expressions
     */
    let ctx = CheckCtx::new(src, opts);

    pass_block(cst, pass_failed, &ctx, err_str);

    if opts.dump_cst_pass_check {
        cst_dump(cst, opts, &format!("{pass_num}-check"));
    }
}
