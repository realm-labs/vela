use super::*;
#[test]
fn semantic_function_lambda_bodies_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn lambda_values() {
    let values = [1, 2, 3];
    let add_one = |value| {
        let next = value + 1;
        next
    };
    let assigned = |value| value;
    assigned = |value| {
        let assigned_next = value + 2;
        assigned_next
    };
    values.map(|value| {
        let doubled = value * 2;
        doubled
    });
    return add_one(1);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("lambda_values")
        .expect("lambda_values function");

    assert_cst_let_initializer_lambda_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let next = value + 1;"),
            (SyntaxStatementKind::Expr, "next"),
        ]],
    );
    assert_cst_assignment_value_lambda_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let assigned_next = value + 2;"),
            (SyntaxStatementKind::Expr, "assigned_next"),
        ]],
    );
    assert_cst_call_argument_lambda_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let doubled = value * 2;"),
            (SyntaxStatementKind::Expr, "doubled"),
        ]],
    );

    compile_program_source(source, text).expect("CST-backed lambda bodies should compile");
}

#[test]
fn mismatched_lambda_payload_does_not_collect_captures_from_cst_body() {
    let source = SourceId::new(1);
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_outer = 1;
    let legacy_outer = 2;
    let cst_lambda = |value| cst_outer + value;
    let legacy_lambda = |value| legacy_outer + value;
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("cst_outer let should compile");
            compiler
                .compile_statement_payload_for_test(&statements[1])
                .expect("legacy_outer let should compile");

            let cst_lambda = statements[2]
                .let_initializer_expression_payload()
                .expect("CST lambda initializer");
            let legacy_lambda = statements[3]
                .let_initializer_expression_payload()
                .expect("legacy lambda initializer");
            let mismatched_lambda = body_payloads::CompilerExpressionPayload::syntax(
                source,
                cst_lambda
                    .syntax_expression()
                    .expect("CST lambda expression")
                    .clone(),
                legacy_lambda.fallback(),
            );

            let error = compiler
                .compile_expr_with_payload(mismatched_lambda.fallback(), Some(&mismatched_lambda))
                .expect_err("mismatched CST lambda payload must not compile");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST lambda expression payload")
            ));
        },
    );
}

#[test]
fn missing_lambda_body_payload_does_not_use_legacy_body() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let lambda = |value|;
}
"#;
    let legacy_text = r#"
fn main() {
    let lambda = |value| value;
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_lambda = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body")
        .statements()
        .next()
        .expect("CST let statement")
        .as_let()
        .expect("CST let")
        .initializer()
        .expect("CST initializer");
    assert_eq!(cst_lambda.expression_kind(), SyntaxExpressionKind::Lambda);

    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_lambda = legacy_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("legacy lambda payload");
    let missing = body_payloads::CompilerExpressionPayload::syntax(
        source,
        cst_lambda,
        legacy_lambda.fallback(),
    );
    assert!(
        missing.fallback_lambda_body().is_some(),
        "test payload should still carry the fallback lambda body"
    );
    assert!(
        missing.lambda_body_payload().is_none(),
        "recovered CST lambda should not expose a body payload"
    );

    let error = compiler
        .compile_expr_with_payload(legacy_lambda.fallback(), Some(&missing))
        .expect_err("missing lambda body payload must not compile legacy body");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST lambda body")
    ));
}

#[test]
fn missing_lambda_block_body_payload_does_not_use_legacy_block() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let lambda = |value| {
        let next = value + 1;
        next
    };
}
"#,
        |compiler, payload| {
            let lambda_payload = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("CST lambda initializer");
            let body_payload = lambda_payload
                .lambda_body_payload()
                .expect("CST lambda body payload");
            let missing_block_body =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    body_payload
                        .syntax_expression()
                        .expect("CST lambda body expression")
                        .clone(),
                    body_payload.fallback(),
                );
            let ExprKind::Lambda { params, body } = &lambda_payload.fallback().kind else {
                panic!("expected lambda expression");
            };

            let error = compiler
                .compile_lambda(
                    lambda_payload.fallback(),
                    params,
                    body,
                    Some(&missing_block_body),
                )
                .expect_err("missing CST lambda block payload must not compile legacy block");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST lambda block body payload")
            ));
        },
    );
}

#[test]
fn syntax_only_lambda_block_body_drops_owned_body_fallback() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let lambda = |value| {
        let nested;
        return;
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (_, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let body = payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .and_then(|payload| payload.lambda_body_payload())
        .and_then(|payload| payload.block_body_payload())
        .expect("lambda block body payload");

    assert!(
        !body.has_fallback_statements(),
        "syntax-only lambda block body should not retain an owned body fallback"
    );
}

#[test]
fn missing_lambda_expression_payload_does_not_use_legacy_lambda() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let lambda = |value| value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_lambda = legacy_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("legacy lambda payload");
    let missing_lambda =
        body_payloads::CompilerExpressionPayload::missing_syntax(source, legacy_lambda.fallback());

    let error = compiler
        .compile_expr_with_payload(legacy_lambda.fallback(), Some(&missing_lambda))
        .expect_err("missing CST lambda payload must not compile legacy lambda");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST expression payload")
    ));
}

fn assert_cst_let_initializer_lambda_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(lambda_body_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_assignment_value_lambda_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .flat_map(lambda_body_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_call_argument_lambda_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.call_argument_value_payloads().unwrap_or_default())
        .flat_map(lambda_body_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn lambda_body_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    let Some(body_payload) = payload.lambda_body_payload() else {
        return Vec::new();
    };
    if let Some(block_payload) = body_payload.block_body_payload() {
        vec![cst_statement_texts(&block_payload)]
    } else {
        Vec::new()
    }
}
