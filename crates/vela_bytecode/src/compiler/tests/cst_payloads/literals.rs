use super::*;

#[test]
fn semantic_function_literal_expressions_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn take(first, second, third) {
    return first;
}

fn literal_values() {
    let count = 42;
    take("gold", true, 3.5);
    return 'x';
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("literal_values")
        .expect("literal_values function");

    assert_cst_let_initializer_literals(&payload.body, &[literal_int("42")]);
    assert_cst_call_argument_literals(
        &payload.body,
        &[
            vela_syntax::ast::Literal::String("gold".to_owned()),
            vela_syntax::ast::Literal::Bool(true),
            vela_syntax::ast::Literal::float("3.5"),
        ],
    );
    assert_cst_return_value_literals(&payload.body, &[vela_syntax::ast::Literal::Char('x')]);

    compile_program_source(source, text).expect("CST-backed literal expressions should compile");
}

#[test]
fn missing_literal_expression_payload_does_not_use_legacy_literal() {
    let source = SourceId::new(1);
    let text = r#"
fn take(value) {
    return value;
}

fn main(input) {
    take(42);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_literal = first_call_argument_value_payload(&payload.body, 0);
    let missing_literal =
        body_payloads::CompilerExpressionPayload::missing_syntax(source, legacy_literal.fallback());

    let error = compiler
        .compile_expr_with_payload(legacy_literal.fallback(), Some(&missing_literal))
        .expect_err("missing CST literal payload must not compile legacy literal");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST expression payload")
    ));
}

#[test]
fn missing_literal_type_payload_does_not_use_legacy_type() {
    let source = SourceId::new(1);
    let text = r#"
fn take(value) {
    return value;
}

fn main(input) {
    take(true);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let literal = first_call_argument_value_payload(&payload.body, 0);
    let missing_literal =
        body_payloads::CompilerExpressionPayload::missing_syntax(source, literal.fallback());

    assert_eq!(
        compiler
            .static_type_for_expr_with_payload(missing_literal.fallback(), Some(&missing_literal),),
        value_types::StaticExprType::Dynamic,
        "missing source-backed CST payload must not use the legacy literal type"
    );
}

#[test]
fn static_literal_type_facts_reject_mismatched_cst_payloads() {
    with_cst_payload_compiler(
        r#"
fn take(value) {
    return value;
}

fn main() {
    take(true);
    take(1);
}
"#,
        |compiler, payload| {
            let cst_literal = first_call_argument_value_payload(&payload.body, 0);
            let legacy_literal = first_call_argument_value_payload(&payload.body, 1);
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_literal
                    .syntax_expression()
                    .expect("CST expression")
                    .clone(),
                legacy_literal.fallback(),
            );

            assert_eq!(
                compiler.static_type_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                value_types::StaticExprType::Dynamic
            );
            assert_eq!(
                compiler
                    .expected_type_for_expr_with_payload(
                        mismatched_payload.fallback(),
                        RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Bool),
                        value_types::TypeContractContext::TypedLet {
                            name: "cst_value".to_owned(),
                        },
                        Some(&mismatched_payload),
                    )
                    .expect("mismatched literal should require a runtime guard"),
                value_types::ExpectedTypeOutcome::RequiresRuntimeGuard(RuntimeTypeFact::primitive(
                    vela_common::PrimitiveTag::Bool
                ),)
            );
        },
    );
}

#[test]
fn literal_expression_payload_mismatch_does_not_use_legacy_literal() {
    with_cst_payload_compiler(
        r#"
fn take(value) {
    return value;
}

fn main(input) {
    take(true);
    take(input);
    take(1);
}
"#,
        |compiler, payload| {
            let cst_literal = first_call_argument_value_payload(&payload.body, 0);
            let non_literal = first_call_argument_value_payload(&payload.body, 1);
            let legacy_literal = first_call_argument_value_payload(&payload.body, 2);
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_literal
                    .syntax_expression()
                    .expect("CST literal expression")
                    .clone(),
                non_literal.fallback(),
            );

            let error = compiler
                .compile_expr_with_payload(legacy_literal.fallback(), Some(&mismatched_payload))
                .expect_err("mismatched literal payload should not use the legacy literal");

            assert_eq!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST literal expression")
            );
        },
    );
}

#[test]
fn literal_payload_value_comes_from_cst_without_literal_fallback() {
    with_cst_payload_compiler(
        r#"
fn take(value) {
    return value;
}

fn main(input) {
    take(true);
    take(input);
}
"#,
        |_compiler, payload| {
            let cst_literal = first_call_argument_value_payload(&payload.body, 0);
            let fallback_path = first_call_argument_value_payload(&payload.body, 1);
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_literal
                    .syntax_expression()
                    .expect("CST literal expression")
                    .clone(),
                fallback_path.fallback(),
            );

            assert_eq!(
                mismatched_payload.syntax_literal(),
                Some(vela_syntax::ast::Literal::Bool(true))
            );
            let missing_source_literal =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    cst_literal
                        .syntax_expression()
                        .expect("CST literal expression")
                        .clone(),
                    fallback_path.fallback(),
                );
            assert_eq!(missing_source_literal.syntax_literal(), None);
        },
    );
}

#[test]
fn contextual_literal_payload_mismatch_requires_runtime_guard() {
    with_cst_payload_compiler(
        r#"
struct Box {
    amount: i64
}

fn main(box: Box) {
    take(box.amount);
    take(10);
}

fn take(value) {
    return value;
}
"#,
        |compiler, payload| {
            let cst_field = first_call_argument_value_payload(&payload.body, 0);
            let legacy_literal = first_call_argument_value_payload(&payload.body, 1);
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_field
                    .syntax_expression()
                    .expect("CST field expression")
                    .clone(),
                legacy_literal.fallback(),
            );

            compiler
                .compile_expr_with_expected_type_and_payload(
                    legacy_literal.fallback(),
                    RuntimeTypeFact::primitive(vela_common::PrimitiveTag::I64),
                    value_types::TypeContractContext::TypedLet {
                        name: "value".to_owned(),
                    },
                    Some(&mismatched_payload),
                )
                .expect("mismatched contextual payload should compile through dynamic guard");

            assert!(
                compiler
                    .code
                    .instructions
                    .iter()
                    .any(|instruction| matches!(
                        instruction.kind,
                        UnlinkedInstructionKind::GuardType { .. }
                    )),
                "mismatched contextual payload should not be treated as a proven literal"
            );
        },
    );
}

#[test]
fn typed_let_and_return_values_prefer_cst_literal_payloads() {
    let cst_semantic = parse_semantic_source(
        SourceId::new(1),
        r#"
fn main() -> bool {
    let value: bool = true;
    return true;
}
"#,
    )
    .expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("main function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
fn main(input) -> bool {
    let value: bool = input == 0;
    return input == 0;
}
"#,
        |compiler, payload| {
            let statements = body_payloads::CompilerBodyPayload::paired_statement_payloads_for_test(
                SourceId::new(1),
                cst_body,
                fallback_statements_for_body(SourceId::new(1), &payload.body),
            );

            compiler
                .compile_statement_payloads(&statements)
                .expect("typed let and return should use CST literal payloads");
        },
    );
}

#[test]
fn typed_control_flow_values_use_cst_static_facts_without_guards() {
    let cst_semantic = parse_semantic_source(
        SourceId::new(1),
        r#"
fn main(input) -> bool {
    let from_block: bool = { true };
    let from_if: bool = if input { true } else { false };
    let from_match: bool = match input {
        true => true,
        false => false,
    };
    return if input { true } else { false };
}
"#,
    )
    .expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("main function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
fn main(input) -> bool {
    let from_block: bool = { 1 };
    let from_if: bool = if input { 1 } else { 2 };
    let from_match: bool = match input {
        true => 1,
        false => 2,
    };
    return if input { 1 } else { 2 };
}
"#,
        |compiler, payload| {
            let statements = body_payloads::CompilerBodyPayload::paired_statement_payloads_for_test(
                SourceId::new(1),
                cst_body,
                fallback_statements_for_body(SourceId::new(1), &payload.body),
            );

            compiler
                .compile_statement_payloads(&statements)
                .expect("typed control-flow values should use CST static facts");

            assert!(
                !compiler
                    .code
                    .instructions
                    .iter()
                    .any(|instruction| matches!(
                        instruction.kind,
                        UnlinkedInstructionKind::GuardType { .. }
                    )),
                "CST-proven bool control-flow contracts should not emit runtime guards"
            );
        },
    );
}

#[test]
fn typed_numeric_literal_constants_prefer_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: i8 = 12;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("CST source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");

    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("typed numeric literal should use CST literal payload");

    assert!(
        compiler
            .code
            .constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I8(12))),
        "typed contextual constant should come from the CST literal"
    );
}

fn assert_cst_let_initializer_literals(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[vela_syntax::ast::Literal],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::let_initializer_syntax_literal)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn assert_cst_call_argument_literals(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[vela_syntax::ast::Literal],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.call_argument_value_payloads().unwrap_or_default())
        .filter_map(literal_payload_value)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn assert_cst_return_value_literals(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[vela_syntax::ast::Literal],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::return_value_syntax_literal)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn literal_payload_value(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Option<vela_syntax::ast::Literal> {
    assert_eq!(payload.kind(), Some(SyntaxExpressionKind::Literal));
    assert_eq!(
        payload
            .syntax_expression()
            .and_then(|expression| expression.as_literal())
            .and_then(|literal| literal.literal()),
        payload.syntax_literal()
    );
    payload.syntax_literal()
}

fn first_call_argument_value_payload<'ast>(
    body: &body_payloads::CompilerBodyPayload<'ast>,
    statement_index: usize,
) -> body_payloads::CompilerExpressionPayload<'ast> {
    body.statement_payloads()[statement_index]
        .call_argument_value_payloads()
        .expect("call argument payloads")
        .into_iter()
        .next()
        .expect("call argument")
}

fn literal_int(text: &str) -> vela_syntax::ast::Literal {
    vela_syntax::ast::Literal::integer(text)
}
