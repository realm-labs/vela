use super::*;

fn container_statement_payloads<'ast>(
    body: &body_payloads::CompilerBodyPayload<'ast>,
) -> Vec<body_payloads::CompilerStatementPayload<'ast>> {
    paired_statement_payloads_for_body(body.syntax_payload().source, body)
}

#[test]
fn semantic_function_array_element_values_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn take(values) {
    return values;
}

fn array_values() {
    let values = [
        {
            let start = 1;
            start
        },
        if true {
            let next = 2;
            next
        } else {
            0
        },
        match 0 {
            0 => {
                let zero = 1;
                zero
            },
            _ => {
                2
            },
        },
    ];
    values = [
        {
            let assigned = 3;
            assigned
        },
    ];
    take([
        {
            let arg = 4;
            arg
        },
    ]);
}

fn return_values() {
    return [
        {
            let ret = 5;
            ret
        },
    ];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("array_values")
        .expect("array_values function");
    assert_cst_let_initializers(
        &payload.body,
        &[(
            SyntaxExpressionKind::Array,
            "[\n        {\n            let start = 1;\n            start\n        },\n        if true {\n            let next = 2;\n            next\n        } else {\n            0\n        },\n        match 0 {\n            0 => {\n                let zero = 1;\n                zero\n            },\n            _ => {\n                2\n            },\n        },\n    ]",
        )],
    );
    assert_cst_let_initializer_array_element_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let start = 1;"),
            (SyntaxStatementKind::Expr, "start"),
        ]],
        &[vec![
            (SyntaxStatementKind::Let, "let next = 2;"),
            (SyntaxStatementKind::Expr, "next"),
        ]],
        &[vec![(SyntaxStatementKind::Expr, "0")]],
        &[
            vec![
                (SyntaxStatementKind::Let, "let zero = 1;"),
                (SyntaxStatementKind::Expr, "zero"),
            ],
            vec![(SyntaxStatementKind::Expr, "2")],
        ],
    );
    assert_cst_assignment_value_array_element_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let assigned = 3;"),
            (SyntaxStatementKind::Expr, "assigned"),
        ]],
    );
    assert_cst_call_argument_array_element_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let arg = 4;"),
            (SyntaxStatementKind::Expr, "arg"),
        ]],
    );
    let (return_payload, _, _) = semantic
        .function("return_values")
        .expect("return_values function");
    assert_cst_return_value_array_element_body_payloads(
        &return_payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let ret = 5;"),
            (SyntaxStatementKind::Expr, "ret"),
        ]],
    );

    compile_program_source(source, text).expect("CST-backed array element values should compile");
}

#[test]
fn equal_count_container_payloads_pair_children_by_position_not_legacy_span() {
    with_cst_payload_compiler(
        r#"
struct Pair {
    first
}

fn main() {
    let cst_array = [true];
    let legacy_array = [1];
    let cst_map = { value: true };
    let legacy_map = { value: 1 };
    let cst_record = Pair { first: true };
    let legacy_record = Pair { first: 1 };
}
"#,
        |_compiler, payload| {
            let statements = container_statement_payloads(&payload.body);

            let cst_array = statements[0]
                .let_initializer_expression_payload()
                .expect("CST array payload");
            let legacy_array = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy array fallback");
            let mismatched_array = expression_payload_with_fallback(
                SourceId::new(1),
                cst_array
                    .syntax_expression()
                    .expect("CST array expression")
                    .clone(),
                legacy_array.fallback(),
            );
            let array_elements = mismatched_array
                .array_element_payloads()
                .expect("array element payloads");
            assert_eq!(array_elements.len(), 1);
            assert_eq!(
                array_elements[0]
                    .syntax_expression()
                    .expect("CST array element")
                    .syntax()
                    .text()
                    .to_string(),
                "true"
            );

            let cst_map = statements[2]
                .let_initializer_expression_payload()
                .expect("CST map payload");
            let legacy_map = statements[3]
                .let_initializer_expression_payload()
                .expect("legacy map fallback");
            let mismatched_map = expression_payload_with_fallback(
                SourceId::new(1),
                cst_map
                    .syntax_expression()
                    .expect("CST map expression")
                    .clone(),
                legacy_map.fallback(),
            );
            let map_entries = mismatched_map
                .map_entry_payloads()
                .expect("map entry payloads");
            let map_values = mismatched_map
                .map_entry_value_payloads()
                .expect("map value payloads");
            assert_eq!(map_entries.len(), 1);
            assert_eq!(map_entries[0].syntax_key_name().as_deref(), Some("value"));
            assert_eq!(
                map_values[0]
                    .syntax_expression()
                    .expect("CST map value")
                    .syntax()
                    .text()
                    .to_string(),
                "true"
            );

            let cst_record = statements[4]
                .let_initializer_expression_payload()
                .expect("CST record payload");
            let legacy_record = statements[5]
                .let_initializer_expression_payload()
                .expect("legacy record fallback");
            let mismatched_record = expression_payload_with_fallback(
                SourceId::new(1),
                cst_record
                    .syntax_expression()
                    .expect("CST record expression")
                    .clone(),
                legacy_record.fallback(),
            );
            let record_fields = mismatched_record
                .record_field_payloads()
                .expect("record field payloads");
            let record_values = mismatched_record
                .record_field_value_payloads()
                .expect("record field value payloads");
            assert_eq!(record_fields.len(), 1);
            assert_eq!(
                record_fields[0].syntax_label_name().as_deref(),
                Some("first")
            );
            assert_eq!(
                record_values[0]
                    .syntax_expression()
                    .expect("CST record field value")
                    .syntax()
                    .text()
                    .to_string(),
                "true"
            );
            let legacy_named_fields = vec![vela_syntax::ast::RecordField {
                name: "legacy_only".to_owned(),
                span: Span::new(SourceId::new(1), 0, 1),
                value: None,
            }];
            assert_eq!(
                crate::compiler::constructors::record_field_names(
                    &legacy_named_fields,
                    &record_fields,
                ),
                vec![Some("first".to_owned())],
                "record field names must come from the CST field payload"
            );
        },
    );
}

#[test]
fn missing_container_expression_payload_does_not_use_legacy_container() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let values = [1];
}
"#,
        |compiler, payload| {
            let array = container_statement_payloads(&payload.body)[0]
                .let_initializer_expression_payload()
                .expect("array payload");
            let missing = body_payloads::CompilerExpressionPayload::missing_syntax(
                SourceId::new(1),
                array.fallback(),
            );

            let error = compiler
                .compile_expr_with_payload(array.fallback(), Some(&missing))
                .expect_err("missing CST array expression must not compile legacy array");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST expression payload")
            ));
        },
    );
}

#[test]
fn missing_record_path_payload_does_not_use_legacy_record_path() {
    with_cst_payload_compiler(
        r#"
struct Pair {
    first
}

fn main() {
    let value = Pair { first: 1 };
}
"#,
        |compiler, payload| {
            let record = container_statement_payloads(&payload.body)[0]
                .let_initializer_expression_payload()
                .expect("record payload");
            let missing_path =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    record.syntax_expression().expect("record syntax").clone(),
                    record.fallback(),
                );

            let error = compiler
                .compile_expr_with_payload(record.fallback(), Some(&missing_path))
                .expect_err("missing CST record path must not compile legacy record path");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST record path")
            ));
        },
    );
}

#[test]
fn missing_array_element_payload_does_not_use_legacy_value() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let value = [];
}
"#;
    let legacy_text = r#"
fn main() {
    let value = [1];
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_array = cst_parse
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
    assert!(cst_array.as_array().is_some());

    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_array = container_statement_payloads(&legacy_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("legacy array payload");
    let missing = expression_payload_with_fallback(source, cst_array, legacy_array.fallback());
    let element_payloads = missing
        .array_element_payloads()
        .expect("array element payloads");

    assert_eq!(element_payloads.len(), 0);

    let error = compiler
        .compile_expr_with_payload(legacy_array.fallback(), Some(&missing))
        .expect_err("missing array element payload must not compile legacy value");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST array elements")
    ));
}

#[test]
fn container_value_types_reject_mismatched_cst_payload_shape() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_array = [true];
    let legacy_array = [1];
    let cst_map = { value: true };
    let legacy_map = { value: 1 };
}
"#,
        |compiler, payload| {
            let statements = container_statement_payloads(&payload.body);
            let cst_array = statements[0]
                .let_initializer_expression_payload()
                .expect("CST array payload");
            let legacy_array = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy array fallback");
            let mismatched_array = expression_payload_with_fallback(
                SourceId::new(1),
                cst_array
                    .syntax_expression()
                    .expect("CST array expression")
                    .clone(),
                legacy_array.fallback(),
            );
            assert_eq!(
                compiler.static_type_for_expr_with_payload(
                    mismatched_array.fallback(),
                    Some(&mismatched_array),
                ),
                value_types::StaticExprType::Dynamic
            );

            let cst_map = statements[2]
                .let_initializer_expression_payload()
                .expect("CST map payload");
            let legacy_map = statements[3]
                .let_initializer_expression_payload()
                .expect("legacy map fallback");
            let mismatched_map = expression_payload_with_fallback(
                SourceId::new(1),
                cst_map
                    .syntax_expression()
                    .expect("CST map expression")
                    .clone(),
                legacy_map.fallback(),
            );
            assert_eq!(
                compiler.static_type_for_expr_with_payload(
                    mismatched_map.fallback(),
                    Some(&mismatched_map),
                ),
                value_types::StaticExprType::Dynamic
            );
        },
    );
}

#[test]
fn container_value_shapes_prefer_cst_payload_shape() {
    with_cst_payload_compiler(
        r#"
struct Pair {
    first
}

fn main() {
    let cst_array = [true];
    let legacy_array = [1];
    let cst_map = { "value": true };
    let legacy_map = { "value": 1 };
    let cst_record = Pair { first: true };
    let legacy_record = Pair { first: 1 };
}
"#,
        |compiler, payload| {
            let statements = container_statement_payloads(&payload.body);
            let cst_array = statements[0]
                .let_initializer_expression_payload()
                .expect("CST array payload");
            let legacy_array = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy array fallback");
            let mismatched_array = expression_payload_with_fallback(
                SourceId::new(1),
                cst_array
                    .syntax_expression()
                    .expect("CST array expression")
                    .clone(),
                legacy_array.fallback(),
            );
            assert_eq!(
                compiler
                    .value_shape_for_expr_with_payload(
                        mismatched_array.fallback(),
                        Some(&mismatched_array),
                    )
                    .and_then(|shape| shape.value_type()),
                Some(RuntimeTypeFact::array(RuntimeTypeFact::primitive(
                    vela_common::PrimitiveTag::Bool,
                )))
            );

            let cst_map = statements[2]
                .let_initializer_expression_payload()
                .expect("CST map payload");
            let legacy_map = statements[3]
                .let_initializer_expression_payload()
                .expect("legacy map fallback");
            let mismatched_map = expression_payload_with_fallback(
                SourceId::new(1),
                cst_map
                    .syntax_expression()
                    .expect("CST map expression")
                    .clone(),
                legacy_map.fallback(),
            );
            assert_eq!(
                compiler
                    .value_shape_for_expr_with_payload(
                        mismatched_map.fallback(),
                        Some(&mismatched_map),
                    )
                    .and_then(|shape| shape.value_type()),
                Some(RuntimeTypeFact::map(
                    RuntimeTypeFact::primitive(vela_common::PrimitiveTag::String),
                    RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Bool),
                ))
            );

            let cst_record = statements[4]
                .let_initializer_expression_payload()
                .expect("CST record payload");
            let legacy_record = statements[5]
                .let_initializer_expression_payload()
                .expect("legacy record fallback");
            let mismatched_record = expression_payload_with_fallback(
                SourceId::new(1),
                cst_record
                    .syntax_expression()
                    .expect("CST record expression")
                    .clone(),
                legacy_record.fallback(),
            );
            let record = compiler
                .value_shape_for_expr_with_payload(
                    mismatched_record.fallback(),
                    Some(&mismatched_record),
                )
                .and_then(|shape| shape.as_record().cloned())
                .expect("CST record shape");
            assert_eq!(
                record.field_value_type("first"),
                Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Bool))
            );
        },
    );
}

#[test]
fn semantic_function_map_entry_values_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn take(values) {
    return values;
}

fn map_values() {
    let values = {
        start: {
            let start = 1;
            start
        },
        next: if true {
            let next = 2;
            next
        } else {
            0
        },
        matched: match 0 {
            0 => {
                let zero = 1;
                zero
            },
            _ => {
                2
            },
        },
    };
    values = {
        assigned: {
            let assigned = 3;
            assigned
        },
    };
    take({
        arg: {
            let arg = 4;
            arg
        },
    });
}

fn return_map() {
    return {
        ret: {
            let ret = 5;
            ret
        },
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("map_values")
        .expect("map_values function");
    assert_cst_let_initializers(
        &payload.body,
        &[(
            SyntaxExpressionKind::Map,
            "{\n        start: {\n            let start = 1;\n            start\n        },\n        next: if true {\n            let next = 2;\n            next\n        } else {\n            0\n        },\n        matched: match 0 {\n            0 => {\n                let zero = 1;\n                zero\n            },\n            _ => {\n                2\n            },\n        },\n    }",
        )],
    );
    let map_entries = container_statement_payloads(&payload.body)
        .into_iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(|payload| payload.map_entry_payloads().unwrap_or_default())
        .collect::<Vec<_>>();
    let map_keys = map_entries
        .iter()
        .filter_map(|entry| entry.syntax_key_name())
        .collect::<Vec<_>>();
    assert_eq!(map_keys, ["start", "next", "matched"]);
    assert_cst_let_initializer_map_entry_value_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let start = 1;"),
            (SyntaxStatementKind::Expr, "start"),
        ]],
        &[vec![
            (SyntaxStatementKind::Let, "let next = 2;"),
            (SyntaxStatementKind::Expr, "next"),
        ]],
        &[vec![(SyntaxStatementKind::Expr, "0")]],
        &[
            vec![
                (SyntaxStatementKind::Let, "let zero = 1;"),
                (SyntaxStatementKind::Expr, "zero"),
            ],
            vec![(SyntaxStatementKind::Expr, "2")],
        ],
    );
    assert_cst_assignment_value_map_entry_value_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let assigned = 3;"),
            (SyntaxStatementKind::Expr, "assigned"),
        ]],
    );
    assert_cst_call_argument_map_entry_value_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let arg = 4;"),
            (SyntaxStatementKind::Expr, "arg"),
        ]],
    );
    let (return_payload, _, _) = semantic
        .function("return_map")
        .expect("return_map function");
    assert_cst_return_value_map_entry_value_body_payloads(
        &return_payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let ret = 5;"),
            (SyntaxStatementKind::Expr, "ret"),
        ]],
    );

    compile_program_source(source, text).expect("CST-backed map entry values should compile");
}

#[test]
fn missing_map_entry_value_payload_does_not_use_legacy_value() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let value = {
        key:
    };
}
"#;
    let legacy_text = r#"
fn main() {
    let value = {
        key: 1
    };
}
"#;
    let missing = first_map_entry_payload_from_cst(source, cst_text);
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_map = container_statement_payloads(&legacy_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("legacy map payload");
    let ExprKind::Map(legacy_entries) = &legacy_map.fallback().kind else {
        panic!("expected legacy map fallback");
    };

    assert_eq!(missing.syntax_key_name().as_deref(), Some("key"));
    assert!(!missing.has_value_syntax());

    let error = compiler
        .compile_map_entry(&legacy_entries[0], &missing)
        .expect_err("missing map entry value payload must not compile legacy value");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST map entry value")
    ));
}

#[test]
fn semantic_function_record_field_values_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
struct Pair {
    first
    second
    third
}

fn take(value) {
    return value;
}

fn record_values() {
    let seed = 1;
    let value = Pair {
        first: {
            let start = seed;
            start
        },
        second: if true {
            let next = seed + 1;
            next
        } else {
            0
        },
        third: match seed {
            1 => {
                let matched = seed;
                matched
            },
            _ => {
                0
            },
        },
    };
    value = Pair {
        first: {
            let assigned = 3;
            assigned
        },
        second: seed,
        third: seed,
    };
    take(Pair {
        first: {
            let arg = 4;
            arg
        },
        second: seed,
        third: seed,
    });
}

fn return_record() {
    return Pair {
        first: {
            let ret = 5;
            ret
        },
        second: 0,
        third: 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("record_values")
        .expect("record_values function");
    assert_cst_let_initializers(
        &payload.body,
        &[
            (SyntaxExpressionKind::Literal, "1"),
            (
                SyntaxExpressionKind::Record,
                "Pair {\n        first: {\n            let start = seed;\n            start\n        },\n        second: if true {\n            let next = seed + 1;\n            next\n        } else {\n            0\n        },\n        third: match seed {\n            1 => {\n                let matched = seed;\n                matched\n            },\n            _ => {\n                0\n            },\n        },\n    }",
            ),
        ],
    );
    let record_fields = container_statement_payloads(&payload.body)
        .into_iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(|payload| payload.record_field_payloads().unwrap_or_default())
        .collect::<Vec<_>>();
    let record_field_names = record_fields
        .iter()
        .filter_map(|field| field.syntax_label_name())
        .collect::<Vec<_>>();
    assert_eq!(record_field_names, ["first", "second", "third"]);
    assert_cst_let_initializer_record_paths(&payload.body, &[&["Pair"]]);
    assert_cst_let_initializer_record_field_value_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let start = seed;"),
            (SyntaxStatementKind::Expr, "start"),
        ]],
        &[vec![
            (SyntaxStatementKind::Let, "let next = seed + 1;"),
            (SyntaxStatementKind::Expr, "next"),
        ]],
        &[vec![(SyntaxStatementKind::Expr, "0")]],
        &[
            vec![
                (SyntaxStatementKind::Let, "let matched = seed;"),
                (SyntaxStatementKind::Expr, "matched"),
            ],
            vec![(SyntaxStatementKind::Expr, "0")],
        ],
    );
    assert_cst_assignment_value_record_field_value_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let assigned = 3;"),
            (SyntaxStatementKind::Expr, "assigned"),
        ]],
    );
    assert_cst_call_argument_record_field_value_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let arg = 4;"),
            (SyntaxStatementKind::Expr, "arg"),
        ]],
    );
    let (return_payload, _, _) = semantic
        .function("return_record")
        .expect("return_record function");
    assert_cst_return_value_record_field_value_body_payloads(
        &return_payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let ret = 5;"),
            (SyntaxStatementKind::Expr, "ret"),
        ]],
    );

    compile_program_source(source, text).expect("CST-backed record field values should compile");
}

#[test]
fn missing_record_field_value_payload_does_not_use_legacy_value() {
    let source = SourceId::new(1);
    let cst_text = r#"
struct Pair {
    first
}

fn main() {
    let value = Pair {
        first:
    };
}
"#;
    let legacy_text = r#"
struct Pair {
    first
}

fn main() {
    let value = Pair {
        first: 1
    };
}
"#;
    let missing = first_record_field_payload_from_cst(source, cst_text);
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_record = container_statement_payloads(&legacy_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("legacy record payload");
    let ExprKind::Record {
        fields: legacy_fields,
        ..
    } = &legacy_record.fallback().kind
    else {
        panic!("expected legacy record fallback");
    };

    assert_eq!(missing.syntax_label_name().as_deref(), Some("first"));
    assert!(!missing.has_value_syntax());

    let error = compiler
        .compile_record_fields(legacy_fields, Vec::new(), None, &[missing])
        .expect_err("missing record field value payload must not compile legacy value");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST record field value")
    ));
}

fn assert_cst_let_initializer_record_paths(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&[&str]],
) {
    let actual = container_statement_payloads(body)
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter_map(|payload| payload.syntax_record_path_segments())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_segments(expected));
}

fn expected_segments(expected: &[&[&str]]) -> Vec<Vec<String>> {
    expected
        .iter()
        .map(|segments| {
            segments
                .iter()
                .map(|segment| (*segment).to_owned())
                .collect()
        })
        .collect()
}

#[test]
fn semantic_function_typed_record_field_values_keep_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
struct TypedPair {
    first: i64
    second
}

fn typed_record_values() {
    let value = TypedPair {
        first: {
            let typed = 6;
            typed
        },
        second: 0,
    };
    return value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("typed_record_values")
        .expect("typed_record_values function");
    assert_cst_let_initializer_record_field_value_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let typed = 6;"),
            (SyntaxStatementKind::Expr, "typed"),
        ]],
        &[],
        &[],
        &[],
    );

    compile_program_source(source, text)
        .expect("CST-backed typed record field values should compile");
}

#[test]
fn semantic_function_block_tail_container_expressions_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
struct TailPair {
    first
    second
}

fn block_tail_containers() {
    let array = {
        let seed = 1;
        [
            {
                let item = seed;
                item
            },
            {
                value: {
                    let entry = seed;
                    entry
                },
            },
        ]
    };
    let record = {
        let seed = 2;
        TailPair {
            first: {
                let field = seed;
                field
            },
            second: seed,
        }
    };
    return array;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("block_tail_containers")
        .expect("block_tail_containers function");
    let statements = container_statement_payloads(&payload.body);
    let block_payloads = statements
        .iter()
        .filter_map(|statement| statement.let_initializer_block_body_payload())
        .collect::<Vec<_>>();
    assert_eq!(block_payloads.len(), 2);

    let array_block_statements = container_statement_payloads(&block_payloads[0]);
    let array_tail = array_block_statements
        .last()
        .expect("array block tail statement")
        .expression_payload()
        .expect("array tail expression payload");
    let array_actual = array_tail
        .array_element_value_payloads()
        .expect("array element payloads")
        .iter()
        .filter_map(|element| {
            let body = element.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        array_actual,
        expected_statement_texts(&[vec![
            (SyntaxStatementKind::Let, "let item = seed;"),
            (SyntaxStatementKind::Expr, "item"),
        ]])
    );

    let map_actual = array_tail
        .array_element_value_payloads()
        .expect("array element payloads")
        .iter()
        .flat_map(|element| element.map_entry_value_payloads().unwrap_or_default())
        .filter_map(|value| {
            let body = value.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        map_actual,
        expected_statement_texts(&[vec![
            (SyntaxStatementKind::Let, "let entry = seed;"),
            (SyntaxStatementKind::Expr, "entry"),
        ]])
    );

    let record_block_statements = container_statement_payloads(&block_payloads[1]);
    let record_tail = record_block_statements
        .last()
        .expect("record block tail statement")
        .expression_payload()
        .expect("record tail expression payload");
    let record_actual = record_tail
        .record_field_value_payloads()
        .expect("record field payloads")
        .into_iter()
        .filter_map(|value| {
            let body = value.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        record_actual,
        expected_statement_texts(&[vec![
            (SyntaxStatementKind::Let, "let field = seed;"),
            (SyntaxStatementKind::Expr, "field"),
        ]])
    );

    compile_program_source(source, text).expect("CST-backed block tail containers should compile");
}

fn assert_cst_let_initializer_record_field_value_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected_block: &[Vec<(SyntaxStatementKind, &str)>],
    expected_then: &[Vec<(SyntaxStatementKind, &str)>],
    expected_else: &[Vec<(SyntaxStatementKind, &str)>],
    expected_match: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = container_statement_payloads(body);
    let values = statements
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(|payload| payload.record_field_value_payloads().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_cst_array_element_body_payloads(
        &values,
        expected_block,
        expected_then,
        expected_else,
        expected_match,
    );
}

fn assert_cst_assignment_value_record_field_value_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected_block: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = container_statement_payloads(body);
    let actual = statements
        .iter()
        .filter_map(|statement| {
            statement
                .expression_payload()
                .and_then(|payload| payload.assignment_value_payload())
        })
        .flat_map(|payload| payload.record_field_value_payloads().unwrap_or_default())
        .filter_map(|value| {
            let body = value.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected_block));
}

fn assert_cst_call_argument_record_field_value_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected_block: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = container_statement_payloads(body);
    let actual = statements
        .iter()
        .flat_map(|statement| {
            statement
                .expression_payload()
                .and_then(|payload| payload.call_argument_value_payloads())
                .unwrap_or_default()
        })
        .flat_map(|payload| payload.record_field_value_payloads().unwrap_or_default())
        .filter_map(|value| {
            let body = value.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected_block));
}

fn assert_cst_return_value_record_field_value_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected_block: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = container_statement_payloads(body);
    let actual = statements
        .iter()
        .filter_map(|statement| statement.return_value_expression_payload())
        .flat_map(|payload| payload.record_field_value_payloads().unwrap_or_default())
        .filter_map(|value| {
            let body = value.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected_block));
}
