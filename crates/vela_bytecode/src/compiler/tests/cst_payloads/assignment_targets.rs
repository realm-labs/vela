use super::*;

#[test]
fn record_assignment_with_non_field_cst_payload_does_not_use_legacy_field_target() {
    with_cst_payload_compiler(
        r#"
struct LegacyBox {
    amount: bool,
}

fn main() {
    let legacy = LegacyBox { amount: false };
    let cst_target = {
        let selected = legacy;
        selected
    };
    legacy.amount = true;
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            compiler
                .compile_statement(statements[0].fallback())
                .expect("legacy local should compile");
            let cst_target = statements[1]
                .let_initializer_expression_payload()
                .expect("CST block initializer");
            let legacy_statement = statements[2]
                .expression_payload()
                .expect("legacy assignment expression");
            let legacy_target = statements[2]
                .assignment_target_expression_payload()
                .expect("legacy assignment target fallback");
            let mismatched_target = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_target
                    .syntax_expression()
                    .expect("CST block syntax")
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
                .expect_err("non-field CST target must not use the legacy field fallback");

            assert!(
                matches!(error.kind, CompileErrorKind::UnsupportedSyntax(_)),
                "{:?}",
                error.kind
            );
        },
    );
}
