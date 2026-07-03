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

#[test]
fn i64_condition_jump_immediate_prefers_cst_rhs_payload() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let value: i64 = 10;
    if value > 5 {
        return 1;
    }
    return 0;
}
"#;
    let cst_semantic = parse_semantic_source(source, cst_text).expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("main function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
fn main() {
    let value: i64 = 10;
    if value > 7 {
        return 1;
    }
    return 0;
}
"#,
        |compiler, payload| {
            let mismatched_body = body_payloads::CompilerBodyPayload::syntax(
                source,
                cst_body,
                payload.body.fallback(),
            );
            let statements = mismatched_body.statement_payloads();

            compiler
                .compile_statement_payloads(&statements)
                .expect("CST-backed i64 condition should compile");

            assert!(
                compiler
                    .code
                    .instructions
                    .iter()
                    .any(|instruction| matches!(
                        instruction.kind,
                        UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
                            op: crate::I64CompareOp::Greater,
                            imm: 5,
                            ..
                        }
                    )),
                "i64 immediate jump should use the CST right-hand literal"
            );
        },
    );
}

#[test]
fn i64_condition_jump_immediate_prefers_cst_operator_payload() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let value: i64 = 10;
    if value > 5 {
        return 1;
    }
    return 0;
}
"#;
    let cst_semantic = parse_semantic_source(source, cst_text).expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("main function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
fn main() {
    let value: i64 = 10;
    if value < 5 {
        return 1;
    }
    return 0;
}
"#,
        |compiler, payload| {
            let mismatched_body = body_payloads::CompilerBodyPayload::syntax(
                source,
                cst_body,
                payload.body.fallback(),
            );
            let statements = mismatched_body.statement_payloads();

            compiler
                .compile_statement_payloads(&statements)
                .expect("CST-backed i64 condition should compile");

            assert!(
                compiler
                    .code
                    .instructions
                    .iter()
                    .any(|instruction| matches!(
                        instruction.kind,
                        UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
                            op: crate::I64CompareOp::Greater,
                            imm: 5,
                            ..
                        }
                    )),
                "i64 immediate jump should use the CST comparison operator"
            );
        },
    );
}

#[test]
fn missing_if_value_condition_payload_does_not_use_legacy_condition() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let value = true;
    let selected = if value {
        1
    } else {
        2
    };
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("condition local should compile");
            let initializer = statements[1]
                .let_initializer_expression_payload()
                .expect("if initializer payload");
            let vela_syntax::ast::ExprKind::If(if_expr) = &initializer.fallback().kind else {
                panic!("expected legacy if fallback");
            };
            let truncated_if_payload = body_payloads::CompilerIfPayload::truncated_for_test();

            let error = compiler
                .compile_if_value_with_payloads(if_expr, Register(0), Some(&truncated_if_payload))
                .expect_err("missing CST if condition payload must not use legacy condition");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST if condition payload")
            ));
        },
    );
}

#[test]
fn missing_if_value_then_body_payload_does_not_use_legacy_then_body() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let selected = if true {
        1
    } else {
        2
    };
}
"#,
        |compiler, payload| {
            let initializer = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("if initializer payload");
            let vela_syntax::ast::ExprKind::If(if_expr) = &initializer.fallback().kind else {
                panic!("expected legacy if fallback");
            };
            let if_payload = initializer
                .if_payload()
                .expect("CST if payload")
                .without_then_body_for_test();

            let error = compiler
                .compile_if_value_with_payloads(if_expr, Register(0), Some(&if_payload))
                .expect_err("missing CST if then body payload must not use legacy then body");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST if then body payload")
            ));
        },
    );
}

#[test]
fn missing_if_value_else_body_payload_does_not_use_legacy_else_body() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let selected = if true {
        1
    } else {
        2
    };
}
"#,
        |compiler, payload| {
            let initializer = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("if initializer payload");
            let vela_syntax::ast::ExprKind::If(if_expr) = &initializer.fallback().kind else {
                panic!("expected legacy if fallback");
            };
            let if_payload = initializer
                .if_payload()
                .expect("CST if payload")
                .without_else_body_for_test();

            let error = compiler
                .compile_if_value_with_payloads(if_expr, Register(0), Some(&if_payload))
                .expect_err("missing CST if else body payload must not use legacy else body");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST if else body payload")
            ));
        },
    );
}

#[test]
fn missing_if_value_else_if_payload_does_not_use_legacy_else_if_body() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let selected = if true {
        1
    } else if false {
        2
    } else {
        3
    };
}
"#,
        |compiler, payload| {
            let initializer = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("if initializer payload");
            let vela_syntax::ast::ExprKind::If(if_expr) = &initializer.fallback().kind else {
                panic!("expected legacy if fallback");
            };
            let if_payload = initializer
                .if_payload()
                .expect("CST if payload")
                .without_else_if_for_test();

            let error = compiler
                .compile_if_value_with_payloads(if_expr, Register(0), Some(&if_payload))
                .expect_err("missing CST else-if payload must not use legacy else-if body");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST else-if payload")
            ));
        },
    );
}

#[test]
fn syntax_only_if_then_body_drops_owned_body_fallback() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let selected = if true {
        let nested;
        return;
    } else {
        2
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (_, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let if_payload = payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .and_then(|payload| payload.if_payload())
        .expect("if initializer payload");
    let then_body = if_payload.then_body().expect("then body payload");

    assert!(
        !then_body.has_fallback_statements(),
        "syntax-only if then body should not retain an owned body fallback"
    );
}

#[test]
fn syntax_only_if_else_body_drops_owned_body_fallback() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let selected = if false {
        1
    } else {
        let nested;
        return;
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (_, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let if_payload = payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .and_then(|payload| payload.if_payload())
        .expect("if initializer payload");
    let else_body = if_payload.else_body().expect("else body payload");

    assert!(
        !else_body.has_fallback_statements(),
        "syntax-only if else body should not retain an owned body fallback"
    );
}

#[test]
fn mismatched_i64_condition_payload_does_not_use_legacy_operator() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let value: i64 = 10;
    if value {
        return 1;
    }
    return 0;
}
"#;
    let cst_semantic = parse_semantic_source(source, cst_text).expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("main function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
fn main() {
    let value: i64 = 10;
    if value < 5 {
        return 1;
    }
    return 0;
}
"#,
        |compiler, payload| {
            let mismatched_body = body_payloads::CompilerBodyPayload::syntax(
                source,
                cst_body,
                payload.body.fallback(),
            );
            let statements = mismatched_body.statement_payloads();

            let error = compiler
                .compile_statement_payloads(&statements)
                .expect_err("mismatched CST condition must not compile the legacy operator");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST if condition payload")
            ));
        },
    );
}

#[test]
fn i64_condition_jump_immediate_does_not_use_legacy_rhs_without_cst_literal() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let value: i64 = 10;
    let other: i64 = 5;
    if value > other {
        return 1;
    }
    return 0;
}
"#;
    let cst_semantic = parse_semantic_source(source, cst_text).expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("main function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
fn main() {
    let value: i64 = 10;
    let other: i64 = 5;
    if value > 5 {
        return 1;
    }
    return 0;
}
"#,
        |compiler, payload| {
            let mismatched_body = body_payloads::CompilerBodyPayload::syntax(
                source,
                cst_body,
                payload.body.fallback(),
            );
            let statements = mismatched_body.statement_payloads();

            compiler
                .compile_statement_payloads(&statements)
                .expect("mismatched CST condition should compile through generic condition path");

            assert!(
                !compiler
                    .code
                    .instructions
                    .iter()
                    .any(|instruction| matches!(
                        instruction.kind,
                        UnlinkedInstructionKind::I64CmpImmJumpIfFalse { .. }
                    )),
                "i64 immediate jump must not use a legacy fallback right-hand literal"
            );
        },
    );
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
            let (_, fallback_left, fallback_right) = condition.fallback_binary_operands()?;
            let (left, right) = condition.binary_operand_payloads(fallback_left, fallback_right)?;
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
