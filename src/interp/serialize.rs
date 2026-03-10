use bitflags::bitflags;
use rustyline::DefaultEditor;
use std::cell::RefCell;
use std::rc::Rc;

use crate::builtin::BuiltinProc;
use crate::cli::InputOpts;
use crate::interp::debug::DebugData;
use crate::interp::exec::exec_proc;
use crate::interp::heap::*;
use crate::interp::memoize::InterpMemoize;
use crate::interp::stack::InterpStack;
use crate::ir::def::*;
use crate::ir::lower::expr::access_expr::block_obj_call_impl_push;
use crate::ir::lower::util::{block_get, tmp_var_new};

bitflags! {
    #[derive(Clone, Copy, Default)]
    pub struct SerializeOpts: u32 {
        const ESCAPE_STR = 1 << 0;
        const AST_BLOCK = 1 << 1;
        const AST_PARAMS = 1 << 2;
        const AST_IF_BRANCHES = 1 << 3;
        const AST_SWITCH_BRANCHES = 1 << 4;
        const AST_IF_BRANCH_FIRST = 1 << 5;
        const UNINIT_TRUE = 1 << 6;
        const UNINIT_LIST = 1 << 7;
        const AST_CATCH_BLOCKS = 1 << 8;
        const AST_SET_EXPRESSIONS = 1 << 9;
        const AST_RANGE = 1 << 10;
        const ESCAPE_STR_LIT = 1 << 11;
        const AST_NOT_BODY = 1 << 12;
        const AST_IMPLY_BODY = 1 << 13;
        const AST_MULT_BODY = 1 << 14;
        const AST_AND_BODY = 1 << 15;
        const AST_OR_BODY = 1 << 16;
        const AST_POW_BODY = 1 << 17;
        const AST_ADD_BODY = 1 << 18;
        const AST_LHS = 1 << 19;
        const AST_RHS = 1 << 20;
    }
}

fn needs_parens_power(opts: SerializeOpts) -> bool {
    opts.contains(SerializeOpts::AST_POW_BODY | SerializeOpts::AST_LHS)
}

fn needs_parens_multiplicative(opts: SerializeOpts) -> bool {
    opts.intersects(SerializeOpts::AST_POW_BODY)
        || opts.contains(SerializeOpts::AST_MULT_BODY | SerializeOpts::AST_RHS)
}

fn needs_parens_additive(opts: SerializeOpts) -> bool {
    opts.intersects(SerializeOpts::AST_MULT_BODY | SerializeOpts::AST_POW_BODY)
        || opts.contains(SerializeOpts::AST_ADD_BODY | SerializeOpts::AST_RHS)
}

fn needs_parens_and(opts: SerializeOpts) -> bool {
    opts.intersects(SerializeOpts::AST_NOT_BODY)
}

fn needs_parens_or(opts: SerializeOpts) -> bool {
    opts.intersects(SerializeOpts::AST_AND_BODY | SerializeOpts::AST_NOT_BODY)
}

fn needs_parens_imply(opts: SerializeOpts) -> bool {
    opts.intersects(SerializeOpts::AST_AND_BODY | SerializeOpts::AST_OR_BODY)
        || opts.contains(SerializeOpts::AST_IMPLY_BODY | SerializeOpts::AST_LHS)
}

fn serialize_ast(
    a: &InterpTaggedList,
    vars: &mut [InterpVal],
    stack: &mut InterpStack,
    memo: &mut InterpMemoize,
    cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    iopts: &InputOpts,
    rl: &mut DefaultEditor,
    opts: SerializeOpts,
) -> String {
    match a.tag.as_str() {
        "class" => {
            format!(
                "class {} ({}) {{ {} static {{ {} }} }}",
                /* name */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::default()
                ),
                /* params */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_PARAMS
                ),
                /* block */
                serialize(
                    &a.list[2],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_BLOCK
                ),
                /* static */
                serialize(
                    &a.list[3],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_BLOCK
                ),
            )
        }
        "scan" => {
            format!(
                "scan ({}) using {} {{ {} }}",
                /* expr */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* variable */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::default()
                ),
                /* branches */
                serialize(
                    &a.list[2],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::default()
                ),
            )
        }
        "for" => {
            format!(
                "for ({} | {}) {{ {} }}",
                /* params */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_PARAMS
                ),
                /* condition */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* block */
                serialize(
                    &a.list[2],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_BLOCK
                ),
            )
        }
        "while" => {
            format!(
                "while ({}), {{ {} }}",
                /* cond */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* block */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_BLOCK
                ),
            )
        }
        "if" => {
            format!(
                "{} else {{ {} }}",
                /* branches */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_IF_BRANCHES
                ),
                /* else */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_BLOCK
                ),
            )
        }
        "switch" => {
            format!(
                "switch {{ {} default: {} }}",
                /* branches */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_BLOCK
                ),
                /* default */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_BLOCK
                ),
            )
        }
        "ifBranch" => {
            if opts.contains(SerializeOpts::AST_SWITCH_BRANCHES) {
                format!(
                    "case {}: {}",
                    /* cond */
                    serialize(
                        &a.list[0],
                        vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        iopts,
                        rl,
                        SerializeOpts::ESCAPE_STR
                    ),
                    /* block */
                    serialize(
                        &a.list[1],
                        vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        iopts,
                        rl,
                        SerializeOpts::AST_BLOCK
                    ),
                )
            } else if opts.contains(SerializeOpts::AST_IF_BRANCH_FIRST) {
                format!(
                    "else if ({}) {{ {} }}",
                    /* cond */
                    serialize(
                        &a.list[0],
                        vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        iopts,
                        rl,
                        SerializeOpts::ESCAPE_STR
                    ),
                    /* block */
                    serialize(
                        &a.list[1],
                        vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        iopts,
                        rl,
                        SerializeOpts::AST_BLOCK
                    ),
                )
            } else
            /* if opts.contains(SerializeOpts::AST_IF_BRANCHES) */
            {
                format!(
                    "if ({}) {{ {} }}",
                    /* cond */
                    serialize(
                        &a.list[0],
                        vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        iopts,
                        rl,
                        SerializeOpts::ESCAPE_STR
                    ),
                    /* block */
                    serialize(
                        &a.list[1],
                        vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        iopts,
                        rl,
                        SerializeOpts::AST_BLOCK
                    ),
                )
            }
        }
        "match" => {
            format!(
                "match ({}) {{ {} default: {}}}",
                /* cond */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* branches */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_SWITCH_BRANCHES
                ),
                /* default */
                serialize(
                    &a.list[2],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_BLOCK
                ),
            )
        }
        "matchBranchCase" => {
            format!(
                "case: {} | {}: {}",
                /* expr */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* cond */
                serialize(
                    &a.list[2],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::UNINIT_TRUE
                ),
                /* stmt */
                serialize(
                    &a.list[3],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_BLOCK
                ),
            )
        }
        "matchBranchRegex" => {
            format!(
                "regex: {} as {} | {}: {}",
                /* pattern */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* pattern out */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::UNINIT_LIST
                ),
                /* cond */
                serialize(
                    &a.list[2],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::UNINIT_TRUE
                ),
                /* stmt */
                serialize(
                    &a.list[3],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_BLOCK
                ),
            )
        }
        "iterParam" => {
            format!(
                "{} in {}",
                /* variable */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* collection */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "param" => unimplemented!(),
        "tryCatch" => {
            format!(
                "try {{ {} }} {}",
                /* try */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_BLOCK
                ),
                /* catch */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_CATCH_BLOCKS
                ),
            )
        }
        "catchUsr" => {
            format!(
                "catch ({}) {{ {} }}",
                /* exception */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::default()
                ),
                /* block */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_BLOCK
                ),
            )
        }
        "catchLng" => {
            format!(
                "catchLng ({}) {{ {} }}",
                /* exception */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::default()
                ),
                /* block */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_BLOCK
                ),
            )
        }
        "catchFinal" => {
            format!(
                "final ({}) {{ {} }}",
                /* exception */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::default()
                ),
                /* block */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_BLOCK
                ),
            )
        }
        "check" => unimplemented!(),
        "return" => format!(
            "return {}",
            /* expr */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
        ),
        "assign" => format!(
            "{} := {}",
            /* assign */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
            /* expr */
            serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
        ),
        "plusEq" => format!(
            "{} += {}",
            /* assign */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
            /* expr */
            serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
        ),
        "minusEq" => format!(
            "{} -= {}",
            /* assign */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
            serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
        ),
        "multEq" => format!(
            "{} *= {}",
            /* assign */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
            /* expr */
            serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
        ),
        "divEq" => format!(
            "{} /= {}",
            /* assign */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
            /* expr */
            serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
        ),
        "intDivEq" => format!(
            "{} \\= {}",
            /* assign */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
            /* expr */
            serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
        ),
        "backtrack" => String::from("backtrack"),
        "break" => String::from("break"),
        "continue" => String::from("continue"),
        "exit" => String::from("exit"),
        "modEq" => format!(
            "{} %= {}",
            /* assign */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
            /* expr */
            serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
        ),
        "set" => {
            format!(
                "{{ [{}..{}] {} | {} }}",
                /* range lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* range rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* expressions */
                serialize(
                    &a.list[2],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_SET_EXPRESSIONS
                ),
                /* rest */
                serialize(
                    &a.list[3],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "list" => {
            format!(
                "[ {} [{}..{}] | {} ]",
                /* range lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* range rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* expressions */
                serialize(
                    &a.list[2],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_SET_EXPRESSIONS
                ),
                /* rest */
                serialize(
                    &a.list[3],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "setComprehension" => {
            format!(
                "{{ {}: {} | {} }}",
                /* expression */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* iterators */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_PARAMS
                ),
                /* cond */
                serialize(
                    &a.list[2],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::UNINIT_TRUE
                ),
            )
        }
        "listComprehension" => {
            format!(
                "[ {}: {} | {} ]",
                /* expression */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* iterators */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::AST_PARAMS
                ),
                /* cond */
                serialize(
                    &a.list[2],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::UNINIT_TRUE
                ),
            )
        }
        "lambda" => format!(
            "{} {} {};",
            /* params */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::default()
            ),
            /* is closure */
            if let InterpVal::Bool(is_closure) = &a.list[1]
                && *is_closure
            {
                "|=>"
            } else {
                "|->"
            },
            /* expression */
            serialize(
                &a.list[2],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
        ),
        "imply" => {
            let lhs = serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR | SerializeOpts::AST_IMPLY_BODY | SerializeOpts::AST_LHS,
            );

            let rhs = serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR,
            );

            if needs_parens_imply(opts) {
                format!("({lhs} => {rhs})")
            } else {
                format!("{lhs} => {rhs}")
            }
        }
        "or" => {
            let lhs = serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR | SerializeOpts::AST_OR_BODY,
            );
            let rhs = serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR,
            );
            if needs_parens_or(opts) {
                format!("({} || {})", lhs, rhs)
            } else {
                format!("{} || {}", lhs, rhs)
            }
        }
        "and" => {
            let inner = format!(
                "{} && {}",
                /* lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR | SerializeOpts::AST_AND_BODY
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR | SerializeOpts::AST_AND_BODY
                ),
            );
            if needs_parens_and(opts) {
                format!("({inner})")
            } else {
                inner
            }
        }
        "eq" => {
            format!(
                "{} == {}",
                /* lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "neq" => {
            format!(
                "{} != {}",
                /* lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "less" => {
            format!(
                "{} < {}",
                /* lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "leq" => {
            format!(
                "{} <= {}",
                /* lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "greater" => {
            format!(
                "{} > {}",
                /* lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "geq" => {
            format!(
                "{} >= {}",
                /* lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "in" => {
            format!(
                "{} in {}",
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "notIn" => {
            format!(
                "{} notin {}",
                /* lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "plus" => {
            let inner = format!(
                "{} + {}",
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR | SerializeOpts::AST_ADD_BODY
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    /* SerializeOpts::AST_RHS needlessly adds a bracket for RHS addition. This
                     * is done to be compatible with the reference implementation */
                    SerializeOpts::ESCAPE_STR
                        | SerializeOpts::AST_ADD_BODY
                        | SerializeOpts::AST_RHS
                ),
            );

            if needs_parens_additive(opts) {
                format!("({inner})")
            } else {
                inner
            }
        }
        "minus" => {
            let inner = format!(
                "{} - {}",
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR | SerializeOpts::AST_ADD_BODY
                ),
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                        | SerializeOpts::AST_ADD_BODY
                        | SerializeOpts::AST_RHS
                ),
            );

            if needs_parens_additive(opts) {
                format!("({inner})")
            } else {
                inner
            }
        }
        "mult" => {
            let inner = format!(
                "{} * {}",
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR | SerializeOpts::AST_MULT_BODY
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    /* SerializeOpts::AST_RHS needlessly adds a bracket for RHS multiplication. This
                     * is done to be compatible with the reference implementation */
                    SerializeOpts::ESCAPE_STR
                        | SerializeOpts::AST_MULT_BODY
                        | SerializeOpts::AST_RHS
                ),
            );

            if needs_parens_multiplicative(opts) {
                format!("({inner})")
            } else {
                inner
            }
        }
        "div" => {
            let inner = format!(
                "{} / {}",
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR | SerializeOpts::AST_MULT_BODY
                ),
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                        | SerializeOpts::AST_MULT_BODY
                        | SerializeOpts::AST_RHS
                ),
            );

            if needs_parens_multiplicative(opts) {
                format!("({inner})")
            } else {
                inner
            }
        }
        "intDiv" => {
            let inner = format!(
                "{} \\ {}",
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR | SerializeOpts::AST_MULT_BODY
                ),
                /* rhs — non-assoc */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                        | SerializeOpts::AST_MULT_BODY
                        | SerializeOpts::AST_RHS
                ),
            );

            if needs_parens_multiplicative(opts) {
                format!("({inner})")
            } else {
                inner
            }
        }
        "mod" => {
            let inner = format!(
                "{} % {}",
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR | SerializeOpts::AST_MULT_BODY
                ),
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                        | SerializeOpts::AST_MULT_BODY
                        | SerializeOpts::AST_RHS
                ),
            );

            if needs_parens_multiplicative(opts) {
                format!("({inner})")
            } else {
                inner
            }
        }
        "cartesian" => {
            format!(
                "{} >< {}",
                /* lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "power" => {
            let lhs = serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR | SerializeOpts::AST_POW_BODY | SerializeOpts::AST_LHS,
            );
            let rhs = serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR | SerializeOpts::AST_POW_BODY,
            );
            let inner = format!("{lhs} ** {rhs}");
            if needs_parens_power(opts) {
                format!("({inner})")
            } else {
                inner
            }
        }
        "sumMem" => {
            format!(
                "{} +/ {}",
                /* lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "prodMem" => {
            format!(
                "{} */ {}",
                /* lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "setEq" => {
            format!(
                "{} <==> {}",
                /* lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "setNeq" => {
            format!(
                "{} <!=> {}",
                /* lhs */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
                /* rhs */
                serialize(
                    &a.list[1],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "unaryMinus" => {
            format!(
                "-{}",
                /* expr */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "card" => {
            format!(
                "#{}",
                /* expr */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "unarySumMem" => {
            format!(
                "+/{}",
                /* expr */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "unaryProdMem" => {
            format!(
                "*/{}",
                /* expr */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "factor" => {
            format!(
                "{}!",
                /* expr */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                ),
            )
        }
        "not" => {
            format!(
                "!{}",
                /* expr */
                serialize(
                    &a.list[0],
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    iopts,
                    rl,
                    SerializeOpts::ESCAPE_STR | SerializeOpts::AST_NOT_BODY
                ),
            )
        }
        "procedure" => format!(
            "procedure({}, *{}) {{ {} }}",
            /* params */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::AST_PARAMS
            ),
            /* list param */
            serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
            /* block */
            serialize(
                &a.list[2],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::AST_BLOCK
            ),
        ),
        "cachedProcedure" => format!(
            "cachedProdecure ({}, *{}) {{ {} }}",
            /* params */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::AST_PARAMS
            ),
            /* list param */
            serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
            /* block */
            serialize(
                &a.list[2],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::AST_BLOCK
            ),
        ),
        "closure" => format!(
            "closure ({}, *{}) {{ {} }}",
            /* params */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::AST_PARAMS
            ),
            /* list param */
            serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
            /* block */
            serialize(
                &a.list[2],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::AST_BLOCK
            ),
        ),
        "call" => format!(
            "{}({}, *{})",
            /* name */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::default()
            ),
            /* params */
            serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::AST_PARAMS
            ),
            /* rest param */
            serialize(
                &a.list[2],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::ESCAPE_STR
            ),
        ),
        "callName" => serialize(
            &a.list[0],
            vars,
            stack,
            memo,
            cstore,
            breakpoints,
            iopts,
            rl,
            SerializeOpts::default(),
        ),
        "term" => format!(
            "@{}({})",
            /* name */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::default()
            ),
            /* params */
            serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::AST_PARAMS
            ),
        ),
        "tterm" => format!(
            "@@@{}({})",
            /* name */
            serialize(
                &a.list[0],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::default()
            ),
            /* params */
            serialize(
                &a.list[1],
                vars,
                stack,
                memo,
                cstore,
                breakpoints,
                iopts,
                rl,
                SerializeOpts::AST_PARAMS
            ),
        ),
        "var" => serialize(
            &a.list[0],
            vars,
            stack,
            memo,
            cstore,
            breakpoints,
            iopts,
            rl,
            SerializeOpts::default(),
        ),
        "accessible" => unimplemented!(),
        "string" => serialize(
            &a.list[0],
            vars,
            stack,
            memo,
            cstore,
            breakpoints,
            iopts,
            rl,
            SerializeOpts::ESCAPE_STR,
        ),
        "literal" => serialize(
            &a.list[0],
            vars,
            stack,
            memo,
            cstore,
            breakpoints,
            iopts,
            rl,
            SerializeOpts::ESCAPE_STR_LIT,
        ),
        "vector" => unimplemented!(),
        "matrix" => unimplemented!(),
        "exists" => unimplemented!(),
        "forall" => unimplemented!(),
        "om" => String::from("om"),
        "ignore" => String::from("_"),
        _ => String::from("/* unknown AST node */"),
    }
}

fn serialize_f64(input: f64) -> String {
    let s = input.to_string();
    if s.contains('.') { s } else { format!("{s}.0") }
}

fn serialize_object(
    o: &InterpClassObj,
    obj_ref: InterpObjRef,
    vars: &mut [InterpVal],
    stack: &mut InterpStack,
    memo: &mut InterpMemoize,
    cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    iopts: &InputOpts,
    rl: &mut DefaultEditor,
) -> String {
    fn proc_get(a_ptr: &InterpVal) -> Option<Rc<RefCell<IRProcedure>>> {
        let a = if let InterpVal::Ptr(p) = a_ptr {
            // SAFETY: IR-PTR
            unsafe { &*p.ptr }
        } else {
            return None;
        };

        match a {
            InterpVal::Procedure(p) => Some(p.clone()),
            InterpVal::Ref(r) => match unsafe { &*r.0 } {
                InterpObj::Procedure(p) => Some(p.proc.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    if let Some(v) = o.get("f_str")
        && let Some(proc) = proc_get(&v)
    {
        fn serialize_proc_stub_new(
            p: Rc<RefCell<IRProcedure>>,
            obj_ref: InterpObjRef,
        ) -> Rc<RefCell<IRProcedure>> {
            /*  t_proc := proc;
             *  t_obj := obj_ref;
             *  t_obj_addr := &t_obj;
             *  t_params := list_new();
             *  t_ret := // block_obj_call_impl_push
             *  _ := invalidate(t_params);
             */
            let proc = Rc::new(RefCell::new(IRProcedure::from_tag("_serialize_obj_proc")));

            let t_proc = tmp_var_new(&mut proc.borrow_mut());
            let t_obj = tmp_var_new(&mut proc.borrow_mut());
            let t_obj_addr = tmp_var_new(&mut proc.borrow_mut());
            let t_params = tmp_var_new(&mut proc.borrow_mut());
            let t_ret = tmp_var_new(&mut proc.borrow_mut());

            let mut init_idx = proc.borrow_mut().blocks.add_node(Vec::new());
            proc.borrow_mut().start_block = init_idx;

            block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_proc),
                    types: IRType::PROCEDURE,
                    source: IRValue::Procedure(p),
                    op: IROp::Assign,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_obj),
                    types: IRType::HEAP_REF,
                    source: IRValue::HeapRef(obj_ref),
                    op: IROp::Assign,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_obj_addr),
                    types: IRType::PTR,
                    source: IRValue::Variable(t_obj),
                    op: IROp::PtrAddress,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_params),
                    types: IRType::LIST,
                    source: IRValue::BuiltinProc(BuiltinProc::ListNew),
                    op: IROp::NativeCall(Vec::new()),
                }),
            ]);

            block_obj_call_impl_push(
                &mut init_idx,
                t_obj_addr,
                t_proc,
                t_params,
                IRTarget::Variable(t_ret),
                &mut proc.borrow_mut(),
            );

            block_get(&mut proc.borrow_mut(), init_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Ignore,
                    types: IRType::UNDEFINED,
                    source: IRValue::BuiltinProc(BuiltinProc::Invalidate),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_params)]),
                }),
                IRStmt::Return(IRValue::Variable(t_ret)),
            ]);

            proc.borrow_mut().end_block = init_idx;

            proc
        }

        let s_proc = serialize_proc_stub_new(proc, obj_ref);

        let res = exec_proc(
            s_proc,
            &InterpVal::Undefined,
            stack,
            memo,
            cstore,
            breakpoints,
            iopts,
            rl,
        );
        serialize(
            &res,
            vars,
            stack,
            memo,
            cstore,
            breakpoints,
            iopts,
            rl,
            SerializeOpts::default(),
        )
    } else {
        format!(
            "object<{}>",
            o.0.iter()
                .map(|(name, val)| format!(
                    "{name} := {};",
                    serialize(
                        val,
                        vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        iopts,
                        rl,
                        SerializeOpts::ESCAPE_STR
                    ),
                ))
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

pub fn serialize(
    input: &InterpVal,
    vars: &mut [InterpVal],
    stack: &mut InterpStack,
    memo: &mut InterpMemoize,
    cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    iopts: &InputOpts,
    rl: &mut DefaultEditor,
    opts: SerializeOpts,
) -> String {
    match input {
        InterpVal::Bool(b) => b.to_string(),
        InterpVal::Double(d) => serialize_f64(*d),
        InterpVal::Char(c) => c.to_string(),
        InterpVal::Type(t) => t.to_string(),
        InterpVal::Slice(s) => match s {
            InterpSlice::StringSlice(sl) => sl.slice.clone().collect(),
            InterpSlice::ListSlice(l) => format!(
                "[{}]",
                l.iter()
                    .map(|i| serialize(
                        i,
                        vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        iopts,
                        rl,
                        SerializeOpts::ESCAPE_STR
                    ))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        },
        InterpVal::ObjIter(i) => {
            format!(
                "{{{}}}",
                i.clone()
                    .map(|(name, val)| format!(
                        "{name}: {}",
                        serialize(
                            val,
                            vars,
                            stack,
                            memo,
                            cstore,
                            breakpoints,
                            iopts,
                            rl,
                            SerializeOpts::ESCAPE_STR
                        )
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        InterpVal::Iter(iter) => match iter {
            InterpIter::StringIter(i) => i.clone().collect(),
            InterpIter::SetIter(i) => format!(
                "{{ {} }}",
                i.clone()
                    .map(|i| serialize(
                        i,
                        vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        iopts,
                        rl,
                        SerializeOpts::ESCAPE_STR
                    ))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            InterpIter::ListIter(i) => format!(
                "[{}]",
                i.clone()
                    .map(|i| serialize(
                        i,
                        vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        iopts,
                        rl,
                        SerializeOpts::ESCAPE_STR
                    ))
                    .collect::<Vec<String>>()
                    .join(", "),
            ),
        },
        InterpVal::Ptr(p) => {
            let sgmt = match &p.sgmt {
                InterpPtrSgmt::Immediate => "immediate",
                InterpPtrSgmt::Stack => "stack",
                InterpPtrSgmt::Heap => "heap",
                InterpPtrSgmt::Class => "class",
            };
            format!("ptr: {sgmt} {:p}", p.ptr,)
        }
        // SAFETY: IR-PTR
        InterpVal::OffsetStrPtr(s) => (unsafe { &*s.val })
            .chars()
            .nth(s.offset)
            .unwrap_or(' ')
            .to_string(),
        InterpVal::Procedure(p) => format!("/* predefined procedure {} */", p.borrow().tag),
        InterpVal::Ref(r) => match unsafe { &*r.0 } {
            InterpObj::Ast(a) => {
                serialize_ast(a, vars, stack, memo, cstore, breakpoints, iopts, rl, opts)
            }
            InterpObj::List(l) => {
                if opts.contains(SerializeOpts::AST_PARAMS) {
                    l.0.iter()
                        .map(|i| {
                            serialize(
                                i,
                                vars,
                                stack,
                                memo,
                                cstore,
                                breakpoints,
                                iopts,
                                rl,
                                SerializeOpts::ESCAPE_STR,
                            )
                        })
                        .collect::<Vec<String>>()
                        .join(", ")
                } else if opts.contains(SerializeOpts::AST_BLOCK) {
                    l.0.iter()
                        .map(|i| {
                            serialize(
                                i,
                                vars,
                                stack,
                                memo,
                                cstore,
                                breakpoints,
                                iopts,
                                rl,
                                SerializeOpts::ESCAPE_STR,
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("; ")
                } else {
                    format!(
                        "[{}]",
                        l.0.iter()
                            .map(|i| serialize(
                                i,
                                vars,
                                stack,
                                memo,
                                cstore,
                                breakpoints,
                                iopts,
                                rl,
                                SerializeOpts::ESCAPE_STR
                            ))
                            .collect::<Vec<String>>()
                            .join(", ")
                    )
                }
            }
            InterpObj::Number(n) => n.to_string(),
            InterpObj::Set(s) => format!(
                "{{{}}}",
                s.0.iter()
                    .map(|i| serialize(
                        i,
                        vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        iopts,
                        rl,
                        SerializeOpts::ESCAPE_STR
                    ))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            InterpObj::String(s) => {
                if opts.contains(SerializeOpts::ESCAPE_STR) {
                    let escaped = s
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('$', "\\$");
                    let mut out = String::with_capacity(escaped.len() + 2);
                    out.push('"');
                    out.push_str(&escaped);
                    out.push('"');
                    out
                } else if opts.contains(SerializeOpts::ESCAPE_STR_LIT) {
                    let escaped = s
                        .replace('\\', "\\\\")
                        .replace('\'', "\\'")
                        .replace('$', "\\$");
                    let mut out = String::with_capacity(escaped.len() + 2);
                    out.push('\'');
                    out.push_str(&escaped);
                    out.push('\'');
                    out
                } else {
                    s.to_string()
                }
            }
            InterpObj::Term(t) => {
                format!(
                    "@{}({})",
                    t.tag,
                    t.list
                        .iter()
                        .map(|i| serialize(
                            i,
                            vars,
                            stack,
                            memo,
                            cstore,
                            breakpoints,
                            iopts,
                            rl,
                            SerializeOpts::ESCAPE_STR
                        ))
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
            InterpObj::TTerm(t) => {
                format!(
                    "@@@{}({})",
                    t.tag,
                    t.list
                        .iter()
                        .map(|i| serialize(
                            i,
                            vars,
                            stack,
                            memo,
                            cstore,
                            breakpoints,
                            iopts,
                            rl,
                            SerializeOpts::ESCAPE_STR
                        ))
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
            InterpObj::Vector(v) => {
                format!(
                    "<<{}>>",
                    v.iter()
                        .map(|i| serialize_f64(*i))
                        .collect::<Vec<String>>()
                        .join(" ")
                )
            }
            InterpObj::Matrix(m) => {
                format!(
                    "<<{}>>",
                    (0..m.ncols())
                        .map(|v| format!(
                            "<<{}>>",
                            m.column(v)
                                .iter()
                                .map(|i| serialize_f64(*i))
                                .collect::<Vec<String>>()
                                .join(" ")
                        ))
                        .collect::<Vec<String>>()
                        .join(" ")
                )
            }
            InterpObj::Procedure(p) => {
                if let Some(info) = &p.info {
                    serialize_ast(
                        info,
                        vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        iopts,
                        rl,
                        opts,
                    )
                } else {
                    format!("/* predefined procedure {} */", p.proc.borrow().tag)
                }
            }
            InterpObj::Class(c) => format!(
                "class () {{ /* constructor not serialized */ static {{ {} }} }}",
                c.static_vars
                    .iter()
                    .map(|(name, val)| format!(
                        "{name} := {};",
                        serialize(
                            val,
                            vars,
                            stack,
                            memo,
                            cstore,
                            breakpoints,
                            iopts,
                            rl,
                            SerializeOpts::ESCAPE_STR
                        )
                    ))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            InterpObj::Object(o) => {
                serialize_object(o, *r, vars, stack, memo, cstore, breakpoints, iopts, rl)
            }
            InterpObj::StackImage(s) => {
                s.0.iter()
                    .map(|(name, val)| {
                        format!(
                            "{name} := {};",
                            serialize(
                                val,
                                vars,
                                stack,
                                memo,
                                cstore,
                                breakpoints,
                                iopts,
                                rl,
                                SerializeOpts::ESCAPE_STR
                            )
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            }
            InterpObj::Regex(_) => String::from("/* compiled regex */"),
            InterpObj::File(_) => String::from("/* file */"),
            InterpObj::Uninitialized => String::from("om"),
        },
        InterpVal::Undefined => String::from("om"),
    }
}
