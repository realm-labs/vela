use super::*;

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
    let map_entries = payload
        .body
        .statement_payloads()
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
    let record_fields = payload
        .body
        .statement_payloads()
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

fn assert_cst_let_initializer_record_paths(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&[&str]],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter_map(|payload| payload.record_path_segments())
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
    let statements = payload.body.statement_payloads();
    let block_payloads = statements
        .iter()
        .filter_map(|statement| statement.let_initializer_block_body_payload())
        .collect::<Vec<_>>();
    assert_eq!(block_payloads.len(), 2);

    let array_block_statements = block_payloads[0].statement_payloads();
    let array_tail = array_block_statements
        .last()
        .expect("array block tail statement")
        .expression_payload()
        .expect("array tail expression payload");
    let array_actual = array_tail
        .array_element_payloads()
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
        .array_element_payloads()
        .expect("array element payloads")
        .iter()
        .flat_map(|element| element.map_entry_payloads().unwrap_or_default())
        .map(|entry| entry.value_expression_payload())
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

    let record_block_statements = block_payloads[1].statement_payloads();
    let record_tail = record_block_statements
        .last()
        .expect("record block tail statement")
        .expression_payload()
        .expect("record tail expression payload");
    let record_actual = record_tail
        .record_field_payloads()
        .expect("record field payloads")
        .iter()
        .filter_map(|field| field.value_expression_payload())
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
    let statements = body.statement_payloads();
    let values = statements
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(|payload| payload.record_field_payloads().unwrap_or_default())
        .filter_map(|field| field.value_expression_payload())
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
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .flat_map(|payload| payload.record_field_payloads().unwrap_or_default())
        .filter_map(|field| field.value_expression_payload())
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
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .flat_map(|statement| statement.call_argument_payloads().unwrap_or_default())
        .map(|argument| argument.value_expression_payload())
        .flat_map(|payload| payload.record_field_payloads().unwrap_or_default())
        .filter_map(|field| field.value_expression_payload())
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
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .filter_map(|statement| statement.return_value_expression_payload())
        .flat_map(|payload| payload.record_field_payloads().unwrap_or_default())
        .filter_map(|field| field.value_expression_payload())
        .filter_map(|value| {
            let body = value.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected_block));
}
