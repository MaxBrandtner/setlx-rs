use ariadne::ReportKind;
use rustyline::DefaultEditor;
use std::any::Any;
use std::panic::{self, AssertUnwindSafe};

use crate::cli::InputOpts;
use crate::diagnostics::report;
use crate::interp::debug::DebugData;
use crate::interp::heap::{InterpClassStore, InterpObj, InterpObjRef, InterpVal};
use crate::interp::memoize::InterpMemoize;
use crate::interp::serialize::{SerializeOpts, serialize};
use crate::interp::stack::InterpStack;

static mut EXCEPTION_VAL: InterpVal = InterpVal::Undefined;
static mut EXCEPTION_KIND: ExceptionKind = ExceptionKind::Lng;

#[derive(Clone, Copy, Debug)]
pub enum ExceptionKind {
    Lng, // 0
    Usr, // 1
    Backtrack, // 2
         //Abort,   // 3
}

pub fn exception_val_set(input: InterpVal) {
    //SAFETY: setlx-rs is single-threaded
    unsafe {
        EXCEPTION_VAL = input;
    }
}

pub fn exception_val_get() -> InterpVal {
    //SAFETY: setlx-rs is single-threaded
    unsafe {
        #[allow(static_mut_refs)]
        EXCEPTION_VAL.clone()
    }
}

pub fn exception_kind_set(input: ExceptionKind) {
    //SAFETY: setlx-rs is single-threaded
    unsafe {
        EXCEPTION_KIND = input;
    }
}

pub fn exception_kind_get() -> ExceptionKind {
    //SAFETY: setlx-rs is single-threaded
    unsafe { EXCEPTION_KIND }
}

pub fn exception_kind_num_get() -> u8 {
    //SAFETY: setlx-rs is single-threaded
    unsafe {
        match EXCEPTION_KIND {
            ExceptionKind::Lng => 0,
            ExceptionKind::Usr => 1,
            ExceptionKind::Backtrack => 2,
        }
    }
}

pub fn exception_throw(cat_msg: &str, msg: &str, data: &DebugData) -> ! {
    let mut input = String::new();
    report(
        ReportKind::Error,
        cat_msg,
        msg,
        data.code_lhs,
        data.code_rhs,
        &data.src,
        &data.srcname,
        &mut input,
    );

    exception_val_set(InterpVal::Ref(InterpObjRef::from_obj(InterpObj::String(
        input.clone(),
    ))));
    exception_kind_set(ExceptionKind::Usr);
    panic!("{input}");
}

pub fn exception_throw_raw(input: &str) -> ! {
    exception_val_set(InterpVal::Ref(InterpObjRef::from_obj(InterpObj::String(
        input.to_string(),
    ))));
    exception_kind_set(ExceptionKind::Usr);
    panic!("{input}");
}

pub fn exception_unwind_str(
    e: Box<dyn Any + Send>,
    vars: &mut [InterpVal],
    stack: &mut InterpStack,
    memo: &mut InterpMemoize,
    cstore: &mut InterpClassStore,
    breakpoints: &mut DebugData,
    opts: &InputOpts,
    rl: &mut DefaultEditor,
) -> String {
    match exception_kind_get() {
        ExceptionKind::Lng => {
            if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.to_string()
            } else {
                String::from("error with non-string payload")
            }
        }
        ExceptionKind::Usr => {
            match panic::catch_unwind(AssertUnwindSafe(|| {
                serialize(
                    &exception_val_get(),
                    vars,
                    stack,
                    memo,
                    cstore,
                    breakpoints,
                    opts,
                    rl,
                    SerializeOpts::default(),
                )
            })) {
                Ok(s) => s,
                Err(e) => exception_unwind_str(e, vars, stack, memo, cstore, breakpoints, opts, rl),
            }
        }
        ExceptionKind::Backtrack => String::from("internal: uncaught backtrack"),
    }
}
