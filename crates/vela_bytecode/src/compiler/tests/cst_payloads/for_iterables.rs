use super::*;

fn for_body_payload<'ast>(
    statement: &body_payloads::CompilerStatementPayload<'ast>,
) -> Option<body_payloads::CompilerBodyPayload<'ast>> {
    Some(body_payloads::CompilerBodyPayload::nested_syntax(
        statement.syntax_statement_span()?.source,
        statement.syntax_statement()?.as_for()?.body()?,
    ))
}

fn for_statement_payloads<'ast>(
    body: &body_payloads::CompilerBodyPayload<'ast>,
) -> Vec<body_payloads::CompilerStatementPayload<'ast>> {
    paired_statement_payloads_for_body(body.syntax_payload().source, body)
}

#[test]
fn semantic_function_for_iterable_values_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn loop_values() {
    let total = 0;
    for value in {
        let start = 0;
        start
    }..{
        let end = 3;
        end
    } {
        total += value;
    }
    for value in {
        let values = [1, 2];
        values
    } {
        total += value;
    }
    return total;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("loop_values")
        .expect("loop_values function");

    let iterable_payloads = for_statement_payloads(&payload.body)
        .into_iter()
        .filter_map(|statement| statement.for_iterable_expression_payload())
        .collect::<Vec<_>>();
    assert_eq!(iterable_payloads.len(), 2);
    assert_eq!(
        iterable_payloads[0].syntax_kind(),
        Some(SyntaxExpressionKind::Binary)
    );
    let (range_start, range_end) = iterable_payloads[0]
        .binary_operand_payloads()
        .expect("range iterable should expose operand payloads");
    assert_eq!(range_start.syntax_kind(), Some(SyntaxExpressionKind::Block));
    assert_eq!(range_end.syntax_kind(), Some(SyntaxExpressionKind::Block));
    assert_eq!(
        iterable_payloads[1].syntax_kind(),
        Some(SyntaxExpressionKind::Block)
    );

    let program = compile_program_source(source, text)
        .expect("CST-backed for iterable values should compile");
    let function = program
        .function("loop_values")
        .expect("loop_values bytecode");
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::I64RangeNext { .. }
    )));
    assert!(
        function.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::IterInit { .. }
        ))
    );
}

#[test]
fn range_for_loop_prefers_cst_operator_payload() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let total = 0;
    for value in 1..=3 {
        total += value;
    }
    return total;
}
"#;
    let cst_semantic = parse_semantic_source(source, cst_text).expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("main function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
fn main() {
    let total = 0;
    for value in 1..3 {
        total += value;
    }
    return total;
}
"#,
        |compiler, payload| {
            let statements = body_payloads::CompilerBodyPayload::paired_statement_payloads_for_test(
                source,
                cst_body,
                fallback_statements_for_body(source, &payload.body),
            );

            compiler
                .compile_statement_payloads(&statements)
                .expect("CST-backed range loop should compile");

            assert!(
                compiler
                    .code
                    .instructions
                    .iter()
                    .any(|instruction| matches!(
                        instruction.kind,
                        UnlinkedInstructionKind::I64RangeNext {
                            inclusive: true,
                            ..
                        }
                    )),
                "range loop should use CST range inclusivity"
            );
        },
    );
}

#[test]
fn mismatched_range_iterable_payload_does_not_use_legacy_operator() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let items = [1, 2, 3];
    let total = 0;
    for value in items {
        total += value;
    }
    return total;
}
"#;
    let cst_semantic = parse_semantic_source(source, cst_text).expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("main function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
fn main() {
    let items = [1, 2, 3];
    let total = 0;
    for value in 1..3 {
        total += value;
    }
    return total;
}
"#,
        |compiler, payload| {
            let statements = body_payloads::CompilerBodyPayload::paired_statement_payloads_for_test(
                source,
                cst_body,
                fallback_statements_for_body(source, &payload.body),
            );

            let error = compiler
                .compile_statement_payloads(&statements)
                .expect_err("mismatched CST iterable must not compile the legacy range");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST for iterable payload")
            ));
        },
    );
}

#[test]
fn missing_for_iterable_payload_does_not_use_legacy_iterable() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    for value in {
    }
}
"#;
    let legacy_text = r#"
fn main() {
    for value in [1] {
        return value;
    }
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_statement = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body")
        .statements()
        .next()
        .expect("CST for statement");
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_statement = for_statement_payloads(&legacy_payload.body)[0].fallback();
    let missing =
        body_payloads::CompilerStatementPayload::syntax(source, cst_statement, legacy_statement);

    assert_eq!(
        missing.stored_statement_kind(),
        Some(SyntaxStatementKind::For)
    );
    assert_eq!(
        missing
            .for_iterable_expression_payload()
            .and_then(|payload| payload.syntax_kind()),
        None
    );
    assert!(for_body_payload(&missing).is_some());

    let error = compiler
        .compile_statement_payload_for_test(&missing)
        .expect_err("missing CST for iterable must not compile legacy iterable");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST for iterable payload")
    ));
}

#[test]
fn missing_for_index_pattern_payload_does_not_use_legacy_index() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    for value in [1] {
        return value;
    }
}
"#;
    let legacy_text = r#"
fn main() {
    for index, value in [1] {
        return index;
    }
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_statement = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body")
        .statements()
        .next()
        .expect("CST for statement");
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_statement = for_statement_payloads(&legacy_payload.body)[0].fallback();
    let missing =
        body_payloads::CompilerStatementPayload::syntax(source, cst_statement, legacy_statement);

    assert_eq!(
        missing.stored_statement_kind(),
        Some(SyntaxStatementKind::For)
    );
    assert!(
        missing
            .for_index_pattern_payload()
            .is_some_and(|payload| payload.syntax_pattern_kind().is_none())
    );

    let error = compiler
        .compile_statement_payload_for_test(&missing)
        .expect_err("missing CST for index pattern must not compile legacy index");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST for index pattern payload")
    ));
}

#[test]
fn missing_for_value_pattern_payload_does_not_use_legacy_value() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    for in [1] {
    }
}
"#;
    let legacy_text = r#"
fn main() {
    for value in [1] {
        return value;
    }
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_statement = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body")
        .statements()
        .next()
        .expect("CST for statement");
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_statement = for_statement_payloads(&legacy_payload.body)[0].fallback();
    let missing =
        body_payloads::CompilerStatementPayload::syntax(source, cst_statement, legacy_statement);

    assert_eq!(
        missing.stored_statement_kind(),
        Some(SyntaxStatementKind::For)
    );
    assert!(
        missing
            .for_value_pattern_payload()
            .is_some_and(|payload| payload.syntax_pattern_kind().is_none())
    );

    let error = compiler
        .compile_statement_payload_for_test(&missing)
        .expect_err("missing CST for value pattern must not compile legacy value");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST for value pattern payload")
    ));
}

#[test]
fn semantic_function_for_patterns_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
enum Result {
    Err { code: i64, message: String }
    Ok(i64)
}

fn loop_patterns(results) {
    let total = 0;
    for index, Result::Err { code: status, message } in results {
        total += status + index;
    }
    return total;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("loop_patterns")
        .expect("loop_patterns function");
    let for_statement = for_statement_payloads(&payload.body)
        .into_iter()
        .find(|statement| statement.stored_statement_kind() == Some(SyntaxStatementKind::For))
        .expect("for statement payload");

    let index_pattern = for_statement
        .for_index_pattern_payload()
        .expect("indexed for statement should expose index pattern payload");
    assert_eq!(
        index_pattern
            .syntax_pattern()
            .and_then(|pattern| pattern.binding_name())
            .as_deref(),
        Some("index")
    );

    let value_pattern = for_statement
        .for_value_pattern_payload()
        .expect("for statement should expose value pattern payload");
    assert_eq!(
        value_pattern
            .syntax_pattern()
            .and_then(|pattern| pattern.pattern_kind()),
        Some(vela_syntax::ast::SyntaxPatternKind::RecordVariant)
    );
    let record_fields = value_pattern
        .record_field_payloads()
        .expect("record pattern should expose field payloads");
    let field_labels = record_fields
        .iter()
        .filter_map(|field| field.syntax_label_name())
        .collect::<Vec<_>>();
    assert_eq!(field_labels, ["code", "message"]);
    assert_eq!(
        record_fields[0]
            .pattern_payload()
            .and_then(|payload| {
                payload
                    .syntax_pattern()
                    .and_then(|pattern| pattern.binding_name())
            })
            .as_deref(),
        Some("status")
    );

    compile_program_source(source, text).expect("CST-backed for patterns should compile");
}
