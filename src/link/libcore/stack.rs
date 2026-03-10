use yansi::Paint;

use crate::builtin::{BuiltinProcParamOwnership, BuiltinProcStruct};
use crate::cli::InputOpts;
use crate::interp::{
    debug::DebugData, get::InterpGet, heap::*, memoize::InterpMemoize, stack::InterpStack,
};
use crate::ir::def::IRValue;
use rustyline::DefaultEditor;

const STACK_FRAME_ADD: BuiltinProcStruct = BuiltinProcStruct {
    name: "stack_frame_add",
    id: "setlx_libcore_stack_frame_add",
    func: setlx_libcore_stack_frame_add,
    ownership: &[],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_stack_frame_add(
    _params: &[IRValue],
    _params_proc: &InterpVal,
    _vars: &mut [InterpVal],
    stack: &mut InterpStack,
    _heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    _breakpoints: &mut DebugData,
    _opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    stack.frame_push();
    InterpVal::Undefined
}

const STACK_FRAME_POP: BuiltinProcStruct = BuiltinProcStruct {
    name: "stack_frame_pop",
    id: "setlx_libcore_stack_frame_pop",
    func: setlx_libcore_stack_frame_pop,
    ownership: &[],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_stack_frame_pop(
    _params: &[IRValue],
    _params_proc: &InterpVal,
    _vars: &mut [InterpVal],
    stack: &mut InterpStack,
    _heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    _breakpoints: &mut DebugData,
    _opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    stack.frame_pop();
    InterpVal::Undefined
}

const STACK_FRAME_SAVE: BuiltinProcStruct = BuiltinProcStruct {
    name: "stack_frame_save",
    id: "setlx_libcore_stack_frame_save",
    func: setlx_libcore_stack_frame_save,
    ownership: &[],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_stack_frame_save(
    _params: &[IRValue],
    _params_proc: &InterpVal,
    _vars: &mut [InterpVal],
    stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    _breakpoints: &mut DebugData,
    _opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    let obj = InterpObj::StackImage(stack.frame_save());
    InterpVal::Ref(heap.push_obj(obj))
}

const STACK_FRAME_RESTORE: BuiltinProcStruct = BuiltinProcStruct {
    name: "stack_frame_restore",
    id: "setlx_libcore_stack_frame_restore",
    func: setlx_libcore_stack_frame_restore,
    ownership: &[BuiltinProcParamOwnership::Borrowed],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_stack_frame_restore(
    params: &[IRValue],
    _params_proc: &InterpVal,
    vars: &mut [InterpVal],
    stack: &mut InterpStack,
    _heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    _breakpoints: &mut DebugData,
    _opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
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

const STACK_FRAME_COPY: BuiltinProcStruct = BuiltinProcStruct {
    name: "stack_frame_copy",
    id: "setlx_libcore_stack_frame_copy",
    func: setlx_libcore_stack_frame_copy,
    ownership: &[],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_stack_frame_copy(
    _params: &[IRValue],
    _params_proc: &InterpVal,
    _vars: &mut [InterpVal],
    stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    _breakpoints: &mut DebugData,
    _opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    let obj = InterpObj::StackImage(stack.frame_copy());
    InterpVal::Ref(heap.push_obj(obj))
}

const STACK_COPY: BuiltinProcStruct = BuiltinProcStruct {
    name: "stack_copy",
    id: "setlx_libcore_stack_copy",
    func: setlx_libcore_stack_copy,
    ownership: &[],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_stack_copy(
    _params: &[IRValue],
    _params_proc: &InterpVal,
    _vars: &mut [InterpVal],
    stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    _breakpoints: &mut DebugData,
    _opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    let obj = InterpObj::StackImage(stack.copy_reachable());
    InterpVal::Ref(heap.push_obj(obj))
}

const STACK_ADD: BuiltinProcStruct = BuiltinProcStruct {
    name: "stack_add",
    id: "setlx_libcore_stack_add",
    func: setlx_libcore_stack_add,
    ownership: &[BuiltinProcParamOwnership::Borrowed],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_stack_add(
    params: &[IRValue],
    _params_proc: &InterpVal,
    _vars: &mut [InterpVal],
    stack: &mut InterpStack,
    _heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    _breakpoints: &mut DebugData,
    _opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    stack.add(params[0].to_immed_str("stack_add"))
}

const STACK_ALIAS: BuiltinProcStruct = BuiltinProcStruct {
    name: "stack_alias",
    id: "setlx_libcore_stack_alias",
    func: setlx_libcore_stack_alias,
    ownership: &[
        BuiltinProcParamOwnership::Borrowed,
        BuiltinProcParamOwnership::Borrowed,
        BuiltinProcParamOwnership::Borrowed,
    ],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_stack_alias(
    params: &[IRValue],
    params_proc: &InterpVal,
    vars: &mut [InterpVal],
    stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    let name = params[0].to_str(vars, params_proc, breakpoints, opts, heap);
    let ptr = params[1].to_ptr(vars, "stack_alias");
    let cross_frame = params[2].to_immed_bool("stack_alias");

    stack.alias(&name, &ptr, cross_frame);
    InterpVal::Undefined
}

const STACK_POP: BuiltinProcStruct = BuiltinProcStruct {
    name: "stack_pop",
    id: "setlx_libcore_stack_pop",
    func: setlx_libcore_stack_pop,
    ownership: &[BuiltinProcParamOwnership::Borrowed],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_stack_pop(
    params: &[IRValue],
    _params_proc: &InterpVal,
    _vars: &mut [InterpVal],
    stack: &mut InterpStack,
    _heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    _breakpoints: &mut DebugData,
    _opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    let s = params[0].to_immed_str("stack_pop");

    stack.pop(s);
    InterpVal::Undefined
}

const STACK_GET_OR_NEW: BuiltinProcStruct = BuiltinProcStruct {
    name: "stack_get_or_new",
    id: "setlx_libcore_stack_get_or_new",
    func: setlx_libcore_stack_get_or_new,
    ownership: &[BuiltinProcParamOwnership::Borrowed],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_stack_get_or_new(
    params: &[IRValue],
    _params_proc: &InterpVal,
    _vars: &mut [InterpVal],
    stack: &mut InterpStack,
    _heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    cstore: &mut InterpClassStore,
    _breakpoints: &mut DebugData,
    opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
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

const STACK_IN_SCOPE: BuiltinProcStruct = BuiltinProcStruct {
    name: "stack_in_scope",
    id: "setlx_libcore_stack_in_scope",
    func: setlx_libcore_stack_in_scope,
    ownership: &[BuiltinProcParamOwnership::Borrowed],
};

#[unsafe(no_mangle)]
extern "C-unwind" fn setlx_libcore_stack_in_scope(
    params: &[IRValue],
    _params_proc: &InterpVal,
    _vars: &mut [InterpVal],
    stack: &mut InterpStack,
    _heap: &mut InterpImmediateHeap,
    _memo: &mut InterpMemoize,
    _cstore: &mut InterpClassStore,
    _breakpoints: &mut DebugData,
    _opts: &InputOpts,
    _rl: &mut DefaultEditor,
) -> InterpVal {
    InterpVal::Bool(
        stack
            .get(params[0].to_immed_str("stack_in_scope"))
            .is_some(),
    )
}
