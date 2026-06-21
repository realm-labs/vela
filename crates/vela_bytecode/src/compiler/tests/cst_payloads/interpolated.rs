use super::*;

#[test]
fn semantic_function_interpolated_expressions_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn messages(input) {
    let text = f"block { {
        let block_value = 1;
        block_value
    } } if {if input > 0 {
        let next = input + 1;
        next
    } else {
        0
    }} match {match input {
        0 => {
            let zero = 1;
            zero
        },
        _ => {
            input
        },
    }}";
    return text;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("messages").expect("messages function");

    assert_cst_let_initializer_interpolation_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let block_value = 1;"),
            (SyntaxStatementKind::Expr, "block_value"),
        ]],
        &[vec![
            (SyntaxStatementKind::Let, "let next = input + 1;"),
            (SyntaxStatementKind::Expr, "next"),
        ]],
        &[vec![(SyntaxStatementKind::Expr, "0")]],
        &[
            vec![
                (SyntaxStatementKind::Let, "let zero = 1;"),
                (SyntaxStatementKind::Expr, "zero"),
            ],
            vec![(SyntaxStatementKind::Expr, "input")],
        ],
    );

    compile_program_source(source, text)
        .expect("CST-backed interpolated string expressions should compile");
}

fn assert_cst_let_initializer_interpolation_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected_block: &[Vec<(SyntaxStatementKind, &str)>],
    expected_then: &[Vec<(SyntaxStatementKind, &str)>],
    expected_else: &[Vec<(SyntaxStatementKind, &str)>],
    expected_match: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let interpolation_payloads = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(|payload| {
            payload
                .interpolated_expression_payloads()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();

    let block_actual = interpolation_payloads
        .iter()
        .filter_map(|payload| {
            let body = payload.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    let if_payloads = interpolation_payloads
        .iter()
        .filter_map(body_payloads::CompilerExpressionPayload::if_payload)
        .collect::<Vec<_>>();
    let then_actual = if_payloads
        .iter()
        .filter_map(body_payloads::CompilerIfPayload::then_body)
        .map(cst_statement_texts)
        .collect::<Vec<_>>();
    let else_actual = if_payloads
        .iter()
        .filter_map(body_payloads::CompilerIfPayload::else_body)
        .map(cst_statement_texts)
        .collect::<Vec<_>>();
    let match_actual = interpolation_payloads
        .iter()
        .flat_map(|payload| payload.match_arm_payloads().unwrap_or_default())
        .filter_map(|arm| {
            let _syntax_arm = arm.syntax_arm()?;
            let body = arm.body_block_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();

    assert_eq!(block_actual, expected_statement_texts(expected_block));
    assert_eq!(then_actual, expected_statement_texts(expected_then));
    assert_eq!(else_actual, expected_statement_texts(expected_else));
    assert_eq!(match_actual, expected_statement_texts(expected_match));
}
