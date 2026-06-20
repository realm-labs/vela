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
