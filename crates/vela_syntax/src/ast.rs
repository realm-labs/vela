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
    Global(GlobalItem),
    Function(FunctionItem),
    Struct(StructItem),
    Enum(EnumItem),
    Trait(TraitItem),
    Impl(ImplItem),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseItem {
    pub path: Vec<String>,
    pub alias: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstItem {
    pub name: String,
    pub type_hint: Option<TypeHint>,
    pub value: Expr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalItem {
    pub name: String,
    pub type_hint: TypeHint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeHint {
    pub path: Vec<String>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Param {
    pub name: String,
    pub span: Span,
    pub type_hint: Option<TypeHint>,
    pub default_value: Option<Expr>,
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
    pub attrs: Vec<Attribute>,
    pub name: String,
    pub span: Span,
    pub type_hint: Option<TypeHint>,
    pub default_value: Option<Expr>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumItem {
    pub name: String,
    pub variants: Vec<EnumVariant>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumVariant {
    pub attrs: Vec<Attribute>,
    pub name: String,
    pub span: Span,
    pub fields: EnumVariantFields,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnumVariantFields {
    Unit,
    Tuple(Vec<Param>),
    Record(Vec<StructField>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitItem {
    pub name: String,
    pub methods: Vec<TraitMethod>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitMethod {
    pub attrs: Vec<Attribute>,
    pub name: String,
    pub span: Span,
    pub params: Vec<Param>,
    pub return_type: Option<TypeHint>,
    pub has_default: bool,
    pub default_body: Option<Block>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplItem {
    pub kind: ImplKind,
    pub target_path: Vec<String>,
    pub methods: Vec<ImplMethod>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImplKind {
    Inherent,
    Trait { trait_path: Vec<String> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplMethod {
    pub attrs: Vec<Attribute>,
    pub function: FunctionItem,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub path: Vec<String>,
    pub value: Option<String>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stmt {
    pub attrs: Vec<Attribute>,
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
        index_pattern: Option<Pattern>,
        pattern: Pattern,
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
    InterpolatedString(Vec<InterpolatedStringPart>),
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
    pub span: Span,
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
pub enum InterpolatedStringPart {
    Text(String),
    Expr(Expr),
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
    Integer(IntegerLiteral),
    Float(FloatLiteral),
    String(String),
    Bytes(Vec<u8>),
}

impl Literal {
    #[must_use]
    pub fn integer(text: impl Into<String>) -> Self {
        Self::Integer(IntegerLiteral::unsuffixed(text))
    }

    #[must_use]
    pub fn float(text: impl Into<String>) -> Self {
        Self::Float(FloatLiteral::unsuffixed(text))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegerLiteral {
    pub text: String,
    pub radix: IntRadix,
    pub suffix: Option<IntegerSuffix>,
}

impl IntegerLiteral {
    #[must_use]
    pub fn unsuffixed(text: impl Into<String>) -> Self {
        let text = text.into();
        let radix = IntRadix::from_literal_text(&text);
        Self {
            text,
            radix,
            suffix: None,
        }
    }

    #[must_use]
    pub fn source_text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn source_text_with_suffix(&self) -> String {
        let mut text = self.text.clone();
        if let Some(suffix) = self.suffix {
            text.push_str(suffix.text());
        }
        text
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntRadix {
    Binary,
    Decimal,
    Hex,
}

impl IntRadix {
    #[must_use]
    pub fn from_literal_text(text: &str) -> Self {
        if text.starts_with("0x") || text.starts_with("0X") {
            Self::Hex
        } else if text.starts_with("0b") || text.starts_with("0B") {
            Self::Binary
        } else {
            Self::Decimal
        }
    }

    #[must_use]
    pub const fn base(self) -> u32 {
        match self {
            Self::Binary => 2,
            Self::Decimal => 10,
            Self::Hex => 16,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegerSuffix {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
}

impl IntegerSuffix {
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FloatLiteral {
    pub text: String,
    pub suffix: Option<FloatSuffix>,
}

impl FloatLiteral {
    #[must_use]
    pub fn unsuffixed(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            suffix: None,
        }
    }

    #[must_use]
    pub fn source_text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn source_text_with_suffix(&self) -> String {
        let mut text = self.text.clone();
        if let Some(suffix) = self.suffix {
            text.push_str(suffix.text());
        }
        text
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FloatSuffix {
    F32,
    F64,
}

impl FloatSuffix {
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
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
    Range,
    RangeInclusive,
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
