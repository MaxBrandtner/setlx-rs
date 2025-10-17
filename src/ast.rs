use num_bigint::BigInt;

pub type CSTBlock = Vec<CSTStatement>;

#[derive(Clone, Debug, PartialEq)]
pub enum CSTStatement {
    Class(CSTClass),
    If(CSTIf),
    Switch(CSTSwitch),
    Match(CSTMatch),
    Scan(CSTScan),
    For(CSTFor),
    While(CSTWhile),
    DoWhile(CSTWhile),
    TryCatch(CSTTryCatch),
    Check(CSTCheck),
    Return(CSTReturn),
    Assign(CSTAssign),
    AssignMod(CSTAssignMod),
    Expression(CSTExpression),

    Backtrack,
    Break,
    Continue,
    Exit,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CSTExpression {
    Lambda(CSTLambda),
    Op(CSTExpressionOp),
    UnaryOp(CSTExpressionUnaryOp),

    /* factors */
    Procedure(CSTProcedure),
    Call(CSTProcedureCall),
    Variable(String),
    Accessible(CSTAccessible),
    Literal(String),
    Bool(bool),
    Double(f64),
    Number(BigInt),
    Collection(CSTCollection),
    Matrix(Vec<Vec<CSTExpression>>),
    Vector(Vec<CSTExpression>),
    Quantifier(CSTQuantifier),

    Om,
    Ignore,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CSTOp {
    Imply,     // =>
    Or,        // ||
    And,       // &&
    Eq,        // ==
    Neq,       // !=
    Less,      // <
    Leq,       // <=
    Greater,   // >
    Geq,       // >=
    In,        // in
    NotIn,     // notin
    Plus,      // +
    Minus,     // -
    Mult,      // *
    Div,       // /
    IntDiv,    // \\
    Mod,       // %
    Cartesian, // ><
    Power,     // **
    SumMem,    // +/
    ProdMem,   // */
    SetEq,     // <==>
    SetNeq,    // <!=>
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTExpressionOp {
    pub op: CSTOp,
    pub left: Box<CSTExpression>,
    pub right: Box<CSTExpression>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CSTUnaryOp {
    Minus,    //   -
    Card,     //   #
    SumMem,   //  +/
    ProdMem,  //  */
    Factor,   //   !
    Not,      //   !
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTExpressionUnaryOp {
    pub op: CSTUnaryOp,
    pub expr: Box<CSTExpression>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTLambda {
    pub params: CSTCollection,
    pub is_closure: bool,
    pub expr: Box<CSTExpression>,

}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTParam {
    pub name: String,
    pub is_rw: bool,
    pub default: Option<CSTExpression>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CSTProcedureKind {
    Normal,
    Cached,
    Closure,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTProcedure {
    pub kind: CSTProcedureKind,
    pub params: Vec<CSTParam>,
    pub list_param: Option<String>,
    pub block: CSTBlock,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTProcedureCall {
    pub is_term: bool,
    pub name: String,
    pub curly_params: bool,
    pub params: Vec<CSTExpression>,
    pub rest_param: Option<Box<CSTExpression>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTAccessible {
    pub head: Box<CSTExpression>,
    pub body: Vec<CSTExpression>, // Variable | Call | Collection
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTIterParam {
    pub variable: CSTExpression,
    pub collection: CSTExpression,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTRange {
    pub left: Option<Box<CSTExpression>>,
    pub right: Option<Box<CSTExpression>>
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTSet {
    pub range: Option<CSTRange>,
    pub expressions: Vec<CSTExpression>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTComprehension {
    pub expressions: Vec<CSTExpression>,
    pub iterators: Vec<CSTIterParam>,
    pub condition: Option<Box<CSTExpression>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CSTCollection {
    Set(CSTSet),
    List(CSTSet),
    SetComprehension(CSTComprehension),
    ListComprehension(CSTComprehension),
}

#[derive(Clone, Debug, PartialEq)]
pub enum CSTQuantifierKind {
    Exists,
    Forall,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTQuantifier {
    pub kind: CSTQuantifierKind,
    pub iterators: Vec<CSTIterParam>,
    pub condition: Box<CSTExpression>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTClass {
    pub name: String,
    pub params: Vec<CSTParam>,
    pub block: CSTBlock,
    pub static_block: Option<CSTBlock>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTIfBranch {
    pub condition: CSTExpression,
    pub block: CSTBlock,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTIf {
    pub branches: Vec<CSTIfBranch>,
    pub alternative: Option<CSTBlock>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTSwitchCase {
    pub condition: CSTExpression,
    pub block: CSTBlock,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTSwitch {
    pub cases: Vec<CSTSwitchCase>,
    pub default: Option<CSTBlock>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTMatch {
    pub expression: CSTExpression,
    pub branches: Vec<CSTMatchBranch>,
    pub default: CSTBlock,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CSTMatchBranch {
    Case(CSTMatchBranchCase),
    Regex(CSTMatchBranchRegex),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTMatchBranchCase {
    pub expressions: Vec<CSTExpression>,
    pub condition: Option<CSTExpression>,
    pub statements: CSTBlock,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTMatchBranchRegex {
    pub pattern: CSTExpression,
    pub pattern_out: Option<CSTExpression>,
    pub condition: Option<CSTExpression>,
    pub statements: CSTBlock,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTScan {
    pub expression: CSTExpression,
    pub variable: Option<String>,
    pub branches: Vec<CSTMatchBranch>, // always resolves to CSTMatchBranchRegex
    pub default: CSTBlock,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTFor {
    pub params: Vec<CSTIterParam>,
    pub condition: Option<CSTExpression>,
    pub block: CSTBlock,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTWhile {
    pub condition: CSTExpression,
    pub block: CSTBlock,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CSTCatchKind {
    Usr,
    Lng,
    Final,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTCatch {
    pub kind: CSTCatchKind,
    pub exception: String,
    pub block: CSTBlock,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTTryCatch {
    pub try_branch: CSTBlock,
    pub catch_branches: Vec<CSTCatch>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTCheck {
    pub block: CSTBlock,
    pub after_backtrack: CSTBlock,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTReturn {
    pub val: Option<CSTExpression>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTAssign {
    pub assign: CSTExpression,
    pub expr: Box<CSTStatement>, // CSTAssign | CSTExpression
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum CSTAssignModKind {
    PlusEq,   //  +=
    MinusEq,  //  -=
    MultEq,   //  *=
    DivEq,    //  /=
    IntDivEq, // \\=
    ModEq,    //  %=
}

#[derive(Clone, Debug, PartialEq)]
pub struct CSTAssignMod {
    pub assign: CSTExpression,
    pub kind: CSTAssignModKind,
    pub expr: CSTExpression,
}
