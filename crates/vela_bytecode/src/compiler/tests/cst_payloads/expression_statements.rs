use super::*;

#[test]
fn semantic_function_generic_expression_statements_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn expression_statements() {
    let values = [1, 2, 3];
    ({
        let selected = values;
        selected
    })[0];
    [{
        let item = 1;
        item
    }];
    f"status { {
        let count = values.len();
        count
    } }";
    return values.len();
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("expression_statements")
        .expect("expression_statements function");

    assert_cst_expr_statements(
        &payload.body,
        &[
            (
                SyntaxExpressionKind::Index,
                "({\n        let selected = values;\n        selected\n    })[0]",
            ),
            (
                SyntaxExpressionKind::Array,
                "[{\n        let item = 1;\n        item\n    }]",
            ),
            (
                SyntaxExpressionKind::Literal,
                "f\"status { {\n        let count = values.len();\n        count\n    } }\"",
            ),
        ],
    );
    assert_cst_expression_statement_index_base_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let selected = values;"),
            (SyntaxStatementKind::Expr, "selected"),
        ]],
    );
    assert_cst_expression_statement_array_element_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let item = 1;"),
            (SyntaxStatementKind::Expr, "item"),
        ]],
    );
    assert_cst_expression_statement_interpolation_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let count = values.len();"),
            (SyntaxStatementKind::Expr, "count"),
        ]],
    );

    compile_program_source(source, text)
        .expect("CST-backed generic expression statements should compile");
}

#[test]
fn mismatched_expression_statement_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn take(value) {
    return value;
}

fn main() {
    let values = [1];
    take(1);
    values[0];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_call = statements[1]
        .syntax_statement()
        .expect("CST call statement")
        .clone();
    let legacy_index = statements[2].fallback();
    let mismatched =
        body_payloads::CompilerStatementPayload::syntax(source, cst_call, legacy_index);

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("mismatched expression statement payload must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST expression statement payload")
    ));
}

fn assert_cst_expression_statement_index_base_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::expression_payload)
        .filter_map(|payload| payload.index_operand_payloads())
        .flat_map(|(base, _)| nested_expression_block_payloads(base))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_expression_statement_array_element_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::expression_payload)
        .flat_map(|payload| payload.array_element_payloads().unwrap_or_default())
        .flat_map(nested_expression_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_expression_statement_interpolation_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::expression_payload)
        .flat_map(|payload| {
            payload
                .interpolated_expression_payloads()
                .unwrap_or_default()
        })
        .flat_map(nested_expression_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn nested_expression_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    if let Some(body) = payload.block_body_payload() {
        return vec![cst_statement_texts(&body)];
    }
    if let Some(inner) = payload.paren_inner_payload() {
        return nested_expression_block_payloads(inner);
    }
    Vec::new()
}
