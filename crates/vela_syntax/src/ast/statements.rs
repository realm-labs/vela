use super::control::SyntaxMatchExpr;
use super::expr::SyntaxExpression;
use super::{AstChildren, AstNode, SyntaxAttribute, SyntaxBlock, SyntaxPattern, SyntaxTypeHint};
use crate::{SyntaxKind, SyntaxNode, SyntaxToken};

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
                | SyntaxKind::Block
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

impl SyntaxStatement {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn statement_kind(&self) -> SyntaxStatementKind {
        match self.syntax.kind() {
            SyntaxKind::LetStmt => SyntaxStatementKind::Let,
            SyntaxKind::ReturnStmt => SyntaxStatementKind::Return,
            SyntaxKind::BreakStmt => SyntaxStatementKind::Break,
            SyntaxKind::ContinueStmt => SyntaxStatementKind::Continue,
            SyntaxKind::ForStmt => SyntaxStatementKind::For,
            SyntaxKind::IfExpr => SyntaxStatementKind::If,
            SyntaxKind::MatchExpr => SyntaxStatementKind::Match,
            SyntaxKind::Block => SyntaxStatementKind::Block,
            SyntaxKind::ExprStmt => SyntaxStatementKind::Expr,
            kind => unreachable!("non-statement syntax kind: {kind:?}"),
        }
    }

    #[must_use]
    pub fn as_let(&self) -> Option<SyntaxLetStmt> {
        SyntaxLetStmt::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_return(&self) -> Option<SyntaxReturnStmt> {
        SyntaxReturnStmt::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_break(&self) -> Option<SyntaxBreakStmt> {
        SyntaxBreakStmt::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_continue(&self) -> Option<SyntaxContinueStmt> {
        SyntaxContinueStmt::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_for(&self) -> Option<SyntaxForStmt> {
        SyntaxForStmt::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_if(&self) -> Option<SyntaxIfExpr> {
        SyntaxIfExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_match(&self) -> Option<SyntaxMatchExpr> {
        SyntaxMatchExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_block(&self) -> Option<SyntaxBlock> {
        SyntaxBlock::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_expr(&self) -> Option<SyntaxExprStmt> {
        SyntaxExprStmt::cast(self.syntax.clone())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxStatementKind {
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
pub struct SyntaxLetStmt {
    syntax: SyntaxNode,
}

impl SyntaxLetStmt {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn let_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LetKw)
    }

    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token_after(&self.syntax, SyntaxKind::LetKw, SyntaxKind::Ident)
    }

    #[must_use]
    pub fn name_text(&self) -> Option<String> {
        self.name_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn type_hint(&self) -> Option<SyntaxTypeHint> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn initializer(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn semicolon_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Semicolon)
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
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn return_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::ReturnKw)
    }

    #[must_use]
    pub fn expression(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn semicolon_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Semicolon)
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

impl SyntaxBreakStmt {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn break_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::BreakKw)
    }

    #[must_use]
    pub fn semicolon_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Semicolon)
    }
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

impl SyntaxContinueStmt {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn continue_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::ContinueKw)
    }

    #[must_use]
    pub fn semicolon_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Semicolon)
    }
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
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn expression(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn semicolon_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Semicolon)
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
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn for_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::ForKw)
    }

    #[must_use]
    pub fn in_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::InKw)
    }

    #[must_use]
    pub fn binding_separator_token(&self) -> Option<SyntaxToken> {
        token_before(&self.syntax, SyntaxKind::Comma, SyntaxKind::InKw)
    }

    #[must_use]
    pub fn patterns(&self) -> AstChildren<SyntaxPattern> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn index_pattern(&self) -> Option<SyntaxPattern> {
        let mut patterns = self.patterns();
        let first = patterns.next()?;
        patterns.next().map(|_| first)
    }

    #[must_use]
    pub fn value_pattern(&self) -> Option<SyntaxPattern> {
        self.patterns().last()
    }

    #[must_use]
    pub fn iterable(&self) -> Option<SyntaxExpression> {
        let in_end = token(&self.syntax, SyntaxKind::InKw)?.text_range().end();
        let body_start = self.body()?.syntax().text_range().start();
        self.syntax.children().find_map(|node| {
            let expression = SyntaxExpression::cast(node)?;
            let range = expression.syntax().text_range();
            (range.start() >= in_end && range.end() <= body_start).then_some(expression)
        })
    }

    #[must_use]
    pub fn body(&self) -> Option<SyntaxBlock> {
        self.syntax.children().filter_map(SyntaxBlock::cast).last()
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
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn if_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::IfKw)
    }

    #[must_use]
    pub fn else_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::ElseKw)
    }

    #[must_use]
    pub fn else_if_else_token(&self) -> Option<SyntaxToken> {
        self.else_if().and_then(|_| self.else_token())
    }

    #[must_use]
    pub fn else_block_else_token(&self) -> Option<SyntaxToken> {
        self.else_block().and_then(|_| self.else_token())
    }

    #[must_use]
    pub fn condition(&self) -> Option<SyntaxExpression> {
        let if_end = token(&self.syntax, SyntaxKind::IfKw)?.text_range().end();
        let then_start = self.then_block()?.syntax().text_range().start();
        self.syntax.children().find_map(|node| {
            let expression = SyntaxExpression::cast(node)?;
            let range = expression.syntax().text_range();
            (range.start() >= if_end && range.end() <= then_start).then_some(expression)
        })
    }

    #[must_use]
    pub fn then_block(&self) -> Option<SyntaxBlock> {
        self.blocks().next()
    }

    #[must_use]
    pub fn blocks(&self) -> AstChildren<SyntaxBlock> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn else_block(&self) -> Option<SyntaxBlock> {
        if self.else_if().is_some() {
            return None;
        }
        self.blocks().nth(1)
    }

    #[must_use]
    pub fn else_branch(&self) -> Option<SyntaxElseBranch> {
        if let Some(if_expr) = self.else_if() {
            return Some(SyntaxElseBranch::If(if_expr));
        }
        self.else_block().map(SyntaxElseBranch::Block)
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxElseBranch {
    If(SyntaxIfExpr),
    Block(SyntaxBlock),
}

fn child<N: AstNode>(parent: &SyntaxNode) -> Option<N> {
    parent.children().find_map(N::cast)
}

fn token(parent: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == kind)
}

fn token_after(parent: &SyntaxNode, after: SyntaxKind, wanted: SyntaxKind) -> Option<SyntaxToken> {
    let mut seen_after = false;
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| {
            if token.kind() == after {
                seen_after = true;
                return false;
            }
            seen_after && token.kind() == wanted
        })
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
