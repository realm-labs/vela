use super::*;

#[test]
fn source_less_field_payload_does_not_expose_cst_name() {
    with_cst_payload_compiler(
        r#"
fn main(object) {
    let value = object.amount;
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
        },
    );
}

#[test]
fn source_less_binary_payload_does_not_expose_cst_operator() {
    with_cst_payload_compiler(
        r#"
fn main(left, right) {
    let total = left + right;
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
            assert!(missing_source.binary_operand_payloads().is_none());
        },
    );
}

#[test]
fn source_less_unary_payload_does_not_expose_operand_payload() {
    with_cst_payload_compiler(
        r#"
fn main(value) {
    let negated = -value;
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
            assert!(missing_source.unary_operand_payload().is_none());
        },
    );
}

#[test]
fn source_less_try_payload_does_not_expose_operand_payload() {
    with_cst_payload_compiler(
        r#"
fn main(value) {
    let result = value?;
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

            assert!(missing_source.try_operand_payload().is_none());
        },
    );
}

#[test]
fn source_less_logical_chain_payload_does_not_expose_operand_payloads() {
    with_cst_payload_compiler(
        r#"
fn main(left, middle, right) {
    let value = left && middle && right;
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
            let entries = missing_source
                .map_entry_payloads()
                .expect("map entry payloads");

            assert_eq!(entries[0].syntax_key_name(), None);
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
            let fields = missing_source
                .record_field_payloads()
                .expect("record field payloads");

            assert_eq!(fields[0].syntax_label_name(), None);
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

            assert_eq!(args[0].syntax_name(), None);
        },
    );
}

#[test]
fn source_less_statement_payload_does_not_expose_cst_value_kinds() {
    with_cst_payload_compiler(
        r#"
fn main(target) {
    let value = 1;
    target = value;
    value;
    return value;
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

            assert_eq!(missing_let.let_initializer_kind(), None);
            assert_eq!(missing_assignment.assignment_value_kind(), None);
            assert_eq!(missing_expression.expression_kind(), None);
            assert_eq!(missing_expression.value_expression_kind(), None);
            assert_eq!(missing_return.return_value_kind(), None);
        },
    );
}

#[test]
fn source_less_match_arm_payload_does_not_expose_cst_body_kind() {
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

            assert_eq!(missing_arm.body_expression_kind(), None);
        },
    );
}
