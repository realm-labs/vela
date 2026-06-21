use super::*;

#[test]
fn semantic_function_path_expressions_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn take(value) {
    return value;
}

fn path_values(input) {
    let copy = input;
    take(copy);
    return copy;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("path_values")
        .expect("path_values function");

    assert_cst_let_initializer_path_segments(&payload.body, &[&["input"]]);
    assert_cst_call_argument_path_segments(&payload.body, &[&["copy"]]);
    assert_cst_return_value_path_segments(&payload.body, &[&["copy"]]);

    compile_program_source(source, text).expect("CST-backed path expressions should compile");
}

fn assert_cst_let_initializer_path_segments(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&[&str]],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter_map(path_payload_segments)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_segments(expected));
}

fn assert_cst_call_argument_path_segments(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&[&str]],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.call_argument_payloads().unwrap_or_default())
        .map(|argument| argument.value_expression_payload())
        .filter_map(path_payload_segments)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_segments(expected));
}

fn assert_cst_return_value_path_segments(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&[&str]],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.return_value_expression_payload())
        .filter_map(path_payload_segments)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_segments(expected));
}

fn path_payload_segments(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Option<Vec<String>> {
    assert_eq!(payload.kind(), Some(SyntaxExpressionKind::Path));
    assert_eq!(
        payload
            .syntax_expression()
            .and_then(|expression| expression.as_path())
            .map(|path| path.path_segments()),
        payload.path_segments()
    );
    payload.path_segments()
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
