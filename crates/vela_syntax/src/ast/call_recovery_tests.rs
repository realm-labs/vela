use crate::{
    ast::{AstNode, SyntaxCallExpr},
    parse::parse_source,
};

#[test]
fn incomplete_outer_call_keeps_the_inner_calls_closing_delimiter() {
    let source = "fn run() { outer(inner(value = 1)";
    let parsed = parse_source(source);
    assert_eq!(parsed.tree().syntax().text().to_string(), source);
    let calls = parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxCallExpr::cast)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    let outer = &calls[0];
    let inner = &calls[1];
    assert_eq!(
        outer.callee().expect("outer").syntax().text().to_string(),
        "outer"
    );
    assert!(outer.r_paren_token().is_none());
    assert_eq!(
        outer.arguments()[0]
            .expression()
            .expect("inner argument")
            .syntax()
            .text()
            .to_string(),
        "inner(value = 1)"
    );
    assert!(inner.r_paren_token().is_some());
    assert_eq!(inner.arguments()[0].name_text().as_deref(), Some("value"));
    assert_eq!(
        inner.arguments()[0]
            .expression()
            .expect("value")
            .syntax()
            .text()
            .to_string(),
        "1"
    );
}

#[test]
fn incomplete_calls_preserve_callee_arguments_and_lossless_text() {
    for (expression, callee, names, closed) in [
        ("make(", "make", vec![], false),
        ("make(value =", "make", vec![Some("value")], false),
        ("make(1, value =", "make", vec![None, Some("value")], false),
        ("factory()(value =", "factory()", vec![Some("value")], false),
        (
            "holder.make(value =",
            "holder.make",
            vec![Some("value")],
            false,
        ),
        ("make(value = 1)", "make", vec![Some("value")], true),
    ] {
        for newline in ["\n", "\r\n"] {
            let text = format!("fn run() {{{newline}/* 中😀 */ {expression}");
            let parsed = parse_source(&text);
            assert_eq!(parsed.tree().syntax().text().to_string(), text);
            let call = parsed
                .tree()
                .syntax()
                .descendants()
                .filter_map(SyntaxCallExpr::cast)
                .find(|call| {
                    call.callee()
                        .is_some_and(|value| value.syntax().text() == callee)
                })
                .unwrap_or_else(|| panic!("{expression}: {:#?}", parsed.tree().syntax()));
            assert_eq!(call.r_paren_token().is_some(), closed, "{expression}");
            let arguments = call.arguments();
            assert_eq!(
                arguments
                    .iter()
                    .map(|argument| argument.name_text())
                    .collect::<Vec<_>>(),
                names
                    .iter()
                    .map(|name| name.map(str::to_owned))
                    .collect::<Vec<_>>(),
                "{expression}"
            );
            if let Some(argument) = arguments.last() {
                if closed {
                    assert_eq!(
                        argument
                            .expression()
                            .expect("value")
                            .syntax()
                            .text()
                            .to_string(),
                        "1"
                    );
                } else {
                    assert!(
                        argument.expression().is_none(),
                        "missing value must remain absent"
                    );
                    assert_eq!(argument.equal_token().expect("equal").text(), "=");
                }
            }
        }
    }
}
