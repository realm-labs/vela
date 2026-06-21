use super::*;

#[test]
fn semantic_function_literal_expressions_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn take(first, second, third) {
    return first;
}

fn literal_values() {
    let count = 42;
    take("gold", true, 3.5);
    return 'x';
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("literal_values")
        .expect("literal_values function");

    assert_cst_let_initializer_literals(&payload.body, &[literal_int("42")]);
    assert_cst_call_argument_literals(
        &payload.body,
        &[
            vela_syntax::ast::Literal::String("gold".to_owned()),
            vela_syntax::ast::Literal::Bool(true),
            vela_syntax::ast::Literal::float("3.5"),
        ],
    );
    assert_cst_return_value_literals(&payload.body, &[vela_syntax::ast::Literal::Char('x')]);

    compile_program_source(source, text).expect("CST-backed literal expressions should compile");
}

fn assert_cst_let_initializer_literals(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[vela_syntax::ast::Literal],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter_map(literal_payload_value)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn assert_cst_call_argument_literals(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[vela_syntax::ast::Literal],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.call_argument_payloads().unwrap_or_default())
        .map(|argument| argument.value_expression_payload())
        .filter_map(literal_payload_value)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn assert_cst_return_value_literals(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[vela_syntax::ast::Literal],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.return_value_expression_payload())
        .filter_map(literal_payload_value)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn literal_payload_value(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Option<vela_syntax::ast::Literal> {
    assert_eq!(payload.kind(), Some(SyntaxExpressionKind::Literal));
    assert_eq!(
        payload
            .syntax_expression()
            .and_then(|expression| expression.as_literal())
            .and_then(|literal| literal.literal()),
        payload.literal()
    );
    payload.literal()
}

fn literal_int(text: &str) -> vela_syntax::ast::Literal {
    vela_syntax::ast::Literal::integer(text)
}
