use super::*;

#[test]
fn semantic_function_field_and_index_operands_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
struct Counter {
    value: i64,
}

fn make_counter(value) {
    return Counter { value: value };
}

fn make_counters(value) {
    return [Counter { value: value }];
}

fn field_and_index_values() {
    let field = make_counter({
        let current = 2;
        current
    }).value;
    let indexed = make_counters({
        let all = 3;
        all
    })[{
        let offset = 0;
        offset
    }].value;
    let assigned = 0;
    assigned = make_counter({
        let assigned_current = 4;
        assigned_current
    }).value;
    assigned = make_counters({
        let assigned_all = 5;
        assigned_all
    })[{
        let assigned_offset = 0;
        assigned_offset
    }].value;
    return field + indexed;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("field_and_index_values")
        .expect("field_and_index_values function");

    assert_cst_let_initializer_field_base_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let current = 2;"),
                (SyntaxStatementKind::Expr, "current"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let all = 3;"),
                (SyntaxStatementKind::Expr, "all"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let offset = 0;"),
                (SyntaxStatementKind::Expr, "offset"),
            ],
        ],
    );
    assert_cst_let_initializer_index_operand_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let all = 3;"),
                (SyntaxStatementKind::Expr, "all"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let offset = 0;"),
                (SyntaxStatementKind::Expr, "offset"),
            ],
        ],
    );
    assert_cst_assignment_value_field_base_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let assigned_current = 4;"),
                (SyntaxStatementKind::Expr, "assigned_current"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let assigned_all = 5;"),
                (SyntaxStatementKind::Expr, "assigned_all"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let assigned_offset = 0;"),
                (SyntaxStatementKind::Expr, "assigned_offset"),
            ],
        ],
    );
    assert_cst_assignment_value_index_operand_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let assigned_all = 5;"),
                (SyntaxStatementKind::Expr, "assigned_all"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let assigned_offset = 0;"),
                (SyntaxStatementKind::Expr, "assigned_offset"),
            ],
        ],
    );
    assert_cst_let_initializer_field_names(&payload.body, &["value", "value"]);
    assert_cst_assignment_value_field_names(&payload.body, &["value", "value"]);

    compile_program_source(source, text).expect("CST-backed field/index operands should compile");
}

#[test]
fn semantic_function_assignment_targets_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
struct Counter {
    value: i64,
}

struct CounterBox {
    counter: Counter,
}

fn make_box(value) {
    return CounterBox { counter: Counter { value: value } };
}

fn assignment_targets() {
    make_box({
        let seed = 1;
        seed
    }).counter.value = 2;
    let counters = [Counter { value: 0 }];
    counters[{
        let offset = 0;
        offset
    }].value = 3;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("assignment_targets")
        .expect("assignment_targets function");

    assert_cst_assignment_target_field_base_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let seed = 1;"),
                (SyntaxStatementKind::Expr, "seed"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let offset = 0;"),
                (SyntaxStatementKind::Expr, "offset"),
            ],
        ],
    );
    assert_cst_assignment_target_index_operand_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let offset = 0;"),
            (SyntaxStatementKind::Expr, "offset"),
        ]],
    );

    compile_program_source(source, text).expect("CST-backed assignment targets should compile");
}

fn assert_cst_let_initializer_field_base_body_payloads(
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

fn assert_cst_let_initializer_index_operand_body_payloads(
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

fn assert_cst_assignment_value_field_base_body_payloads(
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

fn assert_cst_assignment_value_index_operand_body_payloads(
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

fn assert_cst_assignment_target_field_base_body_payloads(
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

fn assert_cst_assignment_target_index_operand_body_payloads(
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

fn assert_cst_let_initializer_field_names(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&str],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter_map(|payload| payload.field_name())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_strings(expected));
}

fn assert_cst_assignment_value_field_names(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&str],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .filter_map(|payload| payload.field_name())
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
        .flat_map(index_operand_block_payloads)
        .collect()
}

fn index_operand_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    nested_block_payloads(payload)
}

fn nested_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    if let Some(body) = payload.block_body_payload() {
        return vec![cst_statement_texts(&body)];
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
            Some(cst_statement_texts(&body))
        })
        .collect()
}
