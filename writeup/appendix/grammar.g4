(* Whitespace and comments are skipped *)

Block
    : Statement*
    ;

Statement
    : ClassDecl
    | IfDecl
    | SwitchDecl
    | MatchDecl
    | ScanDecl
    | ForDecl
    | WhileDecl
    | DoWhileDecl
    | TryCatchDecl
    | CheckDecl
    | ReturnDecl
    | AssignDecl ';'
    | AssignModDecl ';'
    | Expr ';'
    | 'backtrack' ';'
    | 'break' ';'
    | 'continue' ';'
    | 'exit' ';'
    ;

ClassDecl
    : 'class' Ident '(' ProcedureParams ')'
      '{' Block ClassStaticDecl? '}' ';'?
    ;

ClassStaticDecl
    : 'static' '{' Block '}'
    ;

IfDecl
    : 'if' '(' Expr ')' '{' Block '}'
      ElifDecl*
      ElseDecl?
    ;

ElifDecl
    : 'else' 'if' '(' Expr ')' '{' Block '}'
    ;

ElseDecl
    : 'else' '{' Block '}'
    ;

SwitchDecl
    : 'switch' '{' SwitchCase* SwitchDefault? '}'
    ;

SwitchCase
    : 'case' Expr ':' Block
    ;

SwitchDefault
    : 'default' ':' Block
    ;

MatchDecl
    : 'match' '(' Expr ')'
      '{' MatchBranch+ MatchDefault? '}'
    ;

MatchDefault
    : 'default' ':' Block
    ;

MatchBranch
    : MatchCaseBranch
    | MatchRegexBranch
    ;

MatchCaseBranch
    : 'case' Expr (',' Expr)* PipeExpr? ':' Block
    ;

MatchRegexBranch
    : 'regex' Expr MatchRegexBranchAs? PipeExpr? ':' Block
    ;

MatchRegexBranchAs
    : 'as' Expr
    ;

ScanDecl
    : 'scan' '(' Expr ')' ScanUsing? '{' MatchRegexBranch+ '}'
    ;

ScanUsing
    : 'using' Ident
    ;

ForDecl
    : 'for' '(' IteratorChain PipeExpr? ')' '{' Block '}'
    ;

WhileDecl
    : 'while' '(' Expr ')' '{' Block '}'
    ;

DoWhileDecl
    : 'do' '{' Block '}' 'while' '(' Expr ')' ';'
    ;

TryCatchDecl
    : 'try' '{' Block '}' CatchBranches
    ;

CatchBranches
    : CatchCondBranch* CatchFinBranch?
    ;

CatchFinBranch
    : 'catch' '(' Ident ')' '{' Block '}'
    ;

CatchCondBranch
    : CatchLngBranch
    | CatchUsrBranch
    ;

CatchLngBranch
    : 'catchLng' '(' Ident ')' '{' Block '}'
    ;

CatchUsrBranch
    : 'catchUsr' '(' Ident ')' '{' Block '}'
    ;

CheckDecl
    : 'check' '{' Block '}' CheckAfterBacktrack?
    ;

CheckAfterBacktrack
    : 'afterBacktrack' '{' Block '}'
    ;

ReturnDecl
    : 'return' Expr? ';'
    ;

AssignDecl
    : Accessible ':=' AssignSource
    ;

AssignSource
    : AssignDecl
    | Expr
    ;

AssignModDecl
    : Accessible AssignModKind Expr
    ;

AssignModKind
    : '+='
    | '-='
    | '*='
    | '/='
    | '\='
    | '%='
    ;

ExprList
    : Expr (',' Expr)*
    ;

Expr
    : Lambda
    | SetEqual
    ;

Lambda
    : LambdaParams LambdaIsClosure Expr
    ;

LambdaParams
    : Ident
    | BrackCollection
    ;

LambdaIsClosure
    : '|=>'
    | '|->'
    ;

SetEqual
    : Implication (SetEqualOp Implication)?
    ;

SetEqualOp
    : '<==>'
    | '<!=>'
    ;

Implication
    : Disjunction ('=>' Implication)?
    ;

Disjunction
    : Conjunction ('||' Conjunction)*
    ;

Conjunction
    : Comparison ('&&' Comparison)*
    ;

Comparison
    : Sum (ComparisonOp Sum)?
    ;

ComparisonOp
    : '=='
    | '!='
    | '<'
    | '<='
    | '>'
    | '>='
    | 'in'
    | 'notin'
    ;

Sum
    : Product (SumOp Product)*
    ;

SumOp
    : '+'
    | '-'
    ;

Product
    : Reduce (ProductOp Reduce)*
    ;

ProductOp
    : '*'
    | '/'
    | '\'
    | '%'
    | '><'
    ;

Reduce
    : PrefixOperation (ReduceOp PrefixOperation)*
    ;

ReduceOp
    : '+/'
    | '*/'
    ;

PrefixOperation
    : Factor ('**' PrefixOperation)?
    | PrefixUnaryOp PrefixOperation
    ;

PrefixUnaryOp
    : '+/'
    | '*/'
    | '#'
    | '-'
    ;

Factor
    : '!' Factor
    | QuantKind '(' IteratorChain '|' Expr ')'
    | AccessibleProcedure '!'?
    ;

QuantKind
    : 'exists'
    | 'forall'
    ;

AccessibleProcedure
    : Procedure
    | Accessible
    ;

Accessible
    : AccessibleHead AccessibleBodyElem*
    | Value
    ;

AccessibleHead
    : '(' Expr ')'
    | Ident
    | Call
    ;

AccessibleBodyElem
    : '.' Ident
    | '.' Call
    | BrackCollection
    | CurlyCollection
    ;

AccessibleList
    : Accessible (',' Accessible)*
    ;

Call
    : Ident '(' ProcedureCallParams? ')'
    ;

ProcedureCallParams
    : Expr (',' Expr)* ',' '*' Expr
    | Expr (',' Expr)*
    | '*' Expr
    ;

Procedure
    : ProcedureKind '(' ProcedureParams ')' '{' Block '}'
    ;

ProcedureKind
    : 'procedure'
    | 'cachedProcedure'
    | 'closure'
    ;

ProcedureParams
    : ProcedureParam (',' ProcedureParam)* (',' DefaultParam)* (',' ListParam)?
    | DefaultParam (',' DefaultParam)* (',' ListParam)?
    | ListParam?
    ;

ProcedureParam
    : 'rw'? Ident
    ;

DefaultParam
    : Ident ':=' Expr
    ;

ListParam
    : '*' Ident
    ;

PipeExpr
    : '|' Expr
    ;

TermName
    : Term
    | TTerm
    ;

TermExpr
    : TermName '(' Expr (',' Expr)* ')'
    | TermName '(' ')'
    ;

Value
    : Collection
    | StringRule
    | Literal
    | Matrix
    | Vector
    | TermExpr
    | AtomicValue
    | '_'
    ;

Collection
    : CurlyCollection
    | BrackCollection
    ;

ExtendedCollection
    : Expr '..' Expr
    | Expr (',' Expr)* PipeExpr?
    ;

BrackCollection
    : '[' Expr ',' ExtendedCollection ']'
    | '[' Expr '..' Expr? ']'
    | '[' '..' Expr ']'
    | '[' Expr ':' IteratorChain PipeExpr? ']'
    | '[' Expr PipeExpr ']'
    | '[' Expr? ']'
    ;

CurlyCollection
    : '{' Expr ',' ExtendedCollection '}'
    | '{' Expr '..' Expr? '}'
    | '{' '..' Expr '}'
    | '{' Expr ':' IteratorChain PipeExpr? '}'
    | '{' Expr PipeExpr '}'
    | '{' Expr? '}'
    ;

VecNumber
    : Number
    | Double
    ;

Vector
    : '<<' ('-'? VecNumber ('/' Number)?)+ '>>'
    ;

Matrix
    : '<<' Vector+ '>>'
    ;

AtomicValue
    : Number
    | Double
    | 'om'
    | 'true'
    | 'false'
    ;

Iterator
    : Accessible 'in' Expr
    ;

IteratorChain
    : Iterator (',' Iterator)*
    ;

Ident
    : [a-zA-Z][a-zA-Z_0-9]*
    ;

Term
    : '@' [a-zA-Z][a-zA-Z_0-9]*
    ;

TTerm
    : '@@@' [a-zA-Z][a-zA-Z_0-9]*
    ;

Number
    : '0' | [1-9][0-9]*
    ;

Double
    : ([0-9]+)? '.' [0-9]+ ([eE] [+-]? [0-9]+)?
    ;

StringRule
    : '"' ('\\' . | ~['"''\\'])* '"'
    ;

Literal
    : '\'' ('\'\'' | ~['\''])* '\''
    ;
