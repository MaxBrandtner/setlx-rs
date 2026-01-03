use crate::ast::*;
use crate::cli::InputOpts;
use crate::cst::dump::cst_dump;
use crate::setlx_parse;
use crate::util::unescape::unescape;

fn pass_param(p: CSTParam, pass_failed: &mut bool) -> CSTParam {
    CSTParam {
        name: p.name,
        is_rw: p.is_rw,
        default: p.default.map(|i| pass_expr(i, pass_failed)),
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
        block: pass_block(c.block, pass_failed),
        static_block: c.static_block.map(|s| pass_block(s, pass_failed)),
    }
}

fn pass_if_branch(i: CSTIfBranch, pass_failed: &mut bool) -> CSTIfBranch {
    CSTIfBranch {
        condition: pass_expr(i.condition, pass_failed),
        block: pass_block(i.block, pass_failed),
    }
}

fn pass_if(i: CSTIf, pass_failed: &mut bool) -> CSTIf {
    CSTIf {
        branches: i
            .branches
            .into_iter()
            .map(|i| pass_if_branch(i, pass_failed))
            .collect(),
        alternative: i.alternative.map(|a| pass_block(a, pass_failed)),
    }
}

fn pass_match_branch(m: CSTMatchBranch, pass_failed: &mut bool) -> CSTMatchBranch {
    match m {
        CSTMatchBranch::Case(c) => CSTMatchBranch::Case(CSTMatchBranchCase {
            expressions: c
                .expressions
                .into_iter()
                .map(|i| pass_expr(i, pass_failed))
                .collect(),
            condition: c.condition.map(|i| pass_expr(i, pass_failed)),
            statements: pass_block(c.statements, pass_failed),
        }),
        CSTMatchBranch::Regex(r) => CSTMatchBranch::Regex(CSTMatchBranchRegex {
            pattern: pass_expr(r.pattern, pass_failed),
            pattern_out: r.pattern_out.map(|i| pass_expr(i, pass_failed)),
            condition: r.condition.map(|i| pass_expr(i, pass_failed)),
            statements: pass_block(r.statements, pass_failed),
        }),
    }
}

fn pass_match(m: CSTMatch, pass_failed: &mut bool) -> CSTMatch {
    CSTMatch {
        expression: pass_expr(m.expression, pass_failed),
        branches: m
            .branches
            .into_iter()
            .map(|i| pass_match_branch(i, pass_failed))
            .collect(),
        default: pass_block(m.default, pass_failed),
    }
}

fn pass_scan(s: CSTScan, pass_failed: &mut bool) -> CSTScan {
    CSTScan {
        expression: pass_expr(s.expression, pass_failed),
        variable: s.variable,
        branches: s
            .branches
            .into_iter()
            .map(|i| pass_match_branch(i, pass_failed))
            .collect(),
    }
}

fn pass_iter_param(i: CSTIterParam, pass_failed: &mut bool) -> CSTIterParam {
    CSTIterParam {
        variable: pass_expr(i.variable, pass_failed),
        collection: pass_expr(i.collection, pass_failed),
    }
}

fn pass_for(f: CSTFor, pass_failed: &mut bool) -> CSTFor {
    CSTFor {
        params: f
            .params
            .into_iter()
            .map(|i| pass_iter_param(i, pass_failed))
            .collect(),
        condition: f.condition.map(|c| Box::new(pass_expr(*c, pass_failed))),
        block: pass_block(f.block, pass_failed),
    }
}

fn pass_while(w: CSTWhile, pass_failed: &mut bool) -> CSTWhile {
    CSTWhile {
        condition: pass_expr(w.condition, pass_failed),
        block: pass_block(w.block, pass_failed),
    }
}

fn pass_catch(c: CSTCatch, pass_failed: &mut bool) -> CSTCatch {
    CSTCatch {
        kind: c.kind,
        exception: c.exception,
        block: pass_block(c.block, pass_failed),
    }
}

fn pass_try(t: CSTTryCatch, pass_failed: &mut bool) -> CSTTryCatch {
    CSTTryCatch {
        try_branch: pass_block(t.try_branch, pass_failed),
        catch_branches: t
            .catch_branches
            .into_iter()
            .map(|i| pass_catch(i, pass_failed))
            .collect(),
    }
}

fn pass_check(c: CSTCheck, pass_failed: &mut bool) -> CSTCheck {
    CSTCheck {
        block: pass_block(c.block, pass_failed),
        after_backtrack: pass_block(c.after_backtrack, pass_failed),
    }
}

fn pass_return(r: CSTReturn, pass_failed: &mut bool) -> CSTReturn {
    CSTReturn {
        val: r.val.map(|v| pass_expr(v, pass_failed)),
    }
}

fn pass_assign(a: CSTAssign, pass_failed: &mut bool) -> CSTAssign {
    CSTAssign {
        assign: pass_expr(a.assign, pass_failed),
        expr: Box::new(pass_stmt(*a.expr, pass_failed)),
    }
}

fn pass_assign_mod(a: CSTAssignMod, pass_failed: &mut bool) -> CSTAssignMod {
    CSTAssignMod {
        assign: pass_expr(a.assign, pass_failed),
        kind: a.kind,
        expr: pass_expr(a.expr, pass_failed),
    }
}

fn pass_lambda(l: CSTLambda, pass_failed: &mut bool) -> CSTLambda {
    CSTLambda {
        params: pass_collection(l.params, pass_failed),
        is_closure: l.is_closure,
        expr: Box::new(pass_expr(*l.expr, pass_failed)),
    }
}

fn pass_op(o: CSTExpressionOp, pass_failed: &mut bool) -> CSTExpressionOp {
    CSTExpressionOp {
        op: o.op,
        left: Box::new(pass_expr(*o.left, pass_failed)),
        right: Box::new(pass_expr(*o.right, pass_failed)),
    }
}

fn pass_unary_op(o: CSTExpressionUnaryOp, pass_failed: &mut bool) -> CSTExpressionUnaryOp {
    CSTExpressionUnaryOp {
        op: o.op,
        expr: Box::new(pass_expr(*o.expr, pass_failed)),
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
        block: pass_block(p.block, pass_failed),
    }
}

fn pass_call(c: CSTProcedureCall, pass_failed: &mut bool) -> CSTProcedureCall {
    CSTProcedureCall {
        name: c.name,
        params: c
            .params
            .into_iter()
            .map(|i| pass_expr(i, pass_failed))
            .collect(),
        rest_param: c.rest_param.map(|i| Box::new(pass_expr(*i, pass_failed))),
    }
}

fn pass_term(t: CSTTerm, pass_failed: &mut bool) -> CSTTerm {
    CSTTerm {
        name: t.name,
        is_tterm: t.is_tterm,
        params: t
            .params
            .into_iter()
            .map(|i| pass_expr(i, pass_failed))
            .collect(),
    }
}

fn pass_accessible(a: CSTAccessible, pass_failed: &mut bool) -> CSTAccessible {
    CSTAccessible {
        head: Box::new(pass_expr(*a.head, pass_failed)),
        body: a
            .body
            .into_iter()
            .map(|i| pass_expr(i, pass_failed))
            .collect(),
    }
}

fn pass_range(r: CSTRange, pass_failed: &mut bool) -> CSTRange {
    CSTRange {
        left: r.left.map(|i| Box::new(pass_expr(*i, pass_failed))),
        right: r.right.map(|i| Box::new(pass_expr(*i, pass_failed))),
    }
}

fn pass_set(s: CSTSet, pass_failed: &mut bool) -> CSTSet {
    CSTSet {
        range: s.range.map(|i| pass_range(i, pass_failed)),
        expressions: s
            .expressions
            .into_iter()
            .map(|i| pass_expr(i, pass_failed))
            .collect(),
        rest: s.rest.map(|i| Box::new(pass_expr(*i, pass_failed))),
    }
}

fn pass_comprehension(c: CSTComprehension, pass_failed: &mut bool) -> CSTComprehension {
    CSTComprehension {
        expression: Box::new(pass_expr(*c.expression, pass_failed)),
        iterators: c
            .iterators
            .into_iter()
            .map(|i| pass_iter_param(i, pass_failed))
            .collect(),
        condition: c.condition.map(|i| Box::new(pass_expr(*i, pass_failed))),
    }
}

fn pass_collection(c: CSTCollection, pass_failed: &mut bool) -> CSTCollection {
    match c {
        CSTCollection::Set(s) => CSTCollection::Set(pass_set(s, pass_failed)),
        CSTCollection::List(s) => CSTCollection::List(pass_set(s, pass_failed)),
        CSTCollection::SetComprehension(s) => {
            CSTCollection::SetComprehension(pass_comprehension(s, pass_failed))
        }
        CSTCollection::ListComprehension(s) => {
            CSTCollection::ListComprehension(pass_comprehension(s, pass_failed))
        }
    }
}

fn pass_matrix(m: Vec<Vec<CSTExpression>>, pass_failed: &mut bool) -> Vec<Vec<CSTExpression>> {
    m.into_iter()
        .map(|i| i.into_iter().map(|j| pass_expr(j, pass_failed)).collect())
        .collect()
}

fn pass_vector(m: Vec<CSTExpression>, pass_failed: &mut bool) -> Vec<CSTExpression> {
    m.into_iter().map(|i| pass_expr(i, pass_failed)).collect()
}

fn pass_quant(q: CSTQuantifier, pass_failed: &mut bool) -> CSTQuantifier {
    CSTQuantifier {
        kind: q.kind,
        iterators: q
            .iterators
            .into_iter()
            .map(|i| pass_iter_param(i, pass_failed))
            .collect(),
        condition: Box::new(pass_expr(*q.condition, pass_failed)),
    }
}

fn pass_expr(e: CSTExpression, pass_failed: &mut bool) -> CSTExpression {
    match e {
        CSTExpression::Lambda(l) => CSTExpression::Lambda(pass_lambda(l, pass_failed)),
        CSTExpression::Op(o) => CSTExpression::Op(pass_op(o, pass_failed)),
        CSTExpression::UnaryOp(o) => CSTExpression::UnaryOp(pass_unary_op(o, pass_failed)),
        CSTExpression::Procedure(p) => CSTExpression::Procedure(pass_proc(p, pass_failed)),
        CSTExpression::Call(c) => CSTExpression::Call(pass_call(c, pass_failed)),
        CSTExpression::Term(t) => CSTExpression::Term(pass_term(t, pass_failed)),
        CSTExpression::Accessible(a) => CSTExpression::Accessible(pass_accessible(a, pass_failed)),
        CSTExpression::Collection(c) => CSTExpression::Collection(pass_collection(c, pass_failed)),
        CSTExpression::Matrix(m) => CSTExpression::Matrix(pass_matrix(m, pass_failed)),
        CSTExpression::Vector(v) => CSTExpression::Vector(pass_vector(v, pass_failed)),
        CSTExpression::Quantifier(q) => CSTExpression::Quantifier(pass_quant(q, pass_failed)),
        CSTExpression::String(s) => {
            fn split_on_dollar(input: &str) -> Option<Vec<String>> {
                enum State {
                    Normal,
                    Escaped,
                    Inside,
                    InsideEscaped,
                }

                let mut state = State::Normal;
                let mut result = Vec::new();
                let mut current = String::new();

                for c in input.chars() {
                    match state {
                        State::Normal => match c {
                            '\\' => state = State::Escaped,
                            '$' => {
                                result.push(unescape(&current)?);
                                current.clear();
                                state = State::Inside;
                            }
                            _ => current.push(c),
                        },
                        State::Escaped => {
                            current.push(c);
                            state = State::Normal;
                        }
                        State::Inside => match c {
                            '\\' => state = State::InsideEscaped,
                            '$' => {
                                result.push(unescape(&current)?);
                                current.clear();
                                state = State::Normal;
                            }
                            _ => current.push(c),
                        },
                        State::InsideEscaped => {
                            current.push(c);
                            state = State::Inside;
                        }
                    }
                }

                if matches!(state, State::Inside | State::InsideEscaped) {
                    return None;
                }

                if !current.is_empty() {
                    result.push(unescape(&current)?);
                }

                if result.is_empty() {
                    result.push("".to_string());
                }

                Some(result)
            }

            //TODO error handling
            let v = split_on_dollar(&s[1..s.len() - 1]).unwrap();
            let mut out = CSTExpression::Om;

            for (idx, i) in v.into_iter().enumerate() {
                if idx == 0 {
                    out = CSTExpression::Literal(i);
                } else if idx % 2 == 0 {
                    out = CSTExpression::Op(CSTExpressionOp {
                        op: CSTOp::Plus,
                        left: Box::new(out),
                        right: Box::new(CSTExpression::Literal(i)),
                    });
                } else {
                    //TODO error handling
                    let expr = pass_expr(
                        setlx_parse::ExprParser::new().parse(&i).unwrap(),
                        pass_failed,
                    );
                    out = CSTExpression::Op(CSTExpressionOp {
                        op: CSTOp::Plus,
                        left: Box::new(out),
                        right: Box::new(expr),
                    });
                }
            }

            out
        }

        //TODO error handling
        CSTExpression::Literal(l) => CSTExpression::Literal(unescape(&l[1..l.len() - 1]).unwrap()),
        CSTExpression::Bool(b) => CSTExpression::Bool(b),
        CSTExpression::Double(d) => CSTExpression::Double(d),
        CSTExpression::Number(i) => CSTExpression::Number(i),
        CSTExpression::Om => CSTExpression::Om,
        CSTExpression::Ignore => CSTExpression::Ignore,
        CSTExpression::Variable(s) => CSTExpression::Variable(s),
    }
}

fn pass_stmt(s: CSTStatement, pass_failed: &mut bool) -> CSTStatement {
    match s {
        CSTStatement::Class(c) => CSTStatement::Class(pass_class(c, pass_failed)),
        CSTStatement::If(i) => CSTStatement::If(pass_if(i, pass_failed)),
        CSTStatement::Switch(i) => CSTStatement::Switch(pass_if(i, pass_failed)),
        CSTStatement::Match(m) => CSTStatement::Match(pass_match(m, pass_failed)),
        CSTStatement::Scan(s) => CSTStatement::Scan(pass_scan(s, pass_failed)),
        CSTStatement::For(f) => CSTStatement::For(pass_for(f, pass_failed)),
        CSTStatement::DoWhile(w) => CSTStatement::DoWhile(pass_while(w, pass_failed)),
        CSTStatement::While(w) => CSTStatement::While(pass_while(w, pass_failed)),
        CSTStatement::TryCatch(t) => CSTStatement::TryCatch(pass_try(t, pass_failed)),
        CSTStatement::Check(c) => CSTStatement::Check(pass_check(c, pass_failed)),
        CSTStatement::Return(r) => CSTStatement::Return(pass_return(r, pass_failed)),
        CSTStatement::Assign(a) => CSTStatement::Assign(pass_assign(a, pass_failed)),
        CSTStatement::AssignMod(a) => CSTStatement::AssignMod(pass_assign_mod(a, pass_failed)),
        CSTStatement::Expression(e) => CSTStatement::Expression(pass_expr(e, pass_failed)),
        CSTStatement::Backtrack => CSTStatement::Backtrack,
        CSTStatement::Break => CSTStatement::Break,
        CSTStatement::Continue => CSTStatement::Continue,
        CSTStatement::Exit => CSTStatement::Exit,
    }
}

fn pass_block(cst: CSTBlock, pass_failed: &mut bool) -> CSTBlock {
    cst.into_iter().map(|i| pass_stmt(i, pass_failed)).collect()
}

pub fn pass(mut cst: CSTBlock, opts: &InputOpts, pass_num: u64) -> CSTBlock {
    /* - literal formatting
     * - convert strings to literals
     */
    let mut pass_failed = false;

    cst = pass_block(cst, &mut pass_failed);

    // TODO error handling
    assert!(!pass_failed);

    if opts.dump_cst_pass_string {
        cst_dump(&cst, opts, &format!("{pass_num}-string"));
    }

    cst
}
