use super::*;

#[test]
fn present_match_arm_payload_without_syntax_does_not_use_legacy_arm() {
    let source = SourceId::new(1);
    let text = r#"
fn main(value) {
    return match value {
        _ => 1,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    let match_expr = first_return_match_expr(fallback_statements_for_body(source, &payload.body));
    let missing_arm = body_payloads::CompilerMatchArmPayload::missing_syntax();
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "main");

    let error = compiler
        .compile_match_value_with_payloads(match_expr, Register(0), None, Some(&[missing_arm]))
        .expect_err("present match arm payload without CST syntax must not use legacy arm");

    assert!(
        matches!(
            error.kind,
            CompileErrorKind::UnsupportedSyntax("missing CST match arm payload")
        ),
        "expected missing CST match arm payload, got {error:?}"
    );
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
