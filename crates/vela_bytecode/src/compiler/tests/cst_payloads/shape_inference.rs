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
