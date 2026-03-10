use crate::ast::*;
use crate::interp::heap::{InterpList, InterpObj, InterpTaggedList, InterpVal};
use std::str::FromStr;

trait InterpAst {
    fn to_tagged(&self) -> Option<InterpTaggedList>;
    fn to_list(&self) -> Option<&InterpList>;
    fn to_immed_str(&self) -> Option<String>;
    fn to_immed_bool(&self) -> Option<bool>;
}

impl InterpAst for InterpVal {
    fn to_tagged(&self) -> Option<InterpTaggedList> {
        match self {
            InterpVal::Ref(r) => match unsafe { &*r.0 } {
                InterpObj::Ast(tl) | InterpObj::Term(tl) | InterpObj::TTerm(tl) => Some(tl.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    fn to_list(&self) -> Option<&InterpList> {
        match self {
            InterpVal::Ref(r) => match unsafe { &*r.0 } {
                InterpObj::List(l) => {
                    // SAFETY: the ref is alive for at least as long as self
                    Some(unsafe { &*(l as *const InterpList) })
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn to_immed_str(&self) -> Option<String> {
        match self {
            InterpVal::Ref(r) => match unsafe { &*r.0 } {
                InterpObj::String(s) => Some(s.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    fn to_immed_bool(&self) -> Option<bool> {
        match self {
            InterpVal::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

pub fn ast_to_cst_expr(node: &InterpVal) -> Option<CSTExpression> {
    let kind = if let InterpVal::Ref(r) = node {
        match unsafe { &*r.0 } {
            InterpObj::Ast(tl) | InterpObj::Term(tl) | InterpObj::TTerm(tl) => {
                return ast_to_cst_expr_tagged(tl);
            }
            InterpObj::String(s) => CSTExpressionKind::String(s.clone()),
            InterpObj::Number(n) => CSTExpressionKind::Number(n.clone()),
            InterpObj::Set(s) => CSTExpressionKind::Collection(CSTCollection::Set(CSTSet {
                range: None,
                expressions: s
                    .0
                    .iter()
                    .map(ast_to_cst_expr)
                    .collect::<Option<Vec<_>>>()?,
                rest: None,
            })),
            InterpObj::List(l) => CSTExpressionKind::Collection(CSTCollection::List(CSTSet {
                range: None,
                expressions: l
                    .0
                    .iter()
                    .map(ast_to_cst_expr)
                    .collect::<Option<Vec<_>>>()?,
                rest: None,
            })),
            _ => return None,
        }
    } else {
        match node {
            InterpVal::Bool(b) => CSTExpressionKind::Bool(*b),
            InterpVal::Double(d) => CSTExpressionKind::Double(*d),
            InterpVal::Char(c) => CSTExpressionKind::String(c.to_string()),
            _ => return None,
        }
    };
    Some(CSTExpression {
        lhs: 0,
        rhs: 0,
        kind,
    })
}

fn ast_to_cst_expr_tagged(node: &InterpTaggedList) -> Option<CSTExpression> {
    let kind = match node.tag.as_str() {
        "var" => CSTExpressionKind::Variable(node.list[0].to_immed_str()?),
        "literal" => CSTExpressionKind::Literal(node.list[0].to_immed_str()?),
        "string" => CSTExpressionKind::String(node.list[0].to_immed_str()?),
        "om" => CSTExpressionKind::Om,
        "ignore" => CSTExpressionKind::Ignore,
        "call" => {
            let name = if let InterpVal::Ref(r) = node.list[0]
                && let InterpObj::Ast(a) = unsafe { &*r.0 }
                && &a.tag == "callName"
            {
                a.list[0].to_immed_str()?
            } else {
                return None;
            };
            let call_params = node.list[1]
                .to_list()
                .into_iter()
                .flat_map(|l| l.0.iter())
                .map(ast_to_cst_expr)
                .collect::<Option<Vec<_>>>()?;
            let rest_param = ast_to_cst_expr(&node.list[2]).map(Box::new);
            CSTExpressionKind::Call(CSTProcedureCall {
                name,
                params: call_params,
                rest_param,
            })
        }
        "term" | "tterm" => {
            let is_tterm = node.tag == "tterm";
            let name = node.list[0].to_immed_str()?;
            let term_params = node.list[1]
                .to_list()
                .into_iter()
                .flat_map(|l| l.0.iter())
                .map(ast_to_cst_expr)
                .collect::<Option<Vec<_>>>()?;
            CSTExpressionKind::Term(CSTTerm {
                name,
                is_tterm,
                params: term_params,
            })
        }
        "lambda" => {
            let col = node.list[0]
                .to_tagged()
                .and_then(|a| ast_to_cst_collection(&a))?;
            let is_closure = node.list[1].to_immed_bool()?;
            let expr = Box::new(ast_to_cst_expr(&node.list[2])?);
            CSTExpressionKind::Lambda(CSTLambda {
                params: col,
                is_closure,
                expr,
            })
        }
        "procedure" | "cachedProcedure" | "closure" => {
            let kind = CSTProcedureKind::from_str(&node.tag).ok()?;
            let proc_params = node.list[0]
                .to_list()
                .into_iter()
                .flat_map(|l| l.0.iter())
                .map(|i| i.to_tagged().and_then(|a| ast_to_cst_param(&a)))
                .collect::<Option<Vec<_>>>()?;
            let list_param = node.list[1].to_tagged().and(node.list[1].to_immed_str());
            let block = ast_to_cst_block(&node.list[2])?;
            CSTExpressionKind::Procedure(CSTProcedure {
                kind,
                params: proc_params,
                list_param,
                block,
            })
        }
        "accessible" => {
            let head = Box::new(ast_to_cst_expr(&node.list[0])?);
            let body = node.list[1]
                .to_list()
                .into_iter()
                .flat_map(|l| l.0.iter())
                .map(ast_to_cst_expr)
                .collect::<Option<Vec<_>>>()?;
            CSTExpressionKind::Accessible(CSTAccessible { head, body })
        }
        "list" | "set" => {
            CSTExpressionKind::Collection(ast_to_cst_set_or_list(&node.tag.clone(), node)?)
        }
        "listComprehension" | "setComprehension" => {
            CSTExpressionKind::Collection(ast_to_cst_comprehension(&node.tag.clone(), node)?)
        }
        "matrix" => {
            let rows = node.list[0]
                .to_list()
                .into_iter()
                .flat_map(|l| l.0.iter())
                .map(|row| {
                    row.to_list()
                        .into_iter()
                        .flat_map(|l| l.0.iter())
                        .map(ast_to_cst_expr)
                        .collect::<Option<Vec<_>>>()
                })
                .collect::<Option<Vec<_>>>()?;
            CSTExpressionKind::Matrix(rows)
        }
        "vector" => {
            let exprs = node.list[0]
                .to_list()
                .into_iter()
                .flat_map(|l| l.0.iter())
                .map(ast_to_cst_expr)
                .collect::<Option<Vec<_>>>()?;
            CSTExpressionKind::Vector(exprs)
        }
        "exists" | "forall" => {
            let kind = CSTQuantifierKind::from_str(&node.tag).ok()?;
            let iterators = node.list[0]
                .to_list()
                .into_iter()
                .flat_map(|l| l.0.iter())
                .filter_map(|i| i.to_tagged())
                .filter_map(|a| ast_to_cst_iter_param(&a))
                .collect();
            let condition = Box::new(ast_to_cst_expr(&node.list[1])?);
            CSTExpressionKind::Quantifier(CSTQuantifier {
                kind,
                iterators,
                condition,
            })
        }
        tag => {
            if let Ok(op) = CSTOp::from_str(tag) {
                let left = Box::new(ast_to_cst_expr(&node.list[0])?);
                let right = Box::new(ast_to_cst_expr(&node.list[1])?);
                CSTExpressionKind::Op(CSTExpressionOp { op, left, right })
            } else if let Ok(op) = CSTUnaryOp::from_str(tag) {
                let expr = Box::new(ast_to_cst_expr(&node.list[0])?);
                CSTExpressionKind::UnaryOp(CSTExpressionUnaryOp { op, expr })
            } else {
                return None;
            }
        }
    };

    Some(CSTExpression {
        lhs: 0,
        rhs: 0,
        kind,
    })
}

pub fn ast_to_cst_stmt(node: &InterpTaggedList) -> Option<CSTStatement> {
    let kind = match node.tag.as_str() {
        "assign" => {
            let assign = ast_to_cst_expr(&node.list[0])?;
            let expr = Box::new(node.list[1].to_tagged().and_then(|a| ast_to_cst_stmt(&a))?);
            CSTStatementKind::Assign(CSTAssign { assign, expr })
        }
        "return" => {
            let val = ast_to_cst_expr(&node.list[0]);
            CSTStatementKind::Return(CSTReturn { val })
        }
        "if" | "switch" => {
            let branches = node.list[0]
                .to_list()
                .into_iter()
                .flat_map(|l| l.0.iter())
                .map(|i| {
                    i.to_tagged().and_then(|b| {
                        let condition = ast_to_cst_expr(&b.list[0])?;
                        let block = ast_to_cst_block(&b.list[1])?;
                        Some(CSTIfBranch { condition, block })
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            let alternative = node.list[1]
                .to_tagged()
                .and_then(|_| ast_to_cst_block(&node.list[1]));
            let i = CSTIf {
                branches,
                alternative,
            };
            if node.tag == "if" {
                CSTStatementKind::If(i)
            } else {
                CSTStatementKind::Switch(i)
            }
        }
        "match" => {
            let expression = ast_to_cst_expr(&node.list[0])?;
            let branches = node.list[1]
                .to_list()
                .into_iter()
                .flat_map(|l| l.0.iter())
                .map(|i| i.to_tagged().and_then(|a| ast_to_cst_match_branch(&a)))
                .collect::<Option<Vec<_>>>()?;
            let default = ast_to_cst_block(&node.list[2])?;
            CSTStatementKind::Match(CSTMatch {
                expression,
                branches,
                default,
            })
        }
        "scan" => {
            let expression = ast_to_cst_expr(&node.list[0])?;
            let variable = node.list[1].to_tagged().and(node.list[1].to_immed_str());
            let branches = node.list[2]
                .to_list()
                .into_iter()
                .flat_map(|l| l.0.iter())
                .map(|i| i.to_tagged().and_then(|a| ast_to_cst_match_branch(&a)))
                .collect::<Option<Vec<_>>>()?;
            CSTStatementKind::Scan(CSTScan {
                expression,
                variable,
                branches,
                default: None,
            })
        }
        "while" | "doWhile" => {
            let condition = ast_to_cst_expr(&node.list[0])?;
            let block = ast_to_cst_block(&node.list[1])?;
            let w = CSTWhile { condition, block };
            if node.tag == "while" {
                CSTStatementKind::While(w)
            } else {
                CSTStatementKind::DoWhile(w)
            }
        }
        "for" => {
            let iter_params = node.list[0]
                .to_list()
                .into_iter()
                .flat_map(|l| l.0.iter())
                .map(|i| i.to_tagged().and_then(|a| ast_to_cst_iter_param(&a)))
                .collect::<Option<Vec<_>>>()?;
            let condition = ast_to_cst_expr(&node.list[1]).map(Box::new);
            let block = ast_to_cst_block(&node.list[2])?;
            CSTStatementKind::For(CSTFor {
                params: iter_params,
                condition,
                block,
            })
        }
        "tryCatch" => {
            let try_branch = ast_to_cst_block(&node.list[0])?;
            let catch_branches = node.list[1]
                .to_list()
                .into_iter()
                .flat_map(|l| l.0.iter())
                .map(|i| {
                    i.to_tagged().and_then(|c| {
                        let kind = CSTCatchKind::from_str(&c.tag).ok()?;
                        let exception = c.list[0].to_immed_str()?;
                        let block = ast_to_cst_block(&c.list[1])?;
                        Some(CSTCatch {
                            kind,
                            exception,
                            block,
                        })
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            CSTStatementKind::TryCatch(CSTTryCatch {
                try_branch,
                catch_branches,
            })
        }
        "check" => {
            let block = ast_to_cst_block(&node.list[0])?;
            let after_backtrack = ast_to_cst_block(&node.list[1])?;
            CSTStatementKind::Check(CSTCheck {
                block,
                after_backtrack,
            })
        }
        "class" => {
            let name = node.list[0].to_immed_str()?;
            let class_params = node.list[1]
                .to_list()
                .into_iter()
                .flat_map(|l| l.0.iter())
                .map(|i| i.to_tagged().and_then(|a| ast_to_cst_param(&a)))
                .collect::<Option<Vec<_>>>()?;
            let block = ast_to_cst_block(&node.list[2])?;
            let static_block = node.list[3]
                .to_tagged()
                .and_then(|_| ast_to_cst_block(&node.list[3]));
            CSTStatementKind::Class(CSTClass {
                name,
                params: class_params,
                block,
                static_block,
            })
        }
        "backtrack" => CSTStatementKind::Backtrack,
        "break" => CSTStatementKind::Break,
        "Continue" => CSTStatementKind::Continue,
        "exit" => CSTStatementKind::Exit,
        _ => CSTStatementKind::Expression(ast_to_cst_expr_tagged(node)?),
    };

    Some(CSTStatement {
        lhs: 0,
        rhs: 0,
        kind,
    })
}

pub fn ast_to_cst_block(v: &InterpVal) -> Option<CSTBlock> {
    v.to_list()?
        .0
        .iter()
        .map(|i| i.to_tagged().and_then(|a| ast_to_cst_stmt(&a)))
        .collect()
}

fn ast_to_cst_param(node: &InterpTaggedList) -> Option<CSTParam> {
    Some(CSTParam {
        name: node.tag.clone(),
        is_rw: node.list[0].to_immed_bool()?,
        default: None,
    })
}

fn ast_to_cst_iter_param(node: &InterpTaggedList) -> Option<CSTIterParam> {
    let variable = ast_to_cst_expr(&node.list[0])?;
    let collection = ast_to_cst_expr(&node.list[1])?;
    Some(CSTIterParam {
        variable,
        collection,
    })
}

fn ast_to_cst_match_branch(node: &InterpTaggedList) -> Option<CSTMatchBranch> {
    match node.tag.as_str() {
        "matchBranchCase" => {
            let expressions = node.list[0]
                .to_list()
                .into_iter()
                .flat_map(|l| l.0.iter())
                .map(ast_to_cst_expr)
                .collect::<Option<Vec<_>>>()?;
            let condition = ast_to_cst_expr(&node.list[1]);
            let statements = ast_to_cst_block(&node.list[2])?;
            Some(CSTMatchBranch::Case(CSTMatchBranchCase {
                expressions,
                condition,
                statements,
            }))
        }
        "matchBranchRegex" => {
            let pattern = ast_to_cst_expr(&node.list[0])?;
            let pattern_out = ast_to_cst_expr(&node.list[1]);
            let condition = ast_to_cst_expr(&node.list[2]);
            let statements = ast_to_cst_block(&node.list[3])?;
            Some(CSTMatchBranch::Regex(CSTMatchBranchRegex {
                pattern,
                pattern_out,
                condition,
                statements,
            }))
        }
        _ => None,
    }
}

fn ast_to_cst_set_or_list(tag: &str, node: &InterpTaggedList) -> Option<CSTCollection> {
    let range_lhs = ast_to_cst_expr(&node.list[0]).map(Box::new);
    let range_rhs = ast_to_cst_expr(&node.list[1]).map(Box::new);
    let range = if range_lhs.is_some() || range_rhs.is_some() {
        Some(CSTRange {
            left: range_lhs,
            right: range_rhs,
        })
    } else {
        None
    };
    let expressions = node.list[2]
        .to_list()
        .into_iter()
        .flat_map(|l| l.0.iter())
        .map(ast_to_cst_expr)
        .collect::<Option<Vec<_>>>()?;
    let rest = ast_to_cst_expr(&node.list[3]).map(Box::new);
    let s = CSTSet {
        range,
        expressions,
        rest,
    };
    Some(if tag == "set" {
        CSTCollection::Set(s)
    } else {
        CSTCollection::List(s)
    })
}

fn ast_to_cst_comprehension(tag: &str, node: &InterpTaggedList) -> Option<CSTCollection> {
    let expression = Box::new(ast_to_cst_expr(&node.list[0])?);
    let iterators = node.list[1]
        .to_list()
        .into_iter()
        .flat_map(|l| l.0.iter())
        .map(|i| i.to_tagged().and_then(|a| ast_to_cst_iter_param(&a)))
        .collect::<Option<Vec<_>>>()?;
    let condition = ast_to_cst_expr(&node.list[2]).map(Box::new);
    let c = CSTComprehension {
        expression,
        iterators,
        condition,
    };
    Some(if tag == "setComprehension" {
        CSTCollection::SetComprehension(c)
    } else {
        CSTCollection::ListComprehension(c)
    })
}

fn ast_to_cst_collection(node: &InterpTaggedList) -> Option<CSTCollection> {
    match node.tag.as_str() {
        "list" => ast_to_cst_set_or_list("list", node),
        "set" => ast_to_cst_set_or_list("set", node),
        "listComprehension" => ast_to_cst_comprehension("listComprehension", node),
        "setComprehension" => ast_to_cst_comprehension("setComprehension", node),
        _ => None,
    }
}
