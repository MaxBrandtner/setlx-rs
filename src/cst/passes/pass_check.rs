use crate::ast::*;
use crate::cli::InputOpts;
use crate::cst::dump::cst_dump;

fn pass_param(p: CSTParam, pass_failed: &mut bool) -> CSTParam {
    CSTParam {
        name: p.name,
        is_rw: p.is_rw,
        default: p.default.map(|i| pass_expr(i, pass_failed, false, false)),
    }
}

fn pass_class(c: CSTClass, pass_failed: &mut bool) -> CSTClass {
    CSTClass {
        name: c.name,
        params: c
            .params
            .into_iter()
            .map(|i| pass_param(i, pass_failed))
            .collect(),
        block: pass_block(c.block, pass_failed, false),
        static_block: c.static_block.map(|s| pass_block(s, pass_failed, false)),
    }
}

fn pass_if_branch(i: CSTIfBranch, pass_failed: &mut bool, is_loop: bool) -> CSTIfBranch {
    CSTIfBranch {
        condition: pass_expr(i.condition, pass_failed, false, false),
        block: pass_block(i.block, pass_failed, is_loop),
    }
}

fn pass_if(i: CSTIf, pass_failed: &mut bool, is_loop: bool) -> CSTIf {
    CSTIf {
        branches: i
            .branches
            .into_iter()
            .map(|i| pass_if_branch(i, pass_failed, is_loop))
            .collect(),
        alternative: i.alternative.map(|a| pass_block(a, pass_failed, is_loop)),
    }
}

fn pass_match_branch(m: CSTMatchBranch, pass_failed: &mut bool, is_loop: bool) -> CSTMatchBranch {
    match m {
        CSTMatchBranch::Case(c) => CSTMatchBranch::Case(CSTMatchBranchCase {
            expressions: c
                .expressions
                .into_iter()
                .map(|i| pass_expr(i, pass_failed, false, false))
                .collect(),
            condition: c.condition.map(|i| pass_expr(i, pass_failed, false, false)),
            statements: pass_block(c.statements, pass_failed, is_loop),
        }),
        CSTMatchBranch::Regex(r) => CSTMatchBranch::Regex(CSTMatchBranchRegex {
            pattern: pass_expr(r.pattern, pass_failed, false, false),
            pattern_out: r
                .pattern_out
                .map(|i| pass_expr(i, pass_failed, false, false)),
            condition: r.condition.map(|i| pass_expr(i, pass_failed, false, false)),
            statements: pass_block(r.statements, pass_failed, is_loop),
        }),
    }
}

fn pass_match(m: CSTMatch, pass_failed: &mut bool, is_loop: bool) -> CSTMatch {
    CSTMatch {
        expression: pass_expr(m.expression, pass_failed, false, false),
        branches: m
            .branches
            .into_iter()
            .map(|i| pass_match_branch(i, pass_failed, is_loop))
            .collect(),
        default: pass_block(m.default, pass_failed, is_loop),
    }
}

fn pass_scan(s: CSTScan, pass_failed: &mut bool, is_loop: bool) -> CSTScan {
    CSTScan {
        expression: pass_expr(s.expression, pass_failed, false, false),
        variable: s.variable,
        branches: s
            .branches
            .into_iter()
            .map(|i| pass_match_branch(i, pass_failed, is_loop))
            .collect(),
    }
}

fn pass_iter_param(i: CSTIterParam, pass_failed: &mut bool) -> CSTIterParam {
    if !matches!(
        i.variable,
        CSTExpression::Variable(_) | CSTExpression::Collection(_) | CSTExpression::Ignore
    ) {
        panic!("iterator variable must be variable, list, or ignore");
    }
    CSTIterParam {
        variable: pass_expr(i.variable, pass_failed, true, false),
        collection: pass_expr(i.collection, pass_failed, false, false),
    }
}

fn pass_for(f: CSTFor, pass_failed: &mut bool) -> CSTFor {
    CSTFor {
        params: f
            .params
            .into_iter()
            .map(|i| pass_iter_param(i, pass_failed))
            .collect(),
        condition: f
            .condition
            .map(|c| Box::new(pass_expr(*c, pass_failed, false, false))),
        block: pass_block(f.block, pass_failed, true),
    }
}

fn pass_while(w: CSTWhile, pass_failed: &mut bool) -> CSTWhile {
    CSTWhile {
        condition: pass_expr(w.condition, pass_failed, false, false),
        block: pass_block(w.block, pass_failed, true),
    }
}

fn pass_catch(c: CSTCatch, pass_failed: &mut bool, is_loop: bool) -> CSTCatch {
    CSTCatch {
        kind: c.kind,
        exception: c.exception,
        block: pass_block(c.block, pass_failed, is_loop),
    }
}

fn pass_try(t: CSTTryCatch, pass_failed: &mut bool, is_loop: bool) -> CSTTryCatch {
    CSTTryCatch {
        try_branch: pass_block(t.try_branch, pass_failed, is_loop),
        catch_branches: t
            .catch_branches
            .into_iter()
            .map(|i| pass_catch(i, pass_failed, is_loop))
            .collect(),
    }
}

fn pass_check(c: CSTCheck, pass_failed: &mut bool, is_loop: bool) -> CSTCheck {
    CSTCheck {
        block: pass_block(c.block, pass_failed, is_loop),
        after_backtrack: pass_block(c.after_backtrack, pass_failed, is_loop),
    }
}

fn pass_return(r: CSTReturn, pass_failed: &mut bool) -> CSTReturn {
    CSTReturn {
        val: r.val.map(|v| pass_expr(v, pass_failed, false, false)),
    }
}

fn pass_assign(a: CSTAssign, pass_failed: &mut bool) -> CSTAssign {
    CSTAssign {
        assign: pass_expr(a.assign, pass_failed, true, false),
        expr: Box::new(pass_stmt(*a.expr, pass_failed, false)),
    }
}

fn pass_assign_mod(a: CSTAssignMod, pass_failed: &mut bool) -> CSTAssignMod {
    CSTAssignMod {
        assign: pass_expr(a.assign, pass_failed, true, false),
        kind: a.kind,
        expr: pass_expr(a.expr, pass_failed, false, false),
    }
}

fn pass_lambda(l: CSTLambda, pass_failed: &mut bool) -> CSTLambda {
    match &l.params {
        CSTCollection::List(ls) => {
            assert!(ls.range.is_none());
            assert!(ls.rest.is_none());
            for i in &ls.expressions {
                if !matches!(i, CSTExpression::Variable(_)) {
                    panic!("encountered invalid list param");
                }
            }
        }
        _ => {
            panic!("encountered invalid list param");
        }
    }

    CSTLambda {
        params: pass_collection(l.params, pass_failed, false, false),
        is_closure: l.is_closure,
        expr: Box::new(pass_expr(*l.expr, pass_failed, false, false)),
    }
}

fn pass_op(o: CSTExpressionOp, pass_failed: &mut bool) -> CSTExpressionOp {
    CSTExpressionOp {
        op: o.op,
        left: Box::new(pass_expr(*o.left, pass_failed, false, false)),
        right: Box::new(pass_expr(*o.right, pass_failed, false, false)),
    }
}

fn pass_unary_op(o: CSTExpressionUnaryOp, pass_failed: &mut bool) -> CSTExpressionUnaryOp {
    CSTExpressionUnaryOp {
        op: o.op,
        expr: Box::new(pass_expr(*o.expr, pass_failed, false, false)),
    }
}

fn pass_proc(p: CSTProcedure, pass_failed: &mut bool) -> CSTProcedure {
    CSTProcedure {
        kind: p.kind,
        params: p
            .params
            .into_iter()
            .map(|i| pass_param(i, pass_failed))
            .collect(),
        list_param: p.list_param,
        block: pass_block(p.block, pass_failed, false),
    }
}

fn pass_call(c: CSTProcedureCall, pass_failed: &mut bool) -> CSTProcedureCall {
    CSTProcedureCall {
        name: c.name,
        params: c
            .params
            .into_iter()
            .map(|i| pass_expr(i, pass_failed, false, false))
            .collect(),
        rest_param: c
            .rest_param
            .map(|i| Box::new(pass_expr(*i, pass_failed, false, false))),
    }
}

fn pass_term(t: CSTTerm, pass_failed: &mut bool) -> CSTTerm {
    CSTTerm {
        name: t.name,
        is_tterm: t.is_tterm,
        params: t
            .params
            .into_iter()
            .map(|i| pass_expr(i, pass_failed, false, false))
            .collect(),
    }
}

fn pass_accessible(
    a: CSTAccessible,
    pass_failed: &mut bool,
    is_assign_target: bool,
) -> CSTAccessible {
    CSTAccessible {
        head: Box::new(pass_expr(*a.head, pass_failed, is_assign_target, false)),
        body: a
            .body
            .into_iter()
            .map(|i| pass_expr(i, pass_failed, false, true))
            .collect(),
    }
}

fn pass_range(r: CSTRange, pass_failed: &mut bool) -> CSTRange {
    CSTRange {
        left: r
            .left
            .map(|i| Box::new(pass_expr(*i, pass_failed, false, false))),
        right: r
            .right
            .map(|i| Box::new(pass_expr(*i, pass_failed, false, false))),
    }
}

fn pass_set(s: CSTSet, pass_failed: &mut bool, is_assign_target: bool) -> CSTSet {
    if is_assign_target {
        assert!(s.range.is_none());
        assert!(s.rest.is_none());
    }
    CSTSet {
        range: s.range.map(|i| pass_range(i, pass_failed)),
        expressions: s
            .expressions
            .into_iter()
            .map(|i| {
                if is_assign_target
                    && !matches!(
                        i,
                        CSTExpression::Variable(_)
                            | CSTExpression::Accessible(_)
                            | CSTExpression::Collection(_)
                            | CSTExpression::Ignore
                    )
                {
                    panic!("assign target must be variable, accessible, list or ignore");
                }

                pass_expr(i, pass_failed, is_assign_target, false)
            })
            .collect(),
        rest: s
            .rest
            .map(|i| Box::new(pass_expr(*i, pass_failed, false, false))),
    }
}

fn pass_comprehension(c: CSTComprehension, pass_failed: &mut bool) -> CSTComprehension {
    CSTComprehension {
        expression: Box::new(pass_expr(*c.expression, pass_failed, false, false)),
        iterators: c
            .iterators
            .into_iter()
            .map(|i| pass_iter_param(i, pass_failed))
            .collect(),
        condition: c
            .condition
            .map(|i| Box::new(pass_expr(*i, pass_failed, false, false))),
    }
}

fn pass_collection(
    c: CSTCollection,
    pass_failed: &mut bool,
    is_assign_target: bool,
    is_accessible_body: bool,
) -> CSTCollection {
    match c {
        CSTCollection::Set(s) => {
            assert!(!is_assign_target);
            if is_accessible_body {
                assert!(s.range.is_none());
                assert!(s.rest.is_none());
                assert!(s.expressions.len() == 1);
            }

            CSTCollection::Set(pass_set(s, pass_failed, false))
        }
        CSTCollection::List(s) => {
            if is_accessible_body {
                assert!(s.rest.is_none());
                if s.range.is_some() {
                    assert!(s.expressions.is_empty());
                }
            }

            CSTCollection::List(pass_set(s, pass_failed, is_assign_target))
        }
        CSTCollection::SetComprehension(s) => {
            if is_assign_target {
                panic!("assign target should only contain singletons");
            }
            CSTCollection::SetComprehension(pass_comprehension(s, pass_failed))
        }
        CSTCollection::ListComprehension(s) => {
            if is_assign_target {
                panic!("assign target should only contain singletons");
            }
            CSTCollection::ListComprehension(pass_comprehension(s, pass_failed))
        }
    }
}

fn pass_matrix(m: Vec<Vec<CSTExpression>>, pass_failed: &mut bool) -> Vec<Vec<CSTExpression>> {
    m.into_iter()
        .map(|i| {
            i.into_iter()
                .map(|j| pass_expr(j, pass_failed, false, false))
                .collect()
        })
        .collect()
}

fn pass_vector(m: Vec<CSTExpression>, pass_failed: &mut bool) -> Vec<CSTExpression> {
    m.into_iter()
        .map(|i| pass_expr(i, pass_failed, false, false))
        .collect()
}

fn pass_quant(q: CSTQuantifier, pass_failed: &mut bool) -> CSTQuantifier {
    CSTQuantifier {
        kind: q.kind,
        iterators: q
            .iterators
            .into_iter()
            .map(|i| pass_iter_param(i, pass_failed))
            .collect(),
        condition: Box::new(pass_expr(*q.condition, pass_failed, false, false)),
    }
}

fn pass_expr(
    e: CSTExpression,
    pass_failed: &mut bool,
    is_assign_target: bool,
    is_accessible_body: bool,
) -> CSTExpression {
    match e {
        CSTExpression::Lambda(l) => CSTExpression::Lambda(pass_lambda(l, pass_failed)),
        CSTExpression::Op(o) => CSTExpression::Op(pass_op(o, pass_failed)),
        CSTExpression::UnaryOp(o) => CSTExpression::UnaryOp(pass_unary_op(o, pass_failed)),
        CSTExpression::Procedure(p) => CSTExpression::Procedure(pass_proc(p, pass_failed)),
        CSTExpression::Call(c) => CSTExpression::Call(pass_call(c, pass_failed)),
        CSTExpression::Term(t) => {
            if is_assign_target {
                panic!("term can't be regular assign target");
            }

            CSTExpression::Term(pass_term(t, pass_failed))
        }
        CSTExpression::Accessible(a) => {
            CSTExpression::Accessible(pass_accessible(a, pass_failed, is_assign_target))
        }
        CSTExpression::Collection(c) => CSTExpression::Collection(pass_collection(
            c,
            pass_failed,
            is_assign_target,
            is_accessible_body,
        )),
        CSTExpression::Matrix(m) => CSTExpression::Matrix(pass_matrix(m, pass_failed)),
        CSTExpression::Vector(v) => CSTExpression::Vector(pass_vector(v, pass_failed)),
        CSTExpression::Quantifier(q) => CSTExpression::Quantifier(pass_quant(q, pass_failed)),
        CSTExpression::String(s) => CSTExpression::String(s),
        CSTExpression::Literal(l) => CSTExpression::Literal(l),
        CSTExpression::Bool(b) => CSTExpression::Bool(b),
        CSTExpression::Double(d) => CSTExpression::Double(d),
        CSTExpression::Number(i) => CSTExpression::Number(i),
        CSTExpression::Om => CSTExpression::Om,
        CSTExpression::Ignore => CSTExpression::Ignore,
        CSTExpression::Variable(s) => CSTExpression::Variable(s),
    }
}

fn pass_stmt(s: CSTStatement, pass_failed: &mut bool, is_loop: bool) -> CSTStatement {
    match s {
        CSTStatement::Class(c) => CSTStatement::Class(pass_class(c, pass_failed)),
        CSTStatement::If(i) => CSTStatement::If(pass_if(i, pass_failed, is_loop)),
        CSTStatement::Switch(i) => CSTStatement::Switch(pass_if(i, pass_failed, is_loop)),
        CSTStatement::Match(m) => CSTStatement::Match(pass_match(m, pass_failed, is_loop)),
        CSTStatement::Scan(s) => CSTStatement::Scan(pass_scan(s, pass_failed, is_loop)),
        CSTStatement::For(f) => CSTStatement::For(pass_for(f, pass_failed)),
        CSTStatement::DoWhile(w) => CSTStatement::DoWhile(pass_while(w, pass_failed)),
        CSTStatement::While(w) => CSTStatement::While(pass_while(w, pass_failed)),
        CSTStatement::TryCatch(t) => CSTStatement::TryCatch(pass_try(t, pass_failed, is_loop)),
        CSTStatement::Check(c) => CSTStatement::Check(pass_check(c, pass_failed, is_loop)),
        CSTStatement::Return(r) => CSTStatement::Return(pass_return(r, pass_failed)),
        CSTStatement::Assign(a) => CSTStatement::Assign(pass_assign(a, pass_failed)),
        CSTStatement::AssignMod(a) => CSTStatement::AssignMod(pass_assign_mod(a, pass_failed)),
        CSTStatement::Expression(e) => {
            CSTStatement::Expression(pass_expr(e, pass_failed, false, false))
        }
        CSTStatement::Backtrack => CSTStatement::Backtrack,
        CSTStatement::Break => {
            if !is_loop {
                panic!("encountered break statement outside of loop");
            }

            CSTStatement::Break
        }
        CSTStatement::Continue => {
            if !is_loop {
                panic!("encountered continue statement outside of loop");
            }

            CSTStatement::Continue
        }
        CSTStatement::Exit => CSTStatement::Exit,
    }
}

fn pass_block(cst: CSTBlock, pass_failed: &mut bool, is_loop: bool) -> CSTBlock {
    let mut prior_ret = false;
    cst.into_iter()
        .map(|i| {
            if prior_ret {
                eprintln!("warning: encountered unreachable code");
            }

            prior_ret = matches!(
                i,
                CSTStatement::Return(_)
                    | CSTStatement::Break
                    | CSTStatement::Continue
                    | CSTStatement::Backtrack
            );

            pass_stmt(i, pass_failed, is_loop)
        })
        .collect()
}
pub fn pass(mut cst: CSTBlock, opts: &InputOpts, pass_num: u64) -> CSTBlock {
    /* - check lambda params
     * - check assignment targets are ignore, variables, or list
     * - check iterator variables are lists, variables or ignore
     * - check accessible body sets are singletons, and lists are ranges or singletons
     * - check break, continue only in for, while, doWhile
     * - check unreachable code
     *     - catch branches
     *     - duplicate match case expressions
     */
    let mut pass_failed = false;

    cst = pass_block(cst, &mut pass_failed, false);

    assert!(!pass_failed);

    if opts.dump_cst_pass_check {
        cst_dump(&cst, opts, &format!("{pass_num}-check"));
    }

    cst
}
