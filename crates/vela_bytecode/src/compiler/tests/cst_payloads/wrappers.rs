use super::*;

#[test]
fn semantic_function_wrapper_operands_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
enum Result {
    Ok(value)
    Err(message)
}

fn checked(value) {
    return Result::Ok(value);
}

fn take(first, second) {
    return first;
}

fn wrapper_values() {
    let flag = !{
        let ready = false;
        ready
    };
    let amount = -{
        let value = 1;
        value
    };
    amount = -{
        let assigned = 2;
        assigned
    };
    take(!{
        let arg = false;
        arg
    }, {
        let inner = checked(10);
        inner
    }?);
    return {
        let result = checked(amount);
        result
    }?;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("wrapper_values")
        .expect("wrapper_values function");

    assert_cst_let_initializer_unary_operand_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let ready = false;"),
                (SyntaxStatementKind::Expr, "ready"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let value = 1;"),
                (SyntaxStatementKind::Expr, "value"),
            ],
        ],
    );
    assert_cst_assignment_value_unary_operand_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let assigned = 2;"),
            (SyntaxStatementKind::Expr, "assigned"),
        ]],
    );
    assert_cst_call_argument_unary_operand_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let arg = false;"),
            (SyntaxStatementKind::Expr, "arg"),
        ]],
    );
    assert_cst_call_argument_try_operand_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let inner = checked(10);"),
            (SyntaxStatementKind::Expr, "inner"),
        ]],
    );
    assert_cst_return_value_try_operand_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let result = checked(amount);"),
            (SyntaxStatementKind::Expr, "result"),
        ]],
    );

    compile_program_source(source, text).expect("CST-backed wrapper operands should compile");
}

#[test]
fn semantic_function_parenthesized_values_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn paren_values() {
    let value = ({
        let inner = 1;
        inner
    });
    let assigned = 0;
    assigned = ({
        let updated = 2;
        updated
    });
    return value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("paren_values")
        .expect("paren_values function");
    assert_cst_let_initializer_paren_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let inner = 1;"),
            (SyntaxStatementKind::Expr, "inner"),
        ]],
    );
    assert_cst_assignment_value_paren_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let updated = 2;"),
            (SyntaxStatementKind::Expr, "updated"),
        ]],
    );

    compile_program_source(source, text)
        .expect("CST-backed parenthesized expression should compile");
}

fn assert_cst_let_initializer_unary_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter_map(|payload| payload.unary_operand_payload())
        .filter_map(|operand| {
            let body = operand.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_assignment_value_unary_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .filter_map(|payload| payload.unary_operand_payload())
        .filter_map(|operand| {
            let body = operand.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_call_argument_unary_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.call_argument_payloads().unwrap_or_default())
        .map(|argument| argument.value_expression_payload())
        .filter_map(|payload| payload.unary_operand_payload())
        .filter_map(|operand| {
            let body = operand.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_call_argument_try_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.call_argument_payloads().unwrap_or_default())
        .map(|argument| argument.value_expression_payload())
        .filter_map(|payload| payload.try_operand_payload())
        .filter_map(|operand| {
            let body = operand.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_return_value_try_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.return_value_expression_payload())
        .filter_map(|payload| payload.try_operand_payload())
        .filter_map(|operand| {
            let body = operand.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_let_initializer_paren_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter(|payload| payload.kind() == Some(SyntaxExpressionKind::Paren))
        .filter_map(|payload| payload.paren_inner_payload())
        .filter_map(|inner| {
            let body = inner.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_assignment_value_paren_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .filter(|payload| payload.kind() == Some(SyntaxExpressionKind::Paren))
        .filter_map(|payload| payload.paren_inner_payload())
        .filter_map(|inner| {
            let body = inner.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}
