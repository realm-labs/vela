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
fn static_literal_type_facts_prefer_cst_payloads() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_value = true;
    let legacy_value = 1;
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let cst_literal = statements[0]
                .let_initializer_expression_payload()
                .expect("CST literal payload");
            let legacy_literal = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy fallback literal");
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
                value_types::StaticExprType::Exact(RuntimeTypeFact::primitive(
                    vela_common::PrimitiveTag::Bool,
                ))
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
                    .expect("CST literal should satisfy bool contract"),
                value_types::ExpectedTypeOutcome::Proven
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
fn main() -> bool {
    let value: bool = 1;
    return 1;
}
"#,
        |compiler, payload| {
            let mismatched_body = body_payloads::CompilerBodyPayload::syntax(
                SourceId::new(1),
                cst_body,
                payload.body.fallback(),
            );
            let statements = mismatched_body.statement_payloads();

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
            let mismatched_body = body_payloads::CompilerBodyPayload::syntax(
                SourceId::new(1),
                cst_body,
                payload.body.fallback(),
            );
            let statements = mismatched_body.statement_payloads();

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
    let cst_semantic = parse_semantic_source(
        SourceId::new(1),
        r#"
fn main() {
    let value: i8 = 12;
}
"#,
    )
    .expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("main function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
fn main() {
    let value: i8 = true;
}
"#,
        |compiler, payload| {
            let mismatched_body = body_payloads::CompilerBodyPayload::syntax(
                SourceId::new(1),
                cst_body,
                payload.body.fallback(),
            );
            let statements = mismatched_body.statement_payloads();

            compiler
                .compile_statement_payloads(&statements)
                .expect("typed numeric literal should use CST literal payload");

            assert!(
                compiler
                    .code
                    .constants
                    .contains(&Constant::Scalar(vela_common::ScalarValue::I8(12))),
                "typed contextual constant should come from the CST literal"
            );
        },
    );
}

fn assert_cst_let_initializer_literals(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[vela_syntax::ast::Literal],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter_map(literal_payload_value)
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
        .flat_map(|statement| statement.call_argument_payloads().unwrap_or_default())
        .map(|argument| argument.value_expression_payload())
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
        .filter_map(|statement| statement.return_value_expression_payload())
        .filter_map(literal_payload_value)
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

fn literal_int(text: &str) -> vela_syntax::ast::Literal {
    vela_syntax::ast::Literal::integer(text)
}
