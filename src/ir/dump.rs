use petgraph::stable_graph::NodeIndex;
use petgraph::visit::IntoNodeReferences;
use std::io::Write;

use crate::cli::InputOpts;
use crate::ir::def::*;
use crate::util::file::debug_file_create;

fn ir_dump_val(a: &IRValue, out: &mut String) {
    match a {
        IRValue::Undefined => out.push_str("om"),
        IRValue::Variable(i) => out.push_str(&format!("t{i}")),
        IRValue::Number(i) => out.push_str(&format!("{i}")),
        IRValue::Double(i) => out.push_str(&format!("{i}")),
        IRValue::String(i) => out.push_str(&format!("\"{i}\"")),
        IRValue::Bool(b) => out.push_str(&format!("{:?}", b)),
        IRValue::Procedure(p) => out.push_str(&format!("_{} /* procedure */", p.index())),
        IRValue::Matrix(m) => {
            out.push_str("<<");
            m.iter().for_each(|v| {
                out.push('\n');
                out.push_str("<<");
                v.iter().enumerate().for_each(|(idx, i)| {
                    if idx != 0 {
                        out.push_str(", ");
                    }

                    ir_dump_val(i, out);
                });
                out.push_str(">>");
            });

            out.push_str("\n>>");
        }
        IRValue::Vector(v) => {
            out.push_str("<<");
            v.iter().enumerate().for_each(|(idx, i)| {
                if idx != 0 {
                    out.push_str(", ");
                }

                ir_dump_val(i, out);
            });
            out.push_str(">>");
        }
        IRValue::BuiltinProc(p) => out.push_str(&format!("{}", p)),
        IRValue::BuiltinVar(p) => out.push_str(&format!("{}", p)),
        IRValue::Type(p) => out.push_str(&format!("TYPE_{}", p)),
    }
}

fn ir_dump_stmt_assign(a: &IRAssign, out: &mut String) {
    match a.target {
        IRTarget::Ignore => out.push('_'),
        IRTarget::Variable(i) => out.push_str(&format!("t{i}")),
        IRTarget::Deref(i) => out.push_str(&format!("*t{i}")),
    }

    out.push(':');
    out.push_str(&format!("{}", &a.types));

    out.push_str(" = ");

    match &a.op {
        IROp::PtrAddress => out.push('&'),
        IROp::PtrDeref => out.push('*'),
        IROp::Not => out.push('!'),
        _ => (),
    }

    ir_dump_val(&a.source, out);

    match &a.op {
        IROp::AccessArray(i) => {
            out.push('[');
            ir_dump_val(i, out);
            out.push(']');
        }
        IROp::Call(c) => {
            out.push('(');
            out.push_str(&format!("t{}", c));
            out.push(')');
        }
        IROp::NativeCall(c) => {
            out.push('(');
            let mut is_first = true;
            for p in c {
                if !is_first {
                    out.push_str(", ");
                }
                is_first = false;

                ir_dump_val(p, out);
            }
            out.push(')');
        }
        IROp::Or(i) => {
            out.push_str(" || ");
            ir_dump_val(i, out);
        }
        IROp::And(i) => {
            out.push_str(" && ");
            ir_dump_val(i, out);
        }
        IROp::Less(i) => {
            out.push_str(" < ");
            ir_dump_val(i, out);
        }
        IROp::Equal(i) => {
            out.push_str(" == ");
            ir_dump_val(i, out);
        }
        IROp::Plus(i) => {
            out.push_str(" + ");
            ir_dump_val(i, out);
        }
        IROp::Minus(i) => {
            out.push_str(" - ");
            ir_dump_val(i, out);
        }
        IROp::Mult(i) => {
            out.push_str(" * ");
            ir_dump_val(i, out);
        }
        IROp::Divide(i) => {
            out.push_str(" / ");
            ir_dump_val(i, out);
        }
        IROp::IntDivide(i) => {
            out.push_str(" \\ ");
            ir_dump_val(i, out);
        }
        IROp::Mod(i) => {
            out.push_str(" % ");
            ir_dump_val(i, out);
        }
        _ => (),
    }

    out.push(';');
}

fn ir_dump_stmt_br(a: &IRBranch, out: &mut String) {
    out.push_str("if ");
    ir_dump_val(&a.cond, out);
    out.push_str(" \n\t");
    out.push_str(&format!("\tgoto <bb{}>\n\t", a.success.index()));
    out.push_str("else\n\t");
    out.push_str(&format!("\tgoto <bb{}>\n", a.failure.index()));
}

fn ir_dump_stmt_try(a: &IRTry, out: &mut String) {
    out.push_str("try\n\t");
    out.push_str(&format!("\tgoto <bb{}>\n\t", a.attempt.index()));
    out.push_str("catch\n\t");
    out.push_str(&format!("\tgoto <bb{}>\n", a.catch.index()));
}

fn ir_dump_stmt(stmt: &IRStmt, out: &mut String) {
    out.push('\t');

    match stmt {
        IRStmt::Assign(a) => ir_dump_stmt_assign(a, out),
        IRStmt::Branch(a) => ir_dump_stmt_br(a, out),
        IRStmt::Return(a) => {
            out.push_str("return ");
            ir_dump_val(a, out);
            out.push_str(";\n");
        }
        IRStmt::Try(a) => ir_dump_stmt_try(a, out),
        IRStmt::TryEnd => out.push_str("//try_end;"),
        IRStmt::Goto(idx) => out.push_str(&format!("goto <bb{}>\n", idx.index())),
        IRStmt::Unreachable => out.push_str("unreachable;"),
    }

    out.push('\n');
}

fn ir_dump_block(procedure: &IRProcedure, idx: NodeIndex, bb: &IRBlock, out: &mut String) {
    if procedure.start_block == idx {
        out.push_str("//start block\n");
    }

    if procedure.end_block == idx {
        out.push_str("//end block\n");
    }

    out.push_str(&format!("<bb{}>:\n", idx.index()));

    for stmt in bb {
        ir_dump_stmt(stmt, out);
    }
}

fn ir_dump_procedure(cfg: &IRCfg, idx: NodeIndex, procedure: &IRProcedure, out: &mut String) {
    if cfg.main == idx {
        out.push_str("//main\n");
    }

    out.push_str(&format!("_{}(){{\n", idx.index()));

    for (idx, bb) in procedure.blocks.node_references() {
        ir_dump_block(procedure, idx, bb, out);
    }

    out.push('}');
}

pub fn ir_dump_str(cfg: &IRCfg) -> String {
    let mut out = String::from("");

    for (idx, procedure) in cfg.procedures.node_references() {
        ir_dump_procedure(cfg, idx, procedure, &mut out);
        if idx.index() + 1 < cfg.procedures.node_count() {
            out.push_str("\n\n");
        }
    }

    out
}

pub fn ir_dump(cfg: &IRCfg, opts: &InputOpts, pass_name: &str) {
    let mut file = debug_file_create(format!("{}-ir-{pass_name}.dump", &opts.stem));
    writeln!(&mut file, "{:?}", cfg).unwrap();

    let ir_str = ir_dump_str(cfg);
    let mut file = debug_file_create(format!("{}-ir-{pass_name}.ir", &opts.stem));
    file.write_all(ir_str.as_bytes()).unwrap();
}
