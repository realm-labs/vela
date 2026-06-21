use super::*;

#[test]
fn semantic_function_binary_operands_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn take(value) {
    return value;
}

fn binary_values() {
    let amount = -{
        let left = 1;
        left
    } + {
        let right = 2;
        right
    };
    let plus_right = -{
        let base = 3;
        base
    } + 1;
    let plus_left = 1 + -{
        let tail = 4;
        tail
    };
    amount = -{
        let assigned_left = 5;
        assigned_left
    } + {
        let assigned_right = 6;
        assigned_right
    };
    take(-{
        let arg_left = 7;
        arg_left
    } + {
        let arg_right = 8;
        arg_right
    });
    return -{
        let return_left = 9;
        return_left
    } + {
        let return_right = 10;
        return_right
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("binary_values")
        .expect("binary_values function");

    assert_cst_let_initializer_binary_operand_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let left = 1;"),
                (SyntaxStatementKind::Expr, "left"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let right = 2;"),
                (SyntaxStatementKind::Expr, "right"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let base = 3;"),
                (SyntaxStatementKind::Expr, "base"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let tail = 4;"),
                (SyntaxStatementKind::Expr, "tail"),
            ],
        ],
    );
    assert_cst_assignment_value_binary_operand_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let assigned_left = 5;"),
                (SyntaxStatementKind::Expr, "assigned_left"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let assigned_right = 6;"),
                (SyntaxStatementKind::Expr, "assigned_right"),
            ],
        ],
    );
    assert_cst_call_argument_binary_operand_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let arg_left = 7;"),
                (SyntaxStatementKind::Expr, "arg_left"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let arg_right = 8;"),
                (SyntaxStatementKind::Expr, "arg_right"),
            ],
        ],
    );
    assert_cst_return_value_binary_operand_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let return_left = 9;"),
                (SyntaxStatementKind::Expr, "return_left"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let return_right = 10;"),
                (SyntaxStatementKind::Expr, "return_right"),
            ],
        ],
    );

    compile_program_source(source, text).expect("CST-backed binary operands should compile");
}

fn assert_cst_let_initializer_binary_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(binary_block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_assignment_value_binary_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .flat_map(binary_block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_call_argument_binary_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.call_argument_payloads().unwrap_or_default())
        .map(|argument| argument.value_expression_payload())
        .flat_map(binary_block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_return_value_binary_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.return_value_expression_payload())
        .flat_map(binary_block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn binary_block_operand_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    let Some((left, right)) = payload.binary_operand_payloads() else {
        return Vec::new();
    };
    [left, right]
        .into_iter()
        .flat_map(block_operand_payloads)
        .collect()
}

fn block_operand_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    if let Some(body) = payload.block_body_payload() {
        return vec![cst_statement_texts(&body)];
    }
    if let Some(operand) = payload.unary_operand_payload() {
        return block_operand_payloads(operand);
    }
    Vec::new()
}
