use super::*;

fn lambda_statement_payloads<'ast>(
    body: &body_payloads::CompilerBodyPayload<'ast>,
) -> Vec<body_payloads::CompilerStatementPayload<'ast>> {
    paired_statement_payloads_for_body(body.syntax_payload().source, body)
}

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
fn mismatched_lambda_payload_compiles_from_cst_body() {
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
            let statements = lambda_statement_payloads(&payload.body);
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
            let mismatched_lambda = expression_payload_with_fallback(
                source,
                cst_lambda
                    .syntax_expression()
                    .expect("CST lambda expression")
                    .clone(),
                legacy_lambda.fallback(),
            );

            let register = compiler
                .compile_expr_with_payload(mismatched_lambda.fallback(), Some(&mismatched_lambda))
                .expect("mismatched fallback should not block CST lambda compilation");

            assert!(
                compiler
                    .code
                    .instructions
                    .iter()
                    .any(|instruction| matches!(
                        &instruction.kind,
                        UnlinkedInstructionKind::MakeClosure { dst, captures, .. }
                            if *dst == register && captures == &[Register(0)]
                    ))
            );
        },
    );
}

#[test]
fn lambda_static_type_comes_from_cst_payload_not_owned_fallback() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let lambda = |value| value;
}
"#,
        |compiler, payload| {
            let lambda = lambda_statement_payloads(&payload.body)[0]
                .let_initializer_expression_payload()
                .expect("lambda initializer");

            assert_eq!(
                compiler.static_type_for_expr_with_payload(lambda.fallback(), None),
                value_types::StaticExprType::Dynamic,
                "owned fallback lambda must not provide static closure type"
            );
            assert_eq!(
                compiler.static_type_for_expr_with_payload(lambda.fallback(), Some(&lambda)),
                value_types::StaticExprType::Exact(RuntimeTypeFact::standard(
                    value_types::StandardRuntimeType::Closure,
                )),
                "CST lambda payload should provide static closure type"
            );
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
    let legacy_lambda = lambda_statement_payloads(&legacy_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("legacy lambda payload");
    let missing = expression_payload_with_fallback(source, cst_lambda, legacy_lambda.fallback());
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
            let lambda_payload = lambda_statement_payloads(&payload.body)[0]
                .let_initializer_expression_payload()
                .expect("CST lambda initializer");
            let body_payload = lambda_payload
                .lambda_body_payload()
                .expect("CST lambda body payload");
            let missing_block_body = body_payloads::CompilerExpressionPayload::from_syntax(
                None,
                body_payload.syntax_expression().cloned(),
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
    let body = lambda_statement_payloads(&payload.body)[0]
        .let_initializer_expression_payload()
        .and_then(|payload| payload.lambda_body_payload())
        .and_then(|payload| payload.block_body_payload())
        .expect("lambda block body payload");

    assert!(
        body_has_no_statement_fallbacks(&body),
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
    let legacy_lambda = lambda_statement_payloads(&legacy_payload.body)[0]
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
    let actual = lambda_statement_payloads(body)
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
    let actual = lambda_statement_payloads(body)
        .iter()
        .filter_map(|statement| {
            statement
                .expression_payload()
                .and_then(|payload| payload.assignment_value_payload())
        })
        .flat_map(lambda_body_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_call_argument_lambda_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = lambda_statement_payloads(body)
        .iter()
        .flat_map(|statement| {
            statement
                .expression_payload()
                .and_then(|payload| payload.call_argument_value_payloads())
                .unwrap_or_default()
        })
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
