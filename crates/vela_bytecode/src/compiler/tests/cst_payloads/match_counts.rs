use super::*;

#[test]
fn extra_expression_match_arm_payloads_do_not_compile_fallback_arms() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    return match 1 {
        1 => 10,
        _ => 20,
    };
}
"#;
    let legacy_text = r#"
fn main() {
    return match 1 {
        1 => 10,
    };
}
"#;
    let cst_match = cst_match_expression(source, cst_text);
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_match = paired_statement_payloads_for_body(source, &payload.body)[0]
        .return_value_expression_payload()
        .expect("legacy return match payload");
    let mismatched = body_payloads::CompilerExpressionPayload::syntax(
        source,
        cst_match,
        legacy_match.fallback(),
    );
    let ExprKind::Match(match_expr) = &legacy_match.fallback().kind else {
        panic!("expected legacy match fallback");
    };
    let arm_payloads = mismatched.match_arm_payloads();
    assert_eq!(
        arm_payloads.as_ref().map(Vec::len),
        Some(2),
        "extra CST match arms should remain visible to compile-boundary validation"
    );

    let error = compiler
        .compile_match_value_with_payloads(
            match_expr,
            Register(0),
            mismatched.match_scrutinee_payload().as_ref(),
            arm_payloads.as_deref(),
        )
        .expect_err("extra CST expression match arms must not compile fallback match");

    assert!(
        matches!(
            error.kind,
            CompileErrorKind::UnsupportedSyntax("mismatched CST match arms")
        ),
        "expected mismatched CST match arms, got {error:?}"
    );
}

#[test]
fn extra_statement_match_arm_payloads_do_not_compile_fallback_arms() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    match 1 {
        1 => 10,
        _ => 20,
    };
}
"#;
    let legacy_text = r#"
fn main() {
    match 1 {
        1 => 10,
    };
}
"#;
    let cst_body = cst_match_body(source, cst_text);
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let mismatched = statement_payload_from_syntax_body_with_fallbacks(
        source,
        cst_body,
        fallback_statements_for_body(source, &payload.body),
        0,
    );
    let arm_payloads = mismatched
        .expression_payload()
        .and_then(|payload| payload.match_arm_payloads());
    assert_eq!(
        arm_payloads.as_ref().map(Vec::len),
        Some(2),
        "extra CST match arms should remain visible to compile-boundary validation"
    );

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("extra CST statement match arms must not compile fallback match");

    assert!(
        matches!(
            error.kind,
            CompileErrorKind::UnsupportedSyntax("mismatched CST match arms")
        ),
        "expected mismatched CST match arms, got {error:?}"
    );
}

#[test]
fn equal_count_match_arm_payloads_pair_by_position_not_legacy_pattern() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    return match 1 {
        1 => 10,
        _ => 20,
    };
}
"#;
    let legacy_text = r#"
fn main() {
    return match 1 {
        _ => 20,
        1 => 10,
    };
}
"#;
    let cst_match = cst_match_expression(source, cst_text);
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (_, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_match = paired_statement_payloads_for_body(source, &payload.body)[0]
        .return_value_expression_payload()
        .expect("legacy return match payload");
    let mismatched = body_payloads::CompilerExpressionPayload::syntax(
        source,
        cst_match,
        legacy_match.fallback(),
    );

    let arm_texts = mismatched
        .match_arm_payloads()
        .expect("equal arm counts should expose payloads")
        .into_iter()
        .map(|arm| {
            arm.pattern_payload()
                .syntax_pattern()
                .expect("arm pattern syntax")
                .syntax()
                .text()
                .to_string()
        })
        .collect::<Vec<_>>();

    assert_eq!(arm_texts, ["1".to_owned(), "_".to_owned()]);
}

fn cst_match_expression(source: SourceId, text: &str) -> vela_syntax::ast::SyntaxExpression {
    cst_match_statement(source, text)
        .as_return()
        .expect("CST return statement")
        .expression()
        .expect("CST return match expression")
}

fn cst_match_statement(source: SourceId, text: &str) -> vela_syntax::ast::SyntaxStatement {
    cst_match_body(source, text)
        .statements()
        .next()
        .expect("CST statement")
}

fn cst_match_body(source: SourceId, text: &str) -> vela_syntax::ast::SyntaxBlock {
    vela_syntax::parse::parse_source_with_id(source, text)
        .tree()
        .functions()
        .find(|function| function.name_text().as_deref() == Some("main"))
        .expect("CST main function")
        .body()
        .expect("CST body")
}
