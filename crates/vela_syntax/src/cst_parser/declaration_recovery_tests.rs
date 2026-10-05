use crate::{
    ast::{AstNode, SyntaxFunctionItem, SyntaxParam},
    parse::parse_source,
};

#[test]
fn bodyless_function_headers_do_not_borrow_following_declaration_bodies() {
    for newline in ["\n", "\r\n"] {
        for neighbor in [
            "#[doc(\"中😀\")] pub fn intact(value: i64) {\n return value;\n}",
            "pub async fn intact() {\n}",
            "struct Intact {\n value: i64\n}",
            "enum Intact {\n Empty\n}",
            "trait Intact {\n fn read(self);\n}",
            "impl Intact {\n fn read(self) {}\n}",
            "use helper::helper;",
            "const LIMIT = 1;",
            "state value: i64 = 1;",
            "extern state value: i64;",
        ] {
            let header = "fn broken(\n value: i64\n)".replace('\n', newline);
            let neighbor = neighbor.replace('\n', newline);
            let source =
                format!("/* 中😀 */ {header}{newline}// boundary 中😀{newline}{neighbor}{newline}");
            let parsed = parse_source(&source);
            assert_eq!(parsed.tree().syntax().to_string(), source);
            let items = parsed.tree().items().collect::<Vec<_>>();
            assert_eq!(items.len(), 2, "neighbor {neighbor}");
            let broken = SyntaxFunctionItem::cast(items[0].syntax().clone())
                .expect("partial function stays a function");
            assert_eq!(broken.syntax().to_string(), header);
            assert!(broken.body().is_none());
            assert_eq!(items[1].syntax().to_string(), neighbor);
            assert_eq!(
                u32::from(items[1].syntax().text_range().start()) as usize,
                source.find(&neighbor).expect("literal neighbor start")
            );
            let diagnostics = parsed.diagnostics();
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code.as_deref(), Some("E_PARSE"));
            assert_eq!(diagnostics[0].message, "expected function body");
        }
    }
}

#[test]
fn contextual_state_return_types_keep_their_function_body() {
    for newline in ["\n", "\r\n"] {
        for hint in ["state", "Option<state>", "state::Value"] {
            let source = format!("/* 中😀 */ fn state() -> {hint} {{\n return 0;\n}}")
                .replace('\n', newline);
            let parsed = parse_source(&source);
            assert_eq!(parsed.tree().syntax().to_string(), source);
            assert!(parsed.diagnostics().is_empty(), "{hint}");
            let items = parsed.tree().items().collect::<Vec<_>>();
            assert_eq!(items.len(), 1);
            let function = SyntaxFunctionItem::cast(items[0].syntax().clone())
                .expect("function owns contextual type spelling");
            assert!(function.body().is_some());
            assert_eq!(function.name_text().as_deref(), Some("state"));
        }
    }
}

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

#[test]
fn unclosed_function_parameters_do_not_consume_a_healthy_neighbor() {
    for newline in ["\n", "\r\n"] {
        let text = format!("/* 文😀 */ fn broken( {{ }}{newline}pub fn intact(value: i64) {{}}");
        let parsed = parse_source(&text);
        assert_eq!(parsed.tree().syntax().text().to_string(), text);
        assert!(
            parsed
                .diagnostics()
                .iter()
                .any(|d| d.message == "expected `)`")
        );
        let functions = parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(SyntaxFunctionItem::cast)
            .collect::<Vec<_>>();
        assert_eq!(functions.len(), 2, "both declarations retained");
        assert_eq!(functions[0].name_text().as_deref(), Some("broken"));
        assert_eq!(functions[1].name_text().as_deref(), Some("intact"));
        let parameters = functions[1].param_list().expect("neighbor parameters");
        let parameter = parameters.params().next().expect("neighbor value");
        assert_eq!(parameter.name_text().as_deref(), Some("value"));
        assert_eq!(
            parameter
                .type_hint()
                .expect("neighbor type")
                .syntax()
                .text(),
            "i64"
        );
    }
}
