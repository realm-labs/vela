use super::*;

#[test]
fn semantic_function_lambda_bodies_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn lambda_values() {
    let values = [1, 2, 3];
    let add_one = |value| {
        let next = value + 1;
        next
    };
    values.map(|value| {
        let doubled = value * 2;
        doubled
    });
    return add_one(1);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("lambda_values")
        .expect("lambda_values function");

    assert_cst_let_initializer_lambda_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let next = value + 1;"),
            (SyntaxStatementKind::Expr, "next"),
        ]],
    );
    assert_cst_call_argument_lambda_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let doubled = value * 2;"),
            (SyntaxStatementKind::Expr, "doubled"),
        ]],
    );

    compile_program_source(source, text).expect("CST-backed lambda bodies should compile");
}

fn assert_cst_let_initializer_lambda_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(lambda_body_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_call_argument_lambda_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.call_argument_payloads().unwrap_or_default())
        .map(|argument| argument.value_expression_payload())
        .flat_map(lambda_body_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn lambda_body_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    let Some(body_payload) = payload.lambda_body_payload() else {
        return Vec::new();
    };
    if let Some(block_payload) = body_payload.block_body_payload() {
        vec![cst_statement_texts(&block_payload)]
    } else {
        Vec::new()
    }
}
