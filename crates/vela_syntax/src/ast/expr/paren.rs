use crate::ast::{AstChildren, AstNode, SyntaxExpression};
use crate::{SyntaxKind, SyntaxNode, SyntaxToken};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxParenExpr {
    syntax: SyntaxNode,
}

impl SyntaxParenExpr {
    #[must_use]
    pub fn expression(&self) -> Option<SyntaxExpression> {
        self.syntax.children().find_map(SyntaxExpression::cast)
    }

    #[must_use]
    pub fn l_paren_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LParen)
    }

    #[must_use]
    pub fn r_paren_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RParen)
    }
}

impl AstNode for SyntaxParenExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ParenExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxUnitExpr {
    syntax: SyntaxNode,
}

impl SyntaxUnitExpr {
    #[must_use]
    pub fn l_paren_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LParen)
    }

    #[must_use]
    pub fn r_paren_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RParen)
    }
}

impl AstNode for SyntaxUnitExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UnitExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTupleExpr {
    syntax: SyntaxNode,
}

impl SyntaxTupleExpr {
    #[must_use]
    pub fn l_paren_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LParen)
    }

    #[must_use]
    pub fn r_paren_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RParen)
    }

    #[must_use]
    pub fn expressions(&self) -> AstChildren<SyntaxExpression> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn separator_tokens(&self) -> Vec<SyntaxToken> {
        self.syntax
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| token.kind() == SyntaxKind::Comma)
            .collect()
    }
}

impl AstNode for SyntaxTupleExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TupleExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

fn token(parent: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == kind)
}
