use super::*;

#[test]
fn extra_expression_call_argument_payloads_do_not_compile_fallback_args() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn take(value) {
    return value;
}

fn main() {
    take(1, 2);
}
"#;
    let legacy_text = r#"
fn take(value) {
    return value;
}

fn main() {
    take(1);
}
"#;
    let cst_call = cst_call_expression(source, cst_text);
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_call = paired_statement_payloads_for_body(source, &payload.body)[0]
        .expression_payload()
        .expect("legacy call payload");
    let mismatched = expression_payload_with_fallback(source, cst_call, legacy_call.fallback());
    let ExprKind::Call { callee, args } = &legacy_call.fallback().kind else {
        panic!("expected legacy call fallback");
    };
    let arg_payloads = mismatched.call_argument_payloads();
    assert!(
        arg_payloads
            .as_ref()
            .is_some_and(|payloads| payloads.len() == 2),
        "CST call argument payloads should preserve the syntax argument count"
    );

    let error = compiler
        .compile_call_expr_with_arg_payloads(
            legacy_call.fallback(),
            callee,
            args,
            mismatched.call_callee_payload().as_ref(),
            arg_payloads.as_deref(),
        )
        .expect_err("extra CST call arguments must not compile fallback call");

    assert!(
        matches!(
            error.kind,
            CompileErrorKind::UnsupportedSyntax("mismatched CST call arguments")
        ),
        "expected mismatched CST call arguments, got {error:?}"
    );
}

#[test]
fn missing_expression_call_argument_payloads_do_not_compile_fallback_args() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn take(first, second) {
    return first;
}

fn main() {
    take(1);
}
"#;
    let legacy_text = r#"
fn take(first, second) {
    return first;
}

fn main() {
    take(1, 2);
}
"#;
    let cst_call = cst_call_expression(source, cst_text);
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_call = paired_statement_payloads_for_body(source, &payload.body)[0]
        .expression_payload()
        .expect("legacy call payload");
    let mismatched = expression_payload_with_fallback(source, cst_call, legacy_call.fallback());
    let ExprKind::Call { callee, args } = &legacy_call.fallback().kind else {
        panic!("expected legacy call fallback");
    };
    let arg_payloads = mismatched.call_argument_payloads();
    assert!(
        arg_payloads
            .as_ref()
            .is_some_and(|payloads| payloads.len() == 1),
        "CST call argument payloads should preserve the shorter syntax argument count"
    );

    let error = compiler
        .compile_call_expr_with_arg_payloads(
            legacy_call.fallback(),
            callee,
            args,
            mismatched.call_callee_payload().as_ref(),
            arg_payloads.as_deref(),
        )
        .expect_err("missing CST call arguments must not compile fallback call");

    assert!(
        matches!(
            error.kind,
            CompileErrorKind::UnsupportedSyntax("mismatched CST call arguments")
        ),
        "expected mismatched CST call arguments, got {error:?}"
    );
}

#[test]
fn extra_statement_call_argument_payloads_do_not_compile_fallback_args() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn take(value) {
    return value;
}

fn main() {
    take(1, 2);
}
"#;
    let legacy_text = r#"
fn take(value) {
    return value;
}

fn main() {
    take(1);
}
"#;
    let cst_body = cst_call_body(source, cst_text);
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let mismatched = statement_payload_from_syntax_body_with_fallbacks(
        source,
        cst_body,
        fallback_statements_for_body(source, &payload.body),
        0,
    );
    assert!(
        mismatched
            .expression_payload()
            .and_then(|payload| payload.call_argument_payloads())
            .is_some_and(|payloads| payloads.len() == 2)
    );

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("extra CST statement call arguments must not compile fallback call");

    assert!(
        matches!(
            error.kind,
            CompileErrorKind::UnsupportedSyntax("mismatched CST call arguments")
        ),
        "expected mismatched CST call arguments, got {error:?}"
    );
}

fn cst_call_expression(source: SourceId, text: &str) -> vela_syntax::ast::SyntaxExpression {
    cst_call_statement(source, text)
        .as_expr()
        .expect("CST expression statement")
        .expression()
        .expect("CST call expression")
}

fn cst_call_statement(source: SourceId, text: &str) -> vela_syntax::ast::SyntaxStatement {
    cst_call_body(source, text)
        .statements()
        .next()
        .expect("CST statement")
}

fn cst_call_body(source: SourceId, text: &str) -> vela_syntax::ast::SyntaxBlock {
    vela_syntax::parse::parse_source_with_id(source, text)
        .tree()
        .functions()
        .find(|function| function.name_text().as_deref() == Some("main"))
        .expect("CST main function")
        .body()
        .expect("CST body")
}
