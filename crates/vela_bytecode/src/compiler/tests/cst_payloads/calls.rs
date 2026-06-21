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
    let named = take_typed(value = {
        let named_value = 8;
        named_value
    });
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
            vec![
                (SyntaxStatementKind::Let, "let named_value = 8;"),
                (SyntaxStatementKind::Expr, "named_value"),
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
    assert_cst_call_argument_names(&payload.body, &["value"]);
    assert_cst_let_initializer_call_callee_path_segments(
        &payload.body,
        &[&["take"], &["Boxed", "Value"], &["take_typed"]],
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

fn assert_cst_call_argument_names(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&str],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(|payload| payload.call_argument_payloads().unwrap_or_default())
        .filter_map(|argument| argument.syntax_name())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn assert_cst_let_initializer_call_callee_path_segments(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&[&str]],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter_map(|payload| payload.call_callee_payload())
        .filter_map(|callee| callee.path_segments())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_path_segments(expected));
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

#[test]
fn semantic_function_call_callee_and_receiver_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn call_targets() {
    let callable = |value| value;
    let closure_result = ({
        let selected = callable;
        selected
    })({
        let value = 7;
        value
    });
    let receiver_result = ({
        let label = "ready";
        label
    }).len();
    return closure_result + receiver_result;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("call_targets")
        .expect("call_targets function");

    assert_cst_let_initializer_call_callee_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let selected = callable;"),
            (SyntaxStatementKind::Expr, "selected"),
        ]],
    );
    assert_cst_let_initializer_method_receiver_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let label = \"ready\";"),
            (SyntaxStatementKind::Expr, "label"),
        ]],
    );
    assert_cst_let_initializer_method_names(&payload.body, &["len"]);

    compile_program_source(source, text)
        .expect("CST-backed call callees and method receivers should compile");
}

fn assert_cst_let_initializer_call_callee_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(call_callee_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_let_initializer_method_receiver_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(method_receiver_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_let_initializer_method_names(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&str],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter_map(call_method_name)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_strings(expected));
}

fn call_method_name(payload: body_payloads::CompilerExpressionPayload<'_>) -> Option<String> {
    payload.call_callee_payload()?.field_name()
}

fn expected_strings(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|name| (*name).to_owned()).collect()
}

fn expected_path_segments(expected: &[&[&str]]) -> Vec<Vec<String>> {
    expected
        .iter()
        .map(|path| path.iter().map(|segment| (*segment).to_owned()).collect())
        .collect()
}

fn call_callee_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    payload
        .call_callee_payload()
        .into_iter()
        .flat_map(nested_call_target_block_payloads)
        .collect()
}

fn method_receiver_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    payload
        .call_callee_payload()
        .and_then(|callee| callee.field_base_payload())
        .into_iter()
        .flat_map(nested_call_target_block_payloads)
        .collect()
}

fn nested_call_target_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    if let Some(body) = payload.block_body_payload() {
        return vec![cst_statement_texts(&body)];
    }
    if let Some(inner) = payload.paren_inner_payload() {
        return nested_call_target_block_payloads(inner);
    }
    Vec::new()
}
