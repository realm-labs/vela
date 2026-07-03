use super::*;

#[test]
fn source_less_expression_payload_does_not_expose_cst_expression() {
    with_cst_payload_compiler(
        r#"
fn make(value) {
    return value;
}

fn main(value) {
    let result = value && make(value);
}
"#,
        |_, payload| {
            let value = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("value initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    value.syntax_expression().expect("value syntax").clone(),
                    value.fallback(),
                );

            assert_eq!(missing_source.syntax_kind(), None);
            assert!(missing_source.syntax_expression().is_none());
        },
    );
}

#[test]
fn source_less_field_payload_does_not_expose_cst_name() {
    with_cst_payload_compiler(
        r#"
fn make(value) {
    return value;
}

fn main(object) {
    let value = make(object).amount;
}
"#,
        |_, payload| {
            let field = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("field initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    field.syntax_expression().expect("field syntax").clone(),
                    field.fallback(),
                );

            assert_eq!(missing_source.kind(), None);
            assert_eq!(missing_source.syntax_field_name(), None);
            assert!(missing_source.field_base_payload().is_none());
        },
    );
}

#[test]
fn source_less_binary_payload_does_not_expose_cst_operator() {
    with_cst_payload_compiler(
        r#"
fn make(value) {
    return value;
}

fn main(left, right) {
    let total = left && make(right);
}
"#,
        |_, payload| {
            let binary = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("binary initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    binary.syntax_expression().expect("binary syntax").clone(),
                    binary.fallback(),
                );

            assert_eq!(missing_source.syntax_binary_operator(), None);
            assert!(
                missing_source.fallback_binary_operands().is_some(),
                "test payload should still carry fallback binary operands"
            );
            assert!(missing_source.binary_operand_payloads().is_none());
        },
    );
}

#[test]
fn source_less_unary_payload_does_not_expose_operand_payload() {
    with_cst_payload_compiler(
        r#"
fn main(value, other) {
    let negated = -(value + other);
}
"#,
        |_, payload| {
            let unary = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("unary initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    unary.syntax_expression().expect("unary syntax").clone(),
                    unary.fallback(),
                );

            assert_eq!(missing_source.syntax_unary_operator(), None);
            assert!(
                missing_source.fallback_unary_operand().is_some(),
                "test payload should still carry the fallback unary operand"
            );
            assert!(missing_source.unary_operand_payload().is_none());
        },
    );
}

#[test]
fn source_less_try_payload_does_not_expose_operand_payload() {
    with_cst_payload_compiler(
        r#"
fn make(value) {
    return value;
}

fn main(value) {
    let result = make(value)?;
}
"#,
        |_, payload| {
            let try_expression = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("try initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    try_expression
                        .syntax_expression()
                        .expect("try syntax")
                        .clone(),
                    try_expression.fallback(),
                );

            assert!(
                try_expression.fallback_try_operand().is_some(),
                "test payload should still carry the fallback try operand"
            );
            assert!(missing_source.try_operand_payload().is_none());
        },
    );
}

#[test]
fn source_less_paren_payload_does_not_expose_inner_payload() {
    with_cst_payload_compiler(
        r#"
fn make(value) {
    return value;
}

fn main(value) {
    let result = (value && make(value));
}
"#,
        |_, payload| {
            let paren = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("paren initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    paren.syntax_expression().expect("paren syntax").clone(),
                    paren.fallback(),
                );

            assert!(missing_source.paren_inner_payload().is_none());
        },
    );
}

#[test]
fn source_less_assignment_payload_does_not_expose_operator_or_operands() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let value = 1;
    let assigned = value += 2;
}
"#,
        |_, payload| {
            let assignment = payload.body.statement_payloads()[1]
                .let_initializer_expression_payload()
                .expect("assignment initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    assignment
                        .syntax_expression()
                        .expect("assignment syntax")
                        .clone(),
                    assignment.fallback(),
                );

            assert_eq!(missing_source.syntax_assignment_operator(), None);
            assert!(missing_source.assignment_target_payload().is_none());
            assert!(missing_source.assignment_value_payload().is_none());
        },
    );
}

#[test]
fn source_less_lambda_payload_does_not_expose_body_payload() {
    with_cst_payload_compiler(
        r#"
fn main(value) {
    let callback = |input| value + input;
}
"#,
        |_, payload| {
            let lambda = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("lambda initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    lambda.syntax_expression().expect("lambda syntax").clone(),
                    lambda.fallback(),
                );

            assert!(
                missing_source.fallback_lambda_body().is_some(),
                "test payload should still carry the fallback lambda body"
            );
            assert!(missing_source.lambda_body_payload().is_none());
        },
    );
}

#[test]
fn source_less_logical_chain_payload_does_not_expose_operand_payloads() {
    with_cst_payload_compiler(
        r#"
fn make(value) {
    return value;
}

fn main(left, middle, right) {
    let value = left && make(middle) && right;
}
"#,
        |_, payload| {
            let logical = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("logical initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    logical.syntax_expression().expect("logical syntax").clone(),
                    logical.fallback(),
                );

            assert!(
                missing_source
                    .logical_chain_operand_payloads(BinaryOp::And)
                    .is_none()
            );
        },
    );
}

#[test]
fn source_less_index_payload_does_not_expose_operand_payloads() {
    with_cst_payload_compiler(
        r#"
fn main(values, index) {
    let value = values[index];
}
"#,
        |_, payload| {
            let index_expression = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("index initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    index_expression
                        .syntax_expression()
                        .expect("index syntax")
                        .clone(),
                    index_expression.fallback(),
                );

            assert!(missing_source.index_operand_payloads().is_none());
        },
    );
}

#[test]
fn source_less_call_payload_does_not_expose_callee_payload() {
    with_cst_payload_compiler(
        r#"
fn make() {
    return 1;
}

fn main() {
    let value = make();
}
"#,
        |_, payload| {
            let call = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("call initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    call.syntax_expression().expect("call syntax").clone(),
                    call.fallback(),
                );

            assert_eq!(missing_source.syntax_call_callee_path_segments(), None);
            assert!(missing_source.call_callee_payload().is_none());
        },
    );
}

#[test]
fn source_less_statement_call_payload_does_not_expose_callee_payload() {
    with_cst_payload_compiler(
        r#"
fn make() {
    return 1;
}

fn main() {
    make();
}
"#,
        |_, payload| {
            let statement = &payload.body.statement_payloads()[0];
            let missing_source =
                body_payloads::CompilerStatementPayload::missing_child_payload_context(
                    statement
                        .syntax_statement()
                        .expect("call statement syntax")
                        .clone(),
                    statement.fallback(),
                );

            assert!(missing_source.call_callee_payload().is_none());
        },
    );
}

#[test]
fn source_less_array_payload_does_not_expose_cst_element_count() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let values = [1];
}
"#,
        |_, payload| {
            let array = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("array initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    array.syntax_expression().expect("array syntax").clone(),
                    array.fallback(),
                );
            let _items = missing_source
                .fallback_array_items()
                .expect("fallback array items");
            let elements = missing_source
                .array_element_payloads()
                .expect("array element payloads");

            assert!(!missing_source.has_extra_array_elements());
            assert!(elements[0].syntax_expression().is_none());
        },
    );
}

#[test]
fn source_less_map_entry_payload_does_not_expose_cst_key_name() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let values = { key: 1 };
}
"#,
        |_, payload| {
            let map = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("map initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    map.syntax_expression().expect("map syntax").clone(),
                    map.fallback(),
                );
            let fallback_entries = missing_source
                .fallback_map_entries()
                .expect("fallback map entries");
            let entries = missing_source
                .map_entry_payloads()
                .expect("map entry payloads");

            assert!(!missing_source.has_mismatched_map_entries());
            assert!(!entries[0].has_key_syntax());
            assert!(!entries[0].has_value_syntax());
            assert_eq!(entries[0].syntax_key_name(), None);
            assert!(
                entries[0]
                    .value_expression_payload(&fallback_entries[0].value)
                    .syntax_expression()
                    .is_none()
            );
        },
    );
}

#[test]
fn source_less_record_field_payload_does_not_expose_cst_label_name() {
    with_cst_payload_compiler(
        r#"
struct Player {
    level
}

fn main() {
    let player = Player { level: 1 };
}
"#,
        |_, payload| {
            let record = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("record initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    record.syntax_expression().expect("record syntax").clone(),
                    record.fallback(),
                );
            let fallback_fields = missing_source
                .fallback_record_fields()
                .expect("fallback record fields");
            let fields = missing_source
                .record_field_payloads()
                .expect("record field payloads");

            assert!(!missing_source.has_extra_record_fields());
            assert!(!fields[0].has_syntax());
            assert!(!fields[0].has_value_syntax());
            assert_eq!(fields[0].syntax_label_name(), None);
            assert!(
                fields[0]
                    .value_expression_payload(
                        fallback_fields[0]
                            .value
                            .as_ref()
                            .expect("fallback record field value"),
                    )
                    .expect("record field value payload")
                    .syntax_expression()
                    .is_none()
            );
        },
    );
}

#[test]
fn source_less_argument_payload_does_not_expose_cst_name() {
    with_cst_payload_compiler(
        r#"
fn take(value) {
    return value;
}

fn main() {
    let result = take(value = 1);
}
"#,
        |_, payload| {
            let call = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("call initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    call.syntax_expression().expect("call syntax").clone(),
                    call.fallback(),
                );
            let args = missing_source
                .call_argument_payloads()
                .expect("call argument payloads");

            assert!(args[0].syntax_argument().is_none());
            assert!(!args[0].has_value_syntax());
            assert_eq!(args[0].syntax_name(), None);
            let vela_syntax::ast::ExprKind::Call {
                args: fallback_args,
                ..
            } = &call.fallback().kind
            else {
                panic!("expected fallback call");
            };
            assert!(
                args[0]
                    .value_expression_payload(&fallback_args[0].value)
                    .syntax_expression()
                    .is_none()
            );
        },
    );
}

#[test]
fn source_less_interpolated_payload_does_not_expose_cst_expression_count() {
    with_cst_payload_compiler(
        r#"
fn main(value) {
    let text = f"{value}";
}
"#,
        |_, payload| {
            let interpolated = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("interpolated initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    interpolated
                        .syntax_expression()
                        .expect("interpolated syntax")
                        .clone(),
                    interpolated.fallback(),
                );
            let _fallback_parts = missing_source
                .fallback_interpolated_string_parts()
                .expect("fallback interpolated string parts");
            let expressions = missing_source
                .interpolated_expression_payloads()
                .expect("interpolated expression payloads");

            assert!(!missing_source.has_extra_interpolation_expressions());
            assert!(expressions[0].syntax_expression().is_none());
        },
    );
}

#[test]
fn source_less_statement_payload_does_not_expose_cst_value_kinds() {
    with_cst_payload_compiler(
        r#"
fn make(value) {
    return value;
}

fn main(target) {
    let value = target && make(target);
    target = value;
    value && true;
    return value && make(value);
}
"#,
        |_, payload| {
            let statements = payload.body.statement_payloads();
            let missing_let =
                body_payloads::CompilerStatementPayload::missing_child_payload_context(
                    statements[0]
                        .syntax_statement()
                        .expect("let syntax")
                        .clone(),
                    statements[0].fallback(),
                );
            let missing_assignment =
                body_payloads::CompilerStatementPayload::missing_child_payload_context(
                    statements[1]
                        .syntax_statement()
                        .expect("assignment syntax")
                        .clone(),
                    statements[1].fallback(),
                );
            let missing_expression =
                body_payloads::CompilerStatementPayload::missing_child_payload_context(
                    statements[2]
                        .syntax_statement()
                        .expect("expression syntax")
                        .clone(),
                    statements[2].fallback(),
                );
            let missing_return =
                body_payloads::CompilerStatementPayload::missing_child_payload_context(
                    statements[3]
                        .syntax_statement()
                        .expect("return syntax")
                        .clone(),
                    statements[3].fallback(),
                );

            assert_eq!(missing_let.statement_kind(), None);
            assert!(missing_let.syntax_statement().is_none());
            assert_eq!(missing_assignment.statement_kind(), None);
            assert!(missing_assignment.syntax_statement().is_none());
            assert_eq!(missing_expression.statement_kind(), None);
            assert!(missing_expression.syntax_statement().is_none());
            assert_eq!(missing_return.statement_kind(), None);
            assert!(missing_return.syntax_statement().is_none());
            assert_eq!(missing_let.let_initializer_kind(), None);
            assert_eq!(missing_assignment.syntax_assignment_operator(), None);
            assert_eq!(missing_assignment.assignment_value_kind(), None);
            assert!(
                missing_assignment
                    .assignment_target_expression_payload()
                    .is_none()
            );
            assert!(
                missing_assignment
                    .assignment_value_expression_payload()
                    .is_none()
            );
            assert_eq!(missing_expression.expression_kind(), None);
            assert_eq!(missing_expression.value_expression_kind(), None);
            assert_eq!(missing_return.return_value_kind(), None);
        },
    );
}

#[test]
fn source_less_for_payload_does_not_expose_child_payloads() {
    with_cst_payload_compiler(
        r#"
fn main(values) {
    for index, value in values {
        value;
    }
}
"#,
        |_, payload| {
            let statement = &payload.body.statement_payloads()[0];
            let missing_source =
                body_payloads::CompilerStatementPayload::missing_child_payload_context(
                    statement
                        .syntax_statement()
                        .expect("for statement syntax")
                        .clone(),
                    statement.fallback(),
                );

            assert!(missing_source.for_iterable_expression_payload().is_none());
            assert!(missing_source.for_index_pattern_payload().is_none());
            assert!(missing_source.for_value_pattern_payload().is_none());
        },
    );
}

#[test]
fn source_less_match_arm_payload_does_not_expose_cst_body_or_guard() {
    with_cst_payload_compiler(
        r#"
fn main(value) {
    return match value {
        _ if value > 0 => 1,
    };
}
"#,
        |_, payload| {
            let match_value = payload.body.statement_payloads()[0]
                .return_value_expression_payload()
                .expect("match return payload");
            let vela_syntax::ast::ExprKind::Match(fallback_match) = &match_value.fallback().kind
            else {
                panic!("expected fallback match expression");
            };
            let arms = match_value
                .match_arm_payloads()
                .expect("match arm payloads");
            let missing_arm = body_payloads::CompilerMatchArmPayload::missing_child_payload_context(
                arms[0].syntax_arm().expect("arm syntax").clone(),
                &fallback_match.arms[0],
            );

            assert!(!missing_arm.has_syntax());
            assert!(missing_arm.syntax_arm().is_none());
            let missing_pattern = missing_arm.pattern_payload();
            assert!(!missing_pattern.has_syntax());
            assert!(missing_pattern.syntax_pattern().is_none());
            assert_eq!(missing_arm.body_expression_kind(), None);
            assert_eq!(missing_pattern.syntax_pattern_kind(), None);
            assert!(
                missing_arm
                    .body_expression_payload()
                    .syntax_expression()
                    .is_none()
            );
            assert!(missing_arm.guard_payload().is_none());
        },
    );
}

#[test]
fn source_less_match_payload_does_not_expose_scrutinee_payload() {
    with_cst_payload_compiler(
        r#"
fn main(value) {
    return match value {
        _ => 1,
    };
}
"#,
        |_, payload| {
            let match_value = payload.body.statement_payloads()[0]
                .return_value_expression_payload()
                .expect("match return payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    match_value
                        .syntax_expression()
                        .expect("match syntax")
                        .clone(),
                    match_value.fallback(),
                );

            assert!(missing_source.match_scrutinee_payload().is_none());
            let missing_arms = missing_source
                .match_arm_payloads()
                .expect("source-less match arm payload shells");
            assert!(!missing_arms[0].has_syntax());
            assert!(!missing_arms[0].pattern_payload().has_syntax());
        },
    );
}

#[test]
fn source_less_statement_match_payload_does_not_expose_scrutinee_payload() {
    with_cst_payload_compiler(
        r#"
fn main(value) {
    match value {
        _ => 1,
    };
}
"#,
        |_, payload| {
            let statement = &payload.body.statement_payloads()[0];
            let missing_source =
                body_payloads::CompilerStatementPayload::missing_child_payload_context(
                    statement
                        .syntax_statement()
                        .expect("match statement syntax")
                        .clone(),
                    statement.fallback(),
                );

            assert!(missing_source.match_scrutinee_payload().is_none());
            let missing_arms = missing_source
                .match_arm_payloads()
                .expect("source-less statement match arm payload shells");
            assert!(!missing_arms[0].has_syntax());
            assert!(!missing_arms[0].pattern_payload().has_syntax());
            assert!(
                missing_source
                    .expression_match_payloads_with_fallback()
                    .is_none()
            );
        },
    );
}
