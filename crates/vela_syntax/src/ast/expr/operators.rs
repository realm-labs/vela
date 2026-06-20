use crate::ast::{AssignOp, AstChildren, AstNode, BinaryOp, SyntaxExpression, UnaryOp};
use crate::{SyntaxKind, SyntaxNode, SyntaxToken};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxAssignExpr {
    syntax: SyntaxNode,
}

impl SyntaxAssignExpr {
    #[must_use]
    pub fn expressions(&self) -> AstChildren<SyntaxExpression> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn target(&self) -> Option<SyntaxExpression> {
        self.expressions().next()
    }

    #[must_use]
    pub fn value(&self) -> Option<SyntaxExpression> {
        self.expressions().nth(1)
    }

    #[must_use]
    pub fn operator_token(&self) -> Option<SyntaxToken> {
        operator_token(&self.syntax, assign_operator_from_kind)
    }

    #[must_use]
    pub fn operator_kind(&self) -> Option<SyntaxKind> {
        self.operator_token().map(|token| token.kind())
    }

    #[must_use]
    pub fn operator(&self) -> Option<AssignOp> {
        self.operator_kind().and_then(assign_operator_from_kind)
    }
}

impl AstNode for SyntaxAssignExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::AssignExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxBinaryExpr {
    syntax: SyntaxNode,
}

impl SyntaxBinaryExpr {
    #[must_use]
    pub fn expressions(&self) -> AstChildren<SyntaxExpression> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn lhs(&self) -> Option<SyntaxExpression> {
        self.expressions().next()
    }

    #[must_use]
    pub fn rhs(&self) -> Option<SyntaxExpression> {
        self.expressions().nth(1)
    }

    #[must_use]
    pub fn operator_token(&self) -> Option<SyntaxToken> {
        operator_token(&self.syntax, binary_operator_from_kind)
    }

    #[must_use]
    pub fn operator_kind(&self) -> Option<SyntaxKind> {
        self.operator_token().map(|token| token.kind())
    }

    #[must_use]
    pub fn operator(&self) -> Option<BinaryOp> {
        self.operator_kind().and_then(binary_operator_from_kind)
    }
}

impl AstNode for SyntaxBinaryExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BinaryExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxUnaryExpr {
    syntax: SyntaxNode,
}

impl SyntaxUnaryExpr {
    #[must_use]
    pub fn expression(&self) -> Option<SyntaxExpression> {
        self.syntax.children().find_map(SyntaxExpression::cast)
    }

    #[must_use]
    pub fn operator_token(&self) -> Option<SyntaxToken> {
        operator_token(&self.syntax, unary_operator_from_kind)
    }

    #[must_use]
    pub fn operator_kind(&self) -> Option<SyntaxKind> {
        self.operator_token().map(|token| token.kind())
    }

    #[must_use]
    pub fn operator(&self) -> Option<UnaryOp> {
        self.operator_kind().and_then(unary_operator_from_kind)
    }
}

impl AstNode for SyntaxUnaryExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UnaryExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

fn operator_token<O>(
    parent: &SyntaxNode,
    operator_from_kind: impl Fn(SyntaxKind) -> Option<O>,
) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| operator_from_kind(token.kind()).is_some())
}

fn assign_operator_from_kind(kind: SyntaxKind) -> Option<AssignOp> {
    match kind {
        SyntaxKind::Equal => Some(AssignOp::Set),
        SyntaxKind::PlusEqual => Some(AssignOp::Add),
        SyntaxKind::MinusEqual => Some(AssignOp::Sub),
        SyntaxKind::StarEqual => Some(AssignOp::Mul),
        SyntaxKind::SlashEqual => Some(AssignOp::Div),
        SyntaxKind::PercentEqual => Some(AssignOp::Rem),
        _ => None,
    }
}

fn binary_operator_from_kind(kind: SyntaxKind) -> Option<BinaryOp> {
    match kind {
        SyntaxKind::OrOr => Some(BinaryOp::Or),
        SyntaxKind::AndAnd => Some(BinaryOp::And),
        SyntaxKind::EqualEqual => Some(BinaryOp::Equal),
        SyntaxKind::BangEqual => Some(BinaryOp::NotEqual),
        SyntaxKind::EqualEqualEqual => Some(BinaryOp::IdentityEqual),
        SyntaxKind::BangEqualEqual => Some(BinaryOp::IdentityNotEqual),
        SyntaxKind::Less => Some(BinaryOp::Less),
        SyntaxKind::LessEqual => Some(BinaryOp::LessEqual),
        SyntaxKind::Greater => Some(BinaryOp::Greater),
        SyntaxKind::GreaterEqual => Some(BinaryOp::GreaterEqual),
        SyntaxKind::DotDot => Some(BinaryOp::Range),
        SyntaxKind::DotDotEqual => Some(BinaryOp::RangeInclusive),
        SyntaxKind::Plus => Some(BinaryOp::Add),
        SyntaxKind::Minus => Some(BinaryOp::Sub),
        SyntaxKind::Star => Some(BinaryOp::Mul),
        SyntaxKind::Slash => Some(BinaryOp::Div),
        SyntaxKind::Percent => Some(BinaryOp::Rem),
        _ => None,
    }
}

fn unary_operator_from_kind(kind: SyntaxKind) -> Option<UnaryOp> {
    match kind {
        SyntaxKind::Bang => Some(UnaryOp::Not),
        SyntaxKind::Minus => Some(UnaryOp::Negate),
        _ => None,
    }
}
