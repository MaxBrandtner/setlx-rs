use crate::ast::*;
use crate::cli::InputOpts;
use crate::cst::dump::cst_dump;

fn pass_param(p: &mut CSTParam) {
    p.default.iter_mut().for_each(pass_expr);
}

fn pass_class(c: &mut CSTClass) {
    c.params.iter_mut().for_each(pass_param);
    pass_block(&mut c.block);
    c.static_block.iter_mut().for_each(pass_block);
}

fn pass_if_branch(i: &mut CSTIfBranch) {
    pass_expr(&mut i.condition);
    pass_block(&mut i.block);
}

fn pass_if(i: &mut CSTIf) {
    i.branches.iter_mut().for_each(pass_if_branch);
    i.alternative.iter_mut().for_each(pass_block);
}

fn pass_match_branch(m: &mut CSTMatchBranch) {
    match m {
        CSTMatchBranch::Case(c) => {
            c.expressions.iter_mut().for_each(pass_expr);
            c.condition.iter_mut().for_each(pass_expr);
            pass_block(&mut c.statements);
        }
        CSTMatchBranch::Regex(r) => {
            pass_expr(&mut r.pattern);
            r.pattern_out.iter_mut().for_each(pass_expr);
            r.condition.iter_mut().for_each(pass_expr);
            pass_block(&mut r.statements);
        }
    }
}

fn pass_match(m: &mut CSTMatch) {
    pass_expr(&mut m.expression);
    m.branches.iter_mut().for_each(pass_match_branch);
    pass_block(&mut m.default);
}

fn pass_scan(s: &mut CSTScan) {
    pass_expr(&mut s.expression);
    s.branches.iter_mut().for_each(pass_match_branch);
}

fn pass_iter_param(i: &mut CSTIterParam) {
    pass_expr(&mut i.variable);
    pass_expr(&mut i.collection);
}

fn pass_for(f: &mut CSTFor) {
    f.params.iter_mut().for_each(pass_iter_param);
    f.condition.iter_mut().for_each(|i| pass_expr(&mut *i));
    pass_block(&mut f.block);
}

fn pass_while(w: &mut CSTWhile) {
    pass_expr(&mut w.condition);
    pass_block(&mut w.block);
}

fn pass_catch(c: &mut CSTCatch) {
    pass_block(&mut c.block);
}

fn pass_try(t: &mut CSTTryCatch) {
    pass_block(&mut t.try_branch);
    t.catch_branches.iter_mut().for_each(pass_catch);
}

fn pass_check(c: &mut CSTCheck) {
    pass_block(&mut c.block);
    pass_block(&mut c.after_backtrack);
}

fn pass_return(r: &mut CSTReturn) {
    r.val.iter_mut().for_each(pass_expr);
}

fn pass_assign(a: &mut CSTAssign) {
    pass_expr(&mut a.assign);
    pass_stmt(&mut *a.expr);
}

fn pass_assign_mod(a: &mut CSTAssignMod) {
    pass_expr(&mut a.assign);
    pass_expr(&mut a.expr);
}

fn pass_lambda(l: &mut CSTLambda) {
    pass_collection(&mut l.params);
    pass_expr(&mut *l.expr);
}

fn pass_op(o: &mut CSTExpressionOp) {
    pass_expr(&mut *o.left);
    pass_expr(&mut *o.right);
}

fn pass_unary_op(o: &mut CSTExpressionUnaryOp) {
    pass_expr(&mut *o.expr);
}

fn pass_proc(p: &mut CSTProcedure) {
    p.params.iter_mut().for_each(pass_param);
    pass_block(&mut p.block);
}

fn pass_call(c: &mut CSTProcedureCall) {
    c.params.iter_mut().for_each(pass_expr);
    c.rest_param.iter_mut().for_each(|i| pass_expr(&mut *i));
}

fn pass_term(t: &mut CSTTerm) {
    t.params.iter_mut().for_each(pass_expr);
}

fn pass_accessible(a: &mut CSTAccessible) {
    pass_expr(&mut *a.head);
    a.body.iter_mut().for_each(pass_expr);
}

fn pass_range(r: &mut CSTRange) {
    r.left.iter_mut().for_each(|i| pass_expr(&mut *i));
    r.right.iter_mut().for_each(|i| pass_expr(&mut *i));
}

fn pass_set(s: &mut CSTSet) {
    s.range.iter_mut().for_each(pass_range);
    s.expressions.iter_mut().for_each(pass_expr);
    s.rest.iter_mut().for_each(|i| pass_expr(&mut *i));
}

fn pass_comprehension(c: &mut CSTComprehension) {
    pass_expr(&mut c.expression);
    c.iterators.iter_mut().for_each(pass_iter_param);
    c.condition.iter_mut().for_each(|i| pass_expr(&mut *i));
}

fn pass_collection(c: &mut CSTCollection) {
    match c {
        CSTCollection::Set(s) => pass_set(s),
        CSTCollection::List(s) => pass_set(s),
        CSTCollection::SetComprehension(s) =>pass_comprehension(s),
        CSTCollection::ListComprehension(s) => pass_comprehension(s),
    }
}

fn pass_matrix(m: &mut Vec<Vec<CSTExpression>>) {
    m.iter_mut().for_each(|i| i.iter_mut().for_each(pass_expr));
}

fn pass_vector(m: &mut Vec<CSTExpression>) {
    m.iter_mut().for_each(pass_expr);
}

fn pass_quant(q: &mut CSTQuantifier) {
    q.iterators.iter_mut().for_each(pass_iter_param);
    pass_expr(&mut q.condition);
}

pub fn pass_expr(e: &mut CSTExpression) {
    match &mut e.kind {
        CSTExpressionKind::Lambda(l) => pass_lambda(l),
        CSTExpressionKind::Op(o) => pass_op(o),
        CSTExpressionKind::UnaryOp(o) => pass_unary_op(o),
        CSTExpressionKind::Procedure(p) => pass_proc(p),
        CSTExpressionKind::Call(c) => pass_call(c),
        CSTExpressionKind::Term(t) => pass_term(t),
        CSTExpressionKind::Accessible(a) => pass_accessible(a),
        CSTExpressionKind::Collection(c) => pass_collection(c),
        CSTExpressionKind::Matrix(m) => pass_matrix(m),
        CSTExpressionKind::Vector(v) => pass_vector(v),
        CSTExpressionKind::Quantifier(q) => pass_quant(q),
        _ => (),
    }
}

fn pass_stmt(s: &mut CSTStatement) {
    match &mut s.kind {
        CSTStatementKind::Class(c) => pass_class(c),
        CSTStatementKind::If(i) => pass_if(i),
        CSTStatementKind::Switch(i) => pass_if(i),
        CSTStatementKind::Match(m) => pass_match(m),
        CSTStatementKind::Scan(s) => pass_scan(s),
        CSTStatementKind::For(f) => pass_for(f),
        CSTStatementKind::DoWhile(w) => pass_while(w),
        CSTStatementKind::While(w) => pass_while(w),
        CSTStatementKind::TryCatch(t) => pass_try(t),
        CSTStatementKind::Check(c) => pass_check(c),
        CSTStatementKind::Return(r) => pass_return(r),
        CSTStatementKind::Assign(a) => pass_assign(a),
        CSTStatementKind::AssignMod(a) => pass_assign_mod(a),
        CSTStatementKind::Expression(e) => pass_expr(e),
        _ => (),
    }
}

fn pass_block(cst: &mut CSTBlock) {
    let mut idx = 0;

    for i in cst.iter_mut() {
        let break_loop = matches!(i.kind,
            CSTStatementKind::Return(_)
            | CSTStatementKind::Break
            | CSTStatementKind::Continue
            | CSTStatementKind::Exit);

        pass_stmt(i);
        idx += 1;
        if break_loop {
            break;
        }
    }

    cst.truncate(idx);
}
pub fn pass(cst: &mut CSTBlock, opts: &InputOpts, pass_num: u64) {
    pass_block(cst);

    if opts.dump_cst_pass_noop {
        cst_dump(cst, opts, &format!("{pass_num}-noop"));
    }
}
