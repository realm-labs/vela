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

#[test]
fn semantic_function_logical_chain_operands_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn logical_values() {
    let both = ({
        let and_left = true;
        and_left
    }) && ({
        let and_middle = true;
        and_middle
    }) && ({
        let and_right = true;
        and_right
    });
    let either = ({
        let or_left = false;
        or_left
    }) || ({
        let or_middle = false;
        or_middle
    }) || ({
        let or_right = true;
        or_right
    });
    return both || either;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("logical_values")
        .expect("logical_values function");

    let initializers = payload
        .body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .collect::<Vec<_>>();

    let and_payload = initializers
        .iter()
        .find(|payload| {
            payload
                .logical_chain_operand_payloads(BinaryOp::And)
                .is_some_and(|operands| operands.len() == 3)
        })
        .expect("&& initializer should expose flattened logical operands");
    assert_logical_chain_block_payloads(
        and_payload,
        BinaryOp::And,
        &[
            vec![
                (SyntaxStatementKind::Let, "let and_left = true;"),
                (SyntaxStatementKind::Expr, "and_left"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let and_middle = true;"),
                (SyntaxStatementKind::Expr, "and_middle"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let and_right = true;"),
                (SyntaxStatementKind::Expr, "and_right"),
            ],
        ],
    );

    let or_payload = initializers
        .iter()
        .find(|payload| {
            payload
                .logical_chain_operand_payloads(BinaryOp::Or)
                .is_some_and(|operands| operands.len() == 3)
        })
        .expect("|| initializer should expose flattened logical operands");
    assert_logical_chain_block_payloads(
        or_payload,
        BinaryOp::Or,
        &[
            vec![
                (SyntaxStatementKind::Let, "let or_left = false;"),
                (SyntaxStatementKind::Expr, "or_left"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let or_middle = false;"),
                (SyntaxStatementKind::Expr, "or_middle"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let or_right = true;"),
                (SyntaxStatementKind::Expr, "or_right"),
            ],
        ],
    );

    compile_program_source(source, text).expect("CST-backed logical operands should compile");
}

#[test]
fn identity_comparison_diagnostics_prefer_cst_operand_payloads() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_binary = true === false;
    let legacy_binary = 1 === 2;
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let cst_binary = statements[0]
                .let_initializer_expression_payload()
                .expect("CST binary payload");
            let legacy_binary = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy binary fallback");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_binary
                    .syntax_expression()
                    .expect("CST expression")
                    .clone(),
                legacy_binary.fallback(),
            );

            let error = compiler
                .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
                .expect_err("mismatched CST binary payload must not compile");
            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST binary expression payload")
            ));
        },
    );
}

#[test]
fn binary_value_type_inference_rejects_mismatched_cst_payloads() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_sum = 1 + 2;
    let lhs = 1;
    let rhs = 2;
    let cst_diff = lhs - rhs;
    let legacy_bool = true;
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let cst_sum = statements[0]
                .let_initializer_expression_payload()
                .expect("CST binary payload");
            let cst_diff = statements[3]
                .let_initializer_expression_payload()
                .expect("CST binary path payload");
            let legacy_bool = statements[4]
                .let_initializer_expression_payload()
                .expect("legacy literal fallback");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_sum
                    .syntax_expression()
                    .expect("CST binary expression")
                    .clone(),
                legacy_bool.fallback(),
            );

            assert_eq!(
                compiler.static_type_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                value_types::StaticExprType::Dynamic
            );

            compiler.value_types.set_name(
                "lhs",
                Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::I64)),
            );
            compiler.value_types.set_name(
                "rhs",
                Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::I64)),
            );
            let mismatched_path_operand_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_diff
                    .syntax_expression()
                    .expect("CST binary path expression")
                    .clone(),
                legacy_bool.fallback(),
            );
            assert_eq!(
                compiler.static_type_for_expr_with_payload(
                    mismatched_path_operand_payload.fallback(),
                    Some(&mismatched_path_operand_payload),
                ),
                value_types::StaticExprType::Dynamic
            );
        },
    );
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

fn assert_logical_chain_block_payloads(
    payload: &body_payloads::CompilerExpressionPayload<'_>,
    op: BinaryOp,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = payload
        .logical_chain_operand_payloads(op)
        .expect("logical chain should expose operand payloads")
        .into_iter()
        .flat_map(block_operand_payloads)
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
    if let Some(operand) = payload.paren_inner_payload() {
        return block_operand_payloads(operand);
    }
    if let Some(operand) = payload.unary_operand_payload() {
        return block_operand_payloads(operand);
    }
    Vec::new()
}
