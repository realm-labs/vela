use vela_common::Span;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFile {
    pub items: Vec<Item>,
    pub diagnostics: Vec<vela_common::Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    pub attrs: Vec<Attribute>,
    pub visibility: Visibility,
    pub kind: ItemKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Visibility {
    Private,
    Public,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ItemKind {
    Use(UseItem),
    Const(ConstItem),
    Function(FunctionItem),
    Struct(StructItem),
    Enum(EnumItem),
    Trait(TraitItem),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseItem {
    pub path: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstItem {
    pub name: String,
    pub type_hint: Option<TypeHint>,
    pub value: Expr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeHint {
    pub path: Vec<String>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Param {
    pub name: String,
    pub type_hint: Option<TypeHint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionItem {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeHint>,
    pub body: Block,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructItem {
    pub name: String,
    pub fields: Vec<StructField>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructField {
    pub name: String,
    pub type_hint: Option<TypeHint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumItem {
    pub name: String,
    pub variants: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitItem {
    pub name: String,
    pub methods: Vec<TraitMethod>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeHint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub path: Vec<String>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StmtKind {
    Let {
        name: String,
        type_hint: Option<TypeHint>,
        value: Option<Expr>,
    },
    Return(Option<Expr>),
    Break,
    Continue,
    For {
        binding: String,
        iterable: Expr,
        body: Block,
    },
    Expr(Expr),
    Block(Block),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExprKind {
    Literal(Literal),
    Path(Vec<String>),
    SelfValue,
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Assign {
        op: AssignOp,
        target: Box<Expr>,
        value: Box<Expr>,
    },
    Field {
        base: Box<Expr>,
        name: String,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Argument>,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    Try(Box<Expr>),
    Array(Vec<Expr>),
    Map(Vec<MapEntry>),
    Record {
        path: Vec<String>,
        fields: Vec<RecordField>,
    },
    Lambda {
        params: Vec<Param>,
        body: Box<Expr>,
    },
    If(Box<IfExpr>),
    Match(Box<MatchExpr>),
    Block(Block),
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Argument {
    pub name: Option<String>,
    pub value: Expr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapEntry {
    pub key: Expr,
    pub value: Expr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordField {
    pub name: String,
    pub value: Option<Expr>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfExpr {
    pub condition: Expr,
    pub then_branch: Block,
    pub else_branch: Option<ElseBranch>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ElseBranch {
    If(Box<IfExpr>),
    Block(Block),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchExpr {
    pub scrutinee: Expr,
    pub arms: Vec<MatchArm>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pattern {
    Wildcard,
    Literal(Literal),
    Binding(String),
    Path(Vec<String>),
    TupleVariant {
        path: Vec<String>,
        fields: Vec<Pattern>,
    },
    RecordVariant {
        path: Vec<String>,
        fields: Vec<RecordPatternField>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordPatternField {
    pub name: String,
    pub pattern: Option<Pattern>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Literal {
    Null,
    Bool(bool),
    Int(String),
    Float(String),
    String(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Not,
    Negate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Or,
    And,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignOp {
    Set,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}
