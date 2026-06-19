use std::marker::PhantomData;

use crate::{SyntaxKind, SyntaxNode, SyntaxNodeChildren, TextRange};

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
    fn new(parent: &SyntaxNode) -> Self {
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
pub struct SyntaxItem {
    syntax: SyntaxNode,
}

impl SyntaxItem {
    #[must_use]
    pub fn text_range(&self) -> TextRange {
        self.syntax.text_range()
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
pub struct SyntaxFunctionItem {
    syntax: SyntaxNode,
}

impl SyntaxFunctionItem {
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
    pub fn params(&self) -> AstChildren<SyntaxParam> {
        AstChildren::new(&self.syntax)
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
    pub fn type_hint(&self) -> Option<SyntaxTypeHint> {
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
pub struct SyntaxTypeHint {
    syntax: SyntaxNode,
}

impl SyntaxTypeHint {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxBlock {
    syntax: SyntaxNode,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxStructItem {
    syntax: SyntaxNode,
}

impl SyntaxStructItem {
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
    pub fn type_hint(&self) -> Option<SyntaxTypeHint> {
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

fn child<N: AstNode>(parent: &SyntaxNode) -> Option<N> {
    parent.children().find_map(N::cast)
}

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

    #[test]
    fn ast_source_file_iterates_item_children() {
        let mut builder = SyntaxTreeBuilder::default();
        builder.start_node(SyntaxKind::SourceFile);
        builder.start_node(SyntaxKind::FunctionItem);
        builder.token(SyntaxKind::FnKw, "fn");
        builder.finish_node();
        builder.token(SyntaxKind::Whitespace, "\n");
        builder.start_node(SyntaxKind::StructItem);
        builder.token(SyntaxKind::StructKw, "struct");
        builder.finish_node();
        builder.finish_node();

        let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
        let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");

        assert_eq!(
            source
                .items()
                .map(|item| item.syntax().kind())
                .collect::<Vec<_>>(),
            vec![SyntaxKind::FunctionItem, SyntaxKind::StructItem]
        );
    }

    #[test]
    fn ast_function_item_exposes_signature_and_body_children() {
        let mut builder = SyntaxTreeBuilder::default();
        builder.start_node(SyntaxKind::SourceFile);
        builder.start_node(SyntaxKind::FunctionItem);
        builder.token(SyntaxKind::FnKw, "fn");
        builder.start_node(SyntaxKind::ParamList);
        builder.token(SyntaxKind::LParen, "(");
        builder.start_node(SyntaxKind::Param);
        builder.token(SyntaxKind::Ident, "ctx");
        builder.finish_node();
        builder.token(SyntaxKind::RParen, ")");
        builder.finish_node();
        builder.start_node(SyntaxKind::Block);
        builder.token(SyntaxKind::LBrace, "{");
        builder.token(SyntaxKind::RBrace, "}");
        builder.finish_node();
        builder.finish_node();
        builder.finish_node();

        let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
        let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");
        let function = source.functions().next().expect("function item");

        assert_eq!(
            function
                .param_list()
                .expect("param list")
                .params()
                .map(|param| param.syntax().text().to_string())
                .collect::<Vec<_>>(),
            vec!["ctx"]
        );
        assert_eq!(
            function.body().expect("body").syntax().text().to_string(),
            "{}"
        );
    }

    #[test]
    fn ast_function_signature_exposes_type_hint_children() {
        let mut builder = SyntaxTreeBuilder::default();
        builder.start_node(SyntaxKind::SourceFile);
        builder.start_node(SyntaxKind::FunctionItem);
        builder.start_node(SyntaxKind::ParamList);
        builder.token(SyntaxKind::LParen, "(");
        builder.start_node(SyntaxKind::Param);
        builder.token(SyntaxKind::Ident, "items");
        builder.token(SyntaxKind::Colon, ":");
        builder.start_node(SyntaxKind::TypeHint);
        builder.token(SyntaxKind::Ident, "Array");
        builder.start_node(SyntaxKind::TypeArgList);
        builder.token(SyntaxKind::Less, "<");
        builder.token(SyntaxKind::Ident, "String");
        builder.token(SyntaxKind::Greater, ">");
        builder.finish_node();
        builder.finish_node();
        builder.finish_node();
        builder.token(SyntaxKind::RParen, ")");
        builder.finish_node();
        builder.token(SyntaxKind::Arrow, "->");
        builder.start_node(SyntaxKind::TypeHint);
        builder.token(SyntaxKind::Ident, "Result");
        builder.finish_node();
        builder.finish_node();
        builder.finish_node();

        let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
        let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");
        let function = source.functions().next().expect("function item");
        let param = function
            .param_list()
            .expect("param list")
            .params()
            .next()
            .expect("param");

        let hint = param.type_hint().expect("param type hint");
        assert_eq!(hint.syntax().text().to_string(), "Array<String>");
        assert_eq!(
            hint.type_arg_list()
                .expect("type arg list")
                .syntax()
                .text()
                .to_string(),
            "<String>"
        );
        assert_eq!(
            function
                .return_type()
                .expect("return type")
                .syntax()
                .text()
                .to_string(),
            "Result"
        );
    }

    #[test]
    fn ast_struct_item_exposes_field_children() {
        let mut builder = SyntaxTreeBuilder::default();
        builder.start_node(SyntaxKind::SourceFile);
        builder.start_node(SyntaxKind::StructItem);
        builder.token(SyntaxKind::StructKw, "struct");
        builder.start_node(SyntaxKind::StructFieldList);
        builder.token(SyntaxKind::LBrace, "{");
        builder.start_node(SyntaxKind::StructField);
        builder.token(SyntaxKind::Ident, "items");
        builder.token(SyntaxKind::Colon, ":");
        builder.start_node(SyntaxKind::TypeHint);
        builder.token(SyntaxKind::Ident, "Array");
        builder.start_node(SyntaxKind::TypeArgList);
        builder.token(SyntaxKind::Less, "<");
        builder.token(SyntaxKind::Ident, "String");
        builder.token(SyntaxKind::Greater, ">");
        builder.finish_node();
        builder.finish_node();
        builder.finish_node();
        builder.token(SyntaxKind::Comma, ",");
        builder.start_node(SyntaxKind::StructField);
        builder.token(SyntaxKind::Ident, "count");
        builder.finish_node();
        builder.token(SyntaxKind::RBrace, "}");
        builder.finish_node();
        builder.finish_node();
        builder.finish_node();

        let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
        let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");
        let record = source.structs().next().expect("struct item");
        let fields = record
            .field_list()
            .expect("field list")
            .fields()
            .collect::<Vec<_>>();

        assert_eq!(
            fields
                .iter()
                .map(|field| field.syntax().text().to_string())
                .collect::<Vec<_>>(),
            vec!["items:Array<String>", "count"]
        );
        let hint = fields[0].type_hint().expect("field type hint");
        assert_eq!(hint.syntax().text().to_string(), "Array<String>");
        assert_eq!(
            hint.type_arg_list()
                .expect("field type args")
                .syntax()
                .text()
                .to_string(),
            "<String>"
        );
        assert!(fields[1].type_hint().is_none());
    }
}
