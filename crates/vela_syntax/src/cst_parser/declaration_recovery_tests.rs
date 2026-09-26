use crate::{
    ast::{AstNode, SyntaxFunctionItem, SyntaxParam},
    parse::parse_source,
};

#[test]
fn unnamed_type_owners_are_diagnosed_without_consuming_neighbor_functions() {
    for (item, name) in [
        ("pub struct { field: i64 }", "struct"),
        ("pub enum { Empty, Tuple(value: i64) }", "enum"),
        ("pub trait { fn dropped(self) {} }", "trait"),
    ] {
        for newline in ["\n", "\r\n"] {
            let text = format!("/* 文😀 */ {item}{newline}fn intact(value: i64) {{}}");
            let parsed = parse_source(&text);
            assert_eq!(parsed.tree().syntax().text().to_string(), text);
            let diagnostics = parsed.diagnostics();
            assert_eq!(diagnostics.len(), 1, "{item}: {diagnostics:?}");
            assert_eq!(diagnostics[0].code.as_deref(), Some("E_PARSE"));
            assert_eq!(diagnostics[0].message, format!("expected {name} name"));
            let span = diagnostics[0].span.expect("missing name span");
            let brace = text.find('{').expect("opening brace");
            assert_eq!((span.start as usize, span.end as usize), (brace, brace + 1));
            let neighbor = parsed
                .tree()
                .syntax()
                .descendants()
                .filter_map(SyntaxFunctionItem::cast)
                .find(|function| function.name_text().as_deref() == Some("intact"))
                .expect("neighbor retained");
            assert_eq!(
                neighbor.param_list().expect("parameters").params().count(),
                1
            );
        }
    }
}

#[test]
fn incomplete_parameter_lists_keep_names_and_partial_types_inside_their_owner() {
    for header in [
        "fn partial(value: Array<",
        "impl Local { fn partial(value: Array<",
        "trait Work { fn partial(value: Array<",
        "enum Choice { Tuple(value: Array<",
    ] {
        let text = format!("/* 文😀 */ {header}");
        let parsed = parse_source(&text);
        assert_eq!(parsed.tree().syntax().text().to_string(), text);
        assert!(parsed.diagnostics().iter().any(|diagnostic| {
            diagnostic.code.as_deref() == Some("E_PARSE") && diagnostic.message == "expected `)`"
        }));
        let params: Vec<_> = parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(SyntaxParam::cast)
            .collect();
        assert_eq!(params.len(), 1, "{header}");
        assert_eq!(params[0].name_text().as_deref(), Some("value"));
        assert_eq!(
            params[0].type_hint().expect("partial type").syntax().text(),
            "Array<"
        );
    }
}
