use vela_common::SourceId;

use crate::ast::{AstNode, SyntaxExpressionKind};
use crate::parse::parse_source_with_id;

#[test]
fn parser_parse_source_keeps_parameter_default_blocks_inside_function_item() {
    let source = "fn defaults(value = { 1 + 2 }, empty = {}) { return value; }\nfn next() {}\n";
    let parse = parse_source_with_id(SourceId::new(31), source);
    let tree = parse.tree();
    let functions = tree.functions().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(functions.len(), 2);
    assert_eq!(
        functions
            .iter()
            .map(|function| function.name_text().expect("function name"))
            .collect::<Vec<_>>(),
        vec!["defaults", "next"]
    );
    assert_eq!(
        functions[0].syntax().text().to_string(),
        "fn defaults(value = { 1 + 2 }, empty = {}) { return value; }"
    );
    assert_eq!(
        functions[0]
            .param_list()
            .expect("param list")
            .params()
            .filter_map(|param| param.default_value())
            .map(|default| default.expression_kind())
            .collect::<Vec<_>>(),
        vec![SyntaxExpressionKind::Block, SyntaxExpressionKind::Block]
    );
}
