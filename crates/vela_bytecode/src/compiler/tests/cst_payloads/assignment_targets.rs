use super::*;

#[test]
fn missing_assignment_expression_payload_does_not_use_legacy_assignment() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = 1;
    value += 2;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = paired_statement_payloads_for_body(source, &payload.body);
    compiler
        .compile_statement_payload_for_test(&statements[0])
        .expect("local target should compile");
    let legacy_assignment = statements[1]
        .expression_payload()
        .expect("legacy assignment expression");
    let missing_assignment = body_payloads::CompilerExpressionPayload::missing_syntax(
        source,
        legacy_assignment.fallback(),
    );

    let error = compiler
        .compile_expr_with_payload(legacy_assignment.fallback(), Some(&missing_assignment))
        .expect_err("missing CST assignment payload must not compile legacy assignment");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST expression payload")
    ));
}

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
        selected;
        selected && true
    };
    legacy.amount = true;
}
"#,
        |compiler, payload| {
            let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("legacy local should compile");
            let cst_target = statements[1]
                .let_initializer_expression_payload()
                .expect("CST block initializer");
            let legacy_statement = statements[2]
                .expression_payload()
                .expect("legacy assignment expression");
            let ExprKind::Assign { target, .. } = &legacy_statement.fallback().kind else {
                panic!("expected legacy assignment fallback");
            };
            let mismatched_target = expression_payload_with_fallback(
                SourceId::new(1),
                cst_target
                    .syntax_expression()
                    .expect("CST block syntax")
                    .clone(),
                target,
            );

            let error = compiler
                .compile_assignment_with_payloads(
                    legacy_statement.fallback(),
                    crate::compiler::assignments::AssignmentTargetSyntax::new(Some(
                        &mismatched_target,
                    )),
                    crate::compiler::assignments::AssignmentValueSyntax::new(None, None, None),
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
fn record_path_assignment_without_cst_segments_does_not_use_legacy_target_path() {
    let source = SourceId::new(1);
    let cst_text = r#"
struct Box {
    amount: i64,
}

fn main() {
    let box = Box { amount: 0 };
    self = 1;
}
"#;
    let cst_semantic = parse_semantic_source(source, cst_text).expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("main function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
struct Box {
    amount: i64,
}

fn main() {
    let box = Box { amount: 0 };
    box::amount = 1;
}
"#,
        |compiler, payload| {
            let statements = body_payloads::CompilerBodyPayload::paired_statement_payloads_for_test(
                source,
                cst_body,
                fallback_statements_for_body(source, &payload.body),
            );
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("record local should compile");
            let assignment = statements[1]
                .expression_payload()
                .expect("assignment expression payload");
            let target = statements[1]
                .expression_payload()
                .and_then(|payload| payload.assignment_target_payload())
                .expect("assignment target payload");

            let error = compiler
                .compile_assignment_with_payloads(
                    assignment.fallback(),
                    crate::compiler::assignments::AssignmentTargetSyntax::new(Some(&target)),
                    crate::compiler::assignments::AssignmentValueSyntax::new(None, None, None),
                )
                .expect_err("missing CST assignment path must not use legacy target path");

            assert_eq!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST assignment target")
            );
        },
    );
}

#[test]
fn assignment_value_with_misaligned_cst_payload_does_not_use_legacy_value() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let value = [];
    let cst_value = [true];
        value = [1];
}
"#,
        |compiler, payload| {
            let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("local target should compile");
            let cst_value = statements[1]
                .let_initializer_expression_payload()
                .expect("CST array initializer");
            let legacy_assignment = statements[2]
                .expression_payload()
                .expect("legacy assignment expression");

            let error = compiler
                .compile_assignment_with_payloads(
                    legacy_assignment.fallback(),
                    crate::compiler::assignments::AssignmentTargetSyntax::new(None),
                    crate::compiler::assignments::AssignmentValueSyntax::new(
                        Some(SyntaxExpressionKind::Array),
                        None,
                        Some(&cst_value),
                    ),
                )
                .expect_err("misaligned CST assignment value must not use legacy value");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST assignment value")
            ));
        },
    );
}

#[test]
fn unclassified_assignment_value_payload_does_not_use_legacy_value() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let value = 1;
        value = 2;
}
"#,
        |compiler, payload| {
            let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("local target should compile");
            let assignment = statements[1]
                .expression_payload()
                .expect("assignment expression payload");
            let value = statements[1]
                .expression_payload()
                .and_then(|payload| payload.assignment_value_payload())
                .expect("assignment value expression payload");
            let unclassified_value =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    value
                        .syntax_expression()
                        .expect("assignment value syntax")
                        .clone(),
                );

            let error = compiler
                .compile_assignment_with_payloads(
                    assignment.fallback(),
                    crate::compiler::assignments::AssignmentTargetSyntax::new(None),
                    crate::compiler::assignments::AssignmentValueSyntax::new(
                        None,
                        None,
                        Some(&unclassified_value),
                    ),
                )
                .expect_err("unclassified CST assignment value must not use legacy value");

            assert_eq!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST assignment value")
            );
        },
    );
}

#[test]
fn assignment_block_value_without_body_payload_compiles_from_parent_cst() {
    assert_assignment_block_value_without_child_payloads_compiles(
        r#"
fn main() {
    let value = 1;
    value = { value + 1 };
}
"#,
    );
}

#[test]
fn assignment_if_value_without_child_payloads_compiles_from_parent_cst() {
    assert_assignment_if_value_without_child_payloads_compiles(
        r#"
fn main() {
    let value = 1;
    value = if true { 2 } else { 3 };
}
"#,
    );
}

#[test]
fn assignment_match_value_without_child_payloads_compiles_from_parent_cst() {
    assert_assignment_match_value_without_child_payloads_compiles(
        r#"
fn main() {
    let value = 1;
    value = match value {
        1 => 2,
        _ => 3,
    };
}
"#,
    );
}

#[test]
fn typed_field_assignment_block_value_without_body_payload_compiles_from_parent_cst() {
    assert_typed_field_assignment_block_value_without_child_payloads_compiles(
        "box.value = { value + 1 };",
    );
}

#[test]
fn typed_field_assignment_if_value_without_child_payloads_compiles_from_parent_cst() {
    assert_typed_field_assignment_if_value_without_child_payloads_compiles(
        "box.value = if true { 2 } else { 3 };",
    );
}

#[test]
fn typed_field_assignment_match_value_without_child_payloads_compiles_from_parent_cst() {
    assert_typed_field_assignment_match_value_without_child_payloads_compiles(
        "box.value = match value { 1 => 2, _ => 3 };",
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
            let statements = body_payloads::CompilerBodyPayload::paired_statement_payloads_for_test(
                source,
                cst_body,
                fallback_statements_for_body(source, &payload.body),
            );

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

fn assert_assignment_block_value_without_child_payloads_compiles(text: &str) {
    with_cst_payload_compiler(text, |compiler, payload| {
        let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
        compiler
            .compile_statement_payload_for_test(&statements[0])
            .expect("local target should compile");
        let assignment = statements[1]
            .expression_payload()
            .expect("assignment expression payload");
        let value = statements[1]
            .expression_payload()
            .and_then(|payload| payload.assignment_value_payload())
            .expect("assignment value expression payload");

        compiler
            .compile_assignment_with_payloads(
                assignment.fallback(),
                crate::compiler::assignments::AssignmentTargetSyntax::new(None),
                crate::compiler::assignments::AssignmentValueSyntax::new(
                    Some(SyntaxExpressionKind::Block),
                    None,
                    Some(&value),
                ),
            )
            .expect("assignment block value should compile from parent CST payload");
    });
}

fn assert_assignment_match_value_without_child_payloads_compiles(text: &str) {
    with_cst_payload_compiler(text, |compiler, payload| {
        let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
        compiler
            .compile_statement_payload_for_test(&statements[0])
            .expect("local target should compile");
        let assignment = statements[1]
            .expression_payload()
            .expect("assignment expression payload");
        let value = assignment
            .assignment_value_payload()
            .expect("assignment value expression payload");

        compiler
            .compile_assignment_with_payloads(
                assignment.fallback(),
                crate::compiler::assignments::AssignmentTargetSyntax::new(None),
                crate::compiler::assignments::AssignmentValueSyntax::new(
                    Some(SyntaxExpressionKind::Match),
                    None,
                    Some(&value),
                ),
            )
            .expect("assignment match value should compile from parent CST payload");
    });
}

fn assert_assignment_if_value_without_child_payloads_compiles(text: &str) {
    with_cst_payload_compiler(text, |compiler, payload| {
        let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
        compiler
            .compile_statement_payload_for_test(&statements[0])
            .expect("local target should compile");
        let assignment = statements[1]
            .expression_payload()
            .expect("assignment expression payload");
        let value = assignment
            .assignment_value_payload()
            .expect("assignment value expression payload");

        compiler
            .compile_assignment_with_payloads(
                assignment.fallback(),
                crate::compiler::assignments::AssignmentTargetSyntax::new(None),
                crate::compiler::assignments::AssignmentValueSyntax::new(
                    Some(SyntaxExpressionKind::If),
                    None,
                    Some(&value),
                ),
            )
            .expect("assignment if value should compile from parent CST payload");
    });
}

fn assert_typed_field_assignment_block_value_without_child_payloads_compiles(assignment: &str) {
    let text = format!(
        r#"
struct Box {{
    value: i64,
}}

fn main() {{
    let value = 1;
    let box = Box {{ value: 0 }};
    {assignment}
}}
"#
    );
    with_cst_payload_compiler(&text, |compiler, payload| {
        let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
        compiler
            .compile_statement_payload_for_test(&statements[0])
            .expect("typed value local should compile");
        compiler
            .compile_statement_payload_for_test(&statements[1])
            .expect("typed record local should compile");
        let assignment = statements[2]
            .expression_payload()
            .expect("field assignment expression payload");
        let value = statements[2]
            .expression_payload()
            .and_then(|payload| payload.assignment_value_payload())
            .expect("field assignment value expression payload");

        compiler
            .compile_assignment_with_payloads(
                assignment.fallback(),
                crate::compiler::assignments::AssignmentTargetSyntax::new(None),
                crate::compiler::assignments::AssignmentValueSyntax::new(
                    Some(SyntaxExpressionKind::Block),
                    None,
                    Some(&value),
                ),
            )
            .expect("typed field assignment block value should compile from parent CST payload");
    });
}

fn assert_typed_field_assignment_if_value_without_child_payloads_compiles(assignment: &str) {
    let text = format!(
        r#"
struct Box {{
    value: i64,
}}

fn main() {{
    let value = 1;
    let box = Box {{ value: 0 }};
    {assignment}
}}
"#
    );
    with_cst_payload_compiler(&text, |compiler, payload| {
        let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
        compiler
            .compile_statement_payload_for_test(&statements[0])
            .expect("typed value local should compile");
        compiler
            .compile_statement_payload_for_test(&statements[1])
            .expect("typed record local should compile");
        let assignment = statements[2]
            .expression_payload()
            .expect("field assignment expression payload");
        let value = assignment
            .assignment_value_payload()
            .expect("field assignment value expression payload");

        compiler
            .compile_assignment_with_payloads(
                assignment.fallback(),
                crate::compiler::assignments::AssignmentTargetSyntax::new(None),
                crate::compiler::assignments::AssignmentValueSyntax::new(
                    Some(SyntaxExpressionKind::If),
                    None,
                    Some(&value),
                ),
            )
            .expect("typed field assignment if value should compile from parent CST payload");
    });
}

fn assert_typed_field_assignment_match_value_without_child_payloads_compiles(assignment: &str) {
    let text = format!(
        r#"
struct Box {{
    value: i64,
}}

fn main() {{
    let value = 1;
    let box = Box {{ value: 0 }};
    {assignment}
}}
"#
    );
    with_cst_payload_compiler(&text, |compiler, payload| {
        let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
        compiler
            .compile_statement_payload_for_test(&statements[0])
            .expect("typed value local should compile");
        compiler
            .compile_statement_payload_for_test(&statements[1])
            .expect("typed record local should compile");
        let assignment = statements[2]
            .expression_payload()
            .expect("field assignment expression payload");
        let value = assignment
            .assignment_value_payload()
            .expect("field assignment value expression payload");

        compiler
            .compile_assignment_with_payloads(
                assignment.fallback(),
                crate::compiler::assignments::AssignmentTargetSyntax::new(None),
                crate::compiler::assignments::AssignmentValueSyntax::new(
                    Some(SyntaxExpressionKind::Match),
                    None,
                    Some(&value),
                ),
            )
            .expect("typed field assignment match value should compile from parent CST payload");
    });
}

#[test]
fn nested_assignment_expression_lowering_prefers_cst_operator_payload() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let value = 1;
    let values = [value -= 2];
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
    let values = [value += 2];
    return value;
}
"#,
        |compiler, payload| {
            let statements = body_payloads::CompilerBodyPayload::paired_statement_payloads_for_test(
                source,
                cst_body,
                fallback_statements_for_body(source, &payload.body),
            );

            compiler
                .compile_statement_payloads(&statements)
                .expect("CST-backed nested assignment expression should compile");

            assert!(
                compiler.code.instructions.iter().any(|instruction| {
                    matches!(
                        instruction.kind,
                        UnlinkedInstructionKind::Sub { .. }
                            | UnlinkedInstructionKind::I64Sub { .. }
                            | UnlinkedInstructionKind::I64SubImm { .. }
                    )
                }),
                "nested assignment expression should use the CST operator"
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
                "nested assignment expression should not use the legacy fallback operator"
            );
        },
    );
}

#[test]
fn value_position_assignment_lowering_prefers_cst_operator_payload() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let value = 5;
    let assigned = value -= 2;
    return value -= 1;
}
"#;
    let cst_semantic = parse_semantic_source(source, cst_text).expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("main function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
fn main() {
    let value = 5;
    let assigned = value += 2;
    return value += 1;
}
"#,
        |compiler, payload| {
            let statements = body_payloads::CompilerBodyPayload::paired_statement_payloads_for_test(
                source,
                cst_body,
                fallback_statements_for_body(source, &payload.body),
            );

            compiler
                .compile_statement_payloads(&statements)
                .expect("CST-backed value-position assignments should compile");

            assert!(
                compiler.code.instructions.iter().any(|instruction| {
                    matches!(
                        instruction.kind,
                        UnlinkedInstructionKind::Sub { .. }
                            | UnlinkedInstructionKind::I64Sub { .. }
                            | UnlinkedInstructionKind::I64SubImm { .. }
                    )
                }),
                "value-position assignment expressions should use the CST operator"
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
                "value-position assignment expressions should not use the legacy fallback operator"
            );
        },
    );
}
