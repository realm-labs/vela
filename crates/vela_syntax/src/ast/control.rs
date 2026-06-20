use super::{AstChildren, AstNode, SyntaxBlock, SyntaxExpression, SyntaxPattern};
use crate::{SyntaxKind, SyntaxNode, SyntaxToken};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxMatchExpr {
    syntax: SyntaxNode,
}

impl SyntaxMatchExpr {
    #[must_use]
    pub fn match_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::MatchKw)
    }

    #[must_use]
    pub fn scrutinee(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn arm_list(&self) -> Option<SyntaxMatchArmList> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn l_brace_token(&self) -> Option<SyntaxToken> {
        self.arm_list()?.l_brace_token()
    }

    #[must_use]
    pub fn r_brace_token(&self) -> Option<SyntaxToken> {
        self.arm_list()?.r_brace_token()
    }

    #[must_use]
    pub fn arms(&self) -> Vec<SyntaxMatchArm> {
        self.arm_list()
            .map(|arm_list| arm_list.arms().collect())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn separator_tokens(&self) -> Vec<SyntaxToken> {
        self.arm_list()
            .map(|arm_list| arm_list.separator_tokens())
            .unwrap_or_default()
    }
}

impl AstNode for SyntaxMatchExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MatchExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxMatchArmList {
    syntax: SyntaxNode,
}

impl SyntaxMatchArmList {
    #[must_use]
    pub fn l_brace_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LBrace)
    }

    #[must_use]
    pub fn r_brace_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RBrace)
    }

    #[must_use]
    pub fn arms(&self) -> AstChildren<SyntaxMatchArm> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn separator_tokens(&self) -> Vec<SyntaxToken> {
        self.syntax
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| matches!(token.kind(), SyntaxKind::Comma | SyntaxKind::Semicolon))
            .collect()
    }
}

impl AstNode for SyntaxMatchArmList {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MatchArmList
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxMatchArm {
    syntax: SyntaxNode,
}

impl SyntaxMatchArm {
    #[must_use]
    pub fn pattern(&self) -> Option<SyntaxPattern> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn guard_if_token(&self) -> Option<SyntaxToken> {
        token_before(&self.syntax, SyntaxKind::IfKw, SyntaxKind::FatArrow)
    }

    #[must_use]
    pub fn guard(&self) -> Option<SyntaxExpression> {
        self.has_guard()
            .then(|| self.expressions().next())
            .flatten()
    }

    #[must_use]
    pub fn fat_arrow_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::FatArrow)
    }

    #[must_use]
    pub fn expressions(&self) -> AstChildren<SyntaxExpression> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn body(&self) -> Option<SyntaxMatchArmBody> {
        self.body_block()
            .map(SyntaxMatchArmBody::Block)
            .or_else(|| self.body_expression().map(SyntaxMatchArmBody::Expression))
    }

    #[must_use]
    pub fn body_expression(&self) -> Option<SyntaxExpression> {
        child_after_token(&self.syntax, SyntaxKind::FatArrow)
            .filter(|expression: &SyntaxExpression| expression.syntax().kind() != SyntaxKind::Block)
    }

    #[must_use]
    pub fn body_block(&self) -> Option<SyntaxBlock> {
        child_after_token(&self.syntax, SyntaxKind::FatArrow)
    }

    fn has_guard(&self) -> bool {
        self.syntax
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .take_while(|token| token.kind() != SyntaxKind::FatArrow)
            .any(|token| token.kind() == SyntaxKind::IfKw)
    }
}

impl AstNode for SyntaxMatchArm {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MatchArm
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxMatchArmBody {
    Expression(SyntaxExpression),
    Block(SyntaxBlock),
}

fn child<N: AstNode>(parent: &SyntaxNode) -> Option<N> {
    parent.children().find_map(N::cast)
}

fn child_after_token<N: AstNode>(parent: &SyntaxNode, after: SyntaxKind) -> Option<N> {
    let mut seen_after = false;
    for element in parent.children_with_tokens() {
        if let Some(token) = element.as_token() {
            if token.kind() == after {
                seen_after = true;
            }
            continue;
        }
        if !seen_after {
            continue;
        }
        if let Some(node) = element.into_node().and_then(N::cast) {
            return Some(node);
        }
    }
    None
}

fn token(parent: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == kind)
}

fn token_before(
    parent: &SyntaxNode,
    wanted: SyntaxKind,
    before: SyntaxKind,
) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .take_while(|token| token.kind() != before)
        .find(|token| token.kind() == wanted)
}
