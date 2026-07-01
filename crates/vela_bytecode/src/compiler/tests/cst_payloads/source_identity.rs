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
