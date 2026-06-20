use std::marker::PhantomData;

use super::items::{
    SyntaxConstItem, SyntaxEnumItem, SyntaxFunctionItem, SyntaxGlobalItem, SyntaxImplItem,
    SyntaxItem, SyntaxStructItem, SyntaxTraitItem, SyntaxUseItem,
};
use super::statements::{SyntaxLetStmt, SyntaxStatement};
use crate::{SyntaxKind, SyntaxNode, SyntaxNodeChildren, SyntaxToken, TextRange};

pub trait AstNode {
    fn can_cast(kind: SyntaxKind) -> bool
    where
        Self: Sized;

    fn cast(syntax: SyntaxNode) -> Option<Self>
    where
        Self: Sized;

    fn syntax(&self) -> &SyntaxNode;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxSourceFile {
    syntax: SyntaxNode,
}

impl SyntaxSourceFile {
    #[must_use]
    pub fn text_range(&self) -> TextRange {
        self.syntax.text_range()
    }

    #[must_use]
    pub fn items(&self) -> AstChildren<SyntaxItem> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn functions(&self) -> AstChildren<SyntaxFunctionItem> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn structs(&self) -> AstChildren<SyntaxStructItem> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn enums(&self) -> AstChildren<SyntaxEnumItem> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn traits(&self) -> AstChildren<SyntaxTraitItem> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn impls(&self) -> AstChildren<SyntaxImplItem> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn uses(&self) -> AstChildren<SyntaxUseItem> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn consts(&self) -> AstChildren<SyntaxConstItem> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn globals(&self) -> AstChildren<SyntaxGlobalItem> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxSourceFile {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SourceFile
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Debug, Clone)]
pub struct AstChildren<N> {
    inner: SyntaxNodeChildren,
    _marker: PhantomData<N>,
}

impl<N> AstChildren<N> {
    pub(crate) fn new(parent: &SyntaxNode) -> Self {
        Self {
            inner: parent.children(),
            _marker: PhantomData,
        }
    }
}

impl<N: AstNode> Iterator for AstChildren<N> {
    type Item = N;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.find_map(N::cast)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTypeHint {
    syntax: SyntaxNode,
}

impl SyntaxTypeHint {
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
    pub fn type_arg_list(&self) -> Option<SyntaxTypeArgList> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxTypeHint {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TypeHint
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTypeArgList {
    syntax: SyntaxNode,
}

impl AstNode for SyntaxTypeArgList {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TypeArgList
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

impl SyntaxTypeArgList {
    #[must_use]
    pub fn less_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Less)
    }

    #[must_use]
    pub fn greater_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Greater)
    }

    #[must_use]
    pub fn type_hints(&self) -> AstChildren<SyntaxTypeHint> {
        AstChildren::new(&self.syntax)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxBlock {
    syntax: SyntaxNode,
}

impl SyntaxBlock {
    #[must_use]
    pub fn statements(&self) -> AstChildren<SyntaxStatement> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn let_statements(&self) -> AstChildren<SyntaxLetStmt> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxBlock {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Block
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

fn token(parent: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == kind)
}

#[cfg(test)]
mod statement_tests;

#[cfg(test)]
mod tests {
    use crate::ast::{AstNode, SyntaxSourceFile};
    use crate::{SyntaxKind, SyntaxTreeBuilder};

    #[test]
    fn ast_source_file_casts_from_source_file_root() {
        let mut builder = SyntaxTreeBuilder::default();
        builder.start_node(SyntaxKind::SourceFile);
        builder.token(SyntaxKind::Whitespace, "\n");
        builder.finish_node();

        let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
        let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");

        assert_eq!(source.syntax().kind(), SyntaxKind::SourceFile);
        assert_eq!(source.syntax().text().to_string(), "\n");
    }

    #[test]
    fn ast_source_file_rejects_non_source_file_root() {
        let mut builder = SyntaxTreeBuilder::default();
        builder.start_node(SyntaxKind::Block);
        builder.finish_node();

        let parse: crate::Parse<SyntaxSourceFile> = builder.finish();

        assert!(SyntaxSourceFile::cast(parse.syntax_node()).is_none());
    }
}
