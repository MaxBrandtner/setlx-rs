use num_traits::cast::ToPrimitive;
use rustyline::DefaultEditor;
use std::cmp::Ordering;
use std::panic;

use crate::builtin::call::builtin_call;
use crate::cli::InputOpts;
use crate::interp::debug::DebugData;
use crate::interp::except::*;
use crate::interp::exec::exec_proc;
use crate::interp::get::InterpGet;
use crate::interp::heap::*;
use crate::interp::memoize::InterpMemoize;
use crate::interp::ops::{val_cmp, val_int_quot, val_minus, val_mod, val_mult, val_plus, val_quot};
use crate::interp::serialize::{SerializeOpts, serialize};

use crate::interp::stack::InterpStack;
use crate::ir::def::*;

pub fn exec_assign(
    a: &IRAssign,
    vars: &mut [InterpVal],
    params: &InterpVal,
    stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    memo: &mut InterpMemoize,
    cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    opts: &InputOpts,
    rl: &mut DefaultEditor,
) {
    let res: InterpVal;

    match &a.op {
        IROp::AccessArray(r) => {
            let mut rhs = r.to_i64(vars, params, breakpoints, "access array");

            match &a.source.to_val(vars, params, breakpoints, opts, heap).val {
                // NOTE: access array doesn't need to be implemented for ptr slices as no ptrs of
                // slices are taken during codegen
                InterpVal::Ptr(p) => {
                    // SAFETY: IR-PTR
                    let val = unsafe { (*p.ptr).clone() };
                    let r = if let InterpVal::Ref(r) = val {
                        r
                    } else {
                        exception_throw("ir-op", "access array: not defined for type", breakpoints);
                    };
                    match unsafe { &mut *r.0 } {
                        InterpObj::Ast(tl) | InterpObj::Term(tl) | InterpObj::TTerm(tl) => {
                            if rhs == 0 {
                                exception_throw(
                                    "ir-op",
                                    "access array: not defined for tag ptrs",
                                    breakpoints,
                                );
                            }

                            if rhs < 0 {
                                rhs += tl.list.len() as i64 + 1;
                            }

                            res = InterpVal::Ptr(InterpPtr {
                                sgmt: InterpPtrSgmt::Heap,
                                ptr: &mut tl.list[(rhs - 1) as usize] as *mut InterpVal,
                            });
                        }
                        InterpObj::List(l) => {
                            if rhs < 0 {
                                rhs += l.0.len() as i64 + 1;
                            }

                            res = InterpVal::Ptr(InterpPtr {
                                sgmt: InterpPtrSgmt::Heap,
                                ptr: &mut l.0[rhs as usize],
                            });
                        }
                        InterpObj::String(s) => {
                            if rhs < 0 {
                                rhs += s.chars().count() as i64 + 1;
                            }

                            res = InterpVal::OffsetStrPtr(InterpOffsetStrPtr {
                                offset: rhs as usize,
                                val: s as *const String as *mut String,
                            });
                        }
                        _ => exception_throw(
                            "ir-op",
                            "access array: not defined for type",
                            breakpoints,
                        ),
                    }
                }
                InterpVal::Ref(r) => match unsafe { &*r.0 } {
                    InterpObj::Ast(tl) | InterpObj::Term(tl) | InterpObj::TTerm(tl) => {
                        if rhs == 0 {
                            res = InterpVal::Ref(
                                heap.push_obj(InterpObj::String(tl.tag.to_string())),
                            );
                        } else {
                            if rhs < 0 {
                                rhs += tl.list.len() as i64 + 1;
                            }

                            res = tl.list[(rhs - 1) as usize].clone();
                        }
                    }
                    InterpObj::List(l) => {
                        if rhs < 0 {
                            rhs += l.0.len() as i64 + 1;
                        }

                        res = l.0[rhs as usize].clone();
                    }
                    InterpObj::Set(s) => {
                        res =
                            s.0.iter()
                                .nth(rhs as usize)
                                .cloned()
                                .unwrap_or(InterpVal::Undefined);
                    }
                    InterpObj::String(s) => {
                        res = s
                            .chars()
                            .nth(rhs as usize)
                            .map(InterpVal::Char)
                            .unwrap_or(InterpVal::Undefined)
                    }
                    _ => {
                        exception_throw("ir-op", "access array: not defined for type", breakpoints)
                    }
                },
                InterpVal::Slice(sl) => {
                    res = match sl {
                        InterpSlice::StringSlice(s) => s
                            .slice
                            .clone()
                            .nth(rhs as usize)
                            .map(InterpVal::Char)
                            .unwrap_or(InterpVal::Undefined),
                        InterpSlice::ListSlice(l) => l
                            .get(rhs as usize)
                            .cloned()
                            .unwrap_or(InterpVal::Undefined),
                    }
                }
                _ => exception_throw("ir-op", "access array: not defined for type", breakpoints),
            }
        }
        IROp::Call(v) => {
            let params = &vars[*v];
            let proc = a.source.to_proc(vars, breakpoints, "call");

            res = exec_proc(proc, params, stack, memo, cstore, breakpoints, opts, rl);
            if let InterpVal::Ref(r) = res {
                heap.refs.insert(r);
            }
        }
        IROp::NativeCall(v) => {
            let proc = a.source.to_builtin_proc();

            res = builtin_call(
                proc,
                v,
                params,
                vars,
                stack,
                heap,
                memo,
                cstore,
                breakpoints,
                opts,
                rl,
            );
            if let InterpVal::Ref(r) = res {
                heap.refs.insert(r);
            }
        }
        IROp::PtrAddress => {
            if let IRValue::Variable(v) = &a.source {
                res = InterpVal::Ptr(InterpPtr {
                    sgmt: InterpPtrSgmt::Immediate,
                    ptr: &mut vars[*v],
                });
            } else {
                panic!("interal: ptr address can only be taken of variable");
            };
        }
        IROp::PtrDeref => res = a.source.to_ptr_val(vars, "ptr deref"),
        IROp::Assign => {
            res = a
                .source
                .to_val(vars, params, breakpoints, opts, heap)
                .confirm()
                .val
                .clone();
        }
        IROp::Or(v) => {
            let lhs = a.source.to_bool(vars, params, breakpoints, "or");
            let rhs = v.to_bool(vars, params, breakpoints, "or");

            res = InterpVal::Bool(lhs || rhs);
        }
        IROp::And(v) => {
            let lhs = a.source.to_bool(vars, params, breakpoints, "and");
            let rhs = v.to_bool(vars, params, breakpoints, "and");

            res = InterpVal::Bool(lhs && rhs);
        }
        IROp::Not => {
            res = InterpVal::Bool(!a.source.to_bool(vars, params, breakpoints, "not"));
        }
        IROp::Less(r) => {
            let lhs = a.source.to_val(vars, params, breakpoints, opts, heap);
            let rhs = r.to_val(vars, params, breakpoints, opts, heap);

            // SET < SET
            // NUMBER < NUMBER
            // NUMBER < DOUBLE
            // DOUBLE < NUMBER
            // DOUBLE < DOUBLE
            res = match (&lhs.val, &rhs.val) {
                (InterpVal::Ref(rl), InterpVal::Ref(rr)) => {
                    match (unsafe { &*rl.0 }, unsafe { &*rr.0 }) {
                        (InterpObj::Set(sl), InterpObj::Set(sr)) => {
                            InterpVal::Bool(sl.0.is_subset(&sr.0))
                        }
                        (InterpObj::Number(nl), InterpObj::Number(nr)) => InterpVal::Bool(nl < nr),
                        _ => exception_throw("ir-op", "less undefined for type", breakpoints),
                    }
                }
                (InterpVal::Ref(r), InterpVal::Double(dr)) => {
                    if let InterpObj::Number(nl) = unsafe { &*r.0 } {
                        InterpVal::Bool(nl.to_f64().unwrap() < *dr)
                    } else {
                        exception_throw("ir-op", "less undefined for type", breakpoints);
                    }
                }
                (InterpVal::Double(dl), InterpVal::Ref(r)) => {
                    if let InterpObj::Number(nr) = unsafe { &*r.0 } {
                        InterpVal::Bool(*dl < nr.to_f64().unwrap())
                    } else {
                        exception_throw("ir-op", "less undefined for type", breakpoints);
                    }
                }
                (InterpVal::Double(dl), InterpVal::Double(dr)) => InterpVal::Bool(dl < dr),
                _ => exception_throw("ir-op", "less undefined for type", breakpoints),
            };
        }
        IROp::Equal(r) => {
            let lhs = a.source.to_val(vars, params, breakpoints, opts, heap);
            let rhs = r.to_val(vars, params, breakpoints, opts, heap);

            res = InterpVal::Bool(val_cmp(&lhs.val, &rhs.val) == Ordering::Equal);
        }
        IROp::Plus(r) => {
            let lhs = a.source.to_val(vars, params, breakpoints, opts, heap);
            let rhs = r.to_val(vars, params, breakpoints, opts, heap);

            res = val_plus(
                &lhs.val,
                &rhs.val,
                vars,
                stack,
                heap,
                memo,
                cstore,
                breakpoints,
                opts,
                rl,
            );
        }
        IROp::Minus(r) => {
            let lhs = a.source.to_val(vars, params, breakpoints, opts, heap);
            let rhs = r.to_val(vars, params, breakpoints, opts, heap);

            res = val_minus(&lhs.val, &rhs.val, breakpoints, heap);
        }
        IROp::Mult(r) => {
            let lhs = a.source.to_val(vars, params, breakpoints, opts, heap);
            let rhs = r.to_val(vars, params, breakpoints, opts, heap);

            res = val_mult(&lhs.val, &rhs.val, breakpoints, heap);
        }
        IROp::Divide(r) => {
            let lhs = a.source.to_val(vars, params, breakpoints, opts, heap);
            let rhs = r.to_val(vars, params, breakpoints, opts, heap);

            res = val_quot(&lhs.val, &rhs.val, breakpoints, heap);
        }
        IROp::IntDivide(r) => {
            let lhs = a.source.to_val(vars, params, breakpoints, opts, heap);
            let rhs = r.to_val(vars, params, breakpoints, opts, heap);

            res = val_int_quot(&lhs.val, &rhs.val, breakpoints, heap);
        }
        IROp::Mod(r) => {
            let lhs = a.source.to_val(vars, params, breakpoints, opts, heap);
            let rhs = r.to_val(vars, params, breakpoints, opts, heap);

            res = val_mod(&lhs.val, &rhs.val, breakpoints, heap);
        }
    }
    match &a.target {
        IRTarget::Ignore => {}
        IRTarget::Variable(v) => {
            vars[*v] = res;
        }
        IRTarget::Deref(v) => {
            let vars_ptr = vars as *mut [InterpVal];
            match &vars[*v] {
                InterpVal::Ptr(p) => {
                    if !matches!(p.sgmt, InterpPtrSgmt::Immediate)
                        && let InterpVal::Ref(r) = res
                    {
                        heap.refs.remove(&r);
                    }

                    // SAFETY: IR-PTR
                    unsafe {
                        *p.ptr = res;
                    }
                }
                InterpVal::OffsetStrPtr(s) => {
                    let insert = serialize(
                        &res,
                        // SAFETY: non-invalidating mutable borrow
                        unsafe { &mut *vars_ptr },
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        opts,
                        rl,
                        SerializeOpts::default(),
                    );
                    // SAFETY: IR-PTR
                    let val: &mut String = unsafe { &mut *s.val };
                    if let Some((start, ch)) = val.char_indices().nth(s.offset) {
                        let end = start + ch.len_utf8();
                        val.replace_range(start..end, &insert);
                    } else {
                        exception_throw("ir-op", "assign string out of bounds", breakpoints);
                    }
                }
                _ => panic!("internal: ptr deref only implemented for ptr"),
            }
        }
    }
}
