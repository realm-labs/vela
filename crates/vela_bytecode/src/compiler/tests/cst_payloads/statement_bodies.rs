use super::*;

#[test]
fn semantic_function_block_statement_body_is_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn scoped() {
    let total = 0;
    {
        total += 1;
    }
    return total;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("scoped").expect("scoped function");
    assert_cst_statements(
        &payload.body,
        &[
            (SyntaxStatementKind::Let, "let total = 0;"),
            (SyntaxStatementKind::Block, "{\n        total += 1;\n    }"),
            (SyntaxStatementKind::Return, "return total;"),
        ],
    );
    assert_cst_block_statement_payloads(
        &payload.body,
        &[vec![(SyntaxStatementKind::Expr, "total += 1;")]],
    );

    compile_program_source(source, text).expect("CST-backed block statement body should compile");
}

#[test]
fn missing_block_statement_body_payload_does_not_use_legacy_body() {
    with_cst_payload_compiler(
        r#"
fn main() {
    {
        return 1;
    }
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let block = statements
                .iter()
                .find(|statement| statement.statement_kind() == Some(SyntaxStatementKind::Block))
                .expect("block statement payload");
            let missing_children =
                body_payloads::CompilerStatementPayload::missing_child_payload_context(
                    block
                        .syntax_statement()
                        .expect("block statement syntax")
                        .clone(),
                    block.fallback(),
                );

            let error = compiler
                .compile_statement_payload_for_test(&missing_children)
                .expect_err("missing CST block body must not use the legacy block body");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST block statement body payload")
            ));
        },
    );
}

#[test]
fn missing_for_statement_body_payload_does_not_use_legacy_body() {
    with_cst_payload_compiler(
        r#"
fn main() {
    for value in [1] {
        return value;
    }
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let for_statement = statements
                .iter()
                .find(|statement| statement.statement_kind() == Some(SyntaxStatementKind::For))
                .expect("for statement payload");
            let missing_children =
                body_payloads::CompilerStatementPayload::missing_child_payload_context(
                    for_statement
                        .syntax_statement()
                        .expect("for statement syntax")
                        .clone(),
                    for_statement.fallback(),
                );

            let error = compiler
                .compile_statement_payload_for_test(&missing_children)
                .expect_err("missing CST for body must not use the legacy loop body");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST for statement body payload")
            ));
        },
    );
}

#[test]
fn missing_if_statement_payload_does_not_use_legacy_branches() {
    with_cst_payload_compiler(
        r#"
fn main(flag) {
    if flag {
        return 1;
    }
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let if_statement = statements
                .iter()
                .find(|statement| statement.statement_kind() == Some(SyntaxStatementKind::If))
                .expect("if statement payload");
            let missing_children =
                body_payloads::CompilerStatementPayload::missing_child_payload_context(
                    if_statement
                        .syntax_statement()
                        .expect("if statement syntax")
                        .clone(),
                    if_statement.fallback(),
                );

            let error = compiler
                .compile_statement_payload_for_test(&missing_children)
                .expect_err("missing CST if payload must not use legacy branches");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST if statement payload")
            ));
        },
    );
}

#[test]
fn missing_if_statement_child_payloads_do_not_use_legacy_branches() {
    with_cst_payload_compiler(
        r#"
fn main(flag) {
    if flag {
        return 1;
    } else {
        return 2;
    }
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let if_statement = statements
                .iter()
                .find(|statement| statement.statement_kind() == Some(SyntaxStatementKind::If))
                .expect("if statement payload");
            let truncated_if_payload = body_payloads::CompilerIfPayload::truncated_for_test();

            let error = compiler
                .compile_if_statement_with_payload_for_test(
                    if_statement.fallback(),
                    &truncated_if_payload,
                )
                .expect_err("missing CST if branch payload must not use legacy branches");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST if condition payload")
            ));
        },
    );
}

#[test]
fn semantic_function_control_flow_statements_are_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn flow() {
    let total = 0;
    if total == 0 {
        return 1;
    }
    match total {
        0 => { return 0; },
        _ => { return total; },
    }
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("flow").expect("flow function");
    assert_cst_statements(
        &payload.body,
        &[
            (SyntaxStatementKind::Let, "let total = 0;"),
            (
                SyntaxStatementKind::If,
                "if total == 0 {\n        return 1;\n    }",
            ),
            (
                SyntaxStatementKind::Match,
                "match total {\n        0 => { return 0; },\n        _ => { return total; },\n    }",
            ),
        ],
    );
    assert_cst_match_arm_body_payloads(
        &payload.body,
        &[
            vec![(SyntaxStatementKind::Return, "return 0;")],
            vec![(SyntaxStatementKind::Return, "return total;")],
        ],
    );

    compile_program_source(source, text).expect("CST-backed control-flow body should compile");
}
