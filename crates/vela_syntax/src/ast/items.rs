use super::expr::SyntaxExpression;
use super::{AstChildren, AstNode, SyntaxAttribute, SyntaxBlock, SyntaxTypeHint};
use crate::{SyntaxKind, SyntaxNode, SyntaxToken, TextRange};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxItem {
    syntax: SyntaxNode,
}

impl SyntaxItem {
    #[must_use]
    pub fn text_range(&self) -> TextRange {
        self.syntax.text_range()
    }

    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxItem {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::UseItem
                | SyntaxKind::ConstItem
                | SyntaxKind::GlobalItem
                | SyntaxKind::FunctionItem
                | SyntaxKind::StructItem
                | SyntaxKind::EnumItem
                | SyntaxKind::TraitItem
                | SyntaxKind::ImplItem
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
pub struct SyntaxUseItem {
    syntax: SyntaxNode,
}

impl SyntaxUseItem {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn path(&self) -> Option<SyntaxUsePath> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn alias_token(&self) -> Option<SyntaxToken> {
        token_after(&self.syntax, SyntaxKind::AsKw, SyntaxKind::Ident)
    }

    #[must_use]
    pub fn alias_text(&self) -> Option<String> {
        self.alias_token().map(|token| token.text().to_owned())
    }
}

impl AstNode for SyntaxUseItem {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UseItem
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxUsePath {
    syntax: SyntaxNode,
}

impl SyntaxUsePath {
    #[must_use]
    pub fn path_tokens(&self) -> Vec<SyntaxToken> {
        significant_tokens(&self.syntax).collect()
    }

    #[must_use]
    pub fn path_text(&self) -> Option<String> {
        token_text(self.path_tokens())
    }

    #[must_use]
    pub fn path_segments(&self) -> Vec<String> {
        path_segments_from_tokens(&self.path_tokens())
    }
}

impl AstNode for SyntaxUsePath {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UsePath
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxConstItem {
    syntax: SyntaxNode,
}

impl SyntaxConstItem {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token_after(&self.syntax, SyntaxKind::ConstKw, SyntaxKind::Ident)
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
    pub fn value(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxConstItem {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ConstItem
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxGlobalItem {
    syntax: SyntaxNode,
}

impl SyntaxGlobalItem {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token_after(&self.syntax, SyntaxKind::GlobalKw, SyntaxKind::Ident)
    }

    #[must_use]
    pub fn name_text(&self) -> Option<String> {
        self.name_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn type_hint(&self) -> Option<SyntaxTypeHint> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxGlobalItem {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::GlobalItem
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxFunctionItem {
    syntax: SyntaxNode,
}

impl SyntaxFunctionItem {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token_after(&self.syntax, SyntaxKind::FnKw, SyntaxKind::Ident)
    }

    #[must_use]
    pub fn name_text(&self) -> Option<String> {
        self.name_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn param_list(&self) -> Option<SyntaxParamList> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn return_type(&self) -> Option<SyntaxTypeHint> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn body(&self) -> Option<SyntaxBlock> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxFunctionItem {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FunctionItem
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxParamList {
    syntax: SyntaxNode,
}

impl SyntaxParamList {
    #[must_use]
    pub fn l_paren_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LParen)
    }

    #[must_use]
    pub fn r_paren_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RParen)
    }

    #[must_use]
    pub fn pipe_tokens(&self) -> Vec<SyntaxToken> {
        self.syntax
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| token.kind() == SyntaxKind::Pipe)
            .collect()
    }

    #[must_use]
    pub fn opening_pipe_token(&self) -> Option<SyntaxToken> {
        self.pipe_tokens().into_iter().next()
    }

    #[must_use]
    pub fn closing_pipe_token(&self) -> Option<SyntaxToken> {
        self.pipe_tokens().into_iter().nth(1)
    }

    #[must_use]
    pub fn params(&self) -> AstChildren<SyntaxParam> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn separator_tokens(&self) -> Vec<SyntaxToken> {
        separator_tokens(&self.syntax, SyntaxKind::Comma)
    }
}

impl AstNode for SyntaxParamList {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ParamList
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxParam {
    syntax: SyntaxNode,
}

impl SyntaxParam {
    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        first_significant_token(&self.syntax)
            .filter(|token| matches!(token.kind(), SyntaxKind::Ident | SyntaxKind::SelfKw))
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
    pub fn default_equal_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Equal)
    }

    #[must_use]
    pub fn default_value(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxParam {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Param
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxStructItem {
    syntax: SyntaxNode,
}

impl SyntaxStructItem {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token_after(&self.syntax, SyntaxKind::StructKw, SyntaxKind::Ident)
    }

    #[must_use]
    pub fn name_text(&self) -> Option<String> {
        self.name_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn field_list(&self) -> Option<SyntaxStructFieldList> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxStructItem {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::StructItem
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxStructFieldList {
    syntax: SyntaxNode,
}

impl SyntaxStructFieldList {
    #[must_use]
    pub fn fields(&self) -> AstChildren<SyntaxStructField> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxStructFieldList {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::StructFieldList
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxStructField {
    syntax: SyntaxNode,
}

impl SyntaxStructField {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        first_significant_token(&self.syntax).filter(|token| token.kind() == SyntaxKind::Ident)
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
    pub fn default_equal_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Equal)
    }

    #[must_use]
    pub fn default_value(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxStructField {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::StructField
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxEnumItem {
    syntax: SyntaxNode,
}

impl SyntaxEnumItem {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token_after(&self.syntax, SyntaxKind::EnumKw, SyntaxKind::Ident)
    }

    #[must_use]
    pub fn name_text(&self) -> Option<String> {
        self.name_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn variant_list(&self) -> Option<SyntaxEnumVariantList> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxEnumItem {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EnumItem
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxEnumVariantList {
    syntax: SyntaxNode,
}

impl SyntaxEnumVariantList {
    #[must_use]
    pub fn variants(&self) -> AstChildren<SyntaxEnumVariant> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxEnumVariantList {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EnumVariantList
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxEnumVariant {
    syntax: SyntaxNode,
}

impl SyntaxEnumVariant {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        first_significant_token(&self.syntax).filter(|token| token.kind() == SyntaxKind::Ident)
    }

    #[must_use]
    pub fn name_text(&self) -> Option<String> {
        self.name_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn tuple_field_list(&self) -> Option<SyntaxTupleFieldList> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn record_field_list(&self) -> Option<SyntaxRecordFieldList> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxEnumVariant {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EnumVariant
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTupleFieldList {
    syntax: SyntaxNode,
}

impl SyntaxTupleFieldList {
    #[must_use]
    pub fn params(&self) -> AstChildren<SyntaxParam> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxTupleFieldList {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TupleFieldList
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxRecordFieldList {
    syntax: SyntaxNode,
}

impl SyntaxRecordFieldList {
    #[must_use]
    pub fn fields(&self) -> AstChildren<SyntaxStructField> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxRecordFieldList {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RecordFieldList
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTraitItem {
    syntax: SyntaxNode,
}

impl SyntaxTraitItem {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token_after(&self.syntax, SyntaxKind::TraitKw, SyntaxKind::Ident)
    }

    #[must_use]
    pub fn name_text(&self) -> Option<String> {
        self.name_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn methods(&self) -> AstChildren<SyntaxTraitMethod> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxTraitItem {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TraitItem
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTraitMethod {
    syntax: SyntaxNode,
}

impl SyntaxTraitMethod {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token_after(&self.syntax, SyntaxKind::FnKw, SyntaxKind::Ident)
    }

    #[must_use]
    pub fn name_text(&self) -> Option<String> {
        self.name_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn param_list(&self) -> Option<SyntaxParamList> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn return_type(&self) -> Option<SyntaxTypeHint> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn body(&self) -> Option<SyntaxBlock> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxTraitMethod {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TraitMethod
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxImplItem {
    syntax: SyntaxNode,
}

impl SyntaxImplItem {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn impl_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::ImplKw)
    }

    #[must_use]
    pub fn for_token(&self) -> Option<SyntaxToken> {
        token_before(&self.syntax, SyntaxKind::ForKw, SyntaxKind::LBrace)
    }

    #[must_use]
    pub fn trait_path_tokens(&self) -> Vec<SyntaxToken> {
        if self.for_token().is_none() {
            return Vec::new();
        }
        header_path_tokens_between(&self.syntax, SyntaxKind::ImplKw, SyntaxKind::ForKw)
    }

    #[must_use]
    pub fn trait_path_text(&self) -> Option<String> {
        token_text(self.trait_path_tokens())
    }

    #[must_use]
    pub fn target_path_tokens(&self) -> Vec<SyntaxToken> {
        if self.for_token().is_some() {
            header_path_tokens_between(&self.syntax, SyntaxKind::ForKw, SyntaxKind::LBrace)
        } else {
            header_path_tokens_between(&self.syntax, SyntaxKind::ImplKw, SyntaxKind::LBrace)
        }
    }

    #[must_use]
    pub fn target_path_text(&self) -> Option<String> {
        token_text(self.target_path_tokens())
    }

    #[must_use]
    pub fn methods(&self) -> AstChildren<SyntaxImplMethod> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxImplItem {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ImplItem
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxImplMethod {
    syntax: SyntaxNode,
}

impl SyntaxImplMethod {
    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token_after(&self.syntax, SyntaxKind::FnKw, SyntaxKind::Ident)
    }

    #[must_use]
    pub fn name_text(&self) -> Option<String> {
        self.name_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn param_list(&self) -> Option<SyntaxParamList> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn return_type(&self) -> Option<SyntaxTypeHint> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn body(&self) -> Option<SyntaxBlock> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxImplMethod {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ImplMethod
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

fn first_significant_token(parent: &SyntaxNode) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
}

fn token(parent: &SyntaxNode, wanted: SyntaxKind) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == wanted)
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

fn header_path_tokens_between(
    parent: &SyntaxNode,
    after: SyntaxKind,
    before: SyntaxKind,
) -> Vec<SyntaxToken> {
    let mut seen_after = false;
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .take_while(|token| token.kind() != before)
        .filter(|token| {
            if token.kind() == after {
                seen_after = true;
                return false;
            }
            seen_after && !token.kind().is_trivia()
        })
        .collect()
}

fn token_text(tokens: Vec<SyntaxToken>) -> Option<String> {
    let mut text = String::new();
    for token in tokens {
        text.push_str(token.text());
    }
    (!text.is_empty()).then_some(text)
}

fn path_segments_from_tokens(tokens: &[SyntaxToken]) -> Vec<String> {
    tokens
        .iter()
        .filter(|token| token.kind() == SyntaxKind::Ident)
        .map(|token| token.text().to_owned())
        .collect()
}

fn separator_tokens(parent: &SyntaxNode, wanted: SyntaxKind) -> Vec<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == wanted)
        .collect()
}

fn significant_tokens(parent: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> + '_ {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
}
