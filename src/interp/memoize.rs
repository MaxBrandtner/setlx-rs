use std::collections::BTreeMap;

use crate::interp::heap::{InterpList, InterpObj, InterpObjRef, InterpVal};
use crate::ir::def::IRProcedure;

#[derive(Debug)]
pub struct InterpStackImage(pub BTreeMap<String, InterpVal>);

impl Clone for InterpStackImage {
    fn clone(&self) -> Self {
        InterpStackImage(
            self.0
                .iter()
                .map(|(name, i)| (name.clone(), i.unshare()))
                .collect(),
        )
    }
}

#[derive(Default)]
pub struct InterpMemoizeProcSet(BTreeMap<InterpVal, InterpVal>);

impl InterpMemoizeProcSet {
    /// # Arguments
    /// * `params` - corresponds to BuiltinProc::Params (List(Ptr(InterpVal)))
    ///              internally, InterpMemoizeProcSet is stored as a List(InterpVal)
    pub fn contains(&self, params: &InterpVal) -> Option<InterpVal> {
        if let InterpVal::Ref(r) = params
            && let InterpObj::List(list) = unsafe { &*r.0 }
        {
            let cmp_list = InterpVal::Ref(InterpObjRef::from_obj(InterpObj::List(InterpList(
                list.0
                    .iter()
                    .map(|i| {
                        if let InterpVal::Ptr(p) = i {
                            unsafe { &*p.ptr }.clone()
                        } else {
                            unreachable!()
                        }
                    })
                    .collect(),
            ))));

            let out = self.0.get(&cmp_list).cloned();

            if let InterpVal::Ref(r) = cmp_list
                && let InterpObj::List(l) = unsafe { &mut *r.0 }
            {
                l.0.truncate(0);
            }

            out
        } else {
            unreachable!();
        }
    }

    /// # Arguments
    /// * `params` - List(InterpVal) (unlike BuiltinProc::Params *not* List(Ptr(InterpVal)))
    pub fn insert(&mut self, params: InterpVal, ret: InterpVal) {
        self.0.insert(params, ret);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/* SAFETY:
 *
 * cache_lookup and cache_add are only called from within a given procedure. Accordingly, the entry
 * only remains accessible while the procedure is valid.
 */
pub type InterpMemoize = BTreeMap<*const IRProcedure, InterpMemoizeProcSet>;
