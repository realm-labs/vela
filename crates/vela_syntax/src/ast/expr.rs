use super::control::SyntaxMatchExpr;
use super::statements::SyntaxIfExpr;
use super::{AstChildren, AstNode, SyntaxBlock, SyntaxParamList};
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

impl SyntaxExpression {
    #[must_use]
    pub fn expression_kind(&self) -> SyntaxExpressionKind {
        match self.syntax.kind() {
            SyntaxKind::Literal => SyntaxExpressionKind::Literal,
            SyntaxKind::PathExpr => SyntaxExpressionKind::Path,
            SyntaxKind::UnaryExpr => SyntaxExpressionKind::Unary,
            SyntaxKind::BinaryExpr => SyntaxExpressionKind::Binary,
            SyntaxKind::AssignExpr => SyntaxExpressionKind::Assign,
            SyntaxKind::FieldExpr => SyntaxExpressionKind::Field,
            SyntaxKind::CallExpr => SyntaxExpressionKind::Call,
            SyntaxKind::IndexExpr => SyntaxExpressionKind::Index,
            SyntaxKind::TryExpr => SyntaxExpressionKind::Try,
            SyntaxKind::ArrayExpr => SyntaxExpressionKind::Array,
            SyntaxKind::MapExpr => SyntaxExpressionKind::Map,
            SyntaxKind::RecordExpr => SyntaxExpressionKind::Record,
            SyntaxKind::LambdaExpr => SyntaxExpressionKind::Lambda,
            SyntaxKind::Block => SyntaxExpressionKind::Block,
            SyntaxKind::IfExpr => SyntaxExpressionKind::If,
            SyntaxKind::MatchExpr => SyntaxExpressionKind::Match,
            kind => unreachable!("non-expression syntax kind: {kind:?}"),
        }
    }

    #[must_use]
    pub fn as_literal(&self) -> Option<SyntaxLiteral> {
        SyntaxLiteral::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_path(&self) -> Option<SyntaxPathExpr> {
        SyntaxPathExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_unary(&self) -> Option<SyntaxUnaryExpr> {
        SyntaxUnaryExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_binary(&self) -> Option<SyntaxBinaryExpr> {
        SyntaxBinaryExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_assign(&self) -> Option<SyntaxAssignExpr> {
        SyntaxAssignExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_field(&self) -> Option<SyntaxFieldExpr> {
        SyntaxFieldExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_call(&self) -> Option<SyntaxCallExpr> {
        SyntaxCallExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_index(&self) -> Option<SyntaxIndexExpr> {
        SyntaxIndexExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_try(&self) -> Option<SyntaxTryExpr> {
        SyntaxTryExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_array(&self) -> Option<SyntaxArrayExpr> {
        SyntaxArrayExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_map(&self) -> Option<SyntaxMapExpr> {
        SyntaxMapExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_record(&self) -> Option<SyntaxRecordExpr> {
        SyntaxRecordExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_lambda(&self) -> Option<SyntaxLambdaExpr> {
        SyntaxLambdaExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_block(&self) -> Option<SyntaxBlock> {
        SyntaxBlock::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_if(&self) -> Option<SyntaxIfExpr> {
        SyntaxIfExpr::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_match(&self) -> Option<SyntaxMatchExpr> {
        SyntaxMatchExpr::cast(self.syntax.clone())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxExpressionKind {
    Literal,
    Path,
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

    #[must_use]
    pub fn self_token(&self) -> Option<SyntaxToken> {
        let mut tokens = self.path_tokens().into_iter();
        let token = tokens.next()?;
        (tokens.next().is_none() && token.kind() == SyntaxKind::SelfKw).then_some(token)
    }

    #[must_use]
    pub fn is_self(&self) -> bool {
        self.self_token().is_some()
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

    #[must_use]
    pub fn separator_tokens(&self) -> Vec<SyntaxToken> {
        separator_tokens(&self.syntax, SyntaxKind::Comma)
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

    #[must_use]
    pub fn separator_tokens(&self) -> Vec<SyntaxToken> {
        separator_tokens(&self.syntax, SyntaxKind::Comma)
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

    #[must_use]
    pub fn separator_tokens(&self) -> Vec<SyntaxToken> {
        separator_tokens(&self.syntax, SyntaxKind::Comma)
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

    #[must_use]
    pub fn separator_tokens(&self) -> Vec<SyntaxToken> {
        separator_tokens(&self.syntax, SyntaxKind::Comma)
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

fn separator_tokens(parent: &SyntaxNode, wanted: SyntaxKind) -> Vec<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == wanted)
        .collect()
}

fn first_significant_token(parent: &SyntaxNode) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
}
