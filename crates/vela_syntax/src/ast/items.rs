use super::expr::SyntaxExpression;
use super::{AstChildren, AstNode, SyntaxBlock, SyntaxTypeHint};
use crate::{SyntaxKind, SyntaxNode, TextRange};

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
pub struct SyntaxUseItem {
    syntax: SyntaxNode,
}

impl SyntaxUseItem {
    #[must_use]
    pub fn path(&self) -> Option<SyntaxUsePath> {
        child(&self.syntax)
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxEnumItem {
    syntax: SyntaxNode,
}

impl SyntaxEnumItem {
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

#[cfg(test)]
mod tests {
    use crate::ast::{AstNode, SyntaxSourceFile};
    use crate::{SyntaxKind, SyntaxTreeBuilder};

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

    #[test]
    fn ast_enum_item_exposes_variant_children() {
        let mut builder = SyntaxTreeBuilder::default();
        builder.start_node(SyntaxKind::SourceFile);
        builder.start_node(SyntaxKind::EnumItem);
        builder.token(SyntaxKind::EnumKw, "enum");
        builder.start_node(SyntaxKind::EnumVariantList);
        builder.token(SyntaxKind::LBrace, "{");
        builder.start_node(SyntaxKind::EnumVariant);
        builder.token(SyntaxKind::Ident, "Finished");
        builder.start_node(SyntaxKind::TupleFieldList);
        builder.token(SyntaxKind::LParen, "(");
        builder.start_node(SyntaxKind::Param);
        builder.token(SyntaxKind::Ident, "reward");
        builder.token(SyntaxKind::Colon, ":");
        builder.start_node(SyntaxKind::TypeHint);
        builder.token(SyntaxKind::Ident, "Option");
        builder.start_node(SyntaxKind::TypeArgList);
        builder.token(SyntaxKind::Less, "<");
        builder.token(SyntaxKind::Ident, "String");
        builder.token(SyntaxKind::Greater, ">");
        builder.finish_node();
        builder.finish_node();
        builder.finish_node();
        builder.token(SyntaxKind::RParen, ")");
        builder.finish_node();
        builder.finish_node();
        builder.token(SyntaxKind::Comma, ",");
        builder.start_node(SyntaxKind::EnumVariant);
        builder.token(SyntaxKind::Ident, "Active");
        builder.start_node(SyntaxKind::RecordFieldList);
        builder.token(SyntaxKind::LBrace, "{");
        builder.start_node(SyntaxKind::StructField);
        builder.token(SyntaxKind::Ident, "count");
        builder.token(SyntaxKind::Colon, ":");
        builder.start_node(SyntaxKind::TypeHint);
        builder.token(SyntaxKind::Ident, "i64");
        builder.finish_node();
        builder.finish_node();
        builder.token(SyntaxKind::RBrace, "}");
        builder.finish_node();
        builder.finish_node();
        builder.token(SyntaxKind::RBrace, "}");
        builder.finish_node();
        builder.finish_node();
        builder.finish_node();

        let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
        let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");
        let enumeration = source.enums().next().expect("enum item");
        let variants = enumeration
            .variant_list()
            .expect("variant list")
            .variants()
            .collect::<Vec<_>>();

        assert_eq!(
            variants
                .iter()
                .map(|variant| variant.syntax().text().to_string())
                .collect::<Vec<_>>(),
            vec!["Finished(reward:Option<String>)", "Active{count:i64}"]
        );
        let tuple_param = variants[0]
            .tuple_field_list()
            .expect("tuple fields")
            .params()
            .next()
            .expect("tuple param");
        let tuple_hint = tuple_param.type_hint().expect("tuple param type");
        assert_eq!(tuple_hint.syntax().text().to_string(), "Option<String>");
        assert_eq!(
            tuple_hint
                .type_arg_list()
                .expect("tuple type args")
                .syntax()
                .text()
                .to_string(),
            "<String>"
        );
        let record_field = variants[1]
            .record_field_list()
            .expect("record fields")
            .fields()
            .next()
            .expect("record field");
        assert_eq!(
            record_field
                .type_hint()
                .expect("record field type")
                .syntax()
                .text()
                .to_string(),
            "i64"
        );
    }

    #[test]
    fn ast_trait_and_impl_items_expose_method_children() {
        let mut builder = SyntaxTreeBuilder::default();
        builder.start_node(SyntaxKind::SourceFile);
        builder.start_node(SyntaxKind::TraitItem);
        builder.token(SyntaxKind::TraitKw, "trait");
        builder.token(SyntaxKind::LBrace, "{");
        builder.start_node(SyntaxKind::TraitMethod);
        builder.token(SyntaxKind::FnKw, "fn");
        builder.token(SyntaxKind::Ident, "reward");
        builder.start_node(SyntaxKind::ParamList);
        builder.token(SyntaxKind::LParen, "(");
        builder.start_node(SyntaxKind::Param);
        builder.token(SyntaxKind::Ident, "amount");
        builder.token(SyntaxKind::Colon, ":");
        builder.start_node(SyntaxKind::TypeHint);
        builder.token(SyntaxKind::Ident, "i64");
        builder.finish_node();
        builder.finish_node();
        builder.token(SyntaxKind::RParen, ")");
        builder.finish_node();
        builder.token(SyntaxKind::Arrow, "->");
        builder.start_node(SyntaxKind::TypeHint);
        builder.token(SyntaxKind::Ident, "String");
        builder.finish_node();
        builder.token(SyntaxKind::Semicolon, ";");
        builder.finish_node();
        builder.token(SyntaxKind::RBrace, "}");
        builder.finish_node();
        builder.start_node(SyntaxKind::ImplItem);
        builder.token(SyntaxKind::ImplKw, "impl");
        builder.token(SyntaxKind::LBrace, "{");
        builder.start_node(SyntaxKind::ImplMethod);
        builder.token(SyntaxKind::FnKw, "fn");
        builder.token(SyntaxKind::Ident, "reward");
        builder.start_node(SyntaxKind::ParamList);
        builder.token(SyntaxKind::LParen, "(");
        builder.token(SyntaxKind::RParen, ")");
        builder.finish_node();
        builder.start_node(SyntaxKind::Block);
        builder.token(SyntaxKind::LBrace, "{");
        builder.token(SyntaxKind::ReturnKw, "return");
        builder.token(SyntaxKind::String, "\"gold\"");
        builder.token(SyntaxKind::Semicolon, ";");
        builder.token(SyntaxKind::RBrace, "}");
        builder.finish_node();
        builder.finish_node();
        builder.token(SyntaxKind::RBrace, "}");
        builder.finish_node();
        builder.finish_node();

        let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
        let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");
        let trait_item = source.traits().next().expect("trait item");
        let trait_method = trait_item.methods().next().expect("trait method");
        let impl_item = source.impls().next().expect("impl item");
        let impl_method = impl_item.methods().next().expect("impl method");

        assert_eq!(
            trait_method.syntax().text().to_string(),
            "fnreward(amount:i64)->String;"
        );
        assert_eq!(
            trait_method
                .param_list()
                .expect("trait params")
                .params()
                .next()
                .expect("trait param")
                .type_hint()
                .expect("param type")
                .syntax()
                .text()
                .to_string(),
            "i64"
        );
        assert_eq!(
            trait_method
                .return_type()
                .expect("trait return type")
                .syntax()
                .text()
                .to_string(),
            "String"
        );
        assert!(trait_method.body().is_none());
        assert_eq!(
            impl_method
                .body()
                .expect("impl body")
                .syntax()
                .text()
                .to_string(),
            "{return\"gold\";}"
        );
    }
}
