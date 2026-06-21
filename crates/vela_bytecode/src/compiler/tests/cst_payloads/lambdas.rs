use super::*;
use crate::compiler::lambdas::collect_lambda_captures_with_payload;

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
fn lambda_capture_collection_prefers_cst_body_payloads() {
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
                .compile_statement(statements[0].fallback())
                .expect("cst_outer let should compile");
            compiler
                .compile_statement(statements[1].fallback())
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
            let ExprKind::Lambda { body, .. } = &mismatched_lambda.fallback().kind else {
                panic!("expected legacy lambda fallback");
            };
            let mismatched_body = mismatched_lambda
                .lambda_body_payload()
                .expect("mismatched lambda body payload");

            let captures = collect_lambda_captures_with_payload(
                compiler.bindings,
                &compiler.hir_locals,
                body,
                Some(&mismatched_body),
            );

            assert_eq!(
                captures
                    .iter()
                    .map(|capture| capture.name.as_str())
                    .collect::<Vec<_>>(),
                ["cst_outer"],
            );
        },
    );
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
        .flat_map(|statement| statement.call_argument_payloads().unwrap_or_default())
        .map(|argument| argument.value_expression_payload())
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
