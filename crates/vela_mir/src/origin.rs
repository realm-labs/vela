use vela_common::Span;
use vela_hir::ids::{HirBodyId, HirExprId, HirPatternId, HirStmtId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirSourceNode {
    Body(HirBodyId),
    Expression(HirExprId),
    Statement(HirStmtId),
    Pattern(HirPatternId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirSourceOrigin {
    pub body: HirBodyId,
    pub node: MirSourceNode,
    pub span: Span,
}

impl MirSourceOrigin {
    #[must_use]
    pub const fn body(body: HirBodyId, span: Span) -> Self {
        Self {
            body,
            node: MirSourceNode::Body(body),
            span,
        }
    }

    #[must_use]
    pub const fn expression(body: HirBodyId, expression: HirExprId, span: Span) -> Self {
        Self {
            body,
            node: MirSourceNode::Expression(expression),
            span,
        }
    }

    #[must_use]
    pub const fn statement(body: HirBodyId, statement: HirStmtId, span: Span) -> Self {
        Self {
            body,
            node: MirSourceNode::Statement(statement),
            span,
        }
    }

    #[must_use]
    pub const fn pattern(body: HirBodyId, pattern: HirPatternId, span: Span) -> Self {
        Self {
            body,
            node: MirSourceNode::Pattern(pattern),
            span,
        }
    }
}
