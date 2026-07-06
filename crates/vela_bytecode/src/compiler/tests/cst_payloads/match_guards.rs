use super::*;

#[test]
fn mismatched_match_guard_payloads_do_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_guard(value, cst_flag) {
    return match value {
        _ if {
            let allowed = cst_flag;
            allowed
        } => 1,
        _ => 0,
    };
}

fn legacy_guard(value, legacy_flag) {
    return match value {
        _ if legacy_flag => 1,
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_guard").expect("cst guard");
    let (legacy_payload, _, _) = semantic.function("legacy_guard").expect("legacy guard");

    let mismatched_arm = first_return_match_arm_payload(&cst_payload.body);
    let legacy_match =
        first_return_match_expr(fallback_statements_for_body(source, &legacy_payload.body));
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "legacy_guard");

    let err = compiler
        .compile_match_value_with_payloads(legacy_match, Register(0), None, Some(&[mismatched_arm]))
        .expect_err("mismatched guard payload should not use legacy expression");
    assert!(matches!(
        err.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST expression payload")
    ));
}

#[test]
fn missing_match_guard_payload_does_not_use_legacy_guard() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_without_guard(value) {
    return match value {
        _ => 1,
    };
}

fn legacy_guard(value, flag) {
    return match value {
        _ if flag => 1,
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic
        .function("cst_without_guard")
        .expect("CST guard source");
    let (legacy_payload, _, _) = semantic.function("legacy_guard").expect("legacy guard");

    let missing_guard = first_return_match_arm_payload(&cst_payload.body);
    assert!(
        missing_guard
            .syntax_arm()
            .expect("CST match arm")
            .guard()
            .is_none()
    );
    let legacy_match =
        first_return_match_expr(fallback_statements_for_body(source, &legacy_payload.body));
    assert!(legacy_match.arms[0].guard.is_some());
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "legacy_guard");

    let err = compiler
        .compile_match_value_with_payloads(legacy_match, Register(0), None, Some(&[missing_guard]))
        .expect_err("missing CST match guard must not compile legacy guard");
    assert!(
        matches!(
            err.kind,
            CompileErrorKind::UnsupportedSyntax("missing CST match guard payload")
        ),
        "expected missing CST match guard payload, got {err:?}"
    );
}

fn first_return_match_arm_payload(
    body: &body_payloads::CompilerBodyPayload<'_>,
) -> body_payloads::CompilerMatchArmPayload {
    let statements = paired_statement_payloads_for_body(body.syntax_payload().source, body);
    statements[0]
        .return_value_expression_payload()
        .and_then(|payload| payload.match_arm_payloads())
        .expect("return match")
        .remove(0)
}

fn first_return_match_expr(statements: &[vela_syntax::ast::Stmt]) -> &vela_syntax::ast::MatchExpr {
    let statement = statements.first().expect("return statement");
    let vela_syntax::ast::StmtKind::Return(Some(value)) = &statement.kind else {
        panic!("expected return statement");
    };
    let vela_syntax::ast::ExprKind::Match(match_expr) = &value.kind else {
        panic!("expected match expression");
    };
    match_expr
}
