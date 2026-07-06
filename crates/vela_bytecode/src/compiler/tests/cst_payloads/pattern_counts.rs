use super::*;

#[test]
fn extra_tuple_pattern_payloads_do_not_compile_fallback_fields() {
    let source = SourceId::new(1);
    let text = r#"
enum Shape {
    Pair(left: i64, right: i64)
    Single(left: i64)
}

fn cst_tuple(value) {
    return match value {
        Shape::Pair(cst_left, cst_right) => cst_left,
        _ => 0,
    };
}

fn fallback_tuple(value) {
    return match value {
        Shape::Single(legacy_left) => legacy_left,
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_tuple").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_tuple")
        .expect("fallback function");
    let cst_pattern = first_return_match_pattern_syntax(&cst_payload.body);
    let fallback_pattern = first_return_match_fallback_pattern(fallback_statements_for_body(
        source,
        &fallback_payload.body,
    ));
    let mismatched = body_payloads::CompilerPatternPayload::syntax(cst_pattern);
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_tuple");

    let error = compiler
        .compile_match_pattern(Register(0), fallback_pattern, Some(&mismatched))
        .expect_err("extra CST tuple pattern fields must not be ignored");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST tuple pattern fields")
    ));
}

#[test]
fn extra_record_pattern_payloads_do_not_bind_fallback_fields() {
    let source = SourceId::new(1);
    let text = r#"
enum Shape {
    Named { first: i64, second: i64 }
    One { first: i64 }
}

fn cst_record(value) {
    return match value {
        Shape::Named { first: cst_first, second: cst_second } => cst_first,
        _ => 0,
    };
}

fn fallback_record(value) {
    return match value {
        Shape::One { first: legacy_first } => legacy_first,
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_record").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_record")
        .expect("fallback function");
    let cst_pattern = first_return_match_pattern_syntax(&cst_payload.body);
    let fallback_pattern = first_return_match_fallback_pattern(fallback_statements_for_body(
        source,
        &fallback_payload.body,
    ));
    let mismatched = body_payloads::CompilerPatternPayload::syntax(cst_pattern);
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_record");

    let error = compiler
        .bind_pattern_locals(
            Register(0),
            fallback_pattern,
            Some(&mismatched),
            Span::new(source, 0, 1),
            crate::compiler::patterns::PatternBindingFacts::default(),
            LocalBindingKind::Pattern,
        )
        .expect_err("extra CST record pattern fields must not be ignored");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST record pattern fields")
    ));
}

#[test]
fn shorthand_record_pattern_payload_does_not_bind_legacy_explicit_field() {
    let source = SourceId::new(1);
    let text = r#"
enum Shape {
    Named { first: i64 }
}

fn cst_record(value) {
    return match value {
        Shape::Named { first } => first,
        _ => 0,
    };
}

fn fallback_record(value) {
    return match value {
        Shape::Named { first: legacy_first } => legacy_first,
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_record").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_record")
        .expect("fallback function");
    let cst_pattern = first_return_match_pattern_syntax(&cst_payload.body);
    let fallback_pattern = first_return_match_fallback_pattern(fallback_statements_for_body(
        source,
        &fallback_payload.body,
    ));
    let mismatched = body_payloads::CompilerPatternPayload::syntax(cst_pattern.clone());
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_record");

    let error = compiler
        .bind_pattern_locals(
            Register(0),
            fallback_pattern,
            Some(&mismatched),
            Span::new(source, 0, 1),
            crate::compiler::patterns::PatternBindingFacts::default(),
            LocalBindingKind::Pattern,
        )
        .expect_err("CST shorthand field must not bind legacy explicit field pattern");

    assert!(
        matches!(
            error.kind,
            CompileErrorKind::UnsupportedSyntax("record pattern field")
        ),
        "unexpected error: {:?}",
        error.kind
    );

    let vela_syntax::ast::Pattern::RecordVariant { fields, .. } = fallback_pattern else {
        panic!("expected record pattern");
    };
    let direct_field_payload = mismatched
        .record_field_payloads()
        .expect("CST record field payloads")
        .into_iter()
        .next()
        .expect("CST record field payload");
    let direct_error = crate::compiler::patterns::record_pattern_field_payload_declares_locals(
        &direct_field_payload,
        &fields[0],
    )
    .expect_err("direct CST shorthand field must not use legacy explicit binding");
    assert!(matches!(
        direct_error.kind,
        CompileErrorKind::UnsupportedSyntax("record pattern field")
    ));
}

#[test]
fn compound_pattern_kind_mismatch_does_not_use_legacy_fields() {
    let source = SourceId::new(1);
    let text = r#"
enum Shape {
    Pair(left: i64)
    Named { first: i64 }
}

fn cst_tuple(value) {
    return match value {
        Shape::Pair(cst_left) => cst_left,
        _ => 0,
    };
}

fn cst_record(value) {
    return match value {
        Shape::Named { first: cst_first } => cst_first,
        _ => 0,
    };
}

fn legacy_tuple(value) {
    return match value {
        Shape::Pair(legacy_left) => legacy_left,
        _ => 0,
    };
}

fn legacy_record(value) {
    return match value {
        Shape::Named { first: legacy_first } => legacy_first,
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_tuple_payload, _, _) = semantic.function("cst_tuple").expect("cst tuple");
    let (cst_record_payload, _, _) = semantic.function("cst_record").expect("cst record");
    let (legacy_tuple_payload, _, _) = semantic.function("legacy_tuple").expect("legacy tuple");
    let (legacy_record_payload, _, _) = semantic.function("legacy_record").expect("legacy record");
    let cst_tuple = first_return_match_pattern_syntax(&cst_tuple_payload.body);
    let cst_record = first_return_match_pattern_syntax(&cst_record_payload.body);
    let legacy_tuple = first_return_match_fallback_pattern(fallback_statements_for_body(
        source,
        &legacy_tuple_payload.body,
    ));
    let legacy_record = first_return_match_fallback_pattern(fallback_statements_for_body(
        source,
        &legacy_record_payload.body,
    ));
    let tuple_payload_with_record_fallback =
        body_payloads::CompilerPatternPayload::syntax(cst_tuple);
    let record_payload_with_tuple_fallback =
        body_payloads::CompilerPatternPayload::syntax(cst_record);
    let (mut tuple_compiler, _) = cst_payload_compiler_for_function(&semantic, "legacy_record");
    let (mut record_compiler, _) = cst_payload_compiler_for_function(&semantic, "legacy_tuple");

    let tuple_error = tuple_compiler
        .compile_match_pattern(
            Register(0),
            legacy_record,
            Some(&tuple_payload_with_record_fallback),
        )
        .expect_err("tuple CST pattern must not use legacy record fields");
    assert!(matches!(
        tuple_error.kind,
        CompileErrorKind::UnsupportedSyntax("match pattern")
    ));

    let record_error = record_compiler
        .bind_pattern_locals(
            Register(0),
            legacy_tuple,
            Some(&record_payload_with_tuple_fallback),
            Span::new(source, 0, 1),
            crate::compiler::patterns::PatternBindingFacts::default(),
            LocalBindingKind::Pattern,
        )
        .expect_err("record CST pattern must not use legacy tuple fields");
    assert!(matches!(
        record_error.kind,
        CompileErrorKind::UnsupportedSyntax("match pattern")
    ));
}

#[test]
fn tuple_pattern_child_payloads_are_position_based_not_legacy_matched() {
    let source = SourceId::new(1);
    let text = r#"
enum Shape {
    Pair(left: i64, right: i64)
}

fn cst_tuple(value) {
    return match value {
        Shape::Pair(1, 2) => 1,
        _ => 0,
    };
}

fn fallback_tuple(value) {
    return match value {
        Shape::Pair(2, 1) => 1,
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_tuple").expect("cst function");
    let cst_pattern = first_return_match_pattern_syntax(&cst_payload.body);
    let payload = body_payloads::CompilerPatternPayload::syntax(cst_pattern);

    let field_texts = payload
        .tuple_pattern_payloads()
        .expect("tuple pattern payloads")
        .into_iter()
        .map(|field| {
            field
                .syntax_pattern()
                .expect("field syntax")
                .syntax()
                .text()
                .to_string()
        })
        .collect::<Vec<_>>();

    assert_eq!(field_texts, ["1".to_owned(), "2".to_owned()]);
}

#[test]
fn missing_source_backed_match_pattern_payload_does_not_use_legacy_pattern() {
    let source = SourceId::new(1);
    let text = r#"
enum Shape {
    Pair(left: i64)
}

fn main(value) {
    return match value {
        Shape::Pair(legacy_left) => legacy_left,
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    let fallback_pattern =
        first_return_match_fallback_pattern(fallback_statements_for_body(source, &payload.body));
    let missing_payload = body_payloads::CompilerPatternPayload::missing_syntax(source);
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "main");

    let error = compiler
        .compile_match_pattern(Register(0), fallback_pattern, Some(&missing_payload))
        .expect_err("source-backed missing CST pattern must not use legacy pattern");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("match pattern")
    ));
}

#[test]
fn missing_source_backed_binding_pattern_payload_does_not_bind_legacy_locals() {
    let source = SourceId::new(1);
    let text = r#"
enum Shape {
    Pair(left: i64)
}

fn main(value) {
    return match value {
        Shape::Pair(legacy_left) => legacy_left,
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    let fallback_pattern =
        first_return_match_fallback_pattern(fallback_statements_for_body(source, &payload.body));
    let missing_payload = body_payloads::CompilerPatternPayload::missing_syntax(source);
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "main");

    let error = compiler
        .bind_pattern_locals(
            Register(0),
            fallback_pattern,
            Some(&missing_payload),
            Span::new(source, 0, 1),
            crate::compiler::patterns::PatternBindingFacts::default(),
            LocalBindingKind::Pattern,
        )
        .expect_err("source-backed missing CST pattern must not bind legacy locals");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("match pattern")
    ));
}

fn first_return_match_pattern_syntax(
    body: &body_payloads::CompilerBodyPayload<'_>,
) -> vela_syntax::ast::SyntaxPattern {
    let statements = paired_statement_payloads_for_body(body.syntax_payload().source, body);
    statements[0]
        .return_value_expression_payload()
        .and_then(|payload| payload.match_arm_payloads())
        .expect("return match")[0]
        .pattern_payload()
        .syntax_pattern()
        .expect("CST pattern")
        .clone()
}

fn first_return_match_fallback_pattern(
    statements: &[vela_syntax::ast::Stmt],
) -> &vela_syntax::ast::Pattern {
    let statement = statements.first().expect("return statement");
    let vela_syntax::ast::StmtKind::Return(Some(value)) = &statement.kind else {
        panic!("expected return statement");
    };
    let vela_syntax::ast::ExprKind::Match(match_expr) = &value.kind else {
        panic!("expected return match expression");
    };
    &match_expr.arms[0].pattern
}
