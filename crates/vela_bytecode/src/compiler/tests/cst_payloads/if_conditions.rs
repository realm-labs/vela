use super::*;

#[test]
fn semantic_function_if_conditions_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn condition_values(input) {
    if ({
        let ready = true;
        ready
    }) {
        input = input + 1;
    }
    let selected = if ({
        let positive = input > 0;
        positive
    }) {
        input
    } else {
        0
    };
    return selected;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("condition_values")
        .expect("condition_values function");

    assert_cst_statement_if_condition_block_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let ready = true;"),
            (SyntaxStatementKind::Expr, "ready"),
        ]],
    );
    assert_cst_let_initializer_if_condition_block_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let positive = input > 0;"),
            (SyntaxStatementKind::Expr, "positive"),
        ]],
    );

    compile_program_source(source, text)
        .expect("CST-backed if condition block expressions should compile");
}

fn assert_cst_statement_if_condition_block_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::if_payload)
        .filter_map(|if_payload| {
            let condition = if_payload.condition_payload()?;
            let body = condition_block_body_payload(condition)?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_let_initializer_if_condition_block_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_if_payload())
        .filter_map(|if_payload| {
            let condition = if_payload.condition_payload()?;
            let body = condition_block_body_payload(condition)?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn condition_block_body_payload<'ast>(
    condition: &body_payloads::CompilerExpressionPayload<'ast>,
) -> Option<body_payloads::CompilerBodyPayload<'ast>> {
    condition
        .paren_inner_payload()
        .and_then(|inner| inner.block_body_payload())
        .or_else(|| condition.block_body_payload())
}
