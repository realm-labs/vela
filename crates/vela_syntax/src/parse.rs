use std::marker::PhantomData;

use vela_common::{Diagnostic, SourceId};

use crate::ast::{AstNode, SyntaxSourceFile};
use crate::lexer::lex;
use crate::parser::cst;
use crate::{SyntaxNode, SyntaxTreeBuilder};

#[must_use]
pub fn parse_source(text: &str) -> Parse<SyntaxSourceFile> {
    parse_source_with_id(SourceId::new(0), text)
}

#[must_use]
pub fn parse_source_with_id(source: SourceId, text: &str) -> Parse<SyntaxSourceFile> {
    let lexed = lex(source, text);
    let mut builder = SyntaxTreeBuilder::default();
    let mut diagnostics = lexed.diagnostics.clone();
    diagnostics.extend(cst::build_source_tree(&lexed, &mut builder));

    builder.finish_with_diagnostics(diagnostics)
}

#[derive(Debug, Eq, PartialEq)]
pub struct Parse<T> {
    green: rowan::GreenNode,
    diagnostics: Vec<Diagnostic>,
    _ty: PhantomData<fn() -> T>,
}

impl<T> Clone for Parse<T> {
    fn clone(&self) -> Self {
        Self {
            green: self.green.clone(),
            diagnostics: self.diagnostics.clone(),
            _ty: PhantomData,
        }
    }
}

impl<T> Parse<T> {
    #[must_use]
    pub fn new(green: rowan::GreenNode, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            green,
            diagnostics,
            _ty: PhantomData,
        }
    }

    #[must_use]
    pub fn syntax_node(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    #[must_use]
    pub fn green(&self) -> &rowan::GreenNode {
        &self.green
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Parse<SyntaxSourceFile> {
    #[must_use]
    pub fn tree(&self) -> SyntaxSourceFile {
        SyntaxSourceFile::cast(self.syntax_node())
            .expect("parse_source must produce a source-file root")
    }
}

#[cfg(test)]
mod tests {
    use vela_common::{SourceId, Span};

    use crate::ast::AstNode;
    use crate::parse::parse_source_with_id;
    use crate::{SyntaxKind, TextRange, TextSize};

    #[test]
    fn parser_parse_source_builds_lossless_source_file_root() {
        let source = "#!/usr/bin/env vela\n// hello\nfn main() { return 1; }\n";
        let parse = parse_source_with_id(SourceId::new(7), source);
        let tree = parse.tree();

        assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
        assert_eq!(tree.syntax().kind(), SyntaxKind::SourceFile);
        assert_eq!(tree.syntax().text().to_string(), source);
        assert_eq!(
            tree.syntax()
                .children()
                .map(|node| node.kind())
                .collect::<Vec<_>>(),
            vec![SyntaxKind::FunctionItem]
        );
        assert_eq!(
            tree.text_range(),
            TextRange::new(TextSize::from(0), TextSize::of(source))
        );
    }

    #[test]
    fn parser_parse_source_wraps_top_level_items_in_cst_nodes() {
        let source = "# [event(\"tick\")]\npub fn tick() {}\nuse game::state;\nstruct Player { level: i64 }\n";
        let parse = parse_source_with_id(SourceId::new(11), source);
        let tree = parse.tree();

        assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
        assert_eq!(tree.syntax().text().to_string(), source);
        assert_eq!(
            tree.items()
                .map(|item| item.syntax().kind())
                .collect::<Vec<_>>(),
            vec![
                SyntaxKind::FunctionItem,
                SyntaxKind::UseItem,
                SyntaxKind::StructItem,
            ]
        );
    }

    #[test]
    fn parser_parse_source_structures_function_signature_and_body_nodes() {
        let source = "fn award(ctx: Context, items: Array<String>, amount = bonus(1, 2)) -> Result<Map<String, i64>, String> { return amount; }\n";
        let parse = parse_source_with_id(SourceId::new(12), source);
        let tree = parse.tree();
        let function = tree.functions().next().expect("function item");
        let params = function.param_list().expect("param list");
        let body = function.body().expect("body");
        let params_vec = params.params().collect::<Vec<_>>();

        assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
        assert_eq!(function.syntax().text().to_string(), source.trim_end());
        assert_eq!(params.syntax().kind(), SyntaxKind::ParamList);
        assert_eq!(
            params_vec
                .iter()
                .map(|param| param.syntax().text().to_string())
                .collect::<Vec<_>>(),
            vec![
                "ctx: Context",
                " items: Array<String>",
                " amount = bonus(1, 2)",
            ]
        );
        assert_eq!(
            params_vec[0]
                .type_hint()
                .expect("ctx type")
                .syntax()
                .text()
                .to_string(),
            "Context"
        );
        let items_hint = params_vec[1].type_hint().expect("items type");
        assert_eq!(items_hint.syntax().text().to_string(), "Array<String>");
        assert_eq!(
            items_hint
                .type_arg_list()
                .expect("items type args")
                .syntax()
                .text()
                .to_string(),
            "<String>"
        );
        assert!(params_vec[2].type_hint().is_none());
        let return_type = function.return_type().expect("return type");
        assert_eq!(
            return_type.syntax().text().to_string(),
            "Result<Map<String, i64>, String>"
        );
        assert_eq!(
            return_type
                .type_arg_list()
                .expect("return type args")
                .syntax()
                .text()
                .to_string(),
            "<Map<String, i64>, String>"
        );
        assert_eq!(body.syntax().kind(), SyntaxKind::Block);
        assert_eq!(body.syntax().text().to_string(), "{ return amount; }");
    }

    #[test]
    fn parser_parse_source_structures_struct_field_nodes() {
        let source = r#"struct Reward {
    #[doc("Reward item")]
    item_id: String = "gold",
    count: i64 = 1
    tags: Array<String>
}
"#;
        let parse = parse_source_with_id(SourceId::new(13), source);
        let tree = parse.tree();
        let record = tree.structs().next().expect("struct item");
        let field_list = record.field_list().expect("field list");
        let fields = field_list.fields().collect::<Vec<_>>();

        assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
        assert_eq!(record.syntax().text().to_string(), source.trim_end());
        assert_eq!(field_list.syntax().kind(), SyntaxKind::StructFieldList);
        assert_eq!(
            fields
                .iter()
                .map(|field| field.syntax().text().to_string())
                .collect::<Vec<_>>(),
            vec![
                "#[doc(\"Reward item\")]\n    item_id: String = \"gold\"",
                "count: i64 = 1",
                "tags: Array<String>",
            ]
        );
        assert_eq!(
            fields[0]
                .type_hint()
                .expect("item type")
                .syntax()
                .text()
                .to_string(),
            "String"
        );
        let tags_hint = fields[2].type_hint().expect("tags type");
        assert_eq!(tags_hint.syntax().text().to_string(), "Array<String>");
        assert_eq!(
            tags_hint
                .type_arg_list()
                .expect("tags type args")
                .syntax()
                .text()
                .to_string(),
            "<String>"
        );
    }

    #[test]
    fn parser_parse_source_keeps_malformed_fragments_in_cst() {
        let source = "fn main() { @ \"unterminated";
        let parse = parse_source_with_id(SourceId::new(9), source);
        let tree = parse.tree();

        assert_eq!(tree.syntax().text().to_string(), source);
        assert!(
            tree.syntax()
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .any(|token| token.kind() == SyntaxKind::Unknown && token.text() == "@")
        );
        assert_eq!(
            parse
                .diagnostics()
                .iter()
                .filter_map(|diagnostic| diagnostic.span)
                .collect::<Vec<_>>(),
            vec![
                Span::new(SourceId::new(9), 12, 13),
                Span::new(SourceId::new(9), 14, source.len() as u32),
            ]
        );
    }
}
