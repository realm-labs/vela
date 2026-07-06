use super::*;

fn interpolated_statement_payloads<'ast>(
    body: &body_payloads::CompilerBodyPayload<'ast>,
) -> Vec<body_payloads::CompilerStatementPayload<'ast>> {
    paired_statement_payloads_for_body(body.syntax_payload().source, body)
}

#[test]
fn simple_interpolated_let_and_return_compile_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn messages(input) {
    let text = f"value {input.name} {1}";
    return f"done {input.name}";
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("messages").expect("messages function");

    assert!(body_has_no_statement_fallbacks(&payload.body));
    compile_program_source(source, text)
        .expect("simple CST-backed interpolated string bodies should compile");
}

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

#[test]
fn equal_count_interpolated_payloads_pair_expressions_by_position_not_legacy_span() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_value = 1;
    let legacy_value = 2;
    let cst_text = f"{cst_value}";
    let legacy_text = f"{legacy_value}";
}
"#,
        |_compiler, payload| {
            let statements = interpolated_statement_payloads(&payload.body);
            let cst_interpolated = statements[2]
                .let_initializer_expression_payload()
                .expect("CST interpolated payload");
            let legacy_interpolated = statements[3]
                .let_initializer_expression_payload()
                .expect("legacy interpolated fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_interpolated
                    .syntax_expression()
                    .expect("CST interpolated expression")
                    .clone(),
                legacy_interpolated.fallback(),
            );

            let parts = mismatched_payload
                .interpolated_expression_payloads()
                .expect("interpolation expression payloads");
            assert_eq!(parts.len(), 1);
            assert_eq!(
                parts[0]
                    .syntax_expression()
                    .expect("CST interpolation expression")
                    .syntax()
                    .text()
                    .to_string(),
                "cst_value"
            );
        },
    );
}

#[test]
fn extra_interpolated_expression_payloads_do_not_compile_fallback_parts() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_body() {
    let first = 1;
    let second = 2;
    let text = f"{first} {second}";
}

fn fallback_body() {
    let first = 1;
    let text = f"{first}";
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_body")
        .expect("fallback function");
    let cst_interpolated = interpolated_statement_payloads(&cst_payload.body)[2]
        .let_initializer_expression_payload()
        .expect("CST interpolated payload");
    let fallback_interpolated = interpolated_statement_payloads(&fallback_payload.body)[1]
        .let_initializer_expression_payload()
        .expect("fallback interpolated payload");
    let mismatched = expression_payload_with_fallback(
        source,
        cst_interpolated
            .syntax_expression()
            .expect("CST interpolated syntax")
            .clone(),
        fallback_interpolated.fallback(),
    );
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_body");

    let error = compiler
        .compile_expr_with_payload(fallback_interpolated.fallback(), Some(&mismatched))
        .expect_err("extra CST interpolation expressions must not be ignored");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST interpolation expressions")
    ));
}

#[test]
fn missing_interpolated_expression_payloads_do_not_compile_fallback_parts() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_body() {
    let first = 1;
    let text = f"{first}";
}

fn fallback_body() {
    let first = 1;
    let second = 2;
    let text = f"{first} {second}";
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_body")
        .expect("fallback function");
    let cst_interpolated = interpolated_statement_payloads(&cst_payload.body)[1]
        .let_initializer_expression_payload()
        .expect("CST interpolated payload");
    let fallback_interpolated = interpolated_statement_payloads(&fallback_payload.body)[2]
        .let_initializer_expression_payload()
        .expect("fallback interpolated payload");
    let mismatched = expression_payload_with_fallback(
        source,
        cst_interpolated
            .syntax_expression()
            .expect("CST interpolated syntax")
            .clone(),
        fallback_interpolated.fallback(),
    );
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_body");

    let error = compiler
        .compile_expr_with_payload(fallback_interpolated.fallback(), Some(&mismatched))
        .expect_err("missing CST interpolation expressions must not compile fallback parts");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST interpolation expressions")
    ));
}

#[test]
fn missing_interpolated_expression_payload_does_not_use_legacy_interpolated() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = 1;
    let text = f"{value}";
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_interpolated = interpolated_statement_payloads(&legacy_payload.body)[1]
        .let_initializer_expression_payload()
        .expect("legacy interpolated payload");
    let missing_interpolated = body_payloads::CompilerExpressionPayload::missing_syntax(
        source,
        legacy_interpolated.fallback(),
    );

    let error = compiler
        .compile_expr_with_payload(legacy_interpolated.fallback(), Some(&missing_interpolated))
        .expect_err("missing CST interpolated payload must not compile legacy interpolated string");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST expression payload")
    ));
}

#[test]
fn missing_interpolation_child_payloads_do_not_use_legacy_expression() {
    let no_payloads: [body_payloads::CompilerInterpolationPayload; 0] = [];
    let error = match crate::compiler::expressions::interpolated_expression_payload_at(
        Some(&no_payloads),
        0,
    ) {
        Ok(_) => panic!("missing interpolation payload must not look at legacy expression"),
        Err(error) => error,
    };
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST interpolation expression")
    ));

    assert!(
        crate::compiler::expressions::interpolated_expression_payload_at(None, 0)
            .expect("absent interpolation payload vector should preserve non-CST fallback path")
            .is_none()
    );
}

fn assert_cst_let_initializer_interpolation_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected_block: &[Vec<(SyntaxStatementKind, &str)>],
    expected_then: &[Vec<(SyntaxStatementKind, &str)>],
    expected_else: &[Vec<(SyntaxStatementKind, &str)>],
    expected_match: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let interpolation_payloads = interpolated_statement_payloads(body)
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(|payload| {
            payload
                .interpolated_expression_value_payloads()
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
