use super::*;

#[test]
fn semantic_function_value_call_arguments_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn take(value) {
    return value;
}

fn take_typed(value: i64) {
    return value;
}

fn outer(value) {
    return value;
}

enum Boxed {
    Value(value)
}

fn call_values() {
    let result = take({
        let initial = 1;
        initial
    });
    let boxed = Boxed::Value({
        let enum_value = 5;
        enum_value
    });
    result = take({
        let assigned = 2;
        assigned
    });
    outer(take({
        let nested = 3;
        nested
    }));
    outer(take_typed({
        let typed = 6;
        typed
    }));
    return take({
        let returned = 4;
        returned
    });
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("call_values")
        .expect("call_values function");

    assert_cst_let_initializer_call_argument_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let initial = 1;"),
                (SyntaxStatementKind::Expr, "initial"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let enum_value = 5;"),
                (SyntaxStatementKind::Expr, "enum_value"),
            ],
        ],
    );
    assert_cst_assignment_value_call_argument_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let assigned = 2;"),
            (SyntaxStatementKind::Expr, "assigned"),
        ]],
    );
    assert_cst_nested_call_argument_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let nested = 3;"),
                (SyntaxStatementKind::Expr, "nested"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let typed = 6;"),
                (SyntaxStatementKind::Expr, "typed"),
            ],
        ],
    );
    assert_cst_return_value_call_argument_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let returned = 4;"),
            (SyntaxStatementKind::Expr, "returned"),
        ]],
    );

    compile_program_source(source, text).expect("CST-backed value call arguments should compile");
}

fn assert_cst_let_initializer_call_argument_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(call_argument_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_assignment_value_call_argument_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .flat_map(call_argument_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_nested_call_argument_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.call_argument_payloads().unwrap_or_default())
        .map(|argument| argument.value_expression_payload())
        .flat_map(call_argument_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_return_value_call_argument_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.return_value_expression_payload())
        .flat_map(call_argument_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn call_argument_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    payload
        .call_argument_payloads()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|argument| {
            let value = argument.value_expression_payload();
            let body = value.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect()
}
