use crate::ast::*;
use crate::cli::InputOpts;
use crate::cst::dump::cst_dump;

fn pass_param(p: CSTParam) -> CSTParam {
    CSTParam {
        name: p.name,
        is_rw: p.is_rw,
        default: p.default.map(pass_expr),
    }
}

fn pass_class(c: CSTClass) -> CSTClass {
    CSTClass {
        name: c.name,
        params: c.params.into_iter().map(pass_param).collect(),
        block: pass_block(c.block),
        static_block: c.static_block.map(pass_block),
    }
}

fn pass_if_branch(i: CSTIfBranch) -> CSTIfBranch {
    CSTIfBranch {
        condition: pass_expr(i.condition),
        block: pass_block(i.block),
    }
}

fn pass_if(i: CSTIf) -> CSTIf {
    CSTIf {
        branches: i.branches.into_iter().map(pass_if_branch).collect(),
        alternative: i.alternative.map(pass_block),
    }
}

fn pass_match_branch(m: CSTMatchBranch) -> CSTMatchBranch {
    match m {
        CSTMatchBranch::Case(c) => CSTMatchBranch::Case(CSTMatchBranchCase {
            expressions: c.expressions.into_iter().map(pass_expr).collect(),
            condition: c.condition.map(pass_expr),
            statements: pass_block(c.statements),
        }),
        CSTMatchBranch::Regex(r) => CSTMatchBranch::Regex(CSTMatchBranchRegex {
            pattern: pass_expr(r.pattern),
            pattern_out: r.pattern_out.map(pass_expr),
            condition: r.condition.map(pass_expr),
            statements: pass_block(r.statements),
        }),
    }
}

fn pass_match(m: CSTMatch) -> CSTMatch {
    CSTMatch {
        expression: pass_expr(m.expression),
        branches: m
            .branches
            .into_iter()
            .map(pass_match_branch)
            .collect(),
        default: pass_block(m.default),
    }
}

fn pass_scan(s: CSTScan) -> CSTScan {
    CSTScan {
        expression: pass_expr(s.expression),
        variable: s.variable,
        branches: s
            .branches
            .into_iter()
            .map(pass_match_branch)
            .collect(),
    }
}

fn pass_iter_param(i: CSTIterParam) -> CSTIterParam {
    CSTIterParam {
        variable: pass_expr(i.variable),
        collection: pass_expr(i.collection),
    }
}

fn pass_for(f: CSTFor) -> CSTFor {
    CSTFor {
        params: f.params.into_iter().map(pass_iter_param).collect(),
        condition: f.condition.map(|c| Box::new(pass_expr(*c))),
        block: pass_block(f.block),
    }
}

fn pass_while(w: CSTWhile) -> CSTWhile {
    CSTWhile {
        condition: pass_expr(w.condition),
        block: pass_block(w.block),
    }
}

fn pass_catch(c: CSTCatch) -> CSTCatch {
    CSTCatch {
        kind: c.kind,
        exception: c.exception,
        block: pass_block(c.block),
    }
}

fn pass_try(t: CSTTryCatch) -> CSTTryCatch {
    CSTTryCatch {
        try_branch: pass_block(t.try_branch),
        catch_branches: t
            .catch_branches
            .into_iter()
            .map(pass_catch)
            .collect(),
    }
}

fn pass_check(c: CSTCheck) -> CSTCheck {
    CSTCheck {
        block: pass_block(c.block),
        after_backtrack: pass_block(c.after_backtrack),
    }
}

fn pass_return(r: CSTReturn) -> CSTReturn {
    CSTReturn {
        val: r.val.map(pass_expr),
    }
}

fn pass_assign(a: CSTAssign) -> CSTAssign {
    CSTAssign {
        assign: pass_expr(a.assign),
        expr: Box::new(pass_stmt(*a.expr)),
    }
}

fn pass_assign_mod(a: CSTAssignMod) -> CSTAssignMod {
    CSTAssignMod {
        assign: pass_expr(a.assign),
        kind: a.kind,
        expr: pass_expr(a.expr),
    }
}

fn pass_lambda(l: CSTLambda) -> CSTLambda {
    CSTLambda {
        params: pass_collection(l.params),
        is_closure: l.is_closure,
        expr: Box::new(pass_expr(*l.expr)),
    }
}

fn pass_op(o: CSTExpressionOp) -> CSTExpressionOp {
    CSTExpressionOp {
        op: o.op,
        left: Box::new(pass_expr(*o.left)),
        right: Box::new(pass_expr(*o.right)),
    }
}

fn pass_unary_op(o: CSTExpressionUnaryOp) -> CSTExpressionUnaryOp {
    CSTExpressionUnaryOp {
        op: o.op,
        expr: Box::new(pass_expr(*o.expr)),
    }
}

fn pass_proc(p: CSTProcedure) -> CSTProcedure {
    CSTProcedure {
        kind: p.kind,
        params: p.params.into_iter().map(pass_param).collect(),
        list_param: p.list_param,
        block: pass_block(p.block),
    }
}

fn pass_call(c: CSTProcedureCall) -> CSTProcedureCall {
    CSTProcedureCall {
        name: c.name,
        params: c.params.into_iter().map(pass_expr).collect(),
        rest_param: c.rest_param.map(|i| Box::new(pass_expr(*i))),
    }
}

fn pass_term(t: CSTTerm) -> CSTTerm {
    CSTTerm {
        name: t.name,
        is_tterm: t.is_tterm,
        params: t.params.into_iter().map(pass_expr).collect(),
    }
}

fn pass_accessible(a: CSTAccessible) -> CSTAccessible {
    CSTAccessible {
        head: Box::new(pass_expr(*a.head)),
        body: a.body.into_iter().map(pass_expr).collect(),
    }
}

fn pass_range(r: CSTRange) -> CSTRange {
    CSTRange {
        left: r.left.map(|i| Box::new(pass_expr(*i))),
        right: r.right.map(|i| Box::new(pass_expr(*i))),
    }
}

fn pass_set(s: CSTSet) -> CSTSet {
    CSTSet {
        range: s.range.map(pass_range),
        expressions: s.expressions.into_iter().map(pass_expr).collect(),
        rest: s.rest.map(|i| Box::new(pass_expr(*i))),
    }
}

fn pass_comprehension(c: CSTComprehension) -> CSTComprehension {
    CSTComprehension {
        expression: Box::new(pass_expr(*c.expression)),
        iterators: c
            .iterators
            .into_iter()
            .map(pass_iter_param)
            .collect(),
        condition: c.condition.map(|i| Box::new(pass_expr(*i))),
    }
}

fn pass_collection(c: CSTCollection) -> CSTCollection {
    match c {
        CSTCollection::Set(s) => CSTCollection::Set(pass_set(s)),
        CSTCollection::List(s) => CSTCollection::List(pass_set(s)),
        CSTCollection::SetComprehension(s) => {
            CSTCollection::SetComprehension(pass_comprehension(s))
        }
        CSTCollection::ListComprehension(s) => {
            CSTCollection::ListComprehension(pass_comprehension(s))
        }
    }
}

fn pass_matrix(m: Vec<Vec<CSTExpression>>) -> Vec<Vec<CSTExpression>> {
    m.into_iter()
        .map(|i| i.into_iter().map(pass_expr).collect())
        .collect()
}

fn pass_vector(m: Vec<CSTExpression>) -> Vec<CSTExpression> {
    m.into_iter().map(pass_expr).collect()
}

fn pass_quant(q: CSTQuantifier) -> CSTQuantifier {
    CSTQuantifier {
        kind: q.kind,
        iterators: q
            .iterators
            .into_iter()
            .map(pass_iter_param)
            .collect(),
        condition: Box::new(pass_expr(*q.condition)),
    }
}

fn pass_expr(e: CSTExpression) -> CSTExpression {
    match e {
        CSTExpression::Lambda(l) => CSTExpression::Lambda(pass_lambda(l)),
        CSTExpression::Op(o) => CSTExpression::Op(pass_op(o)),
        CSTExpression::UnaryOp(o) => CSTExpression::UnaryOp(pass_unary_op(o)),
        CSTExpression::Procedure(p) => CSTExpression::Procedure(pass_proc(p)),
        CSTExpression::Call(c) => CSTExpression::Call(pass_call(c)),
        CSTExpression::Term(t) => CSTExpression::Term(pass_term(t)),
        CSTExpression::Accessible(a) => CSTExpression::Accessible(pass_accessible(a)),
        CSTExpression::Collection(c) => CSTExpression::Collection(pass_collection(c)),
        CSTExpression::Matrix(m) => CSTExpression::Matrix(pass_matrix(m)),
        CSTExpression::Vector(v) => CSTExpression::Vector(pass_vector(v)),
        CSTExpression::Quantifier(q) => CSTExpression::Quantifier(pass_quant(q)),
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

fn pass_stmt(s: CSTStatement) -> CSTStatement {
    match s {
        CSTStatement::Class(c) => CSTStatement::Class(pass_class(c)),
        CSTStatement::If(i) => CSTStatement::If(pass_if(i)),
        CSTStatement::Switch(i) => CSTStatement::Switch(pass_if(i)),
        CSTStatement::Match(m) => CSTStatement::Match(pass_match(m)),
        CSTStatement::Scan(s) => CSTStatement::Scan(pass_scan(s)),
        CSTStatement::For(f) => CSTStatement::For(pass_for(f)),
        CSTStatement::DoWhile(w) => CSTStatement::DoWhile(pass_while(w)),
        CSTStatement::While(w) => CSTStatement::While(pass_while(w)),
        CSTStatement::TryCatch(t) => CSTStatement::TryCatch(pass_try(t)),
        CSTStatement::Check(c) => CSTStatement::Check(pass_check(c)),
        CSTStatement::Return(r) => CSTStatement::Return(pass_return(r)),
        CSTStatement::Assign(a) => CSTStatement::Assign(pass_assign(a)),
        CSTStatement::AssignMod(a) => CSTStatement::AssignMod(pass_assign_mod(a)),
        CSTStatement::Expression(e) => CSTStatement::Expression(pass_expr(e)),
        CSTStatement::Backtrack => CSTStatement::Backtrack,
        CSTStatement::Break => CSTStatement::Break,
        CSTStatement::Continue => CSTStatement::Continue,
        CSTStatement::Exit => CSTStatement::Exit,
    }
}

fn pass_block(cst: CSTBlock) -> CSTBlock {
    let mut out = Vec::new();

    for i in cst.into_iter() {
        let break_loop = matches!(i,
            CSTStatement::Return(_)
            | CSTStatement::Break
            | CSTStatement::Continue
            | CSTStatement::Exit);

        out.push(pass_stmt(i));
        if break_loop {
            break;
        }
    }

    out
}
pub fn pass(mut cst: CSTBlock, opts: &InputOpts, pass_num: u64) -> CSTBlock {
    /* - remove stmts after continue, break, return, exit
     */

    cst = pass_block(cst);

    if opts.dump_cst_pass_noop {
        cst_dump(&cst, opts, &format!("{pass_num}-noop"));
    }

    cst
}
