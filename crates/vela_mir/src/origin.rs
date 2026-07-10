use vela_common::Span;
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirPatternId, HirStmtId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirSourceNode {
    Declaration(HirDeclId),
    Body(HirBodyId),
    Expression(HirExprId),
    Statement(HirStmtId),
    Pattern(HirPatternId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirSourceOrigin {
    /// The executable body containing this origin, when the source fact is
    /// body-scoped. Compile-target descriptors may instead originate at a
    /// declaration that has no executable body.
    pub body: Option<HirBodyId>,
    pub node: MirSourceNode,
    pub span: Span,
}

impl MirSourceOrigin {
    #[must_use]
    pub const fn declaration(declaration: HirDeclId, span: Span) -> Self {
        Self {
            body: None,
            node: MirSourceNode::Declaration(declaration),
            span,
        }
    }

    #[must_use]
    pub const fn body(body: HirBodyId, span: Span) -> Self {
        Self {
            body: Some(body),
            node: MirSourceNode::Body(body),
            span,
        }
    }

    #[must_use]
    pub const fn expression(body: HirBodyId, expression: HirExprId, span: Span) -> Self {
        Self {
            body: Some(body),
            node: MirSourceNode::Expression(expression),
            span,
        }
    }

    #[must_use]
    pub const fn statement(body: HirBodyId, statement: HirStmtId, span: Span) -> Self {
        Self {
            body: Some(body),
            node: MirSourceNode::Statement(statement),
            span,
        }
    }

    #[must_use]
    pub const fn pattern(body: HirBodyId, pattern: HirPatternId, span: Span) -> Self {
        Self {
            body: Some(body),
            node: MirSourceNode::Pattern(pattern),
            span,
        }
    }
}
