use nalgebra::{DMatrix, DVector};
use num_bigint::BigInt;
use pcre2::bytes::Regex;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, btree_map, btree_set};
use std::fs::File;
use std::iter::{Skip, Take};
use std::rc::Rc;
use std::slice;
use std::str::Chars;

use crate::builtin::BuiltinProc;
use crate::interp::memoize::InterpStackImage;
use crate::interp::ops::val_cmp;
use crate::ir::def::*;
use crate::ir::lower::util::{block_get, tmp_var_new};

pub struct InterpClassStoreEntry {
    pub get_proc: Rc<RefCell<IRProcedure>>,
    pub val: InterpVal,
}

#[derive(Default)]
pub struct InterpClassStore(pub BTreeMap<String, InterpClassStoreEntry>);

impl InterpClassStore {
    pub fn insert(
        &mut self,
        name: String,
        c_proc: Rc<RefCell<IRProcedure>>,
        s_map: BTreeMap<String, Box<InterpVal>>,
    ) {
        let val = InterpVal::Ref(InterpObjRef::from_obj(InterpObj::Class(InterpClass {
            constructor: c_proc,
            static_vars: s_map,
        })));

        fn get_proc_new(name: &str) -> Rc<RefCell<IRProcedure>> {
            /* c_addr := stack_get_or_new(name);
             * c_ref := *c_addr;
             * c := copy(c_ref);
             * return c;
             */

            let proc = Rc::new(RefCell::new(IRProcedure::from_tag("getClass")));
            let block_idx = proc.borrow_mut().blocks.add_node(Vec::new());
            let t_c_addr = tmp_var_new(&mut proc.borrow_mut());
            let t_c_ref = tmp_var_new(&mut proc.borrow_mut());
            let t_c = tmp_var_new(&mut proc.borrow_mut());
            block_get(&mut proc.borrow_mut(), block_idx).extend(vec![
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_c_addr),
                    types: IRType::PTR,
                    source: IRValue::BuiltinProc(BuiltinProc::StackGetOrNew),
                    op: IROp::NativeCall(vec![IRValue::String(name.to_string())]),
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_c_ref),
                    types: IRType::CLASS,
                    source: IRValue::Variable(t_c_addr),
                    op: IROp::PtrDeref,
                }),
                IRStmt::Assign(IRAssign {
                    target: IRTarget::Variable(t_c),
                    types: IRType::CLASS,
                    source: IRValue::BuiltinProc(BuiltinProc::Copy),
                    op: IROp::NativeCall(vec![IRValue::Variable(t_c_ref)]),
                }),
                IRStmt::Return(IRValue::Variable(t_c)),
            ]);

            proc.borrow_mut().start_block = block_idx;
            proc.borrow_mut().end_block = block_idx;

            proc
        }

        let get_proc = get_proc_new(&name);

        self.0.insert(name, InterpClassStoreEntry { get_proc, val });
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct InterpObjRef(pub *mut InterpObj);

impl InterpObjRef {
    pub fn from_obj(o: InterpObj) -> Self {
        InterpObjRef(Box::into_raw(Box::new(o)))
    }

    /// # SAFETY
    ///
    /// IR-Op
    pub unsafe fn unshare(&self) -> Self {
        InterpObjRef::from_obj(unsafe { &*self.0 }.clone())
    }

    /// # SAFETY
    ///
    /// IR-Op
    pub unsafe fn invalidate(self) {
        unsafe { drop(Box::from_raw(self.0)) }
    }
}

#[derive(Default)]
pub struct InterpImmediateHeap {
    pub refs: BTreeSet<InterpObjRef>,
}

impl InterpImmediateHeap {
    pub fn new() -> Self {
        InterpImmediateHeap::default()
    }

    pub fn push(&mut self, obj: InterpObjRef) -> InterpObjRef {
        self.refs.insert(obj);
        obj
    }

    pub fn push_obj(&mut self, obj: InterpObj) -> InterpObjRef {
        let r = InterpObjRef::from_obj(obj);
        self.refs.insert(r);
        r
    }

    /// # SAFETY
    ///
    /// IR-Op
    pub unsafe fn free(&mut self, r: InterpObjRef) {
        self.refs.remove(&r);
        unsafe {
            r.invalidate();
        }
    }
}

impl Drop for InterpImmediateHeap {
    fn drop(&mut self) {
        self.refs.iter().for_each(|i| unsafe { i.invalidate() });
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Debug)]
pub enum InterpIter {
    StringIter(Take<Skip<Chars<'static>>>),
    SetIter(btree_set::Iter<'static, InterpVal>),
    ListIter(Take<slice::Iter<'static, InterpVal>>),
}

impl InterpIter {
    pub fn from_val(input: &InterpVal) -> Option<Self> {
        match input {
            InterpVal::Iter(i) => Some(i.clone()),
            InterpVal::Slice(s) => match s {
                InterpSlice::StringSlice(s) => Some(InterpIter::StringIter(s.slice.clone())),
                InterpSlice::ListSlice(l) => Some(InterpIter::ListIter(l.iter().take(l.len()))),
            },
            InterpVal::Ref(r) => match unsafe { &*r.0 } {
                InterpObj::String(s) =>
                {
                    #[allow(clippy::iter_skip_zero)]
                    Some(InterpIter::StringIter(
                        unsafe { std::mem::transmute::<Chars<'_>, Chars<'static>>(s.chars()) }
                            .skip(0)
                            .take(s.chars().count()),
                    ))
                }
                InterpObj::List(l) => Some(InterpIter::ListIter(unsafe {
                    std::mem::transmute::<
                        std::iter::Take<std::slice::Iter<'_, InterpVal>>,
                        std::iter::Take<std::slice::Iter<'static, InterpVal>>,
                    >(l.0.iter().take(l.0.len()))
                })),
                InterpObj::Set(s) => Some(InterpIter::SetIter(unsafe {
                    std::mem::transmute::<
                        std::collections::btree_set::Iter<'_, InterpVal>,
                        std::collections::btree_set::Iter<'static, InterpVal>,
                    >(s.0.iter())
                })),
                _ => None,
            },
            _ => None,
        }
    }
}

impl Iterator for InterpIter {
    type Item = InterpVal;

    fn next(&mut self) -> Option<InterpVal> {
        match self {
            InterpIter::StringIter(c) => c.next().map(InterpVal::Char),
            InterpIter::SetIter(s) => s.next().cloned(),
            InterpIter::ListIter(l) => l.next().cloned(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct InterpStringSlice {
    pub original: &'static str,
    pub skip: usize,
    pub take: usize,
    pub slice: Take<Skip<Chars<'static>>>,
}

#[derive(Clone, Debug)]
pub enum InterpSlice {
    StringSlice(InterpStringSlice),
    ListSlice(&'static [InterpVal]),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum InterpPtrSgmt {
    Immediate,
    Stack,
    Heap,
    Class,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct InterpPtr {
    pub sgmt: InterpPtrSgmt,
    pub ptr: *mut InterpVal,
}

#[derive(Clone, Debug)]
pub struct InterpOffsetStrPtr {
    pub offset: usize,
    pub val: *mut String,
}

#[derive(Clone, Debug)]
pub enum InterpVal {
    Bool(bool),
    Double(f64),
    Char(char),
    ObjIter(btree_map::Iter<'static, String, Box<InterpVal>>),
    Iter(InterpIter),
    Slice(InterpSlice),
    OffsetStrPtr(InterpOffsetStrPtr),
    Type(IRType),
    Ptr(InterpPtr),
    Ref(InterpObjRef),
    Procedure(Rc<RefCell<IRProcedure>>),
    Undefined,
}

impl InterpVal {
    pub fn tag(&self) -> u8 {
        match self {
            InterpVal::Bool(_) => 0,
            InterpVal::Double(_) => 1,
            InterpVal::Char(_) => 2,
            InterpVal::ObjIter(_) => 3,
            InterpVal::Iter(_) => 4,
            InterpVal::Slice(_) => 5,
            InterpVal::Type(_) => 6,
            InterpVal::Ptr(_) => 7,
            InterpVal::Ref(_) => 8,
            InterpVal::Procedure(_) => 9,
            InterpVal::Undefined => 10,
            InterpVal::OffsetStrPtr(_) => 11,
        }
    }

    pub fn unshare_immed(&self, heap: &mut InterpImmediateHeap) -> InterpVal {
        let out = self.unshare();
        if let InterpVal::Ref(r) = out {
            heap.push(r);
        }

        out
    }

    pub fn unshare(&self) -> InterpVal {
        match self {
            InterpVal::Ref(r) => InterpVal::Ref(unsafe { r.unshare() }),
            InterpVal::Slice(slice) => match slice {
                InterpSlice::StringSlice(s) => InterpVal::Ref(InterpObjRef::from_obj(
                    InterpObj::String(s.slice.clone().collect()),
                )),
                InterpSlice::ListSlice(l) => InterpVal::Ref(InterpObjRef::from_obj(
                    InterpObj::List(InterpList(l.iter().map(|i| i.unshare()).collect())),
                )),
            },
            _ => self.clone(),
        }
    }

    pub fn crosses_frames(&self) -> bool {
        match self {
            InterpVal::Procedure(_) => true,
            InterpVal::Ref(r) => {
                if let InterpObj::Procedure(p) = unsafe { &*r.0 }
                    && p.cross_frame
                {
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn proc_get(&self) -> Option<Rc<RefCell<IRProcedure>>> {
        match self {
            InterpVal::Procedure(p) => Some(p.clone()),
            InterpVal::Ref(r) => match unsafe { &*r.0 } {
                InterpObj::Procedure(p) => Some(p.proc.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn mark_immed(self, heap: &mut InterpImmediateHeap) -> Self {
        if let InterpVal::Ref(r) = self {
            heap.refs.insert(r);
        }

        self
    }

    pub fn persist(self, heap: &mut InterpImmediateHeap) -> Self {
        if let InterpVal::Ref(r) = self {
            heap.refs.remove(&r);
        }

        self
    }
}

impl Ord for InterpVal {
    fn cmp(&self, b: &InterpVal) -> Ordering {
        val_cmp(self, b)
    }
}

impl PartialOrd for InterpVal {
    fn partial_cmp(&self, b: &InterpVal) -> Option<Ordering> {
        Some(self.cmp(b))
    }
}

impl PartialEq for InterpVal {
    fn eq(&self, b: &Self) -> bool {
        self.cmp(b) == Ordering::Equal
    }
}

impl Eq for InterpVal {}

#[derive(Debug)]
pub struct InterpTaggedList {
    pub tag: String,
    pub list: Vec<InterpVal>,
}

impl Clone for InterpTaggedList {
    fn clone(&self) -> Self {
        InterpTaggedList {
            tag: self.tag.clone(),
            list: self.list.iter().map(|i| i.unshare()).collect(),
        }
    }
}

impl Drop for InterpTaggedList {
    fn drop(&mut self) {
        self.list.iter().for_each(|i| {
            if let InterpVal::Ref(r) = i {
                unsafe {
                    r.invalidate();
                }
            }
        })
    }
}

#[derive(Clone, Debug)]
pub struct InterpProc {
    pub info: Option<InterpTaggedList>,
    pub proc: Rc<RefCell<IRProcedure>>,
    pub stack: Option<InterpObjRef>,
    pub cross_frame: bool,
}

#[derive(Debug, Default)]
pub struct InterpList(pub Vec<InterpVal>);

impl Clone for InterpList {
    fn clone(&self) -> Self {
        InterpList(self.0.iter().map(|i| i.unshare()).collect())
    }
}

impl Drop for InterpList {
    fn drop(&mut self) {
        self.0.iter().for_each(|i| {
            if let InterpVal::Ref(r) = i {
                unsafe {
                    r.invalidate();
                }
            }
        });
    }
}

impl InterpList {
    pub fn push(&mut self, rhs: InterpVal) {
        self.0.push(rhs);
    }

    pub fn pop(&mut self, heap: &mut InterpImmediateHeap) -> Option<InterpVal> {
        self.0.pop().map(|o| {
            if let InterpVal::Ref(r) = o {
                heap.refs.insert(r);
            }
            o.clone()
        })
    }

    pub fn extend(&mut self, rhs: &Self) {
        for i in rhs.0.iter() {
            self.push(i.unshare());
        }
    }

    pub fn contains(&self, val: &InterpVal) -> bool {
        self.0.iter().any(|i| i == val)
    }
}
#[derive(Debug)]
pub struct InterpClass {
    pub constructor: Rc<RefCell<IRProcedure>>,
    pub static_vars: BTreeMap<String, Box<InterpVal>>,
}

impl Clone for InterpClass {
    fn clone(&self) -> Self {
        InterpClass {
            constructor: self.constructor.clone(),
            static_vars: self
                .static_vars
                .iter()
                .map(|(key, val)| (key.to_string(), Box::new(val.unshare())))
                .collect(),
        }
    }
}

impl Drop for InterpClass {
    fn drop(&mut self) {
        self.static_vars.iter().for_each(|(_, val)| {
            if let InterpVal::Ref(r) = &**val {
                unsafe {
                    r.invalidate();
                }
            }
        });
    }
}

impl InterpClass {
    pub fn add(&mut self, var: String) -> InterpVal {
        let val = Box::new(InterpVal::Undefined);
        let val_ptr = &*val as *const InterpVal as *mut InterpVal;

        self.static_vars.insert(var, val);

        InterpVal::Ptr(InterpPtr {
            sgmt: InterpPtrSgmt::Heap,
            ptr: val_ptr,
        })
    }

    pub fn get(&mut self, var: &str) -> Option<InterpVal> {
        self.static_vars.get(var).map(|i| {
            InterpVal::Ptr(InterpPtr {
                sgmt: InterpPtrSgmt::Heap,
                ptr: &**i as *const InterpVal as *mut InterpVal,
            })
        })
    }
}

#[derive(Debug)]
pub struct InterpClassObj(pub BTreeMap<String, Box<InterpVal>>);

impl Clone for InterpClassObj {
    fn clone(&self) -> Self {
        InterpClassObj(
            self.0
                .iter()
                .map(|(key, val)| (key.to_string(), Box::new(val.unshare())))
                .collect(),
        )
    }
}

impl Drop for InterpClassObj {
    fn drop(&mut self) {
        self.0.iter().for_each(|(_, i)| {
            if let InterpVal::Ref(r) = &**i {
                unsafe {
                    r.invalidate();
                }
            }
        });
    }
}

impl InterpClassObj {
    pub fn add(&mut self, var: String) -> InterpVal {
        let val = Box::new(InterpVal::Undefined);
        let val_ptr = &*val as *const InterpVal as *mut InterpVal;

        self.0.insert(var, val);

        InterpVal::Ptr(InterpPtr {
            sgmt: InterpPtrSgmt::Heap,
            ptr: val_ptr,
        })
    }

    pub fn get(&self, var: &str) -> Option<InterpVal> {
        self.0.get(var).map(|i| {
            InterpVal::Ptr(InterpPtr {
                sgmt: InterpPtrSgmt::Heap,
                ptr: &**i as *const InterpVal as *mut InterpVal,
            })
        })
    }
}

#[derive(Clone, Debug)]
pub struct InterpRegex {
    pub is_anchored: bool,
    pub regex: Regex,
}

#[derive(Debug)]
pub struct InterpSet(pub BTreeSet<InterpVal>);

impl Clone for InterpSet {
    fn clone(&self) -> Self {
        InterpSet(self.0.iter().map(|i| i.unshare()).collect())
    }
}

impl Drop for InterpSet {
    fn drop(&mut self) {
        self.0.iter().for_each(|i| {
            if let InterpVal::Ref(r) = i {
                unsafe {
                    r.invalidate();
                }
            }
        });
    }
}

#[derive(Clone, Debug, Default)]
pub enum InterpObj {
    Ast(InterpTaggedList),
    List(InterpList),
    Number(BigInt),
    Set(InterpSet),
    String(String),
    Term(InterpTaggedList),
    TTerm(InterpTaggedList),
    Regex(InterpRegex),
    Class(InterpClass),
    Object(InterpClassObj),
    StackImage(InterpStackImage),
    Vector(DVector<f64>),
    Matrix(DMatrix<f64>),
    Procedure(InterpProc),
    File(Rc<RefCell<File>>),
    #[default]
    Uninitialized,
}

impl InterpObj {
    pub fn tag(&self) -> usize {
        match self {
            InterpObj::Ast(_) => 0,
            InterpObj::List(_) => 1,
            InterpObj::Number(_) => 2,
            InterpObj::Set(_) => 3,
            InterpObj::String(_) => 4,
            InterpObj::Term(_) => 5,
            InterpObj::TTerm(_) => 6,
            InterpObj::Regex(_) => 7,
            InterpObj::Class(_) => 8,
            InterpObj::Object(_) => 9,
            InterpObj::StackImage(_) => 10,
            InterpObj::Vector(_) => 11,
            InterpObj::Matrix(_) => 12,
            InterpObj::Procedure(_) => 13,
            InterpObj::File(_) => 14,
            InterpObj::Uninitialized => 15,
        }
    }
}
