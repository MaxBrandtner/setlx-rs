use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::builtin::{BuiltinProcParamOwnership, BuiltinProcStruct};
use crate::cli::InputOpts;
use crate::interp::{
    debug::DebugData, except::*, get::InterpGet, heap::*, memoize::InterpMemoize, ops::val_cmp,
    stack::InterpStack,
};
use crate::ir::def::IRValue;
use rustyline::DefaultEditor;

const LIST_NEW: BuiltinProcStruct = BuiltinProcStruct {
    name: "list_new",
    id: "setlx_libcore_list_new",
    func: setlx_libcore_list_new,
    ownership: &[BuiltinProcParamOwnership::Borrowed],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_list_new(
    params: &[IRValue],
    params_proc: &InterpVal,
    vars: &mut [InterpVal],
    _stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    _opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    let n = if !params.is_empty() {
        params[0].to_usize(vars, params_proc, breakpoints, "list_new")
    } else {
        0
    };

    InterpVal::Ref(heap.push_obj(InterpObj::List(InterpList(
        (0..n).map(|_| InterpVal::Undefined).collect(),
    ))))
}

/*
 * @t_list: ptrs to list invalidated
 * @t_i: consumed
 *
 * _ := list_push(t_list, t_i);
 */
const LIST_PUSH: BuiltinProcStruct = BuiltinProcStruct {
    name: "list_push",
    id: "setlx_libcore_list_push",
    func: setlx_libcore_list_push,
    ownership: &[
        BuiltinProcParamOwnership::MutablyBorrowed,
        BuiltinProcParamOwnership::Consumed,
    ],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_list_push(
    params: &[IRValue],
    params_proc: &InterpVal,
    vars: &mut [InterpVal],
    _stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    let val = params[1]
        .to_val(vars, params_proc, breakpoints, opts, heap)
        .confirm()
        .val
        .clone()
        .persist(heap);
    params[0].to_list(vars, params_proc, breakpoints).push(val);
    InterpVal::Undefined
}

const LIST_RANGE: BuiltinProcStruct = BuiltinProcStruct {
    name: "list_range",
    id: "setlx_libcore_list_range",
    func: setlx_libcore_list_range,
    ownership: &[
        BuiltinProcParamOwnership::Borrowed,
        BuiltinProcParamOwnership::Borrowed,
    ],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_list_range(
    params: &[IRValue],
    params_proc: &InterpVal,
    vars: &mut [InterpVal],
    _stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    _opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
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

/*
 * @t_list: ptrs to list invalidated
 *
 * _ := list_resize(t_list, t_len);
 */
const LIST_RESIZE: BuiltinProcStruct = BuiltinProcStruct {
    name: "list_resize",
    id: "setlx_libcore_list_resize",
    func: setlx_libcore_list_resize,
    ownership: &[
        BuiltinProcParamOwnership::MutablyBorrowed,
        BuiltinProcParamOwnership::Borrowed,
    ],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_list_resize(
    params: &[IRValue],
    params_proc: &InterpVal,
    vars: &mut [InterpVal],
    _stack: &mut InterpStack,
    _heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    _opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    let n = params[1].to_usize(vars, params_proc, breakpoints, "list_resize");
    let l = params[0].to_list(vars, params_proc, breakpoints);
    l.0.truncate(n);

    InterpVal::Undefined
}

/*
 * @t_in: list, set, or string
 *        if list invalidates list ptrs
 *
 * t_out := pop(t_in);
 */
const POP: BuiltinProcStruct = BuiltinProcStruct {
    name: "pop",
    id: "setlx_libcore_pop",
    func: setlx_libcore_pop,
    ownership: &[BuiltinProcParamOwnership::MutablyBorrowed],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_pop(
    params: &[IRValue],
    params_proc: &InterpVal,
    vars: &mut [InterpVal],
    _stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    let coll = params[0].to_val(vars, params_proc, breakpoints, opts, heap);

    if let InterpVal::Ref(r) = &coll.val {
        match unsafe { &mut *r.0 } {
            InterpObj::String(s) => InterpVal::Ref(heap.push_obj(InterpObj::String(
                s.pop().map(|s| s.to_string()).unwrap_or(String::from("")),
            ))),
            InterpObj::List(l) => l.pop(heap).unwrap_or(InterpVal::Undefined),
            InterpObj::Set(s) => s.0.pop_last().unwrap_or(InterpVal::Undefined),
            _ => exception_throw("builtin procedure", "pop undefined for type", breakpoints),
        }
    } else {
        exception_throw("builtin procedure", "pop undefined for type", breakpoints);
    }
}

/* @slice: lifetime bound to input
 *
 * slice := slice(input, lower, upper);
 */
const SLICE: BuiltinProcStruct = BuiltinProcStruct {
    name: "slice",
    id: "setlx_libcore_slice",
    func: setlx_libcore_slice,
    ownership: &[
        BuiltinProcParamOwnership::Borrowed,
        BuiltinProcParamOwnership::Borrowed,
        BuiltinProcParamOwnership::Borrowed,
    ],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_slice(
    params: &[IRValue],
    params_proc: &InterpVal,
    vars: &mut [InterpVal],
    _stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    let lhs = params[1].to_i64(vars, params_proc, breakpoints, "slice");
    let rhs = params[2].to_i64(vars, params_proc, breakpoints, "slice");
    InterpVal::Slice(params[0].to_slice(lhs, rhs, vars, params_proc, breakpoints, opts, heap))
}

const SET_NEW: BuiltinProcStruct = BuiltinProcStruct {
    name: "set_new",
    id: "setlx_libcore_set_new",
    func: setlx_libcore_set_new,
    ownership: &[],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_set_new(
    _params: &[IRValue],
    _params_proc: &InterpVal,
    _vars: &mut [InterpVal],
    _stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    _breakpoints: &mut DebugData,
    _opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    InterpVal::Ref(heap.push_obj(InterpObj::Set(InterpSet(BTreeSet::new()))))
}

/*
 * @t_i: consumed
 *
 * t_ptr:<ptr, om> = set_insert(t_set, t_i);
 */
const SET_INSERT: BuiltinProcStruct = BuiltinProcStruct {
    name: "set_insert",
    id: "setlx_libcore_set_insert",
    func: setlx_libcore_set_insert,
    ownership: &[
        BuiltinProcParamOwnership::MutablyBorrowed,
        BuiltinProcParamOwnership::Consumed,
    ],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_set_insert(
    params: &[IRValue],
    params_proc: &InterpVal,
    vars: &mut [InterpVal],
    _stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
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

/*
 * @t_order: bool
 *  true  -> first
 *  false -> last
 *
 * t_out := set_borrow(t_set, t_order);
 */
const SET_BORROW: BuiltinProcStruct = BuiltinProcStruct {
    name: "set_borrow",
    id: "setlx_libcore_set_borrow",
    func: setlx_libcore_set_borrow,
    ownership: &[
        BuiltinProcParamOwnership::MutablyBorrowed,
        BuiltinProcParamOwnership::Borrowed,
    ],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_set_borrow(
    params: &[IRValue],
    params_proc: &InterpVal,
    vars: &mut [InterpVal],
    _stack: &mut InterpStack,
    _heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    _opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
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

/*
 * @t_order: bool
 *  true  -> first
 *  false -> last
 *
 * t_out := set_take(t_set, t_order);
 */
const SET_TAKE: BuiltinProcStruct = BuiltinProcStruct {
    name: "set_take",
    id: "setlx_libcore_set_take",
    func: setlx_libcore_set_take,
    ownership: &[
        BuiltinProcParamOwnership::MutablyBorrowed,
        BuiltinProcParamOwnership::Borrowed,
    ],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_set_take(
    params: &[IRValue],
    params_proc: &InterpVal,
    vars: &mut [InterpVal],
    _stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
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

/*
 * ptr := set_get_tag(set, expr);
 */
const SET_GET_TAG: BuiltinProcStruct = BuiltinProcStruct {
    name: "set_get_tag",
    id: "setlx_libcore_set_get_tag",
    func: setlx_libcore_set_get_tag,
    ownership: &[
        BuiltinProcParamOwnership::MutablyBorrowed,
        BuiltinProcParamOwnership::Borrowed,
    ],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_set_get_tag(
    params: &[IRValue],
    params_proc: &InterpVal,
    vars: &mut [InterpVal],
    _stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
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

// set := set_get_tag_all(set, expr);
const SET_GET_TAG_ALL: BuiltinProcStruct = BuiltinProcStruct {
    name: "set_get_tag_all",
    id: "setlx_libcore_set_get_tag_all",
    func: setlx_libcore_set_get_tag_all,
    ownership: &[
        BuiltinProcParamOwnership::MutablyBorrowed,
        BuiltinProcParamOwnership::Borrowed,
    ],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_set_get_tag_all(
    params: &[IRValue],
    params_proc: &InterpVal,
    vars: &mut [InterpVal],
    _stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
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
