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

#[test]
fn semantic_function_i64_condition_jump_uses_cst_operand_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn check_limit() {
    let value: i64 = 10;
    if value > 5 {
        return 1;
    }
    return 0;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("check_limit")
        .expect("check_limit function");

    assert_cst_statement_if_condition_operand_payloads(&payload.body, &[("value", "5")]);

    let program =
        compile_program_source(source, text).expect("CST-backed i64 condition should compile");
    let function = program
        .function("check_limit")
        .expect("check_limit bytecode");
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
            op: crate::I64CompareOp::Greater,
            imm: 5,
            ..
        }
    )));
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

fn assert_cst_statement_if_condition_operand_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[(&str, &str)],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::if_payload)
        .filter_map(|if_payload| {
            let condition = if_payload.condition_payload()?;
            let (left, right) = condition.binary_operand_payloads()?;
            Some((payload_text(&left)?, payload_text(&right)?))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|(left, right)| ((*left).to_owned(), (*right).to_owned()))
            .collect::<Vec<_>>()
    );
}

fn condition_block_body_payload<'ast>(
    condition: &body_payloads::CompilerExpressionPayload<'ast>,
) -> Option<body_payloads::CompilerBodyPayload<'ast>> {
    condition
        .paren_inner_payload()
        .and_then(|inner| inner.block_body_payload())
        .or_else(|| condition.block_body_payload())
}

fn payload_text(payload: &body_payloads::CompilerExpressionPayload<'_>) -> Option<String> {
    let expression = payload.syntax_expression()?;
    Some(expression.syntax().text().to_string())
}
