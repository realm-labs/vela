use std::collections::BTreeMap;

use vela_common::{SourceId, Span};
use vela_syntax::ast::{SyntaxExpressionKind, SyntaxPatternKind, SyntaxStatementKind};

use crate::ids::{
    HirBlockId, HirBodyId, HirCaptureId, HirDeclId, HirExprId, HirLocalId, HirNodeId, HirParamId,
    HirPatternId, HirScopeId, HirStmtId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSourceOrigin {
    pub source: SourceId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirBodyOwner {
    Declaration(HirDeclId),
    ConstInitializer(HirDeclId),
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
    pub root_scope: Option<HirScopeId>,
    pub scopes: BTreeMap<HirScopeId, HirScope>,
    pub params: Vec<HirParam>,
    pub blocks: BTreeMap<HirBlockId, HirBlock>,
    pub statements: BTreeMap<HirStmtId, HirStmt>,
    pub expressions: BTreeMap<HirExprId, HirExpr>,
    pub calls: BTreeMap<HirExprId, HirCall>,
    pub patterns: BTreeMap<HirPatternId, HirPattern>,
    pub locals: Vec<HirLocalId>,
    pub self_binding: Option<HirLocalId>,
    pub self_uses: Vec<HirExprId>,
    pub unresolved_references: Vec<HirUnresolvedReference>,
    pub captures: Vec<HirCapture>,
}

impl HirBody {
    #[must_use]
    pub fn new(id: HirBodyId, owner: HirBodyOwner, origin: HirSourceOrigin) -> Self {
        Self {
            id,
            owner,
            origin,
            root: HirBodyRoot::Empty,
            root_scope: None,
            scopes: BTreeMap::new(),
            params: Vec::new(),
            blocks: BTreeMap::new(),
            statements: BTreeMap::new(),
            expressions: BTreeMap::new(),
            calls: BTreeMap::new(),
            patterns: BTreeMap::new(),
            locals: Vec::new(),
            self_binding: None,
            self_uses: Vec::new(),
            unresolved_references: Vec::new(),
            captures: Vec::new(),
        }
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
    pub statements: Vec<HirStmtId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStmt {
    pub id: HirStmtId,
    pub origin: HirSourceOrigin,
    pub kind: HirStmtKind,
    pub patterns: Vec<HirPatternId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirStmtKind {
    Let,
    Return,
    Break,
    Continue,
    For,
    If,
    Match,
    Block,
    Expr,
    Unknown,
}

impl From<SyntaxStatementKind> for HirStmtKind {
    fn from(kind: SyntaxStatementKind) -> Self {
        match kind {
            SyntaxStatementKind::Let => Self::Let,
            SyntaxStatementKind::Return => Self::Return,
            SyntaxStatementKind::Break => Self::Break,
            SyntaxStatementKind::Continue => Self::Continue,
            SyntaxStatementKind::For => Self::For,
            SyntaxStatementKind::If => Self::If,
            SyntaxStatementKind::Match => Self::Match,
            SyntaxStatementKind::Block => Self::Block,
            SyntaxStatementKind::Expr => Self::Expr,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirExpr {
    pub id: HirExprId,
    pub origin: HirSourceOrigin,
    pub kind: HirExprKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCall {
    pub expression: HirExprId,
    pub callee: HirExprId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirExprKind {
    Literal,
    Path,
    Paren,
    Unit,
    Tuple,
    Unary,
    Binary,
    Assign,
    Field,
    Call,
    Index,
    Try,
    Array,
    Map,
    Record,
    Lambda,
    Block,
    If,
    Match,
    Unknown,
}

impl From<SyntaxExpressionKind> for HirExprKind {
    fn from(kind: SyntaxExpressionKind) -> Self {
        match kind {
            SyntaxExpressionKind::Literal => Self::Literal,
            SyntaxExpressionKind::Path => Self::Path,
            SyntaxExpressionKind::Paren => Self::Paren,
            SyntaxExpressionKind::Unit => Self::Unit,
            SyntaxExpressionKind::Tuple => Self::Tuple,
            SyntaxExpressionKind::Unary => Self::Unary,
            SyntaxExpressionKind::Binary => Self::Binary,
            SyntaxExpressionKind::Assign => Self::Assign,
            SyntaxExpressionKind::Field => Self::Field,
            SyntaxExpressionKind::Call => Self::Call,
            SyntaxExpressionKind::Index => Self::Index,
            SyntaxExpressionKind::Try => Self::Try,
            SyntaxExpressionKind::Array => Self::Array,
            SyntaxExpressionKind::Map => Self::Map,
            SyntaxExpressionKind::Record => Self::Record,
            SyntaxExpressionKind::Lambda => Self::Lambda,
            SyntaxExpressionKind::Block => Self::Block,
            SyntaxExpressionKind::If => Self::If,
            SyntaxExpressionKind::Match => Self::Match,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirPattern {
    pub id: HirPatternId,
    pub origin: HirSourceOrigin,
    pub kind: HirPatternKind,
    pub local: Option<HirLocalId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirPatternKind {
    Binding,
    TupleVariant,
    RecordVariant,
    Path,
    Wildcard,
    Literal,
    Unknown,
}

impl From<Option<SyntaxPatternKind>> for HirPatternKind {
    fn from(kind: Option<SyntaxPatternKind>) -> Self {
        match kind {
            Some(SyntaxPatternKind::Binding) => Self::Binding,
            Some(SyntaxPatternKind::TupleVariant) => Self::TupleVariant,
            Some(SyntaxPatternKind::RecordVariant) => Self::RecordVariant,
            Some(SyntaxPatternKind::Path) => Self::Path,
            Some(SyntaxPatternKind::Wildcard) => Self::Wildcard,
            Some(SyntaxPatternKind::Literal) => Self::Literal,
            None => Self::Unknown,
        }
    }
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
