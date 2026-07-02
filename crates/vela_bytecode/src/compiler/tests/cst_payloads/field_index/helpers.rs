use crate::compiler::body_payloads;
use crate::compiler::tests::expected_statement_texts;
use vela_syntax::ast::SyntaxStatementKind;

pub(super) fn assert_cst_let_initializer_field_base_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(field_base_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

pub(super) fn assert_cst_let_initializer_index_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(index_block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

pub(super) fn assert_cst_assignment_value_field_base_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .flat_map(field_base_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

pub(super) fn assert_cst_assignment_value_index_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .flat_map(index_block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

pub(super) fn assert_cst_assignment_target_field_base_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_target_expression_payload())
        .flat_map(field_base_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

pub(super) fn assert_cst_assignment_target_index_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_target_expression_payload())
        .flat_map(index_block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

pub(super) fn assert_cst_let_initializer_field_names(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&str],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter_map(|payload| payload.syntax_field_name())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_strings(expected));
}

pub(super) fn assert_cst_assignment_value_field_names(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&str],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .filter_map(|payload| payload.syntax_field_name())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_strings(expected));
}

pub(super) fn assert_cst_assignment_target_field_names(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&str],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_target_expression_payload())
        .filter_map(|payload| payload.syntax_field_name())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_strings(expected));
}

fn expected_strings(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|name| (*name).to_owned()).collect()
}

fn field_base_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    payload
        .field_base_payload()
        .map(nested_block_payloads)
        .unwrap_or_default()
}

fn index_block_operand_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    let Some(field_base) = payload.field_base_payload() else {
        return Vec::new();
    };
    let Some((base, index)) = field_base.index_operand_payloads() else {
        return Vec::new();
    };
    [base, index]
        .into_iter()
        .flat_map(nested_block_payloads)
        .collect()
}

fn nested_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    if let Some(body) = payload.block_body_payload() {
        return vec![super::cst_statement_texts(&body)];
    }
    if let Some((base, index)) = payload.index_operand_payloads() {
        return [base, index]
            .into_iter()
            .flat_map(nested_block_payloads)
            .collect();
    }
    if let Some(base) = payload.field_base_payload() {
        return nested_block_payloads(base);
    }
    payload
        .call_argument_payloads()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|argument| {
            let value = argument.value_expression_payload();
            let body = value.block_body_payload()?;
            Some(super::cst_statement_texts(&body))
        })
        .collect()
}
