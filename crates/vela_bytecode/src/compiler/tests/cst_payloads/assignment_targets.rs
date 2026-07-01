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

#[test]
fn local_assignment_operator_lowering_prefers_cst_operator_payload() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let value = 1;
    value -= 2;
    return value;
}
"#;
    let cst_semantic = parse_semantic_source(source, cst_text).expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("main function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
fn main() {
    let value = 1;
    value += 2;
    return value;
}
"#,
        |compiler, payload| {
            let mismatched_body = body_payloads::CompilerBodyPayload::syntax(
                source,
                cst_body,
                payload.body.fallback(),
            );
            let statements = mismatched_body.statement_payloads();

            compiler
                .compile_statement_payloads(&statements)
                .expect("CST-backed assignment expression should compile");

            assert!(
                compiler.code.instructions.iter().any(|instruction| {
                    matches!(
                        instruction.kind,
                        UnlinkedInstructionKind::Sub { .. }
                            | UnlinkedInstructionKind::I64Sub { .. }
                            | UnlinkedInstructionKind::I64SubImm { .. }
                    )
                }),
                "assignment expression should use the CST operator"
            );
            assert!(
                compiler.code.instructions.iter().all(|instruction| {
                    !matches!(
                        instruction.kind,
                        UnlinkedInstructionKind::Add { .. }
                            | UnlinkedInstructionKind::I64Add { .. }
                            | UnlinkedInstructionKind::I64AddImm { .. }
                    )
                }),
                "assignment expression should not use the legacy fallback operator"
            );
        },
    );
}
