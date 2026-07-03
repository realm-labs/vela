use super::*;

#[test]
fn syntax_only_constant_expression_statements_drop_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    1 + 2;
    !false;
    [1, 2, 3];
    [[1], [2]];
    ["score", "level"];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    assert!(
        !payload.body.has_fallback_statements(),
        "constant expression statement body should not require owned fallback"
    );
    let statements = payload.body.statement_payloads();
    assert_eq!(statements.len(), 5);
    assert!(statements.iter().all(|statement| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statement.fallback())).is_err()
    }));

    compile_program_source(source, text)
        .expect("CST-only constant expression statements should compile");
}

#[test]
fn syntax_only_path_expression_statements_drop_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main(input) {
    input;
    (input);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    assert!(
        !payload.body.has_fallback_statements(),
        "path expression statement body should not require owned fallback"
    );
    let statements = payload.body.statement_payloads();
    assert_eq!(statements.len(), 2);
    assert!(statements.iter().all(|statement| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statement.fallback())).is_err()
    }));

    compile_program_source(source, text)
        .expect("CST-only path expression statements should compile");
}

#[test]
fn syntax_only_path_value_expression_statements_drop_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main(input, other) {
    !input;
    input == other;
    input < other;
    input * other;
    input > 0;
    input == 0;
    input + 1;
    10 > input;
    10 <= input;
    0 == input;
    0 != input;
    1 + input;
    1 - input;
    2 * input;
    8 / input;
    8 % input;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    assert!(
        !payload.body.has_fallback_statements(),
        "path value expression statement body should not require owned fallback"
    );
    let statements = payload.body.statement_payloads();
    assert_eq!(statements.len(), 16);
    assert!(statements.iter().all(|statement| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statement.fallback())).is_err()
    }));

    compile_program_source(source, text)
        .expect("CST-only path value expression statements should compile");
}

#[test]
fn syntax_only_range_expression_statements_drop_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main(input) {
    1..4;
    input..=10;
    (-2)..(1 + 2);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    assert!(
        !payload.body.has_fallback_statements(),
        "range expression statement body should not require owned fallback"
    );
    let statements = payload.body.statement_payloads();
    assert_eq!(statements.len(), 3);
    assert!(statements.iter().all(|statement| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statement.fallback())).is_err()
    }));

    compile_program_source(source, text)
        .expect("CST-only range expression statements should compile");
}

#[test]
fn syntax_only_field_range_expression_statements_drop_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main(input) {
    input.start..input.end;
    input.offset..=10;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    assert!(
        !payload.body.has_fallback_statements(),
        "field range expression statements should not require owned fallback"
    );

    compile_program_source(source, text)
        .expect("CST-only field range expression statements should compile");
}

#[test]
fn syntax_only_block_expression_statements_drop_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main(input) {
    ({
        let copied = input;
        copied;
    });
    ({
        return input;
    });
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    assert!(
        !payload.body.has_fallback_statements(),
        "block expression statement body should not require owned fallback"
    );
    let statements = payload.body.statement_payloads();
    assert_eq!(statements.len(), 2);
    assert!(statements.iter().all(|statement| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statement.fallback())).is_err()
    }));

    let nested_bodies = statements
        .iter()
        .map(|statement| {
            statement
                .expression_statement_block_body_payload()
                .expect("block expression statement body payload")
        })
        .collect::<Vec<_>>();
    for body in &nested_bodies {
        assert!(
            !body.has_fallback_statements(),
            "nested block expression body should not require owned fallback"
        );
    }

    compile_program_source(source, text)
        .expect("CST-only block expression statements should compile");
}

#[test]
fn syntax_only_unterminated_expression_tails_drop_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn constant_tail() {
    1 + 2
}

fn path_tail(input) {
    input
}

fn range_tail(input) {
    input..=10
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    for function in ["constant_tail", "path_tail", "range_tail"] {
        let (payload, _, _) = semantic.function(function).expect("function payload");
        assert!(
            !payload.body.has_fallback_statements(),
            "{function} body should not require owned fallback"
        );
        let statements = payload.body.statement_payloads();
        assert_eq!(statements.len(), 1);
        let statement_fallback =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));
        assert!(
            statement_fallback.is_err(),
            "{function} tail statement should not require owned fallback"
        );
    }

    compile_program_source(source, text)
        .expect("CST-only unterminated expression tails should compile");
}

#[test]
fn syntax_only_block_expression_tails_drop_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main(input) {
    ({
        let copied = input;
        copied
    })
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    assert!(
        !payload.body.has_fallback_statements(),
        "block expression tail body should not require owned fallback"
    );
    let statements = payload.body.statement_payloads();
    assert_eq!(statements.len(), 1);
    let statement_fallback =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));
    assert!(
        statement_fallback.is_err(),
        "block expression tail statement should not require owned fallback"
    );
    let nested_body = statements[0]
        .expression_statement_block_body_payload()
        .expect("block expression tail body payload");
    assert!(
        !nested_body.has_fallback_statements(),
        "nested block expression tail body should not require owned fallback"
    );

    compile_program_source(source, text).expect("CST-only block expression tails should compile");
}

#[test]
fn semantic_function_generic_expression_statements_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn expression_statements() {
    let values = [1, 2, 3];
    ({
        let selected = values;
        selected
    })[0];
    [{
        let item = 1;
        item
    }];
    f"status { {
        let count = values.len();
        count
    } }";
    return values.len();
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("expression_statements")
        .expect("expression_statements function");

    assert_cst_expr_statements(
        &payload.body,
        &[
            (
                SyntaxExpressionKind::Index,
                "({\n        let selected = values;\n        selected\n    })[0]",
            ),
            (
                SyntaxExpressionKind::Array,
                "[{\n        let item = 1;\n        item\n    }]",
            ),
            (
                SyntaxExpressionKind::Literal,
                "f\"status { {\n        let count = values.len();\n        count\n    } }\"",
            ),
        ],
    );
    assert_cst_expression_statement_index_base_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let selected = values;"),
            (SyntaxStatementKind::Expr, "selected"),
        ]],
    );
    assert_cst_expression_statement_array_element_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let item = 1;"),
            (SyntaxStatementKind::Expr, "item"),
        ]],
    );
    assert_cst_expression_statement_interpolation_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let count = values.len();"),
            (SyntaxStatementKind::Expr, "count"),
        ]],
    );

    compile_program_source(source, text)
        .expect("CST-backed generic expression statements should compile");
}

#[test]
fn mismatched_expression_statement_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn take(value) {
    return value;
}

fn main() {
    let values = [1];
    take(1);
    values[0];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_call = statements[1]
        .syntax_statement()
        .expect("CST call statement")
        .clone();
    let legacy_index = statements[2].fallback();
    let mismatched =
        body_payloads::CompilerStatementPayload::syntax(source, cst_call, legacy_index);

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("mismatched expression statement payload must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST expression statement payload")
    ));
}

#[test]
fn missing_expression_statement_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    ;
}
"#;
    let legacy_text = r#"
fn main() {
    let values = [1];
    values[0];
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
        .expect("CST expression statement");
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_statement = legacy_payload.body.statement_payloads()[1].fallback();
    let missing =
        body_payloads::CompilerStatementPayload::syntax(source, cst_statement, legacy_statement);

    assert_eq!(missing.statement_kind(), Some(SyntaxStatementKind::Expr));
    assert_eq!(missing.expression_kind(), None);

    let error = compiler
        .compile_statement_payload_for_test(&missing)
        .expect_err("missing expression statement payload must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST expression statement payload")
    ));
}

#[test]
fn mismatched_control_flow_expression_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let cst_value = if true {
        1
    } else {
        0
    };
    let legacy_value = match 0 {
        _ => 1,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_if = statements[0]
        .let_initializer_expression_payload()
        .expect("CST if initializer payload");
    let legacy_match = statements[1]
        .let_initializer_expression_payload()
        .expect("legacy match initializer payload");
    let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
        source,
        cst_if
            .syntax_expression()
            .expect("CST if expression")
            .clone(),
        legacy_match.fallback(),
    );

    let error = compiler
        .compile_expr_with_payload(legacy_match.fallback(), Some(&mismatched_payload))
        .expect_err("mismatched control-flow payload must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST expression payload")
    ));
}

fn assert_cst_expression_statement_index_base_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::expression_payload)
        .filter_map(|payload| payload.index_operand_payloads())
        .flat_map(|(base, _)| nested_expression_block_payloads(base))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_expression_statement_array_element_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::expression_payload)
        .flat_map(|payload| {
            let Some(items) = payload.fallback_array_items() else {
                return Vec::new();
            };
            payload.array_element_payloads(items).unwrap_or_default()
        })
        .flat_map(nested_expression_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_expression_statement_interpolation_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::expression_payload)
        .flat_map(|payload| {
            payload
                .fallback_interpolated_string_parts()
                .and_then(|parts| payload.interpolated_expression_payloads(parts))
                .unwrap_or_default()
        })
        .flat_map(nested_expression_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn nested_expression_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    if let Some(body) = payload.block_body_payload() {
        return vec![cst_statement_texts(&body)];
    }
    if let Some(inner) = payload.paren_inner_payload() {
        return nested_expression_block_payloads(inner);
    }
    Vec::new()
}
