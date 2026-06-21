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
    assert_cst_assignment_target_field_names(&payload.body, &["value", "value"]);

    compile_program_source(source, text).expect("CST-backed assignment targets should compile");
}

#[test]
fn field_read_slots_prefer_cst_receiver_payloads() {
    with_cst_payload_compiler(
        r#"
struct CstBox {
    alpha: i64,
    amount: i64,
}

struct LegacyBox {
    amount: i64,
    zed: i64,
}

fn main() {
    let cst = CstBox { alpha: 0, amount: 1 };
    let legacy = LegacyBox { amount: 2, zed: 3 };
    let cst_amount = cst.amount;
    let legacy_amount = legacy.amount;
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            compiler
                .compile_statement(statements[0].fallback())
                .expect("cst local should compile");
            compiler
                .compile_statement(statements[1].fallback())
                .expect("legacy local should compile");
            let cst_field = statements[2]
                .let_initializer_expression_payload()
                .expect("CST field payload");
            let legacy_field = statements[3]
                .let_initializer_expression_payload()
                .expect("legacy field fallback");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_field
                    .syntax_expression()
                    .expect("CST field expression")
                    .clone(),
                legacy_field.fallback(),
            );

            compiler
                .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
                .expect("CST-backed field read should compile");
            let slot = compiler
                .code
                .instructions
                .iter()
                .rev()
                .find_map(|instruction| {
                    let UnlinkedInstructionKind::GetRecordSlot { field, slot, .. } =
                        &instruction.kind
                    else {
                        return None;
                    };
                    (field == "amount").then_some(*slot)
                });
            assert_eq!(slot, Some(1));
        },
    );
}

#[test]
fn record_field_assignment_target_facts_prefer_cst_root_payloads() {
    with_cst_payload_compiler(
        r#"
struct CstBox {
    amount: i64,
}

struct LegacyBox {
    amount: bool,
}

fn main() {
    let cst = CstBox { amount: 0 };
    let legacy = LegacyBox { amount: false };
    cst.amount = true;
    legacy.amount = true;
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            compiler
                .compile_statement(statements[0].fallback())
                .expect("cst local should compile");
            compiler
                .compile_statement(statements[1].fallback())
                .expect("legacy local should compile");
            let cst_target = statements[2]
                .assignment_target_expression_payload()
                .expect("CST assignment target payload");
            let legacy_statement = statements[3]
                .expression_payload()
                .expect("legacy assignment expression");
            let legacy_target = statements[3]
                .assignment_target_expression_payload()
                .expect("legacy assignment target fallback");
            let mismatched_target = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_target
                    .syntax_expression()
                    .expect("CST target expression")
                    .clone(),
                legacy_target.fallback(),
            );

            let error = compiler
                .compile_assignment_with_payloads(
                    legacy_statement.fallback(),
                    crate::compiler::assignments::AssignmentTargetSyntax::new(Some(
                        &mismatched_target,
                    )),
                    crate::compiler::assignments::AssignmentValueSyntax::new(
                        None,
                        None,
                        crate::compiler::assignments::AssignmentValuePayloads::new(
                            None, None, None, None,
                        ),
                    ),
                )
                .expect_err("CST target amount expects i64, not bool");
            let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
                panic!("expected semantic diagnostics: {:?}", error.kind);
            };
            let diagnostic = diagnostics
                .iter()
                .find(|diagnostic| {
                    diagnostic.code.as_deref() == Some("compiler::type_contract_mismatch")
                })
                .expect("type contract mismatch diagnostic");
            assert!(
                diagnostic
                    .labels
                    .iter()
                    .any(|label| label.message.contains("expected `i64`")),
                "{diagnostic:?}"
            );
        },
    );
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

fn assert_cst_assignment_target_field_names(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&str],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_target_expression_payload())
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
