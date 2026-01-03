use bitflags::bitflags;
use num_bigint::BigInt;
use petgraph::Directed;
use petgraph::stable_graph::{NodeIndex, StableGraph};
use std::fmt;

use crate::builtin::{BuiltinProc, BuiltinVar};

#[derive(Debug)]
pub struct IRCfg {
    pub procedures: StableGraph<IRProcedure, (), Directed>,
    pub main: NodeIndex,
    pub n_cached: usize,
}

#[macro_export]
macro_rules! IRTypes {
    ("any") => {
        IRType::PROCEDURE
            | IRType::CLOSURE
            | IRType::OBJECT
            | IRType::CLASS
            | IRType::NATIVE_REGEX
            | IRType::ITERATOR
            | IRType::SET
            | IRType::LIST
            | IRType::TERM
            | IRType::TTERM
            | IRType::AST
            | IRType::STRING
            | IRType::BOOL
            | IRType::NUMBER
            | IRType::DOUBLE
            | IRType::MATRIX
            | IRType::VECTOR
            | IRType::UNDEFINED
            | IRType::TYPE
    };
    ("plus") => {
        IRType::SET
            | IRType::LIST
            | IRType::STRING
            | IRType::NUMBER
            | IRType::DOUBLE
            | IRType::MATRIX
            | IRType::VECTOR
    };
    ("minus") => {
        IRType::NUMBER | IRType::DOUBLE | IRType::MATRIX | IRType::VECTOR
    };
    ("mul") => {
        IRType::STRING | IRType::NUMBER | IRType::DOUBLE | IRType::MATRIX | IRType::VECTOR
    };
    ("quot") => {
        IRType::NUMBER | IRType::DOUBLE | IRType::MATRIX | IRType::VECTOR
    };
}

bitflags! {
    /// IRType:
    ///
    /// Values that contain memory are always passed by reference.
    /// A value can either be owned or borrowed. If the value is owned
    /// it must be invalidated if it references memory. If a value is saved
    /// to the stack the stack takes ownership of the value. Dereferencing
    /// assignments invalidate the value before assigning the new value.
    ///
    /// This applies to the following types in particular:
    /// - [`PROCEDURE`]
    /// - [`CLOSURE`]
    /// - [`OBJECT`]
    /// - [`CLASS`]
    /// - [`NATIVE_REGEX`]
    /// - [`SET`]
    /// - [`TERM`]
    /// - [`TTERM`]
    /// - [`AST`]
    /// - [`STRING`]
    /// - [`NUMBER`]
    /// - [`MATRIX`]
    /// - [`VECTOR`]
    #[derive(Clone, Debug)]
    pub struct IRType: u32 {
        const PTR          = 1 << 0;
        const PROCEDURE    = 1 << 1; // "proc"
        const CLOSURE      = 1 << 2; // "clos"
        const OBJECT       = 1 << 3; // "obj"
        const CLASS        = 1 << 4;
        const NATIVE_REGEX = 1 << 5;
        const ITERATOR     = 1 << 6; // "iter"
        const SET          = 1 << 7;
        const LIST         = 1 << 8;
        const TERM         = 1 << 9;
        const TTERM        = 1 << 10;
        const AST          = 1 << 11;
        const STRING       = 1 << 12;
        const BOOL         = 1 << 13;
        const NUMBER       = 1 << 14;
        const DOUBLE       = 1 << 15; // "float"
        const MATRIX       = 1 << 16;
        const VECTOR       = 1 << 17;
        const TYPE         = 1 << 18;
        const UNDEFINED    = 1 << 19; // "om"
    }
}

impl fmt::Display for IRType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Collect active flag names
        let mut parts = Vec::new();

        if self.contains(IRType::PTR) {
            parts.push("ptr");
        }
        if self.contains(IRType::PROCEDURE) {
            parts.push("proc");
        }
        if self.contains(IRType::CLOSURE) {
            parts.push("clos");
        }
        if self.contains(IRType::OBJECT) {
            parts.push("obj");
        }
        if self.contains(IRType::CLASS) {
            parts.push("class");
        }
        if self.contains(IRType::NATIVE_REGEX) {
            parts.push("native_regex");
        }
        if self.contains(IRType::ITERATOR) {
            parts.push("iter");
        }
        if self.contains(IRType::SET) {
            parts.push("set");
        }
        if self.contains(IRType::LIST) {
            parts.push("list");
        }
        if self.contains(IRType::TERM) {
            parts.push("term");
        }
        if self.contains(IRType::TTERM) {
            parts.push("tterm");
        }
        if self.contains(IRType::AST) {
            parts.push("ast");
        }
        if self.contains(IRType::STRING) {
            parts.push("string");
        }
        if self.contains(IRType::BOOL) {
            parts.push("bool");
        }
        if self.contains(IRType::NUMBER) {
            parts.push("number");
        }
        if self.contains(IRType::DOUBLE) {
            parts.push("float");
        }
        if self.contains(IRType::MATRIX) {
            parts.push("matrix");
        }
        if self.contains(IRType::VECTOR) {
            parts.push("vector");
        }
        if self.contains(IRType::TYPE) {
            parts.push("type");
        }
        if self.contains(IRType::UNDEFINED) {
            parts.push("om");
        }

        // If no flags are set, print nothing or a placeholder
        if parts.is_empty() {
            write!(f, "<empty>")
        } else {
            write!(f, "<{}>", parts.join(", "))
        }
    }
}

pub type IRVar = usize;

#[derive(Debug)]
pub struct IRProcedure {
    pub start_block: NodeIndex,
    pub end_block: NodeIndex,
    pub blocks: StableGraph<IRBlock, (), Directed>,
    pub vars: Vec<IRVar>,
}

pub type IRBlock = Vec<IRStmt>;

#[derive(Clone, Debug)]
pub enum IRStmt {
    Assign(IRAssign),
    Branch(IRBranch),
    Try(IRTry),
    TryEnd,
    Goto(NodeIndex),
    Return(IRValue),
    Unreachable,
}

#[derive(Clone, Copy, Debug)]
pub enum IRTarget {
    Ignore,
    Variable(IRVar),
    Deref(IRVar),
}

#[derive(Clone, Debug)]
pub struct IRAssign {
    pub target: IRTarget,
    pub types: IRType,
    pub source: IRValue,
    pub op: IROp,
}

#[derive(Clone, Debug)]
pub enum IROp {
    AccessArray(IRValue),
    Call(IRVar),
    NativeCall(Vec<IRValue>),
    PtrAddress,
    PtrDeref,
    Assign,
    Or(IRValue),
    And(IRValue),
    Not,
    Less(IRValue),
    Equal(IRValue),
    Plus(IRValue),
    Minus(IRValue),
    Mult(IRValue),
    Divide(IRValue),
    IntDivide(IRValue),
    Mod(IRValue),
}

#[derive(Clone, Debug)]
pub enum IRValue {
    Undefined,
    BuiltinProc(BuiltinProc),
    BuiltinVar(BuiltinVar),
    Type(IRType),
    Variable(IRVar),
    String(String),
    Number(BigInt),
    Double(f64),
    Bool(bool),
    Vector(Vec<IRValue>),
    Matrix(Vec<Vec<IRValue>>),
    Procedure(NodeIndex),
}

#[derive(Clone, Debug)]
pub struct IRBranch {
    pub cond: IRValue,
    pub success: NodeIndex,
    pub failure: NodeIndex,
}

#[derive(Clone, Debug)]
pub struct IRTry {
    pub attempt: NodeIndex,
    pub catch: NodeIndex,
}
