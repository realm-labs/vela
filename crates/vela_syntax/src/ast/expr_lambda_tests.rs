use crate::SyntaxKind;
use crate::ast::{AstNode, SyntaxCallExpr};
use crate::parse::parse_source;

#[test]
fn ast_call_argument_lambda_preserves_if_expression_body_after_array_receiver() {
    let source = r#"fn update() {
    let groups = [1, 2, 3, 4].group_by(|value| if value % 2 == 0 { even } else { odd });
}
"#;
    let parse = parse_source(source);
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let initializer = body
        .let_statements()
        .next()
        .expect("let statement")
        .initializer()
        .expect("initializer");
    let call = SyntaxCallExpr::cast(initializer.syntax().clone()).expect("call expression");
    let arguments = call.arguments();
    let lambda = arguments[0]
        .expression()
        .and_then(|expr| expr.as_lambda())
        .expect("lambda argument");

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(arguments.len(), 1);
    assert_eq!(
        lambda
            .body_expression()
            .expect("lambda body")
            .syntax()
            .kind(),
        SyntaxKind::IfExpr
    );
    let if_expr = lambda
        .body_expression()
        .and_then(|expr| expr.as_if())
        .expect("if body");
    assert_eq!(
        if_expr.condition().expect("condition").syntax().kind(),
        SyntaxKind::BinaryExpr
    );
    assert_eq!(
        if_expr.then_block().expect("then").syntax().text(),
        "{ even }"
    );
    assert_eq!(
        if_expr.else_block().expect("else").syntax().text(),
        "{ odd }"
    );
}

#[test]
fn ast_zero_arg_lambda_exposes_block_body() {
    let source = r#"fn update() {
    let captured = || {
        return reward.count + 9;
    };
}
"#;
    let parse = parse_source(source);
    let body = parse
        .tree()
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let lambda = body
        .let_statements()
        .next()
        .and_then(|stmt| stmt.initializer())
        .and_then(|expr| expr.as_lambda())
        .expect("lambda initializer");

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(lambda.param_list().expect("params").params().count(), 0);
    assert_eq!(
        lambda
            .body_block()
            .expect("block body")
            .statements()
            .count(),
        1
    );
}

#[test]
fn ast_lambda_assignment_expression_bodies_keep_parameters_and_owned_operands() {
    for operator in ["=", "+=", "-=", "*=", "/=", "%="] {
        for (params, count) in [("|value|", 1), ("||", 0)] {
            for newline in ["\n", "\r\n"] {
                let source = format!(
                    "/*中😀*/ fn run() {{{newline}let callback = {params} saved {operator} value;{newline}}}"
                );
                let parsed = parse_source(&source);
                assert!(
                    parsed.diagnostics().is_empty(),
                    "{source}: {:?}",
                    parsed.diagnostics()
                );
                assert_eq!(parsed.tree().syntax().text().to_string(), source);
                let function = parsed.tree().functions().next().expect("function");
                let body = function.body().expect("function body");
                let lambda = body
                    .let_statements()
                    .next()
                    .and_then(|stmt| stmt.initializer())
                    .and_then(|expr| expr.as_lambda())
                    .expect("outer expression remains a lambda");
                let list = lambda.param_list().expect("complete lambda header");
                assert_eq!(list.params().count(), count);
                assert_eq!(list.syntax().text().to_string(), params);
                let assignment = lambda
                    .body_expression()
                    .and_then(|expr| expr.as_assign())
                    .expect("owned assignment body");
                assert_eq!(
                    assignment.syntax().text().to_string(),
                    format!("saved {operator} value")
                );
                assert_eq!(
                    assignment.target().expect("target").syntax().text(),
                    "saved"
                );
                assert_eq!(assignment.value().expect("value").syntax().text(), "value");
                assert_eq!(
                    assignment.operator_token().expect("operator").text(),
                    operator
                );
            }
        }
    }
}

#[test]
fn ast_callback_assignment_bodies_do_not_create_argument_labels() {
    for operator in ["=", "+=", "-=", "*=", "/=", "%="] {
        for prefix in ["", "callback = "] {
            let source = format!(
                "fn run() {{ let result = consume({prefix}|value| saved {operator} value, fallback = seed); }}"
            );
            let parsed = parse_source(&source);
            assert!(
                parsed.diagnostics().is_empty(),
                "{source}: {:?}",
                parsed.diagnostics()
            );
            assert_eq!(parsed.tree().syntax().text().to_string(), source);
            let function = parsed.tree().functions().next().expect("function");
            let body = function.body().expect("body");
            let call = body
                .let_statements()
                .next()
                .and_then(|stmt| stmt.initializer())
                .and_then(|expr| expr.as_call())
                .expect("call initializer");
            let arguments = call.arguments();
            assert_eq!(arguments.len(), 2);
            assert_eq!(
                arguments[0].name_text().as_deref(),
                if prefix.is_empty() {
                    None
                } else {
                    Some("callback")
                }
            );
            let lambda = arguments[0]
                .expression()
                .and_then(|expr| expr.as_lambda())
                .expect("complete callback lambda");
            assert_eq!(
                lambda
                    .param_list()
                    .expect("params")
                    .params()
                    .next()
                    .expect("parameter")
                    .name_text()
                    .as_deref(),
                Some("value")
            );
            let assignment = lambda
                .body_expression()
                .and_then(|expr| expr.as_assign())
                .expect("assignment callback body");
            assert_eq!(
                assignment.target().expect("target").syntax().text(),
                "saved"
            );
            assert_eq!(assignment.value().expect("value").syntax().text(), "value");
            assert_eq!(
                assignment.operator_token().expect("operator").text(),
                operator
            );
            assert_eq!(arguments[1].name_text().as_deref(), Some("fallback"));
            assert_eq!(
                arguments[1]
                    .expression()
                    .expect("fallback value")
                    .syntax()
                    .text(),
                "seed"
            );
        }
    }
}
