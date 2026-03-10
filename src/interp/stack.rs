use std::collections::BTreeMap;

use crate::builtin::stubs::stubs_init;
use crate::interp::heap::*;
use crate::interp::memoize::InterpStackImage;

#[derive(Debug, Clone)]
pub struct InterpStackVar {
    pub var: String,
    pub val: Box<InterpVal>,
}

#[derive(Debug, Clone)]
pub struct InterpStackAlias {
    pub var: String,
    pub ptr: *mut InterpVal,
    pub cross_frame: bool,
}

#[derive(Debug, Clone)]
pub enum InterpStackEntry {
    StackFrameBoundary,
    Alias(InterpStackAlias),
    Variable(InterpStackVar),
}

#[derive(Debug)]
pub struct InterpStack {
    pub frames: Vec<InterpStackEntry>,
}

impl Default for InterpStack {
    fn default() -> Self {
        Self::new()
    }
}

impl InterpStack {
    pub fn new() -> Self {
        InterpStack {
            frames: stubs_init(),
        }
    }

    pub fn frame_push(&mut self) {
        self.frames.push(InterpStackEntry::StackFrameBoundary);
    }
    pub fn frame_pop(&mut self) {
        let pos = self
            .frames
            .iter()
            .rposition(|e| matches!(*e, InterpStackEntry::StackFrameBoundary))
            .unwrap_or(0);

        self.frames.iter().skip(pos).for_each(|i| {
            if let InterpStackEntry::Variable(v) = i
                && let InterpVal::Ref(r) = *v.val
            {
                unsafe {
                    r.invalidate();
                }
            }
        });

        self.frames.truncate(pos);
    }

    pub fn frame_save(&mut self) -> InterpStackImage {
        let mut out: BTreeMap<String, InterpVal> = BTreeMap::new();

        for (idx, entry) in self.frames.iter().enumerate().rev() {
            match entry {
                InterpStackEntry::StackFrameBoundary => {
                    self.frames.truncate(idx);
                    break;
                }
                InterpStackEntry::Variable(v) => {
                    out.entry(v.var.to_string()).or_insert((*v.val).unshare());
                }
                InterpStackEntry::Alias(v) => {
                    out.entry(v.var.to_string())
                        // SAFETY: IR-PTR
                        .or_insert(unsafe { (*v.ptr).unshare() });
                }
            }
        }

        InterpStackImage(out)
    }

    pub fn frame_copy(&self) -> InterpStackImage {
        let mut out: BTreeMap<String, InterpVal> = BTreeMap::new();

        for (_, entry) in self.frames.iter().enumerate().rev() {
            match entry {
                InterpStackEntry::StackFrameBoundary => {
                    break;
                }
                InterpStackEntry::Variable(v) => {
                    out.entry(v.var.to_string()).or_insert((*v.val).unshare());
                }
                InterpStackEntry::Alias(v) => unsafe {
                    out.entry(v.var.to_string()).or_insert((*v.ptr).unshare());
                },
            }
        }

        for entry in self.frames.iter().rev() {
            match entry {
                InterpStackEntry::StackFrameBoundary => (),
                InterpStackEntry::Variable(v) => {
                    if !v.val.crosses_frames() {
                        continue;
                    }
                    out.entry(v.var.to_string()).or_insert((*v.val).unshare());
                }
                InterpStackEntry::Alias(v) => {
                    if !v.cross_frame {
                        continue;
                    }
                    out.entry(v.var.to_string())
                        // SAFETY: IR-PTR
                        .or_insert(unsafe { (*v.ptr).unshare() });
                }
            }
        }

        InterpStackImage(out)
    }

    pub fn copy_reachable(&self) -> InterpStackImage {
        let mut out: BTreeMap<String, InterpVal> = BTreeMap::new();
        let mut crossed_boundary = false;

        for (_, entry) in self.frames.iter().enumerate().rev() {
            match entry {
                InterpStackEntry::StackFrameBoundary => {
                    crossed_boundary = true;
                }
                InterpStackEntry::Variable(v) => {
                    if crossed_boundary && !v.val.crosses_frames() {
                        continue;
                    }

                    out.entry(v.var.to_string()).or_insert((*v.val).unshare());
                }
                InterpStackEntry::Alias(v) => unsafe {
                    if crossed_boundary && !(v.cross_frame || (*v.ptr).crosses_frames()) {
                        continue;
                    }

                    out.entry(v.var.to_string()).or_insert((*v.ptr).unshare());
                },
            }
        }

        InterpStackImage(out)
    }

    pub fn frame_restore(&mut self, image: &InterpStackImage) {
        image.0.iter().for_each(|(name, val)| {
            self.frames.push(InterpStackEntry::Variable(InterpStackVar {
                var: name.to_string(),
                val: Box::new(val.unshare()),
            }))
        });
    }

    pub fn get_pos(&mut self, input: &str) -> Option<(usize, bool)> {
        let mut cross_frame = false;

        for (idx, i) in self.frames.iter_mut().enumerate().rev() {
            match i {
                InterpStackEntry::StackFrameBoundary => {
                    cross_frame = true;
                }
                InterpStackEntry::Variable(v) => {
                    if v.var != input {
                        continue;
                    }

                    if cross_frame {
                        if v.val.crosses_frames() {
                            return Some((idx, cross_frame));
                        }
                    } else {
                        return Some((idx, cross_frame));
                    }
                }
                InterpStackEntry::Alias(v) => {
                    if v.var != input {
                        continue;
                    }

                    if cross_frame {
                        unsafe {
                            if v.cross_frame || (*v.ptr).crosses_frames() {
                                return Some((idx, cross_frame));
                            }
                        }
                    } else {
                        return Some((idx, cross_frame));
                    }
                }
            }
        }

        None
    }

    pub fn get(&mut self, input: &str) -> Option<InterpVal> {
        let mut cross_frame = false;

        for i in self.frames.iter_mut().rev() {
            match i {
                InterpStackEntry::StackFrameBoundary => {
                    cross_frame = true;
                }
                InterpStackEntry::Variable(v) => {
                    if v.var != input {
                        continue;
                    }

                    if cross_frame {
                        if v.val.crosses_frames() {
                            return Some(InterpVal::Ptr(InterpPtr {
                                sgmt: InterpPtrSgmt::Stack,
                                ptr: &mut *v.val,
                            }));
                        }
                    } else {
                        return Some(InterpVal::Ptr(InterpPtr {
                            sgmt: InterpPtrSgmt::Stack,
                            ptr: &mut *v.val,
                        }));
                    }
                }
                InterpStackEntry::Alias(v) => {
                    if v.var != input {
                        continue;
                    }

                    unsafe {
                        if cross_frame {
                            if v.cross_frame || (*v.ptr).crosses_frames() {
                                return Some(InterpVal::Ptr(InterpPtr {
                                    sgmt: InterpPtrSgmt::Stack,
                                    ptr: v.ptr,
                                }));
                            }
                        } else {
                            return Some(InterpVal::Ptr(InterpPtr {
                                sgmt: InterpPtrSgmt::Stack,
                                ptr: v.ptr,
                            }));
                        }
                    }
                }
            }
        }

        None
    }
    pub fn add(&mut self, input: &str) -> InterpVal {
        let entry = InterpStackEntry::Variable(InterpStackVar {
            var: input.to_string(),
            val: if let Some(val) = self.get(input) {
                if let InterpVal::Ptr(p) = val {
                    Box::new(unsafe {&*p.ptr}.unshare())
                } else {
                    unreachable!();
                }
            } else {
                Box::new(InterpVal::Undefined)
            },
        });
        self.frames.push(entry);

        let f_len = self.frames.len() - 1;
        if let InterpStackEntry::Variable(v) = &mut self.frames[f_len] {
            InterpVal::Ptr(InterpPtr {
                sgmt: InterpPtrSgmt::Stack,
                ptr: &mut *v.val,
            })
        } else {
            unreachable!();
        }
    }

    pub fn alias(&mut self, name: &str, ptr: &InterpPtr, cross_frame: bool) {
        self.frames.push(InterpStackEntry::Alias(InterpStackAlias {
            var: name.to_string(),
            ptr: ptr.ptr,
            cross_frame,
        }));
    }

    pub fn pop(&mut self, input: &str) {
        let (pos, _) = self.get_pos(input).unwrap();
        self.frames.remove(pos);
    }
}
