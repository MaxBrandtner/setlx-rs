use num_bigint::BigInt;
use num_traits::cast::ToPrimitive;
use rustyline::DefaultEditor;
use std::cmp::Ordering;
use std::panic;

use crate::cli::InputOpts;
use crate::interp::debug::DebugData;
use crate::interp::except::*;
use crate::interp::heap::*;
use crate::interp::memoize::InterpMemoize;
use crate::interp::serialize::{SerializeOpts, serialize};
use crate::interp::stack::InterpStack;

fn tagged_cmp(t1: &InterpTaggedList, t2: &InterpTaggedList) -> Ordering {
    let tag_ord = t1.tag.cmp(&t2.tag);
    if tag_ord != Ordering::Equal {
        return tag_ord;
    }

    t1.list
        .iter()
        .zip(t2.list.iter())
        .map(|(a, b)| val_cmp(a, b))
        .find(|&ord| ord != Ordering::Equal)
        .unwrap_or(t1.list.len().cmp(&t2.list.len()))
}

pub fn val_cmp(lhs: &InterpVal, rhs: &InterpVal) -> Ordering {
    match (lhs, rhs) {
        (InterpVal::Bool(bl), InterpVal::Bool(br)) => bl.cmp(br),
        (InterpVal::Double(dl), InterpVal::Double(dr)) => {
            dl.partial_cmp(dr).unwrap_or(Ordering::Less)
        }
        (InterpVal::Type(tl), InterpVal::Type(tr)) => tl.cmp(tr),
        (InterpVal::Double(d), InterpVal::Ref(r)) | (InterpVal::Ref(r), InterpVal::Double(d)) => {
            if let InterpObj::Number(i) = unsafe { &*r.0 } {
                i.to_f64().unwrap().partial_cmp(d).unwrap_or(Ordering::Less)
            } else {
                lhs.tag().cmp(&rhs.tag())
            }
        }
        (InterpVal::Slice(sl), InterpVal::Slice(sr)) => match (sl, sr) {
            (InterpSlice::StringSlice(s1), InterpSlice::StringSlice(s2)) => {
                s1.slice.clone().cmp(s2.slice.clone())
            }
            (InterpSlice::ListSlice(ll), InterpSlice::ListSlice(lr)) => ll
                .iter()
                .zip(lr.iter())
                .map(|(a, b)| val_cmp(a, b))
                .find(|&ord| ord != Ordering::Equal)
                .unwrap_or(ll.len().cmp(&lr.len())),
            _ => lhs.tag().cmp(&rhs.tag()),
        },
        (InterpVal::Ref(r), InterpVal::Char(c)) | (InterpVal::Char(c), InterpVal::Ref(r)) => {
            if let InterpObj::String(s) = unsafe { &*r.0 }
                && s.starts_with(*c)
            {
                Ordering::Equal
            } else {
                lhs.tag().cmp(&rhs.tag())
            }
        }
        (InterpVal::Slice(sl), InterpVal::Char(c)) | (InterpVal::Char(c), InterpVal::Slice(sl)) => {
            if let InterpSlice::StringSlice(s) = sl
                && s.slice.clone().next() == Some(*c)
            {
                Ordering::Equal
            } else {
                lhs.tag().cmp(&rhs.tag())
            }
        }
        (InterpVal::Ref(rl), InterpVal::Ref(rr)) => match (unsafe { &*rl.0 }, unsafe { &*rr.0 }) {
            (InterpObj::List(ll), InterpObj::List(lr)) => {
                ll.0.iter()
                    .zip(lr.0.iter())
                    .map(|(a, b)| val_cmp(a, b))
                    .find(|&ord| ord != Ordering::Equal)
                    .unwrap_or(ll.0.len().cmp(&lr.0.len()))
            }
            (InterpObj::Set(ll), InterpObj::Set(lr)) => {
                ll.0.iter()
                    .zip(lr.0.iter())
                    .map(|(a, b)| val_cmp(a, b))
                    .find(|&ord| ord != Ordering::Equal)
                    .unwrap_or(ll.0.len().cmp(&lr.0.len()))
            }
            (InterpObj::String(s1), InterpObj::String(s2)) => s1.cmp(s2),
            (InterpObj::Number(n1), InterpObj::Number(n2)) => n1.cmp(n2),
            (InterpObj::Term(t1), InterpObj::Term(t2))
            | (InterpObj::TTerm(t1), InterpObj::TTerm(t2))
            | (InterpObj::Ast(t1), InterpObj::Ast(t2)) => tagged_cmp(t1, t2),
            (InterpObj::Vector(v1), InterpObj::Vector(v2)) => {
                v1.partial_cmp(v2).unwrap_or(Ordering::Less)
            }
            (InterpObj::Matrix(m1), InterpObj::Matrix(m2)) => {
                m1.partial_cmp(m2).unwrap_or(Ordering::Less)
            }
            (InterpObj::Procedure(p1), InterpObj::Procedure(p2)) => {
                tagged_cmp(p1.info.as_ref().unwrap(), p2.info.as_ref().unwrap())
            }
            _ => unsafe { (&*rl.0).tag().cmp(&(&*rr.0).tag()) },
        },
        (InterpVal::Ref(r), InterpVal::Slice(s)) | (InterpVal::Slice(s), InterpVal::Ref(r)) => {
            match (unsafe { &*r.0 }, s) {
                (InterpObj::String(sl), InterpSlice::StringSlice(sr)) => {
                    sl.cmp(&sr.slice.clone().collect::<String>())
                }
                (InterpObj::List(ll), InterpSlice::ListSlice(lr)) => {
                    ll.0.iter()
                        .zip(lr.iter())
                        .map(|(a, b)| val_cmp(a, b))
                        .find(|&ord| ord != Ordering::Equal)
                        .unwrap_or(ll.0.len().cmp(&lr.len()))
                }
                _ => lhs.tag().cmp(&rhs.tag()),
            }
        }
        _ => lhs.tag().cmp(&rhs.tag()),
    }
}

pub fn val_plus(
    lhs: &InterpVal,
    rhs: &InterpVal,
    vars: &mut [InterpVal],
    stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    memo: &mut InterpMemoize,
    cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    opts: &InputOpts,
    rl: &mut DefaultEditor,
) -> InterpVal {
    // String + anything
    // List + List
    // Set + Set
    // Double + Double
    // Number + Double
    // Number + Number
    // Ast + anything
    // TTerm + anything
    // Term + anything
    // Vec + Vec
    // Matrix + Matrix
    match (lhs, rhs) {
        (InterpVal::Ref(lr), InterpVal::Ref(rr)) => {
            let rlr = unsafe { &*lr.0 };
            let rrr = unsafe { &*rr.0 };
            match (rlr, rrr) {
                (InterpObj::String(sl), InterpObj::String(sr)) => InterpVal::Ref(heap.push(
                    InterpObjRef::from_obj(InterpObj::String(sl.to_string() + sr)),
                )),
                (InterpObj::String(s), _) => {
                    InterpVal::Ref(heap.push(InterpObjRef::from_obj(InterpObj::String(
                        s.to_string()
                            + &serialize(
                                rhs,
                                vars,
                                stack,
                                memo,
                                cstore,
                                breakpoints,
                                opts,
                                rl,
                                SerializeOpts::default(),
                            ),
                    ))))
                }
                (_, InterpObj::String(s)) => {
                    InterpVal::Ref(heap.push(InterpObjRef::from_obj(InterpObj::String(
                        serialize(
                            lhs,
                            vars,
                            stack,
                            memo,
                            cstore,
                            breakpoints,
                            opts,
                            rl,
                            SerializeOpts::default(),
                        ) + s,
                    ))))
                }
                (InterpObj::List(ll), InterpObj::List(lr)) => {
                    let mut list: InterpList = ll.clone();
                    list.0
                        .extend(lr.0.iter().map(|i| i.unshare()).collect::<Vec<_>>());
                    InterpVal::Ref(heap.push_obj(InterpObj::List(list)))
                }
                (InterpObj::Set(sl), InterpObj::Set(sr)) => InterpVal::Ref(heap.push_obj(
                    InterpObj::Set(InterpSet(sl.0.union(&sr.0).map(|i| i.unshare()).collect())),
                )),
                (InterpObj::List(l), InterpObj::Set(s)) => {
                    let mut list: InterpList = l.clone();
                    list.0
                        .extend(s.0.iter().map(|i| i.unshare()).collect::<Vec<_>>());
                    InterpVal::Ref(heap.push_obj(InterpObj::List(list)))
                }
                (InterpObj::Set(s), InterpObj::List(l)) => {
                    InterpVal::Ref(heap.push_obj(InterpObj::Set(InterpSet(
                        l.0.iter().chain(s.0.iter()).map(|i| i.unshare()).collect(),
                    ))))
                }
                (InterpObj::Number(nl), InterpObj::Number(nr)) => {
                    InterpVal::Ref(heap.push_obj(InterpObj::Number(nl + nr)))
                }
                (InterpObj::Vector(vl), InterpObj::Vector(vr)) => {
                    InterpVal::Ref(heap.push_obj(InterpObj::Vector(vl + vr)))
                }
                (InterpObj::Matrix(ml), InterpObj::Matrix(mr)) => {
                    InterpVal::Ref(heap.push_obj(InterpObj::Matrix(ml + mr)))
                }
                (InterpObj::Ast(_), InterpObj::Ast(_))
                | (InterpObj::Ast(_), _)
                | (_, InterpObj::Ast(_))
                | (InterpObj::TTerm(_), InterpObj::TTerm(_))
                | (InterpObj::TTerm(_), _)
                | (_, InterpObj::TTerm(_))
                | (InterpObj::Term(_), InterpObj::Term(_))
                | (InterpObj::Term(_), _)
                | (_, InterpObj::Term(_)) => {
                    let ast_lhs_val: InterpObj = rlr.clone();
                    let ast_rhs_val: InterpObj = rrr.clone();
                    let ast_lhs = InterpVal::Ref(heap.push_obj(ast_lhs_val));
                    let ast_rhs = InterpVal::Ref(heap.push_obj(ast_rhs_val));

                    InterpVal::Ref(heap.push_obj(InterpObj::Ast(InterpTaggedList {
                        tag: "plus".to_string(),
                        list: vec![ast_lhs, ast_rhs],
                    })))
                }
                _ => exception_throw("ir-op", "plus is not defined for type", breakpoints),
            }
        }
        (InterpVal::Double(dl), InterpVal::Double(dr)) => InterpVal::Double(dl + dr),
        (InterpVal::Char(cl), InterpVal::Char(cr)) => {
            InterpVal::Ref(heap.push_obj(InterpObj::String(cl.to_string() + &cr.to_string())))
        }
        (InterpVal::Ref(r), _) => match unsafe { &*r.0 } {
            InterpObj::List(l) => {
                if let InterpVal::Slice(i) = rhs {
                    match i {
                        InterpSlice::ListSlice(sl) => {
                            let mut new = l.clone();
                            new.0
                                .extend(sl.iter().map(|i| i.unshare()).collect::<Vec<_>>());

                            InterpVal::Ref(heap.push_obj(InterpObj::List(new)))
                        }
                        InterpSlice::StringSlice(sl) => {
                            InterpVal::Ref(heap.push_obj(InterpObj::String(
                                serialize(
                                    lhs,
                                    vars,
                                    stack,
                                    memo,
                                    cstore,
                                    breakpoints,
                                    opts,
                                    rl,
                                    SerializeOpts::default(),
                                ) + &sl.slice.clone().collect::<String>(),
                            )))
                        }
                    }
                } else {
                    exception_throw("ir-op", "plus is undefined for type", breakpoints);
                }
            }
            InterpObj::Set(s) => {
                if let InterpVal::Slice(i) = rhs
                    && let InterpSlice::ListSlice(l) = i
                {
                    InterpVal::Ref(heap.push_obj(InterpObj::Set(InterpSet(
                        l.iter().chain(s.0.iter()).map(|i| i.unshare()).collect(),
                    ))))
                } else {
                    exception_throw("ir-op", "plus is undefined for type", breakpoints);
                }
            }
            InterpObj::String(s) => InterpVal::Ref(heap.push_obj(InterpObj::String(
                s.to_string()
                    + &serialize(
                        rhs,
                        vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        opts,
                        rl,
                        SerializeOpts::default(),
                    ),
            ))),
            InterpObj::Number(n) => {
                if let InterpVal::Double(d) = rhs {
                    InterpVal::Double(n.to_f64().unwrap() + d)
                } else {
                    exception_throw("ir-op", "plus is not defined for type", breakpoints);
                }
            }
            _ => exception_throw("ir-op", "plus is not defined for type", breakpoints),
        },
        (_, InterpVal::Ref(r)) => match unsafe { &*r.0 } {
            InterpObj::List(l) => {
                if let InterpVal::Slice(i) = lhs {
                    match i {
                        InterpSlice::ListSlice(sl) => {
                            let mut new = InterpList(sl.iter().map(|i| i.unshare()).collect());
                            new.extend(l);
                            InterpVal::Ref(heap.push_obj(InterpObj::List(new)))
                        }
                        InterpSlice::StringSlice(sl) => {
                            InterpVal::Ref(heap.push_obj(InterpObj::String(
                                sl.slice.clone().collect::<String>()
                                    + &serialize(
                                        rhs,
                                        vars,
                                        stack,
                                        memo,
                                        cstore,
                                        breakpoints,
                                        opts,
                                        rl,
                                        SerializeOpts::default(),
                                    ),
                            )))
                        }
                    }
                } else {
                    exception_throw("ir-op", "plus is undefined for type", breakpoints);
                }
            }
            InterpObj::Set(s) => {
                if let InterpVal::Slice(i) = lhs
                    && let InterpSlice::ListSlice(l) = i
                {
                    let mut list = InterpList(l.iter().map(|i| i.unshare()).collect());
                    list.0
                        .extend(s.0.iter().map(|i| i.unshare()).collect::<Vec<_>>());
                    InterpVal::Ref(heap.push_obj(InterpObj::List(list)))
                } else {
                    exception_throw("ir-op", "plus is undefined for type", breakpoints);
                }
            }
            InterpObj::String(s) => InterpVal::Ref(heap.push_obj(InterpObj::String(
                serialize(
                    lhs,
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    opts,
                    rl,
                    SerializeOpts::default(),
                ) + s,
            ))),
            InterpObj::Number(n) => {
                if let InterpVal::Double(d) = lhs {
                    InterpVal::Double(n.to_f64().unwrap() + d)
                } else {
                    exception_throw("ir-op", "plus is not defined for type", breakpoints);
                }
            }
            _ => exception_throw("ir-op", "plus is not defined for type", breakpoints),
        },
        (InterpVal::Slice(sl), _) => {
            if let InterpSlice::StringSlice(s) = sl {
                InterpVal::Ref(heap.push_obj(InterpObj::String(
                    s.slice.clone().collect::<String>()
                        + &serialize(
                            lhs,
                            vars,
                            stack,
                            memo,
                            cstore,
                            breakpoints,
                            opts,
                            rl,
                            SerializeOpts::default(),
                        ),
                )))
            } else {
                exception_throw("ir-op", "plus is not defined for type", breakpoints);
            }
        }
        (_, InterpVal::Slice(sl)) => {
            if let InterpSlice::StringSlice(s) = sl {
                InterpVal::Ref(heap.push_obj(InterpObj::String(
                    serialize(
                        rhs,
                        vars,
                        stack,
                        memo,
                        cstore,
                        breakpoints,
                        opts,
                        rl,
                        SerializeOpts::default(),
                    ) + &s.slice.clone().collect::<String>(),
                )))
            } else {
                exception_throw("ir-op", "plus is not defined for type", breakpoints);
            }
        }
        _ => exception_throw("ir-op", "plus is not defined for type", breakpoints),
    }
}

pub fn val_minus(
    lhs: &InterpVal,
    rhs: &InterpVal,
    breakpoints: &DebugData,
    heap: &mut InterpImmediateHeap,
) -> InterpVal {
    // set - set
    // vector - vector
    // matrix - matrix
    // number - number
    // number - double
    // double - number
    // double - double
    match (lhs, rhs) {
        (InterpVal::Ref(lr), InterpVal::Ref(rr)) => match (unsafe { &*lr.0 }, unsafe { &*rr.0 }) {
            (InterpObj::Set(sl), InterpObj::Set(sr)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Set(InterpSet(
                    sl.0.difference(&sr.0).map(|i| i.unshare()).collect(),
                ))))
            }
            (InterpObj::Vector(vl), InterpObj::Vector(vr)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Vector(vl - vr)))
            }
            (InterpObj::Matrix(ml), InterpObj::Matrix(mr)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Matrix(ml - mr)))
            }
            (InterpObj::Number(nl), InterpObj::Number(nr)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Number(nl - nr)))
            }
            _ => exception_throw("ir-op", "minus is not defined for type", breakpoints),
        },
        (InterpVal::Ref(r), _) => {
            if let InterpObj::Number(n) = unsafe { &*r.0 }
                && let InterpVal::Double(d) = rhs
            {
                InterpVal::Double(n.to_f64().unwrap() - d)
            } else {
                exception_throw("ir-op", "minus not defined for type", breakpoints)
            }
        }
        (_, InterpVal::Ref(r)) => {
            if let InterpObj::Number(n) = unsafe { &*r.0 }
                && let InterpVal::Double(d) = lhs
            {
                InterpVal::Double(d - n.to_f64().unwrap())
            } else {
                exception_throw("ir-op", "minus not defined for type", breakpoints)
            }
        }
        (InterpVal::Double(ld), InterpVal::Double(rd)) => InterpVal::Double(ld - rd),
        _ => exception_throw("ir-op", "minus is not defined for type", breakpoints),
    }
}

pub fn val_mult(
    lhs: &InterpVal,
    rhs: &InterpVal,
    breakpoints: &DebugData,
    heap: &mut InterpImmediateHeap,
) -> InterpVal {
    // set * set
    // matrix * matrix
    // matrix * vector
    // matrix * number
    // matrix * double
    // vector * vector
    // vector * number
    // vector * double
    // number * number
    // number * string
    // number * char
    // number * double
    // double * double
    match (lhs, rhs) {
        (InterpVal::Double(dl), InterpVal::Double(dr)) => InterpVal::Double(dl * dr),
        (InterpVal::Ref(lr), InterpVal::Ref(rr)) => match (unsafe { &*lr.0 }, unsafe { &*rr.0 }) {
            (InterpObj::Set(sl), InterpObj::Set(sr)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Set(InterpSet(
                    sl.0.intersection(&sr.0).map(|i| i.unshare()).collect(),
                ))))
            }
            (InterpObj::Matrix(l), InterpObj::Matrix(r)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Matrix(l * r)))
            }
            (InterpObj::Matrix(l), InterpObj::Vector(r)) => {
                let r_col = r.clone().transpose();

                InterpVal::Ref(heap.push_obj(InterpObj::Matrix(l * r_col)))
            }
            (InterpObj::Vector(l), InterpObj::Matrix(r)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Matrix(l * r)))
            }
            (InterpObj::Matrix(l), InterpObj::Number(r)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Matrix(l * r.to_f64().unwrap())))
            }
            (InterpObj::Number(l), InterpObj::Matrix(r)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Matrix(l.to_f64().unwrap() * r)))
            }
            (InterpObj::Vector(l), InterpObj::Vector(r)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Vector(l * r)))
            }
            (InterpObj::Vector(l), InterpObj::Number(r)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Vector(l * r.to_f64().unwrap())))
            }
            (InterpObj::Number(l), InterpObj::Vector(r)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Vector(l.to_f64().unwrap() * r)))
            }
            (InterpObj::Number(l), InterpObj::Number(r)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Number(l * r)))
            }
            (InterpObj::String(l), InterpObj::Number(r)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::String(l.repeat(r.to_usize().unwrap()))))
            }
            (InterpObj::Number(l), InterpObj::String(r)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::String(r.repeat(l.to_usize().unwrap()))))
            }
            _ => panic!("multiply is not defined for type"),
        },
        (InterpVal::Ref(r), InterpVal::Char(c)) | (InterpVal::Char(c), InterpVal::Ref(r)) => {
            match unsafe { &*r.0 } {
                InterpObj::Number(n) => InterpVal::Ref(heap.push_obj(InterpObj::String(
                    c.to_string().repeat(n.to_usize().unwrap()),
                ))),
                _ => exception_throw("ir-op", "multiply is not defined for type", breakpoints),
            }
        }
        (InterpVal::Ref(r), InterpVal::Double(d)) | (InterpVal::Double(d), InterpVal::Ref(r)) => {
            match unsafe { &*r.0 } {
                InterpObj::Matrix(m) => InterpVal::Ref(heap.push_obj(InterpObj::Matrix(m * *d))),
                InterpObj::Vector(v) => InterpVal::Ref(heap.push_obj(InterpObj::Vector(v * *d))),
                InterpObj::Number(n) => InterpVal::Double(n.to_f64().unwrap() * d),
                _ => exception_throw("ir-op", "multiply is not defined for type", breakpoints),
            }
        }
        _ => exception_throw("ir-op", "multiply is not defined for type", breakpoints),
    }
}

pub fn val_quot(
    lhs: &InterpVal,
    rhs: &InterpVal,
    breakpoints: &DebugData,
    heap: &mut InterpImmediateHeap,
) -> InterpVal {
    // matrix / number
    // matrix / double
    // vector / number
    // vector / double
    // number / number
    // number / double
    // double / double
    // double / number
    match (lhs, rhs) {
        (InterpVal::Ref(rl), InterpVal::Ref(rr)) => match (unsafe { &*rl.0 }, unsafe { &*rr.0 }) {
            (InterpObj::Matrix(m), InterpObj::Number(n)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Matrix(m / n.to_f64().unwrap())))
            }
            (InterpObj::Vector(v), InterpObj::Number(n)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Vector(v / n.to_f64().unwrap())))
            }
            (InterpObj::Number(l), InterpObj::Number(r)) => {
                InterpVal::Double(l.to_f64().unwrap() / r.to_f64().unwrap())
            }
            _ => exception_throw("ir-op", "divide is not defined for type", breakpoints),
        },
        (InterpVal::Ref(r), InterpVal::Double(d)) => match unsafe { &*r.0 } {
            InterpObj::Matrix(m) => InterpVal::Ref(heap.push_obj(InterpObj::Matrix(m / *d))),
            InterpObj::Vector(v) => InterpVal::Ref(heap.push_obj(InterpObj::Vector(v / *d))),
            InterpObj::Number(n) => InterpVal::Double(n.to_f64().unwrap() / d),
            _ => exception_throw("ir-op", "divide is not defined for type", breakpoints),
        },
        (InterpVal::Double(d), InterpVal::Ref(r)) => match unsafe { &*r.0 } {
            InterpObj::Number(n) => InterpVal::Double(d / n.to_f64().unwrap()),
            _ => exception_throw("ir-op", "divide is not defined for type", breakpoints),
        },
        (InterpVal::Double(dl), InterpVal::Double(dr)) => InterpVal::Double(dl / dr),
        _ => exception_throw("ir-op", "divide is not defined for type", breakpoints),
    }
}

pub fn val_int_quot(
    lhs: &InterpVal,
    rhs: &InterpVal,
    breakpoints: &DebugData,
    heap: &mut InterpImmediateHeap,
) -> InterpVal {
    // number / number
    // number / double
    // double / double
    // double / number
    match (lhs, rhs) {
        (InterpVal::Ref(rl), InterpVal::Ref(rr)) => match (unsafe { &*rl.0 }, unsafe { &*rr.0 }) {
            (InterpObj::Number(l), InterpObj::Number(r)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Number(l / r)))
            }
            _ => exception_throw("ir-op", "int divide is not defined for type", breakpoints),
        },
        (InterpVal::Ref(r), InterpVal::Double(d)) => match unsafe { &*r.0 } {
            InterpObj::Number(n) => InterpVal::Ref(heap.push_obj(InterpObj::Number(BigInt::from(
                (n.to_f64().unwrap() / d) as i64,
            )))),
            _ => exception_throw("ir-op", "int divide is not defined for type", breakpoints),
        },
        (InterpVal::Double(d), InterpVal::Ref(r)) => match unsafe { &*r.0 } {
            InterpObj::Number(n) => InterpVal::Ref(heap.push_obj(InterpObj::Number(BigInt::from(
                (d / n.to_f64().unwrap()) as i64,
            )))),
            _ => exception_throw("ir-op", "int divide is not defined for type", breakpoints),
        },
        (InterpVal::Double(dl), InterpVal::Double(dr)) => {
            InterpVal::Ref(heap.push_obj(InterpObj::Number(BigInt::from((dl / dr) as i64))))
        }
        _ => exception_throw("ir-op", "int divide is not defined for type", breakpoints),
    }
}

pub fn val_mod(
    lhs: &InterpVal,
    rhs: &InterpVal,
    breakpoints: &DebugData,
    heap: &mut InterpImmediateHeap,
) -> InterpVal {
    // set % set
    // number % number
    // double % number
    // number % double
    // double % double
    match (lhs, rhs) {
        (InterpVal::Ref(lr), InterpVal::Ref(rr)) => match (unsafe { &*lr.0 }, unsafe { &*rr.0 }) {
            (InterpObj::Set(sl), InterpObj::Set(sr)) => InterpVal::Ref(
                heap.push_obj(InterpObj::Set(InterpSet(
                    sl.0.symmetric_difference(&sr.0)
                        .map(|i| i.unshare())
                        .collect(),
                ))),
            ),
            (InterpObj::Number(nl), InterpObj::Number(nr)) => {
                InterpVal::Ref(heap.push_obj(InterpObj::Number(nl % nr)))
            }
            _ => exception_throw("ir-op", "mod is not defined for type", breakpoints),
        },
        (InterpVal::Double(d), InterpVal::Ref(r)) => match unsafe { &*r.0 } {
            InterpObj::Number(n) => InterpVal::Ref(heap.push_obj(InterpObj::Number(BigInt::from(
                (d % n.to_f64().unwrap()) as i64,
            )))),
            _ => exception_throw("ir-op", "mod is not defined for type", breakpoints),
        },
        (InterpVal::Ref(r), InterpVal::Double(d)) => match unsafe { &*r.0 } {
            InterpObj::Number(n) => InterpVal::Ref(heap.push_obj(InterpObj::Number(BigInt::from(
                (n.to_f64().unwrap() % d) as i64,
            )))),
            _ => exception_throw("ir-op", "mod is not defined for type", breakpoints),
        },
        (InterpVal::Double(dl), InterpVal::Double(dr)) => {
            InterpVal::Ref(heap.push_obj(InterpObj::Number(BigInt::from((dl % dr) as i64))))
        }
        _ => exception_throw("ir-op", "mod is not defined for type", breakpoints),
    }
}
