use std::collections::BTreeMap;

use vela_common::{SourceId, Span};

use crate::ids::{
    HirBlockId, HirBodyId, HirCaptureId, HirDeclId, HirExprId, HirLocalId, HirMatchArmId,
    HirNodeId, HirParamId, HirPathId, HirPatternId, HirScopeId, HirStmtId,
};
use crate::type_hint::HirTypeHint;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirSourceOrigin {
    pub source: SourceId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirBodyOwner {
    Declaration(HirDeclId),
    ConstInitializer(HirDeclId),
    StateInitializer(HirDeclId),
    SchemaFieldDefault(HirDeclId),
    TraitDefaultMethod(HirNodeId),
    ImplMethod(HirNodeId),
    Lambda {
        parent: HirBodyId,
        expression: HirExprId,
    },
    ParameterDefault {
        parent: HirBodyId,
        parameter: HirParamId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirBodyRoot {
    Block(HirBlockId),
    Expr(HirExprId),
    Empty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirBody {
    pub id: HirBodyId,
    pub owner: HirBodyOwner,
    pub origin: HirSourceOrigin,
    pub root: HirBodyRoot,
    pub root_scope: HirScopeId,
    pub scopes: BTreeMap<HirScopeId, HirScope>,
    pub params: Vec<HirParam>,
    pub blocks: BTreeMap<HirBlockId, HirBlock>,
    pub statements: BTreeMap<HirStmtId, HirStmt>,
    pub expressions: BTreeMap<HirExprId, HirExpr>,
    pub paths: HirPaths,
    pub patterns: BTreeMap<HirPatternId, HirPattern>,
    pub match_arms: BTreeMap<HirMatchArmId, HirMatchArm>,
    pub locals: Vec<HirLocalId>,
    pub self_binding: Option<HirLocalId>,
    pub self_uses: Vec<HirExprId>,
    pub unresolved_references: Vec<HirUnresolvedReference>,
    pub captures: Vec<HirCapture>,
}

impl HirBody {
    #[must_use]
    pub fn new(
        id: HirBodyId,
        owner: HirBodyOwner,
        origin: HirSourceOrigin,
        root_scope: HirScopeId,
    ) -> Self {
        Self {
            id,
            owner,
            origin,
            root: HirBodyRoot::Empty,
            root_scope,
            scopes: BTreeMap::new(),
            params: Vec::new(),
            blocks: BTreeMap::new(),
            statements: BTreeMap::new(),
            expressions: BTreeMap::new(),
            paths: HirPaths::default(),
            patterns: BTreeMap::new(),
            match_arms: BTreeMap::new(),
            locals: Vec::new(),
            self_binding: None,
            self_uses: Vec::new(),
            unresolved_references: Vec::new(),
            captures: Vec::new(),
        }
    }

    #[must_use]
    pub fn expression(&self, id: HirExprId) -> Option<&HirExpr> {
        self.expressions.get(&id)
    }

    #[must_use]
    pub fn call(&self, id: HirExprId) -> Option<&HirCall> {
        match &self.expressions.get(&id)?.kind {
            HirExprKind::Call(call) => Some(call),
            _ => None,
        }
    }

    #[must_use]
    pub fn field(&self, id: HirExprId) -> Option<&HirField> {
        match &self.expressions.get(&id)?.kind {
            HirExprKind::Field(field) => Some(field),
            _ => None,
        }
    }

    #[must_use]
    pub fn index(&self, id: HirExprId) -> Option<&HirIndex> {
        match &self.expressions.get(&id)?.kind {
            HirExprKind::Index(index) => Some(index),
            _ => None,
        }
    }

    pub fn calls(&self) -> impl Iterator<Item = (HirExprId, &HirCall)> {
        self.expressions.iter().filter_map(|(id, expression)| {
            let HirExprKind::Call(call) = &expression.kind else {
                return None;
            };
            Some((*id, call))
        })
    }

    pub fn fields(&self) -> impl Iterator<Item = (HirExprId, &HirField)> {
        self.expressions.iter().filter_map(|(id, expression)| {
            let HirExprKind::Field(field) = &expression.kind else {
                return None;
            };
            Some((*id, field))
        })
    }

    #[must_use]
    pub fn pattern_preorder(&self, roots: &[HirPatternId]) -> Vec<HirPatternId> {
        let mut ordered = Vec::new();
        let mut pending = roots.iter().rev().copied().collect::<Vec<_>>();
        while let Some(pattern_id) = pending.pop() {
            ordered.push(pattern_id);
            let Some(pattern) = self.patterns.get(&pattern_id) else {
                continue;
            };
            match &pattern.kind {
                HirPatternKind::TupleVariant { fields, .. } => {
                    pending.extend(fields.iter().rev().copied());
                }
                HirPatternKind::RecordVariant { fields, .. } => {
                    pending.extend(fields.iter().rev().filter_map(|field| field.pattern));
                }
                HirPatternKind::Binding { .. }
                | HirPatternKind::Path { .. }
                | HirPatternKind::Wildcard
                | HirPatternKind::Literal(_)
                | HirPatternKind::Missing => {}
            }
        }
        ordered
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirScope {
    pub id: HirScopeId,
    pub parent: Option<HirScopeId>,
    pub origin: HirSourceOrigin,
    pub kind: HirScopeKind,
    pub locals: Vec<HirLocalId>,
    pub children: Vec<HirScopeId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirScopeKind {
    Body,
    Block,
    For,
    Lambda,
    MatchArm,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirBlock {
    pub id: HirBlockId,
    pub origin: HirSourceOrigin,
    pub scope: HirScopeId,
    pub statements: Vec<HirStmtId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStmt {
    pub id: HirStmtId,
    pub origin: HirSourceOrigin,
    pub scope: HirScopeId,
    pub kind: HirStmtKind,
}

impl HirStmt {
    #[must_use]
    pub fn tag(&self) -> HirStmtTag {
        match &self.kind {
            HirStmtKind::Let { .. } => HirStmtTag::Let,
            HirStmtKind::Return { .. } => HirStmtTag::Return,
            HirStmtKind::Break => HirStmtTag::Break,
            HirStmtKind::Continue => HirStmtTag::Continue,
            HirStmtKind::For { .. } => HirStmtTag::For,
            HirStmtKind::If(_) => HirStmtTag::If,
            HirStmtKind::Match(_) => HirStmtTag::Match,
            HirStmtKind::Block(_) => HirStmtTag::Block,
            HirStmtKind::Expr { .. } => HirStmtTag::Expr,
        }
    }

    #[must_use]
    pub fn patterns(&self) -> &[HirPatternId] {
        match &self.kind {
            HirStmtKind::Let {
                pattern: Some(pattern),
                ..
            } => std::slice::from_ref(pattern),
            HirStmtKind::For { patterns, .. } => patterns,
            _ => &[],
        }
    }

    #[must_use]
    pub fn initializer(&self) -> Option<HirExprId> {
        match &self.kind {
            HirStmtKind::Let { initializer, .. } => *initializer,
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirStmtTag {
    Let,
    Return,
    Break,
    Continue,
    For,
    If,
    Match,
    Block,
    Expr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirStmtKind {
    Let {
        pattern: Option<HirPatternId>,
        type_hint: Option<HirTypeHint>,
        initializer: Option<HirExprId>,
    },
    Return {
        value: Option<HirExprId>,
    },
    Break,
    Continue,
    For {
        patterns: Vec<HirPatternId>,
        iterable: Option<HirExprId>,
        body: Option<HirBlockId>,
    },
    If(HirIf),
    Match(HirMatch),
    Block(HirBlockId),
    Expr {
        expression: Option<HirExprId>,
        terminated: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirExpr {
    pub id: HirExprId,
    pub origin: HirSourceOrigin,
    pub scope: HirScopeId,
    pub kind: HirExprKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirExprKind {
    Literal(HirLiteral),
    Path(HirPathId),
    Paren {
        expression: Option<HirExprId>,
    },
    Unit,
    Tuple {
        elements: Vec<HirExprId>,
    },
    Unary {
        op: Option<HirUnaryOp>,
        operand: Option<HirExprId>,
    },
    Binary {
        op: Option<HirBinaryOp>,
        lhs: Option<HirExprId>,
        rhs: Option<HirExprId>,
    },
    Assign {
        op: Option<HirAssignOp>,
        target: Option<HirExprId>,
        value: Option<HirExprId>,
    },
    Field(HirField),
    Call(HirCall),
    Index(HirIndex),
    Try {
        expression: Option<HirExprId>,
    },
    Await {
        expression: Option<HirExprId>,
    },
    Array {
        elements: Vec<HirExprId>,
    },
    Map {
        entries: Vec<HirMapEntry>,
    },
    Record {
        constructor: Option<HirPathId>,
        fields: Vec<HirRecordField>,
    },
    Lambda {
        body: HirBodyId,
    },
    Block {
        block: HirBlockId,
    },
    If(HirIf),
    Match(HirMatch),
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirLiteral {
    Bool(bool),
    Integer(HirIntegerLiteral),
    Float(HirFloatLiteral),
    Char(char),
    String(String),
    Bytes(Vec<u8>),
    Interpolated {
        parts: Vec<HirInterpolatedStringPart>,
    },
    Invalid {
        source_text: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirInterpolatedStringPart {
    Text(String),
    Expr(HirExprId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirIntegerLiteral {
    pub text: String,
    pub radix: HirIntRadix,
    pub suffix: Option<HirIntegerSuffix>,
}

impl HirIntegerLiteral {
    /// Returns the spelling retained in HIR, including its suffix, without
    /// consulting syntax or source text.
    #[must_use]
    pub fn source_spelling(&self) -> String {
        let mut spelling = self.text.clone();
        if let Some(suffix) = self.suffix {
            spelling.push_str(suffix.source_spelling());
        }
        spelling
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirIntRadix {
    Binary,
    Decimal,
    Hex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirIntegerSuffix {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
}

impl HirIntegerSuffix {
    #[must_use]
    pub const fn source_spelling(self) -> &'static str {
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
pub struct HirFloatLiteral {
    pub text: String,
    pub suffix: Option<HirFloatSuffix>,
}

impl HirFloatLiteral {
    /// Returns the spelling retained in HIR, including its suffix, without
    /// consulting syntax or source text.
    #[must_use]
    pub fn source_spelling(&self) -> String {
        let mut spelling = self.text.clone();
        if let Some(suffix) = self.suffix {
            spelling.push_str(suffix.source_spelling());
        }
        spelling
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirFloatSuffix {
    F32,
    F64,
}

impl HirFloatSuffix {
    #[must_use]
    pub const fn source_spelling(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirUnaryOp {
    Not,
    Negate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirBinaryOp {
    Or,
    And,
    Equal,
    NotEqual,
    IdentityEqual,
    IdentityNotEqual,
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
pub enum HirAssignOp {
    Set,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCall {
    pub expression: HirExprId,
    pub callee: HirExprId,
    pub arguments: Vec<HirArgument>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArgument {
    pub name: Option<String>,
    pub name_origin: Option<HirSourceOrigin>,
    pub value: Option<HirExprId>,
    pub origin: HirSourceOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirField {
    pub expression: HirExprId,
    pub receiver: HirExprId,
    pub name: String,
    pub member_origin: HirSourceOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirIndex {
    pub expression: HirExprId,
    pub receiver: HirExprId,
    pub index: HirExprId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMapEntry {
    pub key: Option<HirExprId>,
    /// The static string key represented by a supported literal or path key.
    /// Dynamic, unsupported, and missing keys retain `None` explicitly.
    pub logical_key: Option<String>,
    pub value: Option<HirExprId>,
    pub origin: HirSourceOrigin,
}

impl HirMapEntry {
    #[must_use]
    pub fn logical_key(&self) -> Option<&str> {
        self.logical_key.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirRecordField {
    pub name: String,
    pub name_origin: HirSourceOrigin,
    pub value: Option<HirExprId>,
    pub shorthand: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirIf {
    pub condition: Option<HirExprId>,
    pub then_block: Option<HirBlockId>,
    pub else_branch: Option<HirElseBranch>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirElseBranch {
    If(Box<HirIf>),
    Block(HirBlockId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMatch {
    pub scrutinee: Option<HirExprId>,
    pub arms: Vec<HirMatchArmId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMatchArm {
    pub id: HirMatchArmId,
    pub origin: HirSourceOrigin,
    pub scope: HirScopeId,
    pub pattern: Option<HirPatternId>,
    pub guard: Option<HirExprId>,
    pub body: Option<HirMatchArmBody>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirMatchArmBody {
    Expr(HirExprId),
    Block(HirBlockId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirPath {
    pub id: HirPathId,
    pub owner: HirPathOwner,
    pub kind: HirPathKind,
    pub path: Vec<String>,
    pub origin: HirSourceOrigin,
    pub segment_origin: HirSourceOrigin,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirPaths(BTreeMap<HirPathId, HirPath>);

impl HirPaths {
    pub fn insert(&mut self, id: HirPathId, path: HirPath) -> Option<HirPath> {
        self.0.insert(id, path)
    }

    #[must_use]
    pub fn get(&self, id: &HirPathId) -> Option<&HirPath> {
        self.0.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &HirPath> {
        self.0.values()
    }

    pub fn values(&self) -> impl Iterator<Item = &HirPath> {
        self.0.values()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirPathOwner {
    Expression(HirExprId),
    Pattern(HirPatternId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirPathKind {
    Value,
    Callee,
    Constructor,
    Pattern,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirPattern {
    pub id: HirPatternId,
    pub origin: HirSourceOrigin,
    pub scope: HirScopeId,
    pub kind: HirPatternKind,
}

impl HirPattern {
    #[must_use]
    pub fn local(&self) -> Option<HirLocalId> {
        match &self.kind {
            HirPatternKind::Binding { local } => *local,
            _ => None,
        }
    }

    #[must_use]
    pub fn is_binding(&self) -> bool {
        matches!(&self.kind, HirPatternKind::Binding { .. })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirPatternKind {
    Binding {
        local: Option<HirLocalId>,
    },
    TupleVariant {
        path: Option<HirPathId>,
        fields: Vec<HirPatternId>,
    },
    RecordVariant {
        path: Option<HirPathId>,
        fields: Vec<HirRecordPatternField>,
    },
    Path {
        path: Option<HirPathId>,
    },
    Wildcard,
    Literal(Option<HirLiteral>),
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirRecordPatternField {
    pub name: String,
    pub name_origin: HirSourceOrigin,
    pub pattern: Option<HirPatternId>,
    pub shorthand: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirParam {
    pub id: HirParamId,
    pub local: HirLocalId,
    pub origin: HirSourceOrigin,
    pub default_body: Option<HirBodyId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCapture {
    pub id: HirCaptureId,
    pub local: HirLocalId,
    pub use_expression: HirExprId,
    pub owner: HirBodyId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirUnresolvedReference {
    pub expression: HirExprId,
    pub name: String,
    pub origin: HirSourceOrigin,
}
