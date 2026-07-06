use super::*;

fn statement_body_payloads<'ast>(
    body: &body_payloads::CompilerBodyPayload<'ast>,
) -> Vec<body_payloads::CompilerStatementPayload<'ast>> {
    paired_statement_payloads_for_body(body.syntax_payload().source, body)
}

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
fn make(value) {
    return value;
}

fn main() {
    {
        let value = 1;
        return value && make(value);
    }
}
"#,
        |compiler, payload| {
            let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
            let block = statements
                .iter()
                .find(|statement| {
                    statement.stored_statement_kind() == Some(SyntaxStatementKind::Block)
                })
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
fn empty_block_statement_payload_uses_cst_empty_body() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    {
    }
    {
        let legacy_value = [1];
    }
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let mismatched =
        statement_payload_with_fallback_offset(source, &payload.body, &payload.body, 1, 0);

    compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect("empty CST block must not compile legacy block body");

    assert!(
        compiler
            .code
            .constants
            .iter()
            .all(|constant| *constant != Constant::i64(1)),
        "fallback block statement body must not be emitted"
    );
}

#[test]
fn syntax_only_block_statement_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    {
        let cst_value;
        return;
    }
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");

    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only block statement body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::Null),
        "syntax-only nested empty let must emit null"
    );
}

#[test]
fn syntax_only_block_statement_payload_drops_owned_body_fallback() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    {
        let cst_value;
        return;
    }
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (_, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let body = statement_body_payloads(&payload.body)[0]
        .block_body_payload()
        .expect("block body payload");

    assert!(
        body_has_no_statement_fallbacks(&body),
        "syntax-only nested block should not retain an owned body fallback"
    );
}

#[test]
fn syntax_only_block_statement_in_mixed_body_drops_owned_statement_fallback() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    {
        let cst_value;
        return;
    }
    return 1;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (_, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only block statement should not retain an owned statement fallback"
    );
    assert_eq!(
        statements[1].stored_return_value_kind(),
        Some(SyntaxExpressionKind::Literal)
    );
}

#[test]
fn syntax_only_match_statement_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main(input) {
    match input {
        value if value > 0 => {
            return value;
        },
        _ => {
            return 0;
        },
    }
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only match statement should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only match statement should compile");
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
            let statements = statement_body_payloads(&payload.body);
            let for_statement = statements
                .iter()
                .find(|statement| {
                    statement.stored_statement_kind() == Some(SyntaxStatementKind::For)
                })
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
    if ({
        let selected = flag;
        selected
    }) {
        return 1;
    }
}
"#,
        |compiler, payload| {
            let statements = statement_body_payloads(&payload.body);
            let if_statement = statements
                .iter()
                .find(|statement| {
                    statement.stored_statement_kind() == Some(SyntaxStatementKind::If)
                })
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
fn mismatched_break_statement_payload_uses_cst_kind() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_body() {
    break;
}

fn fallback_body() {
    let value = 1;
    return value && make(value);
}

fn make(value) {
    return value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_body")
        .expect("fallback function");
    let mismatched = statement_payload_with_fallback_offset(
        source,
        &cst_payload.body,
        &fallback_payload.body,
        1,
        0,
    );
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_body");

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("CST break payload must not compile the fallback return statement");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("break outside loop")
    ));
}

#[test]
fn mismatched_continue_statement_payload_uses_cst_kind() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_body() {
    continue;
}

fn fallback_body() {
    let value = 1;
    return value && make(value);
}

fn make(value) {
    return value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_body")
        .expect("fallback function");
    let mismatched = statement_payload_with_fallback_offset(
        source,
        &cst_payload.body,
        &fallback_payload.body,
        1,
        0,
    );
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_body");

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("CST continue payload must not compile the fallback return statement");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("continue outside loop")
    ));
}

#[test]
fn missing_if_statement_child_payloads_do_not_use_legacy_branches() {
    with_cst_payload_compiler(
        r#"
fn main(flag) {
    if ({
        let selected = flag;
        selected
    }) {
        return 1;
    } else {
        return 2;
    }
}
"#,
        |compiler, payload| {
            let statements = statement_body_payloads(&payload.body);
            let if_statement = statements
                .iter()
                .find(|statement| {
                    statement.stored_statement_kind() == Some(SyntaxStatementKind::If)
                })
                .expect("if statement payload");
            let truncated_if_payload = body_payloads::CompilerIfPayload::truncated_for_test();
            let vela_syntax::ast::StmtKind::Expr(expr) = &if_statement.fallback().kind else {
                panic!("expected legacy if expression statement");
            };
            let vela_syntax::ast::ExprKind::If(if_expr) = &expr.kind else {
                panic!("expected legacy if expression statement");
            };

            let error = compiler
                .compile_if_value_with_payloads(if_expr, Register(0), &truncated_if_payload)
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
