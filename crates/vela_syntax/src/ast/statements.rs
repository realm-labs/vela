use super::expr::SyntaxExpression;
use super::{AstChildren, AstNode, SyntaxBlock, SyntaxPattern, SyntaxTypeHint};
use crate::{SyntaxKind, SyntaxNode};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxStatement {
    syntax: SyntaxNode,
}

impl AstNode for SyntaxStatement {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::LetStmt
                | SyntaxKind::ReturnStmt
                | SyntaxKind::BreakStmt
                | SyntaxKind::ContinueStmt
                | SyntaxKind::ForStmt
                | SyntaxKind::IfExpr
                | SyntaxKind::MatchExpr
                | SyntaxKind::ExprStmt
        )
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxLetStmt {
    syntax: SyntaxNode,
}

impl SyntaxLetStmt {
    #[must_use]
    pub fn type_hint(&self) -> Option<SyntaxTypeHint> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn initializer(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxLetStmt {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LetStmt
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxReturnStmt {
    syntax: SyntaxNode,
}

impl SyntaxReturnStmt {
    #[must_use]
    pub fn expression(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxReturnStmt {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ReturnStmt
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxBreakStmt {
    syntax: SyntaxNode,
}

impl AstNode for SyntaxBreakStmt {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BreakStmt
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxContinueStmt {
    syntax: SyntaxNode,
}

impl AstNode for SyntaxContinueStmt {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ContinueStmt
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxExprStmt {
    syntax: SyntaxNode,
}

impl SyntaxExprStmt {
    #[must_use]
    pub fn expression(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxExprStmt {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ExprStmt
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxForStmt {
    syntax: SyntaxNode,
}

impl SyntaxForStmt {
    #[must_use]
    pub fn patterns(&self) -> AstChildren<SyntaxPattern> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn iterable(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn body(&self) -> Option<SyntaxBlock> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxForStmt {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ForStmt
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxIfExpr {
    syntax: SyntaxNode,
}

impl SyntaxIfExpr {
    #[must_use]
    pub fn condition(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn blocks(&self) -> AstChildren<SyntaxBlock> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn else_if(&self) -> Option<SyntaxIfExpr> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxIfExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IfExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

fn child<N: AstNode>(parent: &SyntaxNode) -> Option<N> {
    parent.children().find_map(N::cast)
}
