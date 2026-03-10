#![allow(clippy::mutable_key_type)]

use glass_pumpkin::prime;
use num_prime::nt_funcs::is_prime as num_is_prime;
use num_prime::{Primality, PrimalityTestConfig};
use num_traits::{Pow, ToPrimitive};
use pcre2::bytes::RegexBuilder;
use rand::RngExt;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::panic;
use std::panic::AssertUnwindSafe;
use std::process::{Command, exit};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use yansi::Paint;

use crate::builtin::*;
use crate::cli::InputOpts;
use crate::cst::{cst_expr_parse, cst_parse};
use crate::interp::{
    ast::{ast_to_cst_block, ast_to_cst_expr},
    debug::DebugData,
    except::*,
    exec::exec_proc,
    get::InterpGet,
    heap::*,
    memoize::InterpMemoize,
    ops::val_cmp,
    serialize::{SerializeOpts, serialize},
    stack::InterpStack,
};
use crate::ir::def::*;
use crate::ir::lower::CSTIRLower;
use crate::ir::lower::expr::term_expr::{ast_tterm_tag_get, tterm_ast_tag_get};

fn amount_val(input: &InterpVal, data: &DebugData) -> usize {
    match input {
        InterpVal::Slice(s) => match s {
            InterpSlice::StringSlice(s) => s.slice.clone().count(),
            InterpSlice::ListSlice(l) => l.len(),
        },
        InterpVal::Ref(r) => match unsafe { &*r.0 } {
            InterpObj::List(l) => l.0.len(),
            InterpObj::Set(s) => s.0.len(),
            InterpObj::String(s) => s.chars().count(),
            InterpObj::Ast(tl) | InterpObj::Term(tl) | InterpObj::TTerm(tl) => tl.list.len(),
            _ => exception_throw("builtin procedure", "amount not defined for type", data),
        },
        InterpVal::Char(_) => 1,
        // SAFETY: IR-PTR
        InterpVal::Ptr(p) => unsafe { amount_val(&*p.ptr, data) },
        _ => exception_throw("builtin procedure", "amount not defined for type", data),
    }
}

fn set_card(
    sl: &BTreeSet<InterpVal>,
    sr: &BTreeSet<InterpVal>,
    heap: &mut InterpImmediateHeap,
) -> InterpVal {
    let obj = InterpObj::Set(InterpSet(
        sl.iter()
            .flat_map(|il| {
                sr.iter()
                    .map(|ir| {
                        let list = InterpObj::List(InterpList(vec![il.unshare(), ir.unshare()]));
                        InterpVal::Ref(InterpObjRef::from_obj(list))
                    })
                    .collect::<BTreeSet<_>>()
            })
            .collect::<BTreeSet<_>>(),
    ));
    InterpVal::Ref(heap.push_obj(obj))
}

fn list_card(ll: &InterpList, lr: &InterpList, heap: &mut InterpImmediateHeap) -> InterpVal {
    let obj = InterpObj::List(InterpList(
        ll.0.iter()
            .zip(lr.0.iter())
            .map(|(il, ir)| {
                let list = InterpObj::List(InterpList(vec![il.unshare(), ir.unshare()]));
                InterpVal::Ref(InterpObjRef::from_obj(list))
            })
            .collect::<Vec<_>>(),
    ));
    InterpVal::Ref(heap.push_obj(obj))
}

pub fn builtin_call(
    proc: BuiltinProc,
    params: &[IRValue],
    params_proc: &InterpVal,
    vars: &mut [InterpVal],
    stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    memo: &mut InterpMemoize,
    cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    opts: &InputOpts,
    rl: &mut DefaultEditor,
) -> InterpVal {
    match proc {
        BuiltinProc::Amount => {
            let val = params[0].to_val(vars, params_proc, breakpoints, opts, heap);
            let len = amount_val(&val.val, breakpoints);

            InterpVal::Ref(heap.push_obj(InterpObj::Number(len.into())))
        }
        BuiltinProc::Contains => {
            let set = params[0].to_val(vars, params_proc, breakpoints, opts, heap);
            let i = params[1].to_val(vars, params_proc, breakpoints, opts, heap);

            let out = match &set.val {
                InterpVal::Ref(r) => match unsafe { &*r.0 } {
                    // FIXME: don't use InterpObjVal
                    InterpObj::Set(s) => InterpVal::Bool(s.0.contains(&i.val)),
                    InterpObj::List(l) => InterpVal::Bool(l.contains(&i.val)),
                    InterpObj::String(s) => {
                        let needle = if let InterpVal::Ref(r) = i.val
                            && let InterpObj::String(n) = unsafe { &*r.0 }
                        {
                            n.to_string()
                        } else {
                            serialize(
                                &i.val,
                                vars,
                                stack,
                                memo,
                                cstore,
                                breakpoints,
                                opts,
                                rl,
                                SerializeOpts::default(),
                            )
                        };

                        InterpVal::Bool(s.contains(&needle))
                    }
                    _ => exception_throw(
                        "builtin procedure",
                        "contains not defined for type",
                        breakpoints,
                    ),
                },
                _ => exception_throw(
                    "builtin procedure",
                    "contains not defined for type",
                    breakpoints,
                ),
            };

            set.confirm();
            i.confirm();

            out
        }
        BuiltinProc::Cartesian => {
            let lhs = params[0].to_val(vars, params_proc, breakpoints, opts, heap);
            let rhs = params[1].to_val(vars, params_proc, breakpoints, opts, heap);

            let out = match (&lhs.val, &rhs.val) {
                (InterpVal::Ref(rl), InterpVal::Ref(rr)) => {
                    match (unsafe { &*rl.0 }, unsafe { &*rr.0 }) {
                        (InterpObj::List(ll), InterpObj::List(lr)) => list_card(ll, lr, heap),
                        (InterpObj::Set(sl), InterpObj::Set(sr)) => set_card(&sl.0, &sr.0, heap),
                        _ => exception_throw(
                            "builtin procedure",
                            "cartesian undefined for type",
                            breakpoints,
                        ),
                    }
                }
                _ => exception_throw(
                    "builtin procedure",
                    "cartesian undefined for type",
                    breakpoints,
                ),
            };

            lhs.confirm();
            rhs.confirm();

            out
        }
        BuiltinProc::Pow => {
            let lhs = params[0].to_val(vars, params_proc, breakpoints, opts, heap);
            let rhs = params[1].to_val(vars, params_proc, breakpoints, opts, heap);

            fn powset(r: InterpObjRef, heap: &mut InterpImmediateHeap) -> InterpVal {
                let set = if let InterpObj::Set(s) = unsafe { &*r.0 } {
                    s
                } else {
                    unreachable!()
                };
                let items = set.0.iter().cloned().collect::<Vec<_>>();
                let n = items.len();
                let mut result = BTreeSet::new();
                for mask in 0..(1 << n) {
                    let mut subset = BTreeSet::new();
                    for (idx, i) in items.iter().enumerate() {
                        if (mask & (1 << idx)) != 0 {
                            subset.insert(i.unshare());
                        }
                    }
                    result.insert(InterpVal::Ref(InterpObjRef::from_obj(InterpObj::Set(
                        InterpSet(subset),
                    ))));
                }

                InterpVal::Ref(heap.push(InterpObjRef::from_obj(InterpObj::Set(InterpSet(result)))))
            }

            /* 2 ** set
             * 2.0 ** set
             * set ** 2
             * set ** 2.0
             * NUMBER ** NUMBER
             * NUMBER ** DOUBLE
             * DOUBLE ** NUMBER
             * DOUBLE ** DOUBLE
             */
            let out = match (&lhs.val, &rhs.val) {
                (InterpVal::Ref(rl), InterpVal::Ref(rr)) => {
                    match (unsafe { &*rl.0 }, unsafe { &*rr.0 }) {
                        (InterpObj::Number(nl), InterpObj::Set(_)) => {
                            if *nl == 2.into() {
                                powset(*rr, heap)
                            } else {
                                exception_throw(
                                    "builtin procedure",
                                    "pow undefined for type",
                                    breakpoints,
                                );
                            }
                        }
                        (InterpObj::Set(sl), InterpObj::Number(nr)) => {
                            if *nr == 2.into() {
                                set_card(&sl.0, &sl.0, heap)
                            } else {
                                exception_throw(
                                    "builtin procedure",
                                    "pow undefined for type",
                                    breakpoints,
                                );
                            }
                        }
                        (InterpObj::Number(nl), InterpObj::Number(nr)) => {
                            InterpVal::Double(nl.to_f64().unwrap().pow(nr.to_i32().unwrap()))
                        }
                        _ => exception_throw(
                            "builtin procedure",
                            "pow undefined for type",
                            breakpoints,
                        ),
                    }
                }
                (InterpVal::Ref(r), InterpVal::Double(dr)) => match unsafe { &*r.0 } {
                    InterpObj::Set(s) => {
                        if *dr == 2.0 {
                            set_card(&s.0, &s.0, heap)
                        } else {
                            exception_throw(
                                "builtin procedure",
                                "pow undefined for type",
                                breakpoints,
                            );
                        }
                    }
                    InterpObj::Number(n) => InterpVal::Double(n.to_f64().unwrap().powf(*dr)),
                    _ => {
                        exception_throw("builtin procedure", "pow undefined for type", breakpoints)
                    }
                },
                (InterpVal::Double(dl), InterpVal::Ref(r)) => match unsafe { &*r.0 } {
                    InterpObj::Set(_) => {
                        if *dl == 2.0 {
                            powset(*r, heap)
                        } else {
                            exception_throw(
                                "builtin procedure",
                                "pow undefined for type",
                                breakpoints,
                            );
                        }
                    }
                    InterpObj::Number(n) => InterpVal::Double(dl.powf(n.to_f64().unwrap())),
                    _ => {
                        exception_throw("builtin procedure", "pow undefined for type", breakpoints)
                    }
                },
                (InterpVal::Double(dl), InterpVal::Double(dr)) => InterpVal::Double(dl.powf(*dr)),
                _ => exception_throw("builtin procedure", "pow undefined for type", breakpoints),
            };

            lhs.confirm();
            rhs.confirm();

            out
        }
        BuiltinProc::TypeOf => InterpVal::Type(params[0].to_type(vars, params_proc, opts, heap)),
        BuiltinProc::TermNew => {
            let tag = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
            let len = params[1].to_usize(vars, params_proc, breakpoints, "term_new");
            let is_tterm = params[2].to_immed_bool("term_new");

            let obj = InterpTaggedList {
                tag,
                list: vec![InterpVal::Undefined; len],
            };

            if is_tterm {
                InterpVal::Ref(heap.push_obj(InterpObj::TTerm(obj)))
            } else {
                InterpVal::Ref(heap.push_obj(InterpObj::Term(obj)))
            }
        }
        BuiltinProc::TermKindEq => {
            let lhs = params[0].to_val(vars, params_proc, breakpoints, opts, heap);
            let rhs = params[1].to_val(vars, params_proc, breakpoints, opts, heap);

            match (&lhs.val, &rhs.val) {
                (InterpVal::Ref(rl), InterpVal::Ref(rr)) => {
                    match (unsafe { &*rl.0 }, unsafe { &*rr.0 }) {
                        (InterpObj::Ast(ll), InterpObj::Ast(lr))
                        | (InterpObj::Term(ll), InterpObj::Term(lr))
                        | (InterpObj::TTerm(ll), InterpObj::TTerm(lr)) => {
                            InterpVal::Bool(ll.tag == lr.tag)
                        }

                        _ => InterpVal::Bool(false),
                    }
                }
                _ => InterpVal::Bool(false),
            }
        }
        BuiltinProc::Invalidate => {
            if let Some(r) = params[0].to_ref(vars) {
                unsafe {
                    heap.free(r);
                }
            }

            InterpVal::Undefined
        }
        BuiltinProc::MarkPersist => {
            if let Some(r) = params[0].to_ref(vars) {
                InterpVal::Ref(r).persist(heap);
            }

            InterpVal::Undefined
        }
        BuiltinProc::MarkImmed => {
            if let Some(r) = params[0].to_ref(vars) {
                InterpVal::Ref(r).mark_immed(heap);
            }

            InterpVal::Undefined
        }
        BuiltinProc::Copy => params[0]
            .to_val(vars, params_proc, breakpoints, opts, heap)
            .confirm()
            .val
            .unshare_immed(heap),
        BuiltinProc::StackFrameAdd => {
            stack.frame_push();
            InterpVal::Undefined
        }
        BuiltinProc::StackFramePop => {
            stack.frame_pop();
            InterpVal::Undefined
        }
        BuiltinProc::StackFrameSave => {
            let obj = InterpObj::StackImage(stack.frame_save());
            InterpVal::Ref(heap.push_obj(obj))
        }
        BuiltinProc::StackFrameRestore => {
            if let IRValue::Variable(v) = &params[0]
                && let InterpVal::Ref(r) = &vars[*v]
                && let InterpObj::StackImage(i) = unsafe { &*r.0 }
            {
                stack.frame_restore(i)
            } else {
                panic!("internal: stack_frame_restore undefined for type");
            }
            InterpVal::Undefined
        }
        BuiltinProc::StackFrameCopy => {
            let obj = InterpObj::StackImage(stack.frame_copy());
            InterpVal::Ref(heap.push_obj(obj))
        }
        BuiltinProc::StackCopy => {
            let obj = InterpObj::StackImage(stack.copy_reachable());
            InterpVal::Ref(heap.push_obj(obj))
        }
        BuiltinProc::StackAdd => stack.add(params[0].to_immed_str("stack_add")),
        BuiltinProc::StackAlias => {
            let name = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
            let ptr = params[1].to_ptr(vars, "stack_alias");
            let cross_frame = params[2].to_immed_bool("stack_alias");

            stack.alias(&name, &ptr, cross_frame);
            InterpVal::Undefined
        }
        BuiltinProc::StackPop => {
            let s = params[0].to_immed_str("stack_pop");

            stack.pop(s);
            InterpVal::Undefined
        }
        BuiltinProc::StackGetOrNew => {
            let s = if let IRValue::String(s) = &params[0] {
                s
            } else {
                panic!("internal: stack get assert requires a string literal parameter");
            };

            if let Some(c) = cstore.0.get_mut(s) {
                InterpVal::Ptr(InterpPtr {
                    sgmt: InterpPtrSgmt::Class,
                    ptr: &mut c.val as *mut InterpVal,
                })
            } else if let Some(i) = stack.get(s) {
                i
            } else {
                if opts.warn_implicit_decl {
                    eprintln!(
                        "{}: implicit declaration of {}",
                        "Warning".yellow().bold(),
                        s.as_str().cyan()
                    );
                }

                stack.add(s)
            }
        }
        BuiltinProc::StackInScope => InterpVal::Bool(
            stack
                .get(params[0].to_immed_str("stack_in_scope"))
                .is_some(),
        ),
        BuiltinProc::ObjectNew => {
            let name = if let IRValue::String(s) = &params[0] {
                s
            } else {
                panic!("internal: object_new undefined for type");
            };

            let mut stack = BTreeMap::new();

            if let Some(c) = cstore.0.get(name) {
                stack.insert(
                    String::from("getClass"),
                    Box::new(InterpVal::Procedure(c.get_proc.clone())),
                );
                #[allow(clippy::collapsible_if)]
                if let InterpVal::Ref(r) = c.val {
                    if let InterpObj::Class(c) = unsafe { &*r.0 } {
                        stack.extend(
                            c.static_vars
                                .iter()
                                .map(|(s, v)| (s.to_string(), Box::new(v.unshare())))
                                .collect::<Vec<(String, Box<InterpVal>)>>(),
                        );
                    }
                }
            } else {
                panic!("internal: object can't be created without an associated class");
            }

            InterpVal::Ref(heap.push_obj(InterpObj::Object(InterpClassObj(stack))))
        }
        BuiltinProc::ObjectGetOrNew => {
            let obj = params[0]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .confirm()
                .val
                .clone();
            let var = params[1].to_immed_str("object_get_or_new");

            if let InterpVal::Ref(r) = obj {
                match unsafe { &mut *r.0 } {
                    InterpObj::Object(o) => o.get(var).unwrap_or_else(|| o.add(var.to_string())),
                    InterpObj::Class(c) => c.get(var).unwrap_or_else(|| c.add(var.to_string())),
                    _ => {
                        exception_throw("builtin procedure", "value is not an object", breakpoints)
                    }
                }
            } else {
                exception_throw("builtin procedure", "value is not an object", breakpoints);
            }
        }
        BuiltinProc::ObjectGet => {
            let obj = params[0]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .confirm()
                .val
                .clone();
            let var = params[1].to_str(vars, params_proc, breakpoints, opts, heap);
            if let InterpVal::Ref(r) = obj {
                match unsafe { &mut *r.0 } {
                    InterpObj::Object(o) => o.get(&var).unwrap_or(InterpVal::Undefined),
                    InterpObj::Class(c) => c.get(&var).unwrap_or(InterpVal::Undefined),
                    _ => {
                        exception_throw("builtin procedure", "value is not an object", breakpoints)
                    }
                }
            } else {
                exception_throw("builtin procedure", "value is not an object", breakpoints);
            }
        }
        BuiltinProc::ObjectAdd => {
            let obj = params[0]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .confirm()
                .val
                .clone();
            let var = params[1].to_str(vars, params_proc, breakpoints, opts, heap);
            if let InterpVal::Ref(r) = obj {
                match unsafe { &mut *r.0 } {
                    InterpObj::Object(o) => o.add(var),
                    InterpObj::Class(c) => c.add(var),
                    _ => {
                        exception_throw("builtin procedure", "value is not an object", breakpoints)
                    }
                }
            } else {
                exception_throw("builtin procedure", "value is not an object", breakpoints);
            }
        }
        BuiltinProc::ObjectAddImage => {
            let obj = params[0]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .confirm()
                .val
                .clone();
            let mut stack = BTreeMap::new();

            let s_obj = params[1].to_val(vars, params_proc, breakpoints, opts, heap);
            if let InterpVal::Ref(r) = &s_obj.val
                && let InterpObj::StackImage(s) = unsafe { &*r.0 }
            {
                stack.extend(
                    s.0.iter()
                        .map(|(name, i)| (name.clone(), Box::new(i.clone())))
                        .collect::<BTreeMap<String, Box<InterpVal>>>(),
                )
            } else {
                panic!("internal: object_new undefined for type");
            }

            s_obj.confirm();

            if let InterpVal::Ref(r) = obj {
                match unsafe { &mut *r.0 } {
                    InterpObj::Object(o) => o.0.extend(stack),
                    InterpObj::Class(c) => c.static_vars.extend(stack),
                    _ => {
                        exception_throw("builtin procedure", "value is not an object", breakpoints)
                    }
                }
            } else {
                exception_throw("builtin procedure", "value is not an object", breakpoints);
            }

            InterpVal::Undefined
        }
        BuiltinProc::ObjectIterNew => {
            InterpVal::ObjIter(params[0].to_obj_iter(vars, params_proc, breakpoints))
        }
        BuiltinProc::ObjectIterNext => {
            let key_ptr = params[1].to_ptr(vars, "obj_iter_next").ptr;
            let val_ptr_ptr = params[2].to_ptr(vars, "obj_iter_next").ptr;
            let iter = params[0].to_obj_iter_ref(vars, breakpoints);

            if let Some((key, val)) = iter.next() {
                // SAFETY: IR-PTR
                unsafe {
                    *key_ptr = InterpVal::Ref(heap.push_obj(InterpObj::String(key.to_string())));
                    *val_ptr_ptr = InterpVal::Ptr(InterpPtr {
                        sgmt: InterpPtrSgmt::Heap,
                        ptr: &**val as *const InterpVal as *mut InterpVal,
                    });
                }
                InterpVal::Bool(true)
            } else {
                InterpVal::Bool(false)
            }
        }
        BuiltinProc::ClassAdd => {
            let name = if let IRValue::String(s) = &params[0] {
                s
            } else {
                panic!("internal: class_add undefined for type");
            };
            let s_proc = params[1]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .confirm()
                .val
                .proc_get()
                .unwrap_or_else(|| panic!("internal: class_add undefined for type"));
            let c_proc = params[2]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .confirm()
                .val
                .proc_get()
                .unwrap_or_else(|| panic!("internal: class_add undefined for type"));

            let mut new_opts = InputOpts::none();
            new_opts.debug_ir = opts.debug_ir;
            let s_map = if let InterpVal::Ref(r) = exec_proc(
                s_proc,
                &InterpVal::Undefined,
                stack,
                memo,
                cstore,
                breakpoints,
                &new_opts,
                rl,
            ) && let InterpObj::StackImage(s) = unsafe { &*r.0 }
            {
                s.0.iter()
                    .map(|(name, i)| (name.clone(), Box::new(i.clone())))
                    .collect::<BTreeMap<String, Box<InterpVal>>>()
            } else {
                panic!("internal: class_add static proc must return a stack image");
            };

            cstore.insert(name.to_string(), c_proc, s_map);

            InterpVal::Undefined
        }
        BuiltinProc::SetNew => {
            InterpVal::Ref(heap.push_obj(InterpObj::Set(InterpSet(BTreeSet::new()))))
        }
        BuiltinProc::SetInsert => {
            let push_val = params[1]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .confirm()
                .val
                .clone()
                .persist(heap);
            let push_val_ptr = &push_val as *const InterpVal as *mut InterpVal;
            if let InterpVal::Ref(r) = params[0]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .confirm()
                .val
                && let InterpObj::Set(s) = unsafe { &mut *r.0 }
            {
                if !matches!(&push_val, InterpVal::Undefined) {
                    s.0.insert(push_val);
                    InterpVal::Ptr(InterpPtr {
                        sgmt: InterpPtrSgmt::Heap,
                        ptr: push_val_ptr,
                    })
                } else {
                    InterpVal::Undefined
                }
            } else {
                panic!("internal: list push undefined for type");
            }
        }
        BuiltinProc::SetRange => {
            let lhs = params[0].to_i64(vars, params_proc, breakpoints, "set_range");
            let rhs = params[1].to_i64(vars, params_proc, breakpoints, "set_range");

            let obj = if lhs > rhs {
                InterpObj::Set(InterpSet(BTreeSet::new()))
            } else {
                InterpObj::Set(InterpSet(
                    (lhs..=rhs)
                        .map(|i| {
                            InterpVal::Ref(InterpObjRef::from_obj(InterpObj::Number(i.into())))
                        })
                        .collect(),
                ))
            };

            InterpVal::Ref(heap.push_obj(obj))
        }
        BuiltinProc::SetBorrow => {
            if params[1].to_bool(vars, params_proc, breakpoints, "set_borrow") {
                if let Some(i) = params[0].to_set(vars, params_proc, breakpoints).first() {
                    i.clone()
                } else {
                    exception_throw(
                        "builtin procedure",
                        "set_borrow is only implemented for sets",
                        breakpoints,
                    );
                }
            } else if let Some(i) = params[0].to_set(vars, params_proc, breakpoints).last() {
                i.clone()
            } else {
                exception_throw(
                    "builtin procedure",
                    "set_borrow is only implemented for sets",
                    breakpoints,
                );
            }
        }
        BuiltinProc::SetTake => {
            if params[1].to_bool(vars, params_proc, breakpoints, "set_take") {
                if let Some(i) = params[0].to_set(vars, params_proc, breakpoints).pop_first() {
                    i.clone().mark_immed(heap)
                } else {
                    exception_throw(
                        "builtin procedure",
                        "set_take is only implemented for sets",
                        breakpoints,
                    );
                }
            } else if let Some(i) = params[0].to_set(vars, params_proc, breakpoints).pop_last() {
                i.clone().mark_immed(heap)
            } else {
                exception_throw(
                    "builtin procedure",
                    "set_take is only implemented for sets",
                    breakpoints,
                );
            }
        }
        BuiltinProc::SetGetTag => {
            let cmp_val = params[1].to_val(vars, params_proc, breakpoints, opts, heap);

            params[0]
                .to_set(vars, params_proc, breakpoints)
                .iter()
                .filter_map(|i| {
                    if let InterpVal::Ref(r) = i
                        && let InterpObj::List(l) = unsafe { &*r.0 }
                        && let Some(val) = l.0.get(1)
                    {
                        Some((val as *const InterpVal, l))
                    } else {
                        None
                    }
                })
                .find(|(_, l)| {
                    l.0.first()
                        .map(|j| val_cmp(j, &cmp_val.val) == Ordering::Equal)
                        .unwrap_or(false)
                })
                .map(|(i, _)| {
                    InterpVal::Ptr(InterpPtr {
                        sgmt: InterpPtrSgmt::Heap,
                        ptr: i as *mut InterpVal,
                    })
                })
                .unwrap_or(InterpVal::Undefined)
        }
        BuiltinProc::SetGetTagAll => {
            let cmp_val = params[1].to_val(vars, params_proc, breakpoints, opts, heap);

            let set = params[0]
                .to_set(vars, params_proc, breakpoints)
                .iter()
                .filter_map(|i| {
                    if let InterpVal::Ref(r) = i
                        && let InterpObj::List(l) = unsafe { &*r.0 }
                        && let Some(key) = l.0.first()
                        && let Some(val) = l.0.get(1)
                        && val_cmp(key, &cmp_val.val) == Ordering::Equal
                    {
                        Some(val.unshare())
                    } else {
                        None
                    }
                })
                .collect::<BTreeSet<InterpVal>>();

            InterpVal::Ref(heap.push_obj(InterpObj::Set(InterpSet(set))))
        }
        BuiltinProc::ListNew => {
            let n = if !params.is_empty() {
                params[0].to_usize(vars, params_proc, breakpoints, "list_new")
            } else {
                0
            };

            InterpVal::Ref(heap.push_obj(InterpObj::List(InterpList(
                (0..n).map(|_| InterpVal::Undefined).collect(),
            ))))
        }
        BuiltinProc::ListPush => {
            let val = params[1]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .confirm()
                .val
                .clone()
                .persist(heap);
            params[0].to_list(vars, params_proc, breakpoints).push(val);
            InterpVal::Undefined
        }
        BuiltinProc::Pop => {
            let coll = params[0].to_val(vars, params_proc, breakpoints, opts, heap);

            if let InterpVal::Ref(r) = &coll.val {
                match unsafe { &mut *r.0 } {
                    InterpObj::String(s) => InterpVal::Ref(heap.push_obj(InterpObj::String(
                        s.pop().map(|s| s.to_string()).unwrap_or(String::from("")),
                    ))),
                    InterpObj::List(l) => l.pop(heap).unwrap_or(InterpVal::Undefined),
                    InterpObj::Set(s) => s.0.pop_last().unwrap_or(InterpVal::Undefined),
                    _ => {
                        exception_throw("builtin procedure", "pop undefined for type", breakpoints)
                    }
                }
            } else {
                exception_throw("builtin procedure", "pop undefined for type", breakpoints);
            }
        }
        BuiltinProc::ListRange => {
            let lhs = params[0].to_i64(vars, params_proc, breakpoints, "list_range");
            let rhs = params[1].to_i64(vars, params_proc, breakpoints, "list_range");

            let list: Vec<_> = if lhs > rhs {
                Vec::new()
            } else {
                (lhs..=rhs)
                    .map(|i| InterpVal::Ref(InterpObjRef::from_obj(InterpObj::Number(i.into()))))
                    .collect()
            };
            let obj = InterpObj::List(InterpList(list));

            InterpVal::Ref(heap.push_obj(obj))
        }
        BuiltinProc::ListResize => {
            let n = params[1].to_usize(vars, params_proc, breakpoints, "list_resize");
            let l = params[0].to_list(vars, params_proc, breakpoints);
            // FIXME
            l.0.truncate(n);

            InterpVal::Undefined
        }
        BuiltinProc::Slice => {
            let lhs = params[1].to_i64(vars, params_proc, breakpoints, "slice");
            let rhs = params[2].to_i64(vars, params_proc, breakpoints, "slice");
            InterpVal::Slice(params[0].to_slice(
                lhs,
                rhs,
                vars,
                params_proc,
                breakpoints,
                opts,
                heap,
            ))
        }
        BuiltinProc::IterNew => InterpVal::Iter(params[0].to_iter(vars, params_proc, breakpoints)),
        BuiltinProc::IterNext => {
            let ptr = params[1].to_ptr(vars, "iter_next").ptr;
            let i = params[0].to_iter_ref(vars, breakpoints);

            if let Some(val) = i.next() {
                // SAFETY: IR-ptr
                unsafe {
                    *ptr = val;
                }
                InterpVal::Bool(true)
            } else {
                InterpVal::Bool(false)
            }
        }
        BuiltinProc::AstNodeNew => {
            let obj = InterpObj::Ast(InterpTaggedList {
                tag: params[0].to_immed_str("ast_node_new tag").to_string(),
                list: params
                    .iter()
                    .skip(1)
                    .map(|i| {
                        i.to_val(vars, params_proc, breakpoints, opts, heap)
                            .confirm()
                            .val
                            .unshare()
                    })
                    .collect::<Vec<InterpVal>>(),
            });

            InterpVal::Ref(heap.push_obj(obj))
        }
        BuiltinProc::AstNodeNewSized => {
            let obj = InterpObj::Ast(InterpTaggedList {
                tag: params[0].to_str(vars, params_proc, breakpoints, opts, heap),
                list: vec![
                    InterpVal::Undefined;
                    params[1].to_usize(vars, params_proc, breakpoints, "ast_node_new_sized")
                ],
            });

            InterpVal::Ref(heap.push_obj(obj))
        }
        BuiltinProc::AstTagGet => tterm_ast_tag_get(
            &params[0].to_str(vars, params_proc, breakpoints, opts, heap),
            params[1].to_i64(vars, params_proc, breakpoints, "ast_tag_get") as usize,
        )
        .map(|tag| InterpVal::Ref(heap.push_obj(InterpObj::String(tag))))
        .unwrap_or(InterpVal::Undefined),
        BuiltinProc::AstTTermTagGet => {
            ast_tterm_tag_get(&params[0].to_str(vars, params_proc, breakpoints, opts, heap))
                .map(|tag| InterpVal::Ref(heap.push_obj(InterpObj::String(tag))))
                .unwrap_or(InterpVal::Undefined)
        }
        BuiltinProc::ProcedureNew => {
            let proc = params[0].to_proc(vars, breakpoints, "procedure_new");
            let info = params[1].to_ast(vars, params_proc);
            let stack = params[2].to_ref(vars).inspect(|s| {
                InterpVal::Ref(*s).persist(heap);
            });
            let cross_frame = params[3].to_bool(vars, params_proc, breakpoints, "procedure_new");

            InterpVal::Ref(heap.push_obj(InterpObj::Procedure(InterpProc {
                proc,
                stack,
                info,
                cross_frame,
            })))
        }
        BuiltinProc::ProcedureStackGet => params[0]
            .to_ref(vars)
            .map(|r| {
                if let InterpObj::Procedure(p) = unsafe { &*r.0 }
                    && let Some(s) = &p.stack
                {
                    InterpVal::Ref(*s)
                } else {
                    InterpVal::Undefined
                }
            })
            .unwrap_or(InterpVal::Undefined),
        BuiltinProc::CacheLookup => {
            let proc = params[0].to_proc_ptr(vars, "cache_lookup");
            let p = params[1].to_val(vars, params_proc, breakpoints, opts, heap);
            let out = params[2].to_ptr(vars, "cache_lookup");

            let res = memo.get(&proc).and_then(|i| i.contains(&p.val));

            if let Some(r) = &res {
                unsafe {
                    *out.ptr = r.unshare();
                }
            }
            InterpVal::Bool(res.is_some())
        }
        BuiltinProc::CacheAdd => {
            let proc = params[0].to_proc_ptr(vars, "cache_add");
            let val = params[1]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .confirm()
                .val
                .unshare();
            let ret_val = params[2]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .confirm()
                .val
                .unshare();
            memo.entry(proc).or_default().insert(val, ret_val);

            InterpVal::Undefined
        }
        BuiltinProc::CacheClear => {
            let proc = params[0].to_proc_ptr(vars, "cache_add");
            if let Some(i) = memo.get_mut(&proc) {
                i.clear()
            };
            InterpVal::Undefined
        }
        BuiltinProc::Exit => {
            exit(0);
        }
        BuiltinProc::ExceptionSet => {
            let v = params[0]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .confirm()
                .val
                .clone();
            if let InterpVal::Ref(r) = v {
                heap.refs.remove(&r);
            }
            exception_val_set(v);
            InterpVal::Undefined
        }
        BuiltinProc::ExceptionReset => {
            exception_val_set(InterpVal::Undefined);
            exception_kind_set(ExceptionKind::Lng);
            InterpVal::Undefined
        }
        BuiltinProc::Throw => {
            if let IRValue::Number(n) = &params[0] {
                match n.to_u8().unwrap() {
                    0 => exception_kind_set(ExceptionKind::Lng),
                    1 => exception_kind_set(ExceptionKind::Usr),
                    2 => exception_kind_set(ExceptionKind::Backtrack),
                    3 => {
                        eprintln!(
                            "{}",
                            serialize(
                                &params[1]
                                    .to_val(vars, params_proc, breakpoints, opts, heap)
                                    .val,
                                vars,
                                stack,
                                memo,
                                cstore,
                                breakpoints,
                                opts,
                                rl,
                                SerializeOpts::default()
                            )
                        );
                        exit(1);
                    }
                    _ => panic!("internal: throw encountered undefined exception kind"),
                }
            }

            let v = params[1]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .confirm()
                .val
                .clone()
                .persist(heap);
            exception_val_set(v.clone());
            panic!(
                "{}",
                serialize(
                    &v,
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    opts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                )
            );
        }
        BuiltinProc::Rethrow => {
            panic!(
                "{}",
                serialize(
                    &exception_val_get(),
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    opts,
                    rl,
                    SerializeOpts::ESCAPE_STR
                )
            );
        }
        BuiltinProc::ExceptionThrow => {
            let cat_msg = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
            let msg = params[1].to_str(vars, params_proc, breakpoints, opts, heap);
            exception_throw(&cat_msg, &msg, breakpoints);
        }
        BuiltinProc::RegexCompile => {
            let flags = params[1].to_i64(vars, params_proc, breakpoints, "regex_compile");
            let pattern = params[0].to_str(vars, params_proc, breakpoints, opts, heap);

            let mut builder = RegexBuilder::new();
            if flags & 0x02 != 0 {
                builder.multi_line(true);
            }

            InterpVal::Ref(heap.push_obj(InterpObj::Regex(InterpRegex {
                is_anchored: flags & 0x01 != 0,
                regex: match builder.build(&pattern) {
                    Ok(r) => r,
                    Err(e) => exception_throw(
                        "builtin procedure",
                        &format!(
                            "PCRE2 compile error: code={}, offset={:?}, message={}",
                            e.code(),
                            e.offset(),
                            e
                        ),
                        breakpoints,
                    ),
                },
            })))
        }
        BuiltinProc::RegexMatch => {
            let input = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
            let regex = params[1].to_regex(vars, params_proc, breakpoints);

            let out = if regex.is_anchored {
                match regex.regex.find(input.as_bytes()) {
                    Ok(Some(m)) => m.start() == 0 && m.end() == input.len(),
                    _ => false,
                }
            } else {
                regex.regex.is_match(input.as_bytes()).unwrap_or(false)
            };

            InterpVal::Bool(out)
        }
        BuiltinProc::RegexMatchGroups => {
            let input = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
            let regex = params[1].to_regex(vars, params_proc, breakpoints);
            let matched_addr = params[2].to_ptr(vars, "regex_match_groups");

            let caps_opt = regex.regex.captures(input.as_bytes()).unwrap_or(None);

            if let Some(caps) = caps_opt
                && let Some(full) = caps.get(0)
                && (!regex.is_anchored || (full.start() == 0 && full.end() == input.len()))
            {
                // SAFETY: IR-PTR
                unsafe {
                    *matched_addr.ptr = InterpVal::Bool(true);
                }

                let obj = (0..caps.len())
                    .filter_map(|i| caps.get(i))
                    .map(|m| String::from_utf8_lossy(m.as_bytes()).to_string())
                    .map(|s| InterpVal::Ref(InterpObjRef::from_obj(InterpObj::String(s))))
                    .collect::<Vec<_>>();

                InterpVal::Ref(heap.push_obj(InterpObj::List(InterpList(obj))))
            } else {
                // SAFETY: IR-PTR
                unsafe {
                    *matched_addr.ptr = InterpVal::Bool(false);
                }
                InterpVal::Ref(heap.push_obj(InterpObj::List(InterpList(Vec::new()))))
            }
        }
        BuiltinProc::RegexMatchLen => {
            let input = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
            let regex = params[1].to_regex(vars, params_proc, breakpoints);
            let len_addr = params[2].to_ptr(vars, "regex_match_groups");
            let pos_addr = params[3].to_ptr(vars, "regex_match_groups");

            let caps_opt = regex.regex.captures(input.as_bytes()).unwrap_or(None);

            if let Some(caps) = caps_opt
                && let Some(full) = caps.get(0)
                && (!regex.is_anchored || (full.start() == 0 && full.end() == input.len()))
            {
                // SAFETY: IR-PTR
                unsafe {
                    *len_addr.ptr = InterpVal::Ref(
                        heap.push_obj(InterpObj::Number((full.end() - full.start()).into())),
                    );
                    *pos_addr.ptr =
                        InterpVal::Ref(heap.push_obj(InterpObj::Number(full.start().into())))
                }
                InterpVal::Bool(true)
            } else {
                InterpVal::Bool(false)
            }
        }
        BuiltinProc::RegexMatchGroupsLen => {
            let input = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
            let regex = params[1].to_regex(vars, params_proc, breakpoints);
            let matched_addr = params[2].to_ptr(vars, "regex_match_groups");
            let len_addr = params[3].to_ptr(vars, "regex_match_groups");
            let pos_addr = params[4].to_ptr(vars, "regex_match_groups");

            let caps_opt = regex.regex.captures(input.as_bytes()).unwrap_or(None);

            if let Some(caps) = caps_opt
                && let Some(full) = caps.get(0)
                && (!regex.is_anchored || (full.start() == 0 && full.end() == input.len()))
            {
                // SAFETY: IR-PTR
                unsafe {
                    *len_addr.ptr = InterpVal::Ref(
                        heap.push_obj(InterpObj::Number((full.end() - full.start()).into())),
                    );
                    *pos_addr.ptr =
                        InterpVal::Ref(heap.push_obj(InterpObj::Number(full.start().into())));
                    *matched_addr.ptr = InterpVal::Bool(true);
                }

                let obj = (0..caps.len())
                    .filter_map(|i| caps.get(i))
                    .map(|m| String::from_utf8_lossy(m.as_bytes()).to_string())
                    .map(|s| InterpVal::Ref(InterpObjRef::from_obj(InterpObj::String(s))))
                    .collect::<Vec<_>>();

                InterpVal::Ref(heap.push_obj(InterpObj::List(InterpList(obj))))
            } else {
                // SAFETY: IR-PTR
                unsafe {
                    *matched_addr.ptr = InterpVal::Bool(false);
                }
                InterpVal::Ref(heap.push_obj(InterpObj::List(InterpList(Vec::new()))))
            }
        }
        BuiltinProc::Serialize => match &params[0] {
            IRValue::Undefined => {
                InterpVal::Ref(heap.push_obj(InterpObj::String("undefined".to_string())))
            }
            IRValue::Type(t) => InterpVal::Ref(heap.push_obj(InterpObj::String(t.to_string()))),
            IRValue::Variable(v) => {
                let vars_ptr = vars as *mut [InterpVal];
                InterpVal::Ref(heap.push_obj(InterpObj::String(serialize(
                    &vars[*v],
                    // SAFETY: non-invalidating mutable borrow
                    unsafe { &mut *vars_ptr },
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    opts,
                    rl,
                    SerializeOpts::default(),
                ))))
            }
            IRValue::String(s) => InterpVal::Ref(heap.push_obj(InterpObj::String(s.to_string()))),
            IRValue::Number(n) => InterpVal::Ref(heap.push_obj(InterpObj::String(n.to_string()))),
            IRValue::Double(d) => InterpVal::Ref(heap.push_obj(InterpObj::String(d.to_string()))),
            IRValue::Bool(b) => InterpVal::Ref(heap.push_obj(InterpObj::String(b.to_string()))),
            _ => panic!("internal: serialize undefined for internal type"),
        },
        BuiltinProc::PrintStderr => {
            eprint!(
                "{}",
                params[0].to_str(vars, params_proc, breakpoints, opts, heap)
            );
            InterpVal::Undefined
        }
        BuiltinProc::PrintStdout => {
            print!(
                "{}",
                params[0].to_str(vars, params_proc, breakpoints, opts, heap)
            );
            io::stdout().flush().unwrap();
            InterpVal::Undefined
        }
        BuiltinProc::ReadLineStdin => {
            let s = params[0].to_str(vars, params_proc, breakpoints, opts, heap);

            let mut input = match rl.readline(&s) {
                Ok(line) => {
                    _ = rl.add_history_entry(line.as_str());
                    line
                }
                Err(ReadlineError::Interrupted) => {
                    exit(1);
                }
                Err(ReadlineError::Eof) => {
                    exit(0);
                }
                Err(err) => {
                    panic!("internal: readline error: {err}");
                }
            };
            if input.ends_with('\n') {
                input.pop();
            }
            InterpVal::Ref(heap.push_obj(InterpObj::String(input)))
        }
        BuiltinProc::Eval => {
            let input = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
            let (src, srcname) = breakpoints.get_src();
            breakpoints.set_src(input.clone(), String::from("eval"));
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                let new_opts = opts.exec_opts();
                let expr = cst_expr_parse(&input, &new_opts);
                let eval_proc = IRCfg::from_expr(&expr, &new_opts);
                exec_proc(
                    eval_proc,
                    &InterpVal::Undefined,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    &new_opts,
                    rl,
                )
            }));

            breakpoints.set_src(src, srcname);

            match result {
                Ok(o) => o,
                Err(e) => {
                    if let Some(s) = e.downcast_ref::<&str>() {
                        exception_throw_raw(s);
                    } else if let Some(s) = e.downcast_ref::<String>() {
                        exception_throw_raw(s);
                    } else {
                        exception_throw_raw("eval yielded error with non-string payload");
                    }
                }
            }
        }
        BuiltinProc::EvalTerm => {
            let input = params[0]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .confirm()
                .val
                .clone();
            let mut new_opts = opts.exec_opts();
            new_opts.disable_annotations = true;
            new_opts.bogus_annotations = false;
            let result = if let Some(expr) = ast_to_cst_expr(&input) {
                panic::catch_unwind(AssertUnwindSafe(|| {
                    let eval_proc = IRCfg::from_expr(&expr, &new_opts);
                    exec_proc(
                        eval_proc,
                        &InterpVal::Undefined,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        &new_opts,
                        rl,
                    )
                }))
            } else if let Some(stmt) = ast_to_cst_block(&input) {
                panic::catch_unwind(AssertUnwindSafe(|| {
                    eprintln!("{:?}", stmt);
                    let stmt_proc = IRCfg::from_stmt(&stmt, &new_opts);
                    let out = exec_proc(
                        stmt_proc,
                        &InterpVal::Undefined,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        &new_opts,
                        rl,
                    );

                    if let InterpVal::Ref(r) = out {
                        unsafe {
                            r.invalidate();
                        }
                    }

                    InterpVal::Undefined
                }))
            } else {
                exception_throw("parse error", "input isn't a valid term", breakpoints)
            };
            match result {
                Ok(o) => o,
                Err(e) => {
                    if let Some(s) = e.downcast_ref::<&str>() {
                        exception_throw_raw(s);
                    } else if let Some(s) = e.downcast_ref::<String>() {
                        exception_throw_raw(s);
                    } else {
                        exception_throw_raw("eval yielded error with non-string payload");
                    }
                }
            }
        }
        BuiltinProc::Execute => {
            let input = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
            let (src, srcname) = breakpoints.get_src();
            breakpoints.set_src(input.clone(), String::from("execute"));
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                let new_opts = opts.exec_opts();
                let stmt = cst_parse(&input, &new_opts);
                let stmt_proc = IRCfg::from_stmt(&stmt, &new_opts);
                let out = exec_proc(
                    stmt_proc,
                    &InterpVal::Undefined,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    &new_opts,
                    rl,
                );

                if let InterpVal::Ref(r) = out {
                    unsafe {
                        r.invalidate();
                    }
                }

                InterpVal::Bool(true)
            }));

            breakpoints.set_src(src, srcname);

            match result {
                Ok(o) => o,
                Err(e) => {
                    if let Some(s) = e.downcast_ref::<&str>() {
                        exception_throw_raw(s);
                    } else if let Some(s) = e.downcast_ref::<String>() {
                        exception_throw_raw(s);
                    } else {
                        exception_throw_raw("execute yielded error with non-string payload");
                    }
                }
            }
        }
        BuiltinProc::ParseAst => {
            let input = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
            let (src, srcname) = breakpoints.get_src();
            breakpoints.set_src(input.clone(), String::from("execute"));
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                let new_opts = opts.exec_opts();
                let expr = cst_expr_parse(&input, &new_opts);
                let eval_proc = IRCfg::from_ast_expr(&expr, &new_opts);
                exec_proc(
                    eval_proc,
                    &InterpVal::Undefined,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    &new_opts,
                    rl,
                )
            }));

            breakpoints.set_src(src, srcname);

            match result {
                Ok(o) => o,
                Err(e) => {
                    if let Some(s) = e.downcast_ref::<&str>() {
                        exception_throw_raw(s);
                    } else if let Some(s) = e.downcast_ref::<String>() {
                        exception_throw_raw(s);
                    } else {
                        exception_throw_raw("eval yielded error with non-string payload");
                    }
                }
            }
        }
        BuiltinProc::ParseAstBlock => {
            let input = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
            let (src, srcname) = breakpoints.get_src();
            breakpoints.set_src(input.clone(), String::from("execute"));
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                let new_opts = opts.exec_opts();
                let expr = cst_parse(&input, &new_opts);
                let eval_proc = IRCfg::from_ast_block(&expr, &new_opts);
                exec_proc(
                    eval_proc,
                    &InterpVal::Undefined,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    &new_opts,
                    rl,
                )
            }));

            breakpoints.set_src(src, srcname);

            match result {
                Ok(o) => o,
                Err(e) => {
                    if let Some(s) = e.downcast_ref::<&str>() {
                        exception_throw_raw(s);
                    } else if let Some(s) = e.downcast_ref::<String>() {
                        exception_throw_raw(s);
                    } else {
                        exception_throw_raw("eval yielded error with non-string payload");
                    }
                }
            }
        }
        BuiltinProc::OpenAt => {
            let base_path = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
            let rel_path = params[1].to_str(vars, params_proc, breakpoints, opts, heap);
            let opts = params[2].to_i64(vars, params_proc, breakpoints, "open_at");

            let mut o_opts = OpenOptions::new();
            if opts & 0x01 != 0 {
                o_opts.read(true);
            }

            if opts & 0x02 != 0 {
                o_opts.write(true);
            }

            if opts & 0x04 != 0 {
                o_opts.append(true);
            }

            if opts & 0x08 != 0 {
                o_opts.create(true);
            }

            match o_opts.open(std::path::PathBuf::from(base_path).join(rel_path)) {
                Ok(file) => {
                    InterpVal::Ref(heap.push_obj(InterpObj::File(Rc::new(RefCell::new(file)))))
                }
                Err(e) => exception_throw("builtin procedure", &e.to_string(), breakpoints),
            }
        }
        BuiltinProc::ReadAll => {
            let file = params[0].to_file(vars, params_proc, breakpoints);

            let mut contents = Vec::new();
            file.borrow_mut()
                .read_to_end(&mut contents)
                .unwrap_or_else(|e| {
                    exception_throw("builtin procedure", &e.to_string(), breakpoints)
                });
            InterpVal::Ref(heap.push_obj(InterpObj::String(
                String::from_utf8_lossy(&contents).to_string(),
            )))
        }
        BuiltinProc::ReadAllList => {
            let file = params[0].to_file(vars, params_proc, breakpoints);

            let mut contents = Vec::new();
            file.borrow_mut()
                .read_to_end(&mut contents)
                .unwrap_or_else(|e| {
                    exception_throw("builtin procedure", &e.to_string(), breakpoints)
                });
            InterpVal::Ref(
                heap.push_obj(InterpObj::List(InterpList(
                    String::from_utf8_lossy(&contents)
                        .trim_end_matches('\n')
                        .split("\n")
                        .map(|i| {
                            InterpVal::Ref(InterpObjRef::from_obj(InterpObj::String(i.to_string())))
                        })
                        .collect(),
                ))),
            )
        }
        BuiltinProc::Write => {
            let file = params[0].to_file(vars, params_proc, breakpoints);
            let s = params[1].to_str(vars, params_proc, breakpoints, opts, heap);

            file.borrow_mut()
                .write_all(s.as_bytes())
                .unwrap_or_else(|e| {
                    exception_throw("builtin procedure", &e.to_string(), breakpoints)
                });

            InterpVal::Undefined
        }
        BuiltinProc::Delete => {
            let file = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
            fs::remove_file(file).unwrap_or_else(|e| {
                exception_throw("failed to delete file: {:?}", &e.to_string(), breakpoints);
            });
            InterpVal::Undefined
        }
        BuiltinProc::Ln => {
            InterpVal::Double(params[0].to_f64(vars, params_proc, breakpoints, "ln").ln())
        }
        BuiltinProc::Exp => InterpVal::Double(
            params[0]
                .to_f64(vars, params_proc, breakpoints, "exp")
                .exp(),
        ),
        BuiltinProc::Sqrt => InterpVal::Double(
            params[0]
                .to_f64(vars, params_proc, breakpoints, "sqrt")
                .sqrt(),
        ),
        BuiltinProc::Round => InterpVal::Double(
            params[0]
                .to_f64(vars, params_proc, breakpoints, "round")
                .round(),
        ),
        BuiltinProc::Floor => InterpVal::Double(
            params[0]
                .to_f64(vars, params_proc, breakpoints, "floor")
                .floor(),
        ),
        BuiltinProc::Ceil => InterpVal::Double(
            params[0]
                .to_f64(vars, params_proc, breakpoints, "ceil")
                .ceil(),
        ),
        BuiltinProc::Sin => InterpVal::Double(
            params[0]
                .to_f64(vars, params_proc, breakpoints, "sin")
                .sin(),
        ),
        BuiltinProc::Cos => InterpVal::Double(
            params[0]
                .to_f64(vars, params_proc, breakpoints, "cos")
                .cos(),
        ),
        BuiltinProc::Tan => InterpVal::Double(
            params[0]
                .to_f64(vars, params_proc, breakpoints, "tan")
                .tan(),
        ),
        BuiltinProc::SinH => InterpVal::Double(
            params[0]
                .to_f64(vars, params_proc, breakpoints, "sinh")
                .sinh(),
        ),
        BuiltinProc::CosH => InterpVal::Double(
            params[0]
                .to_f64(vars, params_proc, breakpoints, "cosh")
                .cosh(),
        ),
        BuiltinProc::TanH => InterpVal::Double(
            params[0]
                .to_f64(vars, params_proc, breakpoints, "tanh")
                .tanh(),
        ),
        BuiltinProc::Ulp => {
            let x = params[0].to_f64(vars, params_proc, breakpoints, "ulp");
            if x.is_nan() {
                return InterpVal::Double(f64::NAN);
            }

            if x.is_infinite() {
                return InterpVal::Double(f64::INFINITY);
            }

            let bits = x.to_bits();
            let next_bits = if x > 0.0 { bits + 1 } else { bits - 1 };

            let next = f64::from_bits(next_bits);

            InterpVal::Double((next - x).abs())
        }
        BuiltinProc::RndFloat => InterpVal::Double(rand::rng().random_range(0.0..=1.0)),
        BuiltinProc::ToChar => {
            InterpVal::Char(
                params[0].to_usize(vars, params_proc, breakpoints, "to_char") as u8 as char,
            )
        }
        BuiltinProc::Sleep => {
            thread::sleep(Duration::from_millis(params[0].to_usize(
                vars,
                params_proc,
                breakpoints,
                "sleep",
            ) as u64));
            InterpVal::Undefined
        }
        BuiltinProc::UnixEpoch => InterpVal::Ref(
            heap.push_obj(InterpObj::Number(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    .into(),
            )),
        ),
        BuiltinProc::Cmp => match val_cmp(
            &params[0]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .val,
            &params[1]
                .to_val(vars, params_proc, breakpoints, opts, heap)
                .val,
        ) {
            Ordering::Less => InterpVal::Ref(heap.push_obj(InterpObj::Number((-1).into()))),
            Ordering::Equal => InterpVal::Ref(heap.push_obj(InterpObj::Number(0.into()))),
            Ordering::Greater => InterpVal::Ref(heap.push_obj(InterpObj::Number(1.into()))),
        },
        BuiltinProc::ParseInt => params[0]
            .to_str(vars, params_proc, breakpoints, opts, heap)
            .parse::<i64>()
            .map(|i| InterpVal::Ref(heap.push_obj(InterpObj::Number(i.into()))))
            .unwrap_or(InterpVal::Undefined),
        BuiltinProc::ParseFloat => params[0]
            .to_str(vars, params_proc, breakpoints, opts, heap)
            .parse::<f64>()
            .map(InterpVal::Double)
            .unwrap_or(InterpVal::Undefined),
        BuiltinProc::Cmd => {
            pub fn system_capture(cmd: &str) -> (String, String) {
                #[cfg(target_family = "unix")]
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .output()
                    .expect("failed to open shell");

                #[cfg(target_family = "windows")]
                let output = Command::new("cmd")
                    .arg("/C")
                    .arg(cmd)
                    .output()
                    .expect("failed to open shell");

                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                (stdout, stderr)
            }

            let (stdout, stderr) =
                system_capture(&params[0].to_str(vars, params_proc, breakpoints, opts, heap));

            let mut stdout_obj = InterpList::default();
            stdout.lines().for_each(|i| {
                stdout_obj.push(InterpVal::Ref(InterpObjRef::from_obj(InterpObj::String(
                    i.to_string(),
                ))))
            });

            let mut stderr_obj = InterpList::default();
            stderr.lines().for_each(|i| {
                stderr_obj.push(InterpVal::Ref(InterpObjRef::from_obj(InterpObj::String(
                    i.to_string(),
                ))))
            });

            let mut list_obj = InterpList::default();
            list_obj.push(InterpVal::Ref(InterpObjRef::from_obj(InterpObj::List(
                stdout_obj,
            ))));
            list_obj.push(InterpVal::Ref(InterpObjRef::from_obj(InterpObj::List(
                stderr_obj,
            ))));

            InterpVal::Ref(heap.push_obj(InterpObj::List(list_obj)))
        }
        BuiltinProc::IsPrime => {
            let n = params[0]
                .to_num(vars, params_proc, breakpoints, "is_prime")
                .to_biguint();
            if let Some(n) = n {
                let res = match num_is_prime(&n, Some(PrimalityTestConfig::strict())) {
                    Primality::Yes => true,
                    Primality::No => false,
                    Primality::Probable(_) => prime::check(&n),
                };
                InterpVal::Bool(res)
            } else {
                InterpVal::Bool(false)
            }
        }
        BuiltinProc::IsProbablePrime => {
            let n = params[0]
                .to_num(vars, params_proc, breakpoints, "is_prime")
                .to_biguint();
            if let Some(n) = n {
                InterpVal::Bool(matches!(
                    num_is_prime(&n, None),
                    Primality::Yes | Primality::Probable(_)
                ))
            } else {
                InterpVal::Bool(false)
            }
        }
        BuiltinProc::StrVal => {
            let s = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
            InterpVal::Ref(
                heap.push_obj(InterpObj::Number(
                    (s.chars().next().unwrap_or_else(|| {
                        exception_throw(
                            "builtin procedure",
                            "empty string undefined for StrVal",
                            breakpoints,
                        )
                    }) as u32)
                        .into(),
                )),
            )
        }
    }
}
