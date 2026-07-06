use super::*;

fn control_flow_statement_payloads<'ast>(
    body: &body_payloads::CompilerBodyPayload<'ast>,
) -> Vec<body_payloads::CompilerStatementPayload<'ast>> {
    paired_statement_payloads_for_body(body.syntax_payload().source, body)
}

#[test]
fn missing_if_expression_payload_does_not_use_legacy_if_body() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let value = if true {
        1
    } else {
        2
    };
}
"#;
    let legacy_text = r#"
fn main() {
    let value = if true {
        3
    } else {
        4
    };
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_if = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body")
        .statements()
        .next()
        .expect("CST let statement")
        .as_let()
        .expect("CST let")
        .initializer()
        .expect("CST initializer");
    assert!(cst_if.as_if().is_some());

    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_if = control_flow_statement_payloads(&legacy_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("legacy if initializer");
    let missing = body_payloads::CompilerExpressionPayload::missing_child_payload_context(
        cst_if,
        legacy_if.fallback(),
    );

    let error = compiler
        .compile_expr_with_payload(legacy_if.fallback(), Some(&missing))
        .expect_err("missing CST if payload must not compile legacy if body");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST if expression payload")
    ));
}

#[test]
fn missing_let_initializer_if_expression_payload_does_not_use_legacy_if_body() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let value = if true {
        1
    } else {
        2
    };
}
"#,
        |compiler, payload| {
            let statements = control_flow_statement_payloads(&payload.body);
            let statement = statements[0].syntax_statement().expect("CST let").clone();
            let missing_child =
                body_payloads::CompilerStatementPayload::missing_let_child_payload_context(
                    statement,
                    statements[0]
                        .let_initializer_fallback_for_test()
                        .expect("let initializer fallback"),
                );

            let error = compiler
                .compile_statement_payload_for_test(&missing_child)
                .expect_err("missing CST let if payload must not compile legacy if body");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST let initializer if payload")
            ));
        },
    );
}

#[test]
fn let_initializer_match_compiles_from_parent_cst_without_child_payloads() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let value = match 1 {
        1 => 2,
        _ => 3,
    };
}
"#,
        |compiler, payload| {
            let expression = control_flow_statement_payloads(&payload.body)[0]
                .let_initializer_expression_payload()
                .expect("CST match initializer");

            let (register, returned) = compiler
                .compile_let_initializer_value_payload_for_test(
                    expression.fallback(),
                    Some(&expression),
                )
                .expect("parent CST match payload should compile without legacy match body");

            assert_eq!(register, Register(0));
            assert!(!returned);
        },
    );
}

#[test]
fn missing_return_if_payload_does_not_use_legacy_if_body() {
    with_cst_payload_compiler(
        r#"
fn main() {
    return if true {
        1
    } else {
        2
    };
}
"#,
        |compiler, payload| {
            let statements = control_flow_statement_payloads(&payload.body);
            let statement = statements[0]
                .syntax_statement()
                .expect("CST return")
                .clone();
            let missing_child =
                body_payloads::CompilerStatementPayload::missing_return_child_payload_context(
                    statement,
                    statements[0]
                        .return_value_fallback_for_test()
                        .expect("return value fallback"),
                );

            let error = compiler
                .compile_statement_payload_for_test(&missing_child)
                .expect_err("missing CST return if payload must not compile legacy if body");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST return if payload")
            ));
        },
    );
}

#[test]
fn return_match_compiles_from_parent_cst_without_child_payloads() {
    with_cst_payload_compiler(
        r#"
fn main() {
    return match 1 {
        1 => 2,
        _ => 3,
    };
}
"#,
        |compiler, payload| {
            let expression = control_flow_statement_payloads(&payload.body)[0]
                .return_value_expression_payload()
                .expect("CST match return");

            let (register, returned) = compiler
                .compile_return_value_payload_for_test(expression.fallback(), Some(&expression))
                .expect("parent CST match payload should compile without legacy match body");

            assert_eq!(register, Register(0));
            assert!(!returned);
        },
    );
}
