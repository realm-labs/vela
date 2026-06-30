use super::*;

#[test]
fn field_shape_inference_with_non_field_cst_payload_does_not_use_legacy_field() {
    with_cst_payload_compiler(
        r#"
struct LegacyBox {
    amount: i64,
}

fn main() {
    let legacy = LegacyBox { amount: 1 };
    let cst_block = {
        let selected = legacy;
        selected
    };
    let legacy_amount = legacy.amount;
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            compiler
                .compile_statement(statements[0].fallback())
                .expect("legacy local should compile");
            let cst_block = statements[1]
                .let_initializer_expression_payload()
                .expect("CST block initializer");
            let legacy_field = statements[2]
                .let_initializer_expression_payload()
                .expect("legacy field fallback");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_block
                    .syntax_expression()
                    .expect("CST block syntax")
                    .clone(),
                legacy_field.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                None,
                "non-field CST payload must not use the legacy field shape"
            );
            assert_eq!(
                compiler.record_field_value_type_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                None,
                "non-field CST payload must not use the legacy field value type"
            );
        },
    );
}

#[test]
fn shape_inference_with_unsupported_cst_payload_does_not_use_legacy_shape() {
    with_cst_payload_compiler(
        r#"
struct LegacyBox {
    amount: i64,
}

fn main(input) {
    let cst_binary = input + 1;
    let legacy = LegacyBox { amount: 1 };
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let cst_binary = statements[0]
                .let_initializer_expression_payload()
                .expect("CST binary initializer");
            let legacy_record = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy record initializer");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_binary
                    .syntax_expression()
                    .expect("CST binary syntax")
                    .clone(),
                legacy_record.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                None,
                "unsupported CST payload must not use the legacy record shape"
            );
        },
    );
}

#[test]
fn paren_shape_inference_prefers_inner_cst_payload_shape() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_paren = (["cst"]);
    let legacy_array = [true];
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let cst_paren = statements[0]
                .let_initializer_expression_payload()
                .expect("CST parenthesized initializer");
            let legacy_array = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy array fallback");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_paren
                    .syntax_expression()
                    .expect("CST parenthesized syntax")
                    .clone(),
                legacy_array.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                Some(record_shapes::ValueShape::Array(Box::new(
                    record_shapes::ValueShape::Scalar("String".to_owned())
                ))),
                "parenthesized CST payload must not use the old fallback array shape"
            );
        },
    );
}

#[test]
fn native_call_shape_inference_prefers_cst_payload_shape() {
    with_cst_payload_compiler(
        r#"
struct LegacyBox {
    amount: i64,
}

fn main() {
    let cst_call = option::some(["cst"]);
    let legacy_call = result::ok(LegacyBox { amount: 1 });
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let cst_call = statements[0]
                .let_initializer_expression_payload()
                .expect("CST call initializer");
            let legacy_call = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy call fallback");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_call
                    .syntax_expression()
                    .expect("CST call syntax")
                    .clone(),
                legacy_call.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                Some(record_shapes::ValueShape::Option(Box::new(
                    record_shapes::ValueShape::Array(Box::new(record_shapes::ValueShape::Scalar(
                        "String".to_owned()
                    )))
                ))),
                "call-shaped CST payload must not use the old fallback call shape"
            );
        },
    );
}

#[test]
fn index_shape_inference_prefers_cst_payload_shape() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_array_index = ["cst"][0];
    let legacy_array_index = [true][0];
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let cst_array_index = statements[0]
                .let_initializer_expression_payload()
                .expect("CST array index initializer");
            let legacy_array_index = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy array index fallback");

            let mismatched_array_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_array_index
                    .syntax_expression()
                    .expect("CST array index syntax")
                    .clone(),
                legacy_array_index.fallback(),
            );
            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_array_payload.fallback(),
                    Some(&mismatched_array_payload),
                ),
                Some(record_shapes::ValueShape::Scalar("String".to_owned())),
                "array-index CST payload must not use the old fallback index shape"
            );
        },
    );
}
