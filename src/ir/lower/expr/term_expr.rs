use petgraph::stable_graph::NodeIndex;

use crate::ast::*;
use crate::builtin::BuiltinProc;
use crate::ir::def::*;
use crate::ir::lower::IRSharedProc;
use crate::ir::lower::expr::block_expr_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub fn tterm_ast_tag_get(tterm: &str, n_args: usize) -> Option<String> {
    match (tterm, n_args) {
        ("sum", 2) => Some(String::from("plus")),
        ("minus", 1) => Some(String::from("unaryMinus")),
        ("difference", 2) => Some(String::from("minus")),
        ("product", 2) => Some(String::from("mult")),
        ("quotient", 2) => Some(String::from("div")),
        ("modulo", 2) => Some(String::from("mod")),
        ("variable", 1) => Some(String::from("var")),
        ("cardinality", 1) => Some(String::from("card")),
        ("cartesianProduct", 2) => Some(String::from("cartesian")),
        ("power", 2) => Some(String::from("power")),
        ("factorial", 1) => Some(String::from("factor")),
        ("not", 1) => Some(String::from("not")),
        ("conjunction", 2) => Some(String::from("and")),
        ("disjunction", 2) => Some(String::from("or")),
        ("implication", 2) => Some(String::from("imply")),
        ("booleanEqual", 2) => Some(String::from("setEq")),
        ("booleanNotEqual", 2) => Some(String::from("setNeq")),
        ("equals", 2) => Some(String::from("eq")),
        ("notEqual", 2) => Some(String::from("neq")),
        ("lessThan", 2) => Some(String::from("less")),
        ("lessOrEqual", 2) => Some(String::from("leq")),
        ("greaterThan", 2) => Some(String::from("greater")),
        ("greaterOrEqual", 2) => Some(String::from("geq")),
        ("in", 2) => Some(String::from("in")),
        ("notIn", 2) => Some(String::from("notIn")),
        ("integerDivision", 2) => Some(String::from("intDiv")),
        ("sumOfMembersBinary", 2) => Some(String::from("sumMem")),
        ("productOfMembersBinary", 2) => Some(String::from("prodMem")),
        ("sumOfMembers", 1) => Some(String::from("unarySumMem")),
        ("productOfMembers", 1) => Some(String::from("unaryProdMem")),
        ("call", _) => Some(String::from("call")),
        _ => None,
    }
}

pub fn ast_tterm_tag_get(ast_tag: &str) -> Option<String> {
    match ast_tag {
        "plus" => Some(String::from("sum")),
        "unaryMinus" => Some(String::from("minus")),
        "minus" => Some(String::from("difference")),
        "mod" => Some(String::from("modulo")),
        "mult" => Some(String::from("product")),
        "div" => Some(String::from("quotient")),
        "var" => Some(String::from("variable")),
        "card" => Some(String::from("cardinality")),
        "cartesian" => Some(String::from("cartesianProduct")),
        "power" => Some(String::from("power")),
        "factor" => Some(String::from("factorial")),
        "not" => Some(String::from("not")),
        "and" => Some(String::from("conjunction")),
        "or" => Some(String::from("disjunction")),
        "imply" => Some(String::from("implication")),
        "setEq" => Some(String::from("booleanEqual")),
        "setNeq" => Some(String::from("booleanNotEqual")),
        "eq" => Some(String::from("equals")),
        "neq" => Some(String::from("notEqual")),
        "less" => Some(String::from("lessThan")),
        "leq" => Some(String::from("lessOrEqual")),
        "greater" => Some(String::from("greaterThan")),
        "geq" => Some(String::from("greaterOrEqual")),
        "in" => Some(String::from("in")),
        "notIn" => Some(String::from("notIn")),
        "intDiv" => Some(String::from("integerDivision")),
        "sumMem" => Some(String::from("sumOfMembersBinary")),
        "prodMem" => Some(String::from("productOfMembersBinary")),
        "unarySumMem" => Some(String::from("sumOfMembers")),
        "unaryProdMem" => Some(String::from("productOfMembers")),
        "call" => Some(String::from("call")),
        _ => None,
    }
}

fn term_op_bilateral(op: &str) -> bool {
    matches!(
        op,
        "plus" | "minus" | "mult" | "div" | "mod" | "power" | "cartesian" | "setEq" | "setNeq"
    )
}

/// Emits IR to check whether `t_var` is a term, tterm, or AST node,
pub fn block_term_type_check_push(
    block_idx: NodeIndex,
    t_var: IRVar,
    t_target: IRVar,
    proc: &mut IRProcedure,
) {
    /* t_type := type_of(t_var);
     * t_type_term := t_type == TYPE_TERM;
     * t_type_tterm := t_type == TYPE_TTERM;
     * t_type_ast := t_type == TYPE_AST;
     * t_target := t_type_term || t_type_tterm;
     * t_target := t_type_t || t_type_ast;
     */
    let t_type = tmp_var_new(proc);
    let t_type_term = tmp_var_new(proc);
    let t_type_tterm = tmp_var_new(proc);
    let t_type_ast = tmp_var_new(proc);

    block_get(proc, block_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type),
            types: IRType::TYPE,
            source: IRValue::BuiltinProc(BuiltinProc::TypeOf),
            op: IROp::NativeCall(vec![IRValue::Variable(t_var)]),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type_term),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type),
            op: IROp::Equal(IRValue::Type(IRType::TERM)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type_tterm),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type),
            op: IROp::Equal(IRValue::Type(IRType::TTERM)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_type_ast),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type),
            op: IROp::Equal(IRValue::Type(IRType::AST)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_target),
            types: IRType::BOOL,
            source: IRValue::Variable(t_type_term),
            op: IROp::Or(IRValue::Variable(t_type_tterm)),
        }),
        IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_target),
            types: IRType::BOOL,
            source: IRValue::Variable(t_target),
            op: IROp::Or(IRValue::Variable(t_type_ast)),
        }),
    ]);
}

pub fn block_term_op_push(
    block_idx: NodeIndex,
    dfl_idx: NodeIndex,
    follow_idx: NodeIndex,
    t_lhs: IRVar,
    lhs_owned: bool, /* consumed on success */
    t_rhs: IRVar,
    rhs_owned: bool, /* consumed on success */
    target: IRTarget,
    op: &str,
    proc: &mut IRProcedure,
) {
    /*  t_lhs_t := // block_term_type_check_push
     *  // if term_op_bilateral {
     *   t_rhs_t := // block_term_type_check_push
     *   t_lhs_t := t_lhs_t || t_rhs_t;
     *  // }
     *  if t_lhs_t
     *   goto <build_term_idx>
     *  else
     *   goto <dfl_idx>
     *
     * <build_term_idx>:
     *  // if !lhs_owned {
     *   t_lhs := copy(t_lhs);
     *  // }
     *  // if !rhs_owned {
     *   t_rhs := copy(t_rhs);
     *  // }
     *  target := ast_node_new(op, t_lhs, t_rhs);
     *  goto <follow_idx>
     */
    let t_lhs_t = tmp_var_new(proc);

    let build_term_idx = proc.blocks.add_node(Vec::new());

    block_term_type_check_push(block_idx, t_lhs, t_lhs_t, proc);
    if term_op_bilateral(op) {
        let t_rhs_t = tmp_var_new(proc);
        block_term_type_check_push(block_idx, t_rhs, t_rhs_t, proc);
        block_get(proc, block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_lhs_t),
            types: IRType::BOOL,
            source: IRValue::Variable(t_lhs_t),
            op: IROp::Or(IRValue::Variable(t_rhs_t)),
        }));
    }

    block_get(proc, block_idx).push(IRStmt::Branch(IRBranch {
        cond: IRValue::Variable(t_lhs_t),
        success: build_term_idx,
        failure: dfl_idx,
    }));

    proc.blocks.add_edge(block_idx, build_term_idx, ());
    proc.blocks.add_edge(block_idx, dfl_idx, ());

    if !lhs_owned {
        block_get(proc, build_term_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_lhs),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_lhs)]),
        }));
    }

    if !rhs_owned {
        block_get(proc, build_term_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_rhs),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_rhs)]),
        }));
    }

    block_get(proc, build_term_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target,
            types: IRType::AST,
            source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
            op: IROp::NativeCall(vec![
                IRValue::String(op.to_string()),
                IRValue::Variable(t_lhs),
                IRValue::Variable(t_rhs),
            ]),
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(build_term_idx, follow_idx, ());
}

pub fn block_term_unary_op_push(
    block_idx: NodeIndex,
    dfl_idx: NodeIndex,
    follow_idx: NodeIndex,
    t_lhs: IRVar,
    lhs_owned: bool, /* consumed on success */
    target: IRTarget,
    op: &str,
    proc: &mut IRProcedure,
) {
    /*  t_lhs_t := // block_term_type_check_push
     *  if t_lhs_t
     *   goto <build_term_idx>
     *  else
     *   goto <dfl_idx>
     *
     * <build_term_idx>:
     *  // if !lhs_owned {
     *   t_lhs := copy(t_lhs);
     *  // }
     *  target := ast_node_new(op, t_lhs);
     *  goto <follow_idx>
     */
    let t_lhs_t = tmp_var_new(proc);

    let build_term_idx = proc.blocks.add_node(Vec::new());

    block_term_type_check_push(block_idx, t_lhs, t_lhs_t, proc);

    block_get(proc, block_idx).push(IRStmt::Branch(IRBranch {
        cond: IRValue::Variable(t_lhs_t),
        success: build_term_idx,
        failure: dfl_idx,
    }));

    proc.blocks.add_edge(block_idx, build_term_idx, ());
    proc.blocks.add_edge(block_idx, dfl_idx, ());

    if !lhs_owned {
        block_get(proc, build_term_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_lhs),
            types: IRTypes!("any"),
            source: IRValue::BuiltinProc(BuiltinProc::Copy),
            op: IROp::NativeCall(vec![IRValue::Variable(t_lhs)]),
        }));
    }

    block_get(proc, build_term_idx).extend(vec![
        IRStmt::Assign(IRAssign {
            target,
            types: IRType::AST,
            source: IRValue::BuiltinProc(BuiltinProc::AstNodeNew),
            op: IROp::NativeCall(vec![
                IRValue::String(op.to_string()),
                IRValue::Variable(t_lhs),
            ]),
        }),
        IRStmt::Goto(follow_idx),
    ]);

    proc.blocks.add_edge(build_term_idx, follow_idx, ());
}

pub fn block_term_push(
    t: &CSTTerm,
    block_idx: &mut NodeIndex,
    target: IRTarget,
    proc: &mut IRProcedure,
    shared_proc: &mut IRSharedProc,
    cfg: &mut IRCfg,
) {
    /* t_term := term_new(t.name, t.params.len(), t.is_tterm);
     * t_term_addr := &t_term;
     *
     * t_i := // params[0];
     * t_term_i_ptr := t_term_addr[1];
     * *t_term_i_ptr := t_i;
     *
     * target := t_term;
     */
    let t_term = tmp_var_new(proc);
    let t_term_addr = tmp_var_new(proc);

    if let Some(tag) = tterm_ast_tag_get(&t.name, t.params.len()) {
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_term),
            types: IRType::AST,
            source: IRValue::BuiltinProc(BuiltinProc::AstNodeNewSized),
            op: IROp::NativeCall(vec![
                IRValue::String(tag),
                IRValue::Number(t.params.len().into()),
            ]),
        }));
    } else {
        block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
            target: IRTarget::Variable(t_term),
            types: if t.is_tterm {
                IRType::TTERM
            } else {
                IRType::TERM
            },
            source: IRValue::BuiltinProc(BuiltinProc::TermNew),
            op: IROp::NativeCall(vec![
                IRValue::String(t.name.to_string()),
                IRValue::Number(t.params.len().into()),
                IRValue::Bool(t.is_tterm),
            ]),
        }));
    }

    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
        target: IRTarget::Variable(t_term_addr),
        types: IRType::PTR,
        source: IRValue::Variable(t_term),
        op: IROp::PtrAddress,
    }));

    t.params.iter().enumerate().for_each(|(idx, i)| {
        let t_i = tmp_var_new(proc);
        let t_term_i_ptr = tmp_var_new(proc);
        let owned = block_expr_push(
            i,
            block_idx,
            IRTarget::Variable(t_i),
            proc,
            shared_proc,
            cfg,
        );

        if !owned {
            block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_i),
                types: IRTypes!("any"),
                source: IRValue::BuiltinProc(BuiltinProc::Copy),
                op: IROp::NativeCall(vec![IRValue::Variable(t_i)]),
            }));
        }

        block_get(proc, *block_idx).extend(vec![
            IRStmt::Assign(IRAssign {
                target: IRTarget::Variable(t_term_i_ptr),
                types: IRType::PTR,
                source: IRValue::Variable(t_term_addr),
                op: IROp::AccessArray(IRValue::Number((idx + 1).into())),
            }),
            IRStmt::Assign(IRAssign {
                target: IRTarget::Deref(t_term_i_ptr),
                types: IRTypes!("any"),
                source: IRValue::Variable(t_i),
                op: IROp::Assign,
            }),
        ]);
    });

    block_get(proc, *block_idx).push(IRStmt::Assign(IRAssign {
        target,
        types: IRType::TTERM | IRType::AST | IRType::TERM,
        source: IRValue::Variable(t_term),
        op: IROp::Assign,
    }));
}
