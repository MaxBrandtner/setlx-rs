use nalgebra::{DMatrix, DVector};
use num_bigint::BigInt;
use num_traits::cast::ToPrimitive;
use std::cell::RefCell;
use std::cmp::min;
use std::collections::{BTreeSet, btree_map};
use std::fs::File;
use std::panic;
use std::rc::Rc;

use crate::builtin::BuiltinProc;
use crate::cli::InputOpts;
use crate::interp::debug::DebugData;
use crate::interp::except::*;
use crate::interp::heap::*;
use crate::ir::def::*;

pub struct InterpImmedVal {
    heap: *mut InterpImmediateHeap,
    r: Option<InterpObjRef>,
    pub val: InterpVal,
}

impl InterpImmedVal {
    pub fn confirm(mut self) -> Self {
        self.r = None;
        self
    }

    pub fn from_val(val: InterpVal, heap: &mut InterpImmediateHeap) -> Self {
        InterpImmedVal {
            heap: heap as *mut InterpImmediateHeap,
            r: if let InterpVal::Ref(r) = val {
                Some(r)
            } else {
                None
            },
            val,
        }
    }
}

impl Drop for InterpImmedVal {
    fn drop(&mut self) {
        if let Some(r) = self.r {
            // SAFETY: heap outlives immediate values
            unsafe {
                (&mut *self.heap).free(r);
            }
        }
    }
}

pub trait InterpGet {
    fn to_bool(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        op: &str,
    ) -> bool;
    fn to_f64(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        op: &str,
    ) -> f64;
    fn to_i64(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        op: &str,
    ) -> i64;
    fn to_usize(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        op: &str,
    ) -> usize;
    fn to_num(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        op: &str,
    ) -> BigInt;
    fn to_ptr(&self, vars: &[InterpVal], op: &str) -> InterpPtr;
    fn to_ptr_val(&self, vars: &[InterpVal], op: &str) -> InterpVal;
    // NOTE: Doesn't handle builtin vars. This function should only be used in contexts where
    // accessing builtin variables would constitute undefined behavior
    fn to_ref(&self, vars: &[InterpVal]) -> Option<InterpObjRef>;
    fn to_ast(&self, vars: &[InterpVal], proc_params: &InterpVal) -> Option<InterpTaggedList>;
    fn to_type(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        opts: &InputOpts,
        heap: &mut InterpImmediateHeap,
    ) -> IRType;
    fn to_proc_ptr(&self, vars: &[InterpVal], fnc: &str) -> *const IRProcedure;
    fn to_proc(&self, vars: &[InterpVal], data: &DebugData, fnc: &str) -> Rc<RefCell<IRProcedure>>;
    fn to_file(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
    ) -> Rc<RefCell<File>>;
    fn to_str(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        opts: &InputOpts,
        heap: &mut InterpImmediateHeap,
    ) -> String;
    fn to_val(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        opts: &InputOpts,
        heap: &mut InterpImmediateHeap,
    ) -> InterpImmedVal;
    fn to_regex(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
    ) -> &'static mut InterpRegex;
    fn to_set(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
    ) -> &'static mut BTreeSet<InterpVal>;
    fn to_list(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
    ) -> &'static mut InterpList;
    fn to_iter(&self, vars: &[InterpVal], proc_params: &InterpVal, data: &DebugData) -> InterpIter;
    fn to_obj_iter(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
    ) -> btree_map::Iter<'static, String, Box<InterpVal>>;
    fn to_obj_iter_ref<'a>(
        &self,
        vars: &'a mut [InterpVal],
        data: &DebugData,
    ) -> &'a mut btree_map::Iter<'static, String, Box<InterpVal>>;
    fn to_iter_ref<'a>(&self, vars: &'a mut [InterpVal], data: &DebugData) -> &'a mut InterpIter;
    fn to_slice(
        &self,
        lhs: i64,
        rhs: i64,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        opts: &InputOpts,
        heap: &mut InterpImmediateHeap,
    ) -> InterpSlice;
    fn to_immed_str<'a>(&'a self, op: &str) -> &'a str;
    fn to_immed_bool(&self, op: &str) -> bool;
    fn to_builtin_proc(&self) -> BuiltinProc;
}

impl InterpGet for IRValue {
    fn to_bool(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        op: &str,
    ) -> bool {
        fn val_to_bool(v: &InterpVal, data: &DebugData, op: &str) -> bool {
            if let InterpVal::Bool(b) = v {
                *b
            } else {
                exception_throw(
                    "cast",
                    &format!("{op} is only implemented for boolean values"),
                    data,
                );
            }
        }

        match self {
            IRValue::Variable(v) => val_to_bool(&vars[*v], data, op),
            IRValue::BuiltinVar(v) => val_to_bool(&v.to_val(proc_params), data, op),
            IRValue::Bool(b) => *b,
            _ => exception_throw(
                "cast",
                &format!("{op} is only implemented for boolean values"),
                data,
            ),
        }
    }

    fn to_f64(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        op: &str,
    ) -> f64 {
        fn val_to_f64(v: &InterpVal, data: &DebugData, op: &str) -> f64 {
            match v {
                InterpVal::Double(d) => *d,
                InterpVal::Ref(r) => match unsafe { &*r.0 } {
                    InterpObj::Number(n) => n.to_f64().unwrap_or_else(|| {
                        exception_throw("cast", "{op} out of bounds number", data)
                    }),
                    _ => exception_throw("cast", &format!("{op} undefined for type"), data),
                },
                _ => exception_throw("cast", &format!("{op} undefined for type"), data),
            }
        }

        match self {
            IRValue::Variable(v) => val_to_f64(&vars[*v], data, op),
            IRValue::BuiltinVar(v) => val_to_f64(&v.to_val(proc_params), data, op),
            IRValue::Double(d) => *d,
            IRValue::Number(n) => n.to_f64().unwrap_or_else(|| {
                exception_throw("cast", &format!("{op} out of bounds number"), data)
            }),
            _ => exception_throw("cast", &format!("{op} undefined for type"), data),
        }
    }

    fn to_i64(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        op: &str,
    ) -> i64 {
        fn val_to_i64(v: &InterpVal, data: &DebugData, op: &str) -> i64 {
            match v {
                InterpVal::Double(d) => *d as i64,
                InterpVal::Ref(r) => match unsafe { &*r.0 } {
                    InterpObj::Number(n) => n.to_i64().unwrap_or_else(|| {
                        exception_throw("cast", "{op} out of bounds number", data)
                    }),
                    _ => exception_throw("cast", &format!("{op} undefined for type"), data),
                },
                _ => exception_throw("cast", &format!("{op} undefined for type"), data),
            }
        }

        match self {
            IRValue::Variable(v) => val_to_i64(&vars[*v], data, op),
            IRValue::BuiltinVar(v) => val_to_i64(&v.to_val(proc_params), data, op),
            IRValue::Double(d) => *d as i64,
            IRValue::Number(n) => n.to_i64().unwrap_or_else(|| {
                exception_throw("cast", &format!("{op} out of bounds number"), data)
            }),
            _ => exception_throw("cast", &format!("{op} undefined for type"), data),
        }
    }

    fn to_usize(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        op: &str,
    ) -> usize {
        fn val_to_usize(v: &InterpVal, data: &DebugData, op: &str) -> usize {
            match v {
                InterpVal::Double(d) => *d as usize,
                InterpVal::Ref(r) => match unsafe { &*r.0 } {
                    InterpObj::Number(n) => n.to_usize().unwrap_or_else(|| {
                        exception_throw("cast", &format!("{op} out of bounds number"), data)
                    }),
                    _ => exception_throw("cast", &format!("{op} undefined for type"), data),
                },
                _ => exception_throw("cast", &format!("{op} undefined for type"), data),
            }
        }

        match self {
            IRValue::Variable(v) => val_to_usize(&vars[*v], data, op),
            IRValue::BuiltinVar(v) => val_to_usize(&v.to_val(proc_params), data, op),
            IRValue::Double(d) => *d as usize,
            IRValue::Number(n) => n.to_usize().unwrap_or_else(|| {
                exception_throw("cast", &format!("{op} out of bounds number"), data)
            }),
            _ => exception_throw("cast", &format!("{op} undefined for type"), data),
        }
    }

    fn to_num(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        op: &str,
    ) -> BigInt {
        fn val_to_num(v: &InterpVal, data: &DebugData, op: &str) -> BigInt {
            match v {
                InterpVal::Ref(r) => match unsafe { &*r.0 } {
                    InterpObj::Number(n) => n.clone(),
                    _ => exception_throw("cast", &format!("{op} undefined for type"), data),
                },
                _ => exception_throw("cast", &format!("{op} undefined for type"), data),
            }
        }

        match self {
            IRValue::Variable(v) => val_to_num(&vars[*v], data, op),
            IRValue::BuiltinVar(v) => val_to_num(&v.to_val(proc_params), data, op),
            IRValue::Number(n) => n.clone(),
            _ => exception_throw("cast", &format!("{op} undefined for type"), data),
        }
    }

    fn to_ptr(&self, vars: &[InterpVal], op: &str) -> InterpPtr {
        if let IRValue::Variable(v) = self
            && let InterpVal::Ptr(p) = &vars[*v]
        {
            *p
        } else {
            panic!("internal: {op} is only defined for ptrs")
        }
    }

    fn to_ptr_val(&self, vars: &[InterpVal], op: &str) -> InterpVal {
        if let IRValue::Variable(v) = self {
            match &vars[*v] {
                // SAFETY: IR-PTR
                InterpVal::Ptr(p) => unsafe { &*p.ptr }.clone(),
                // SAFETY: IR-PTR
                InterpVal::OffsetStrPtr(s) => {
                    InterpVal::Char(unsafe { &*s.val }.chars().nth(s.offset).unwrap_or(' '))
                }
                _ => panic!("internal: {op} is only defined for ptrs"),
            }
        } else {
            panic!("internal: {op} is only defined for ptrs")
        }
    }

    fn to_ref(&self, vars: &[InterpVal]) -> Option<InterpObjRef> {
        match self {
            IRValue::Variable(v) => {
                if let InterpVal::Ref(r) = &vars[*v] {
                    Some(*r)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn to_ast(&self, vars: &[InterpVal], proc_params: &InterpVal) -> Option<InterpTaggedList> {
        fn val_to_ast(input: &InterpVal) -> Option<InterpTaggedList> {
            if let InterpVal::Ref(r) = input
                && let InterpObj::Ast(a) = unsafe { &*r.0 }
            {
                Some(a.clone())
            } else {
                None
            }
        }

        match self {
            IRValue::Variable(v) => val_to_ast(&vars[*v]),
            IRValue::BuiltinVar(v) => val_to_ast(&v.to_val(proc_params)),
            _ => None,
        }
    }

    fn to_type(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        opts: &InputOpts,
        heap: &mut InterpImmediateHeap,
    ) -> IRType {
        fn val_to_type(input: &InterpVal) -> IRType {
            match input {
                InterpVal::Bool(_) => IRType::BOOL,
                InterpVal::Double(_) => IRType::DOUBLE,
                InterpVal::Char(_) => IRType::STRING,
                InterpVal::ObjIter(_) => IRType::OBJ_ITER,
                InterpVal::Iter(_) => IRType::ITERATOR,
                InterpVal::Slice(s) => match s {
                    InterpSlice::StringSlice(_) => IRType::STRING,
                    InterpSlice::ListSlice(_) => IRType::LIST,
                },
                InterpVal::OffsetStrPtr(_) => IRType::PTR,
                InterpVal::Type(_) => IRType::TYPE,
                InterpVal::Ptr(_) => IRType::PTR,
                InterpVal::Ref(r) => match unsafe { &*r.0 } {
                    InterpObj::Ast(_) => IRType::AST,
                    InterpObj::List(_) => IRType::LIST,
                    InterpObj::Number(_) => IRType::NUMBER,
                    InterpObj::Set(_) => IRType::SET,
                    InterpObj::String(_) => IRType::STRING,
                    InterpObj::Term(_) => IRType::TERM,
                    InterpObj::TTerm(_) => IRType::TTERM,
                    InterpObj::Regex(_) => IRType::NATIVE_REGEX,
                    InterpObj::Vector(_) => IRType::VECTOR,
                    InterpObj::Matrix(_) => IRType::MATRIX,
                    InterpObj::Procedure(_) => IRType::PROCEDURE,
                    InterpObj::Uninitialized => IRType::UNDEFINED,
                    InterpObj::Class(_) => IRType::CLASS,
                    InterpObj::Object(_) => IRType::OBJECT,
                    InterpObj::StackImage(_) => {
                        panic!("internal: stack images should only be used internally")
                    }
                    InterpObj::File(_) => IRType::FILE,
                },
                InterpVal::Procedure(_) => IRType::PROCEDURE,
                InterpVal::Undefined => IRType::UNDEFINED,
            }
        }
        match self {
            IRValue::Undefined => IRType::UNDEFINED,
            IRValue::BuiltinProc(_) => panic!("internal: builtin procedures should only be called"),
            IRValue::BuiltinVar(v) => val_to_type(&v.to_immed_val(proc_params, opts, heap).val),
            IRValue::Type(_) => IRType::TYPE,
            IRValue::Variable(v) => val_to_type(&vars[*v]),
            IRValue::String(_) => IRType::STRING,
            IRValue::Number(_) => IRType::NUMBER,
            IRValue::Double(_) => IRType::DOUBLE,
            IRValue::Bool(_) => IRType::BOOL,
            IRValue::Vector(_) => IRType::VECTOR,
            IRValue::Matrix(_) => IRType::MATRIX,
            IRValue::Procedure(_) => IRType::PROCEDURE,
            IRValue::HeapRef(_) => IRType::HEAP_REF,
        }
    }

    fn to_file(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
    ) -> Rc<RefCell<File>> {
        fn var_file_get(v: &InterpVal, data: &DebugData) -> Rc<RefCell<File>> {
            if let InterpVal::Ref(r) = v
                && let InterpObj::File(f) = unsafe { &*r.0 }
            {
                f.clone()
            } else {
                exception_throw("cast", "value isn't a file", data)
            }
        }

        match self {
            IRValue::BuiltinVar(v) => var_file_get(&v.to_val(proc_params), data),
            IRValue::Variable(v) => var_file_get(&vars[*v], data),
            _ => exception_throw("cast", "value isn't a file", data),
        }
    }

    fn to_proc(&self, vars: &[InterpVal], data: &DebugData, fnc: &str) -> Rc<RefCell<IRProcedure>> {
        match self {
            IRValue::Procedure(p) => p.clone(),
            IRValue::Variable(v) => match &vars[*v] {
                InterpVal::Procedure(p) => p.clone(),
                InterpVal::Ref(r) => match unsafe { &*r.0 } {
                    InterpObj::Class(c) => c.constructor.clone(),
                    InterpObj::Procedure(p) => p.proc.clone(),
                    _ => exception_throw("cast", &format!("{fnc} undefined for type"), data),
                },
                _ => exception_throw("cast", &format!("{fnc} undefined for type"), data),
            },
            _ => exception_throw("cast", &format!("{fnc} undefined for type"), data),
        }
    }

    fn to_proc_ptr(&self, vars: &[InterpVal], fnc: &str) -> *const IRProcedure {
        match self {
            IRValue::Procedure(p) => &*p.borrow() as *const IRProcedure,
            IRValue::Variable(v) => match &vars[*v] {
                InterpVal::Procedure(p) => &*p.borrow() as *const IRProcedure,
                InterpVal::Ref(r) => match unsafe { &*r.0 } {
                    InterpObj::Procedure(p) => &*p.proc.borrow() as *const IRProcedure,
                    _ => panic!("internal: {fnc} undefined for type"),
                },
                _ => panic!("internal: {fnc} undefined for type"),
            },
            _ => panic!("internal: {fnc} undefined for type"),
        }
    }

    fn to_str(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        opts: &InputOpts,
        heap: &mut InterpImmediateHeap,
    ) -> String {
        fn val_to_str(val: &InterpVal, data: &DebugData) -> String {
            match val {
                InterpVal::Slice(InterpSlice::StringSlice(sl)) => {
                    sl.slice.clone().collect::<String>()
                }
                InterpVal::Char(c) => c.to_string(),
                InterpVal::Ref(r) => match unsafe { &*r.0 } {
                    InterpObj::String(s) => s.to_string(),
                    _ => exception_throw("cast", "type isn't a string", data),
                },
                _ => exception_throw("cast", "type isn't a string", data),
            }
        }

        match self {
            IRValue::String(s) => s.to_string(),
            IRValue::Variable(v) => val_to_str(&vars[*v], data),
            IRValue::BuiltinVar(v) => {
                val_to_str(&v.to_immed_val(proc_params, opts, heap).val, data)
            }
            _ => exception_throw("cast", "type isn't a string", data),
        }
    }

    fn to_val(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        opts: &InputOpts,
        heap: &mut InterpImmediateHeap,
    ) -> InterpImmedVal {
        match self {
            IRValue::Undefined => InterpImmedVal::from_val(InterpVal::Undefined, heap),
            IRValue::BuiltinProc(_) => panic!("internal: builtin procedures are not assignable"),
            IRValue::BuiltinVar(v) => v.to_immed_val(proc_params, opts, heap),
            IRValue::Type(t) => InterpImmedVal::from_val(InterpVal::Type(*t), heap),
            IRValue::Variable(v) => InterpImmedVal::from_val(vars[*v].clone(), heap).confirm(),
            IRValue::String(s) => InterpImmedVal::from_val(
                InterpVal::Ref(heap.push_obj(InterpObj::String(s.to_string()))),
                heap,
            ),
            IRValue::Number(n) => InterpImmedVal::from_val(
                InterpVal::Ref(heap.push_obj(InterpObj::Number(n.clone()))),
                heap,
            ),
            IRValue::Double(f) => InterpImmedVal::from_val(InterpVal::Double(*f), heap),
            IRValue::Bool(b) => InterpImmedVal::from_val(InterpVal::Bool(*b), heap),
            IRValue::Vector(v) => {
                let mut o = DVector::<f64>::zeros(v.len());
                v.iter().enumerate().for_each(|(idx, i)| {
                    o[idx] = i.to_f64(vars, proc_params, data, "invalid vector element");
                });
                InterpImmedVal::from_val(InterpVal::Ref(heap.push_obj(InterpObj::Vector(o))), heap)
            }
            IRValue::Matrix(m) => {
                let m_len = if !m.is_empty() { m[0].len() } else { 0 };

                let mut o = DMatrix::<f64>::zeros(m.len(), m_len);
                m.iter().enumerate().for_each(|(row, i)| {
                    i.iter().enumerate().for_each(|(col, j)| {
                        o[(row, col)] = j.to_f64(vars, proc_params, data, "invalid matrix element");
                    });
                });
                InterpImmedVal::from_val(InterpVal::Ref(heap.push_obj(InterpObj::Matrix(o))), heap)
            }
            IRValue::Procedure(p) => {
                InterpImmedVal::from_val(InterpVal::Procedure(p.clone()), heap)
            }
            IRValue::HeapRef(r) => InterpImmedVal::from_val(InterpVal::Ref(*r), heap).confirm(),
        }
    }

    fn to_regex(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
    ) -> &'static mut InterpRegex {
        fn val_to_regex(input: &InterpVal, data: &DebugData) -> &'static mut InterpRegex {
            if let InterpVal::Ref(r) = input
                && let InterpObj::Regex(r) = unsafe { &mut *r.0 }
            {
                r
            } else {
                exception_throw("cast", "undefined for type", data);
            }
        }

        match self {
            IRValue::Variable(v) => val_to_regex(&vars[*v], data),
            IRValue::BuiltinVar(v) => val_to_regex(&v.to_val(proc_params), data),
            _ => exception_throw("cast", "undefined for type", data),
        }
    }

    fn to_set(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
    ) -> &'static mut BTreeSet<InterpVal> {
        fn val_to_set(input: &InterpVal, data: &DebugData) -> &'static mut BTreeSet<InterpVal> {
            if let InterpVal::Ref(r) = input
                && let InterpObj::Set(s) = unsafe { &mut *r.0 }
            {
                &mut s.0
            } else {
                exception_throw("cast", "undefined for type", data);
            }
        }

        match self {
            IRValue::Variable(v) => val_to_set(&vars[*v], data),
            IRValue::BuiltinVar(v) => val_to_set(&v.to_val(proc_params), data),
            _ => exception_throw("cast", "undefined for type", data),
        }
    }

    fn to_list(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
    ) -> &'static mut InterpList {
        fn val_to_list(input: &InterpVal, data: &DebugData) -> &'static mut InterpList {
            if let InterpVal::Ref(r) = input
                && let InterpObj::List(l) = unsafe { &mut *r.0 }
            {
                l
            } else {
                exception_throw("cast", "undefined for type", data);
            }
        }

        match self {
            IRValue::Variable(v) => val_to_list(&vars[*v], data),
            IRValue::BuiltinVar(v) => val_to_list(&v.to_val(proc_params), data),
            _ => exception_throw("cast", "undefined for type", data),
        }
    }

    fn to_iter(&self, vars: &[InterpVal], proc_params: &InterpVal, data: &DebugData) -> InterpIter {
        match self {
            IRValue::BuiltinVar(v) => InterpIter::from_val(&v.to_val(proc_params)),
            IRValue::Variable(v) => InterpIter::from_val(&vars[*v]),
            _ => None,
        }
        .unwrap_or_else(|| exception_throw("cast", "iterator undefined for type", data))
    }

    fn to_obj_iter(
        &self,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
    ) -> btree_map::Iter<'static, String, Box<InterpVal>> {
        fn to_iter_val(
            input: &InterpVal,
            data: &DebugData,
        ) -> btree_map::Iter<'static, String, Box<InterpVal>> {
            match input {
                InterpVal::ObjIter(i) => unsafe {
                    std::mem::transmute::<
                        btree_map::Iter<'_, String, Box<InterpVal>>,
                        btree_map::Iter<'static, String, Box<InterpVal>>,
                    >(i.clone())
                },
                InterpVal::Ref(r) => {
                    if let InterpObj::Object(c) = unsafe { &*r.0 } {
                        unsafe {
                            std::mem::transmute::<
                                btree_map::Iter<'_, String, Box<InterpVal>>,
                                btree_map::Iter<'static, String, Box<InterpVal>>,
                            >(c.0.iter())
                        }
                    } else {
                        exception_throw("cast", "iterator undefined for type", data);
                    }
                }
                _ => exception_throw("cast", "iterator undefined for type", data),
            }
        }

        match self {
            IRValue::BuiltinVar(v) => to_iter_val(&v.to_val(proc_params), data),
            IRValue::Variable(v) => to_iter_val(&vars[*v], data),
            _ => exception_throw("cast", "iter undefined for type", data),
        }
    }

    fn to_obj_iter_ref<'a>(
        &self,
        vars: &'a mut [InterpVal],
        data: &DebugData,
    ) -> &'a mut btree_map::Iter<'static, String, Box<InterpVal>> {
        fn to_iter_val<'a>(
            input: &'a mut InterpVal,
            data: &DebugData,
        ) -> &'a mut btree_map::Iter<'static, String, Box<InterpVal>> {
            if let InterpVal::ObjIter(i) = input {
                i
            } else {
                exception_throw("cast", "iter ref undefined for type", data);
            }
        }

        match self {
            IRValue::Variable(v) => to_iter_val(&mut vars[*v], data),
            _ => exception_throw("cast", "iter ref undefined for type", data),
        }
    }

    fn to_iter_ref<'a>(&self, vars: &'a mut [InterpVal], data: &DebugData) -> &'a mut InterpIter {
        fn to_iter_val<'a>(input: &'a mut InterpVal, data: &DebugData) -> &'a mut InterpIter {
            if let InterpVal::Iter(i) = input {
                i
            } else {
                exception_throw("cast", "iter ref undefined for type", data);
            }
        }

        match self {
            IRValue::Variable(v) => to_iter_val(&mut vars[*v], data),
            _ => exception_throw("cast", "iter ref undefined for type", data),
        }
    }
    fn to_slice(
        &self,
        lhs: i64,
        rhs: i64,
        vars: &[InterpVal],
        proc_params: &InterpVal,
        data: &DebugData,
        opts: &InputOpts,
        heap: &mut InterpImmediateHeap,
    ) -> InterpSlice {
        fn to_slice_val(
            input: &InterpVal,
            mut lhs: i64,
            mut rhs: i64,
            data: &DebugData,
        ) -> InterpSlice {
            if lhs < 0 {
                lhs = 0;
            }
            if rhs < 0 {
                rhs = 0;
            }

            match input {
                InterpVal::Ref(r) => match unsafe { &*r.0 } {
                    InterpObj::String(s) => {
                        rhs -= lhs;

                        unsafe {
                            InterpSlice::StringSlice(InterpStringSlice {
                                original: std::mem::transmute::<&'_ str, &'static str>(s.as_str()),
                                skip: lhs as usize,
                                take: rhs as usize,
                                slice: std::mem::transmute::<
                                    std::iter::Take<std::iter::Skip<std::str::Chars<'_>>>,
                                    std::iter::Take<std::iter::Skip<std::str::Chars<'static>>>,
                                >(
                                    s.chars().skip(lhs as usize).take((rhs) as usize)
                                ),
                            })
                        }
                    }
                    InterpObj::List(l) => {
                        rhs -= 1;

                        if rhs >= l.0.len() as i64 {
                            rhs = l.0.len() as i64 - 1;
                        }

                        if lhs > rhs || lhs >= l.0.len() as i64 {
                            return unsafe {
                                InterpSlice::ListSlice(std::mem::transmute::<
                                    &'_ [InterpVal],
                                    &'static [InterpVal],
                                >(
                                    &l.0[l.0.len()..l.0.len()]
                                ))
                            };
                        }

                        unsafe {
                            InterpSlice::ListSlice(std::mem::transmute::<
                                &'_ [InterpVal],
                                &'static [InterpVal],
                            >(
                                &l.0[(lhs as usize)..=(rhs as usize)]
                            ))
                        }
                    }
                    _ => exception_throw("cast", "slice undefined for type", data),
                },
                InterpVal::Slice(s) => match s {
                    InterpSlice::StringSlice(s) => {
                        rhs -= lhs;

                        let skip = s.skip + lhs as usize;
                        let take = min(s.take - lhs as usize, rhs as usize);

                        InterpSlice::StringSlice(InterpStringSlice {
                            original: s.original,
                            skip,
                            take,
                            slice: s.original.chars().skip(skip).take(take),
                        })
                    }
                    InterpSlice::ListSlice(l) => {
                        rhs -= 1;

                        if rhs >= l.len() as i64 {
                            rhs = l.len() as i64 - 1;
                        }

                        if lhs > rhs || lhs >= l.len() as i64 {
                            return unsafe {
                                InterpSlice::ListSlice(std::mem::transmute::<
                                    &'_ [InterpVal],
                                    &'static [InterpVal],
                                >(
                                    &l[l.len()..l.len()]
                                ))
                            };
                        }

                        unsafe {
                            InterpSlice::ListSlice(std::mem::transmute::<
                                &'_ [InterpVal],
                                &'static [InterpVal],
                            >(
                                &l[(lhs as usize)..=(rhs as usize)]
                            ))
                        }
                    }
                },
                _ => exception_throw("cast", "slice undefined for type", data),
            }
        }

        to_slice_val(
            &self
                .to_val(vars, proc_params, data, opts, heap)
                .confirm()
                .val,
            lhs,
            rhs,
            data,
        )
    }

    fn to_immed_str<'a>(&'a self, op: &str) -> &'a str {
        if let IRValue::String(s) = self {
            s
        } else {
            panic!("internal: {op} is only defined for immediate strings")
        }
    }

    fn to_immed_bool(&self, op: &str) -> bool {
        if let IRValue::Bool(b) = self {
            *b
        } else {
            panic!("internal: {op} is only defined for immediate booleans")
        }
    }

    fn to_builtin_proc(&self) -> BuiltinProc {
        if let IRValue::BuiltinProc(p) = self {
            *p
        } else {
            panic!("internal: native call is only implemented for builtin procedures");
        }
    }
}
