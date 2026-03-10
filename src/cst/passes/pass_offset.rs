use crate::ast::*;

fn pass_param(p: &mut CSTParam, offset: usize) {
    p.default.iter_mut().for_each(|i| pass_expr(i, offset));
}

fn pass_class(c: &mut CSTClass, offset: usize) {
    c.params.iter_mut().for_each(|i| pass_param(i, offset));
    pass_block(&mut c.block, offset);
    c.static_block
        .iter_mut()
        .for_each(|i| pass_block(i, offset));
}

fn pass_if_branch(i: &mut CSTIfBranch, offset: usize) {
    pass_expr(&mut i.condition, offset);
    pass_block(&mut i.block, offset);
}

fn pass_if(i: &mut CSTIf, offset: usize) {
    i.branches
        .iter_mut()
        .for_each(|i| pass_if_branch(i, offset));
    i.alternative.iter_mut().for_each(|i| pass_block(i, offset));
}

fn pass_match_branch(m: &mut CSTMatchBranch, offset: usize) {
    match m {
        CSTMatchBranch::Case(c) => {
            c.expressions.iter_mut().for_each(|i| pass_expr(i, offset));
            c.condition.iter_mut().for_each(|i| pass_expr(i, offset));
            pass_block(&mut c.statements, offset);
        }
        CSTMatchBranch::Regex(r) => {
            pass_expr(&mut r.pattern, offset);
            r.pattern_out.iter_mut().for_each(|i| pass_expr(i, offset));
            r.condition.iter_mut().for_each(|i| pass_expr(i, offset));
            pass_block(&mut r.statements, offset);
        }
    }
}

fn pass_match(m: &mut CSTMatch, offset: usize) {
    pass_expr(&mut m.expression, offset);
    m.branches
        .iter_mut()
        .for_each(|i| pass_match_branch(i, offset));
    pass_block(&mut m.default, offset);
}

fn pass_scan(s: &mut CSTScan, offset: usize) {
    pass_expr(&mut s.expression, offset);
    s.branches
        .iter_mut()
        .for_each(|i| pass_match_branch(i, offset));
}

fn pass_iter_param(i: &mut CSTIterParam, offset: usize) {
    pass_expr(&mut i.variable, offset);
    pass_expr(&mut i.collection, offset);
}

fn pass_for(f: &mut CSTFor, offset: usize) {
    f.params.iter_mut().for_each(|i| pass_iter_param(i, offset));
    f.condition
        .iter_mut()
        .for_each(|i| pass_expr(&mut *i, offset));
    pass_block(&mut f.block, offset);
}

fn pass_while(w: &mut CSTWhile, offset: usize) {
    pass_expr(&mut w.condition, offset);
    pass_block(&mut w.block, offset);
}

fn pass_catch(c: &mut CSTCatch, offset: usize) {
    pass_block(&mut c.block, offset);
}

fn pass_try(t: &mut CSTTryCatch, offset: usize) {
    pass_block(&mut t.try_branch, offset);
    t.catch_branches
        .iter_mut()
        .for_each(|i| pass_catch(i, offset));
}

fn pass_check(c: &mut CSTCheck, offset: usize) {
    pass_block(&mut c.block, offset);
    pass_block(&mut c.after_backtrack, offset);
}

fn pass_return(r: &mut CSTReturn, offset: usize) {
    r.val.iter_mut().for_each(|i| pass_expr(i, offset));
}

fn pass_assign(a: &mut CSTAssign, offset: usize) {
    pass_expr(&mut a.assign, offset);
    pass_stmt(&mut *a.expr, offset);
}

fn pass_assign_mod(a: &mut CSTAssignMod, offset: usize) {
    pass_expr(&mut a.assign, offset);
    pass_expr(&mut a.expr, offset);
}

fn pass_lambda(l: &mut CSTLambda, offset: usize) {
    pass_collection(&mut l.params, offset);
    pass_expr(&mut *l.expr, offset);
}

fn pass_op(o: &mut CSTExpressionOp, offset: usize) {
    pass_expr(&mut *o.left, offset);
    pass_expr(&mut *o.right, offset);
}

fn pass_unary_op(o: &mut CSTExpressionUnaryOp, offset: usize) {
    pass_expr(&mut *o.expr, offset);
}

fn pass_proc(p: &mut CSTProcedure, offset: usize) {
    p.params.iter_mut().for_each(|i| pass_param(i, offset));
    pass_block(&mut p.block, offset);
}

fn pass_call(c: &mut CSTProcedureCall, offset: usize) {
    c.params.iter_mut().for_each(|i| pass_expr(i, offset));
    c.rest_param
        .iter_mut()
        .for_each(|i| pass_expr(&mut *i, offset));
}

fn pass_term(t: &mut CSTTerm, offset: usize) {
    t.params.iter_mut().for_each(|i| pass_expr(i, offset));
}

fn pass_accessible(a: &mut CSTAccessible, offset: usize) {
    pass_expr(&mut *a.head, offset);
    a.body.iter_mut().for_each(|i| pass_expr(i, offset));
}

fn pass_range(r: &mut CSTRange, offset: usize) {
    r.left.iter_mut().for_each(|i| pass_expr(&mut *i, offset));
    r.right.iter_mut().for_each(|i| pass_expr(&mut *i, offset));
}

fn pass_set(s: &mut CSTSet, offset: usize) {
    s.range.iter_mut().for_each(|i| pass_range(i, offset));
    s.expressions.iter_mut().for_each(|i| pass_expr(i, offset));
    s.rest.iter_mut().for_each(|i| pass_expr(&mut *i, offset));
}

fn pass_comprehension(c: &mut CSTComprehension, offset: usize) {
    pass_expr(&mut c.expression, offset);
    c.iterators
        .iter_mut()
        .for_each(|i| pass_iter_param(i, offset));
    c.condition
        .iter_mut()
        .for_each(|i| pass_expr(&mut *i, offset));
}

fn pass_collection(c: &mut CSTCollection, offset: usize) {
    match c {
        CSTCollection::Set(s) => pass_set(s, offset),
        CSTCollection::List(s) => pass_set(s, offset),
        CSTCollection::SetComprehension(s) => pass_comprehension(s, offset),
        CSTCollection::ListComprehension(s) => pass_comprehension(s, offset),
    }
}

fn pass_matrix(m: &mut Vec<Vec<CSTExpression>>, offset: usize) {
    m.iter_mut()
        .for_each(|i| i.iter_mut().for_each(|i| pass_expr(i, offset)));
}

fn pass_vector(m: &mut Vec<CSTExpression>, offset: usize) {
    m.iter_mut().for_each(|i| pass_expr(i, offset));
}

fn pass_quant(q: &mut CSTQuantifier, offset: usize) {
    q.iterators
        .iter_mut()
        .for_each(|i| pass_iter_param(i, offset));
    pass_expr(&mut q.condition, offset);
}

pub fn pass_expr(e: &mut CSTExpression, offset: usize) {
    e.lhs += offset;
    e.rhs += offset;

    match &mut e.kind {
        CSTExpressionKind::Lambda(l) => pass_lambda(l, offset),
        CSTExpressionKind::Op(o) => pass_op(o, offset),
        CSTExpressionKind::UnaryOp(o) => pass_unary_op(o, offset),
        CSTExpressionKind::Procedure(p) => pass_proc(p, offset),
        CSTExpressionKind::Call(c) => pass_call(c, offset),
        CSTExpressionKind::Term(t) => pass_term(t, offset),
        CSTExpressionKind::Accessible(a) => pass_accessible(a, offset),
        CSTExpressionKind::Collection(c) => pass_collection(c, offset),
        CSTExpressionKind::Matrix(m) => pass_matrix(m, offset),
        CSTExpressionKind::Vector(v) => pass_vector(v, offset),
        CSTExpressionKind::Quantifier(q) => pass_quant(q, offset),
        _ => (),
    }
}

fn pass_stmt(s: &mut CSTStatement, offset: usize) {
    s.lhs += offset;
    s.rhs += offset;

    match &mut s.kind {
        CSTStatementKind::Class(c) => pass_class(c, offset),
        CSTStatementKind::If(i) => pass_if(i, offset),
        CSTStatementKind::Switch(i) => pass_if(i, offset),
        CSTStatementKind::Match(m) => pass_match(m, offset),
        CSTStatementKind::Scan(s) => pass_scan(s, offset),
        CSTStatementKind::For(f) => pass_for(f, offset),
        CSTStatementKind::DoWhile(w) => pass_while(w, offset),
        CSTStatementKind::While(w) => pass_while(w, offset),
        CSTStatementKind::TryCatch(t) => pass_try(t, offset),
        CSTStatementKind::Check(c) => pass_check(c, offset),
        CSTStatementKind::Return(r) => pass_return(r, offset),
        CSTStatementKind::Assign(a) => pass_assign(a, offset),
        CSTStatementKind::AssignMod(a) => pass_assign_mod(a, offset),
        CSTStatementKind::Expression(e) => pass_expr(e, offset),
        _ => (),
    }
}

fn pass_block(cst: &mut CSTBlock, offset: usize) {
    let mut idx = 0;

    for i in cst.iter_mut() {
        let break_loop = matches!(
            i.kind,
            CSTStatementKind::Return(_)
                | CSTStatementKind::Break
                | CSTStatementKind::Continue
                | CSTStatementKind::Exit
        );

        pass_stmt(i, offset);
        idx += 1;
        if break_loop {
            break;
        }
    }

    cst.truncate(idx);
}
