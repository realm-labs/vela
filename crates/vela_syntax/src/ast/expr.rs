use super::{AstChildren, AstNode, SyntaxBlock, SyntaxParamList, SyntaxPattern};
use crate::{SyntaxKind, SyntaxNode, SyntaxToken};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxExpression {
    syntax: SyntaxNode,
}

impl AstNode for SyntaxExpression {
    fn can_cast(kind: SyntaxKind) -> bool {
        expression_kind(kind)
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxLiteral {
    syntax: SyntaxNode,
}

impl SyntaxLiteral {
    #[must_use]
    pub fn token(&self) -> Option<SyntaxToken> {
        self.syntax
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| literal_token_kind(token.kind()))
    }

    #[must_use]
    pub fn token_kind(&self) -> Option<SyntaxKind> {
        self.token().map(|token| token.kind())
    }

    #[must_use]
    pub fn token_text(&self) -> Option<String> {
        self.token().map(|token| token.text().to_owned())
    }
}

impl AstNode for SyntaxLiteral {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Literal
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxPathExpr {
    syntax: SyntaxNode,
}

impl AstNode for SyntaxPathExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PathExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

impl SyntaxPathExpr {
    #[must_use]
    pub fn path_tokens(&self) -> Vec<SyntaxToken> {
        self.syntax
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| !token.kind().is_trivia())
            .collect()
    }

    #[must_use]
    pub fn path_text(&self) -> Option<String> {
        let mut text = String::new();
        for token in self.path_tokens() {
            text.push_str(token.text());
        }
        (!text.is_empty()).then_some(text)
    }
}

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
        self.syntax
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| assignment_operator_kind(token.kind()))
    }

    #[must_use]
    pub fn operator_kind(&self) -> Option<SyntaxKind> {
        self.operator_token().map(|token| token.kind())
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
        self.syntax
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| binary_operator_kind(token.kind()))
    }

    #[must_use]
    pub fn operator_kind(&self) -> Option<SyntaxKind> {
        self.operator_token().map(|token| token.kind())
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
        child(&self.syntax)
    }

    #[must_use]
    pub fn operator_token(&self) -> Option<SyntaxToken> {
        self.syntax
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| unary_operator_kind(token.kind()))
    }

    #[must_use]
    pub fn operator_kind(&self) -> Option<SyntaxKind> {
        self.operator_token().map(|token| token.kind())
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxFieldExpr {
    syntax: SyntaxNode,
}

impl SyntaxFieldExpr {
    #[must_use]
    pub fn receiver(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn dot_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Dot)
    }

    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        let mut past_dot = false;
        self.syntax
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| {
                if token.kind() == SyntaxKind::Dot {
                    past_dot = true;
                    return false;
                }
                past_dot && token.kind() == SyntaxKind::Ident
            })
    }

    #[must_use]
    pub fn name_text(&self) -> Option<String> {
        self.name_token().map(|token| token.text().to_owned())
    }
}

impl AstNode for SyntaxFieldExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FieldExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxCallExpr {
    syntax: SyntaxNode,
}

impl SyntaxCallExpr {
    #[must_use]
    pub fn callee(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn arg_list(&self) -> Option<SyntaxArgList> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxCallExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CallExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxIndexExpr {
    syntax: SyntaxNode,
}

impl SyntaxIndexExpr {
    #[must_use]
    pub fn expressions(&self) -> AstChildren<SyntaxExpression> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn l_bracket_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LBracket)
    }

    #[must_use]
    pub fn r_bracket_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RBracket)
    }

    #[must_use]
    pub fn receiver(&self) -> Option<SyntaxExpression> {
        self.expressions().next()
    }

    #[must_use]
    pub fn index(&self) -> Option<SyntaxExpression> {
        self.expressions().nth(1)
    }
}

impl AstNode for SyntaxIndexExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IndexExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTryExpr {
    syntax: SyntaxNode,
}

impl SyntaxTryExpr {
    #[must_use]
    pub fn expression(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn question_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Question)
    }
}

impl AstNode for SyntaxTryExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TryExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxArgList {
    syntax: SyntaxNode,
}

impl SyntaxArgList {
    #[must_use]
    pub fn l_paren_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LParen)
    }

    #[must_use]
    pub fn r_paren_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RParen)
    }

    #[must_use]
    pub fn arguments(&self) -> AstChildren<SyntaxArgument> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxArgList {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ArgList
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxArgument {
    syntax: SyntaxNode,
}

impl SyntaxArgument {
    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        first_significant_token(&self.syntax)
            .filter(|token| token.kind() == SyntaxKind::Ident && self.equal_token().is_some())
    }

    #[must_use]
    pub fn name_text(&self) -> Option<String> {
        self.name_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn equal_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Equal)
    }

    #[must_use]
    pub fn expression(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxArgument {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Argument
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxArrayExpr {
    syntax: SyntaxNode,
}

impl SyntaxArrayExpr {
    #[must_use]
    pub fn l_bracket_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LBracket)
    }

    #[must_use]
    pub fn r_bracket_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RBracket)
    }

    #[must_use]
    pub fn expressions(&self) -> AstChildren<SyntaxExpression> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxArrayExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ArrayExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxMapExpr {
    syntax: SyntaxNode,
}

impl SyntaxMapExpr {
    #[must_use]
    pub fn l_brace_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LBrace)
    }

    #[must_use]
    pub fn r_brace_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RBrace)
    }

    #[must_use]
    pub fn entries(&self) -> AstChildren<SyntaxMapEntry> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxMapExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MapExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxMapEntry {
    syntax: SyntaxNode,
}

impl SyntaxMapEntry {
    #[must_use]
    pub fn expressions(&self) -> AstChildren<SyntaxExpression> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn key(&self) -> Option<SyntaxExpression> {
        self.expressions().next()
    }

    #[must_use]
    pub fn value(&self) -> Option<SyntaxExpression> {
        self.expressions().nth(1)
    }

    #[must_use]
    pub fn colon_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Colon)
    }
}

impl AstNode for SyntaxMapEntry {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MapEntry
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxRecordExpr {
    syntax: SyntaxNode,
}

impl SyntaxRecordExpr {
    #[must_use]
    pub fn path(&self) -> Option<SyntaxPathExpr> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn field_list(&self) -> Option<SyntaxRecordExprFieldList> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxRecordExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RecordExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxRecordExprFieldList {
    syntax: SyntaxNode,
}

impl SyntaxRecordExprFieldList {
    #[must_use]
    pub fn l_brace_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LBrace)
    }

    #[must_use]
    pub fn r_brace_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RBrace)
    }

    #[must_use]
    pub fn fields(&self) -> AstChildren<SyntaxRecordExprField> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxRecordExprFieldList {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RecordExprFieldList
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxRecordExprField {
    syntax: SyntaxNode,
}

impl SyntaxRecordExprField {
    #[must_use]
    pub fn label_token(&self) -> Option<SyntaxToken> {
        first_significant_token(&self.syntax).filter(|token| token.kind() == SyntaxKind::Ident)
    }

    #[must_use]
    pub fn label_kind(&self) -> Option<SyntaxKind> {
        self.label_token().map(|token| token.kind())
    }

    #[must_use]
    pub fn label_text(&self) -> Option<String> {
        self.label_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn colon_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Colon)
    }

    #[must_use]
    pub fn expression(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn is_shorthand(&self) -> bool {
        self.label_token().is_some() && self.colon_token().is_none() && self.expression().is_none()
    }
}

impl AstNode for SyntaxRecordExprField {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RecordExprField
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxLambdaExpr {
    syntax: SyntaxNode,
}

impl SyntaxLambdaExpr {
    #[must_use]
    pub fn param_list(&self) -> Option<SyntaxParamList> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn body_expression(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn body_block(&self) -> Option<SyntaxBlock> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxLambdaExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LambdaExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

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
    pub fn body_expression(&self) -> Option<SyntaxExpression> {
        if self.body_block().is_some() {
            return None;
        }
        let mut expressions = self.expressions();
        if self.has_guard() {
            expressions.next();
        }
        expressions.next()
    }

    #[must_use]
    pub fn body_block(&self) -> Option<SyntaxBlock> {
        child(&self.syntax)
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

fn expression_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Literal
            | SyntaxKind::PathExpr
            | SyntaxKind::UnaryExpr
            | SyntaxKind::BinaryExpr
            | SyntaxKind::AssignExpr
            | SyntaxKind::FieldExpr
            | SyntaxKind::CallExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::TryExpr
            | SyntaxKind::ArrayExpr
            | SyntaxKind::MapExpr
            | SyntaxKind::RecordExpr
            | SyntaxKind::LambdaExpr
            | SyntaxKind::Block
            | SyntaxKind::IfExpr
            | SyntaxKind::MatchExpr
    )
}

fn binary_operator_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::OrOr
            | SyntaxKind::AndAnd
            | SyntaxKind::EqualEqual
            | SyntaxKind::BangEqual
            | SyntaxKind::EqualEqualEqual
            | SyntaxKind::BangEqualEqual
            | SyntaxKind::Less
            | SyntaxKind::LessEqual
            | SyntaxKind::Greater
            | SyntaxKind::GreaterEqual
            | SyntaxKind::DotDot
            | SyntaxKind::DotDotEqual
            | SyntaxKind::Plus
            | SyntaxKind::Minus
            | SyntaxKind::Star
            | SyntaxKind::Slash
            | SyntaxKind::Percent
    )
}

fn assignment_operator_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Equal
            | SyntaxKind::PlusEqual
            | SyntaxKind::MinusEqual
            | SyntaxKind::StarEqual
            | SyntaxKind::SlashEqual
            | SyntaxKind::PercentEqual
    )
}

fn unary_operator_kind(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Bang | SyntaxKind::Minus)
}

fn literal_token_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::TrueKw
            | SyntaxKind::FalseKw
            | SyntaxKind::NullKw
            | SyntaxKind::Int
            | SyntaxKind::Float
            | SyntaxKind::Char
            | SyntaxKind::String
            | SyntaxKind::InterpolatedString
            | SyntaxKind::Bytes
    )
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

fn first_significant_token(parent: &SyntaxNode) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
}
