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

#[test]
fn script_type_facts_prefer_cst_payload_shape() {
    let source = SourceId::new(1);
    let text = r#"
struct CstBox {}
struct LegacyBox {}

fn cst_record() {
    return CstBox {};
}

fn legacy_path(legacy) {
    return legacy;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_record").expect("cst function");
    let (legacy_payload, _, _) = semantic.function("legacy_path").expect("legacy function");
    let cst_return = cst_payload
        .body
        .statement_payloads()
        .into_iter()
        .find_map(|statement| statement.return_value_expression_payload())
        .expect("CST record return expression");
    let legacy_return = legacy_payload
        .body
        .statement_payloads()
        .into_iter()
        .find_map(|statement| statement.return_value_expression_payload())
        .expect("legacy path return expression");
    let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
        source,
        cst_return
            .syntax_expression()
            .expect("record CST expression")
            .clone(),
        legacy_return.fallback(),
    );

    let fact = script_types::expression_script_fact_with_payload(
        mismatched_payload.fallback(),
        Some(&mismatched_payload),
        |_| None,
        |_| None,
        |_| None,
    )
    .expect("CST record payload should produce a script type fact");
    assert_eq!(fact, script_types::ScriptTypeFact::new("CstBox"));
}

#[test]
fn script_type_facts_with_non_matching_cst_payload_do_not_use_legacy_shape() {
    let source = SourceId::new(1);
    let text = r#"
struct LegacyBox {}

enum LegacyResult {
    Ok(value),
}

fn cst_block(cst) {
    return {
        let selected = cst;
        selected
    };
}

fn legacy_record() {
    return LegacyBox {};
}

fn legacy_path(legacy) {
    return legacy;
}

fn legacy_call(legacy) {
    return LegacyResult::Ok(legacy);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_block").expect("cst function");
    let cst_block = cst_payload
        .body
        .statement_payloads()
        .into_iter()
        .find_map(|statement| statement.return_value_expression_payload())
        .expect("CST block return expression");
    assert_eq!(cst_block.kind(), Some(SyntaxExpressionKind::Block));

    for function in ["legacy_record", "legacy_path", "legacy_call"] {
        let (legacy_payload, _, _) = semantic.function(function).expect("legacy function");
        let legacy_return = legacy_payload
            .body
            .statement_payloads()
            .into_iter()
            .find_map(|statement| statement.return_value_expression_payload())
            .expect("legacy return expression");
        let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
            source,
            cst_block
                .syntax_expression()
                .expect("block CST expression")
                .clone(),
            legacy_return.fallback(),
        );

        let fact = script_types::expression_script_fact_with_payload(
            mismatched_payload.fallback(),
            Some(&mismatched_payload),
            |_| Some("LegacyResult".to_owned()),
            |_| None,
            |name| match name {
                "legacy" => Some(script_types::ScriptTypeFact::new("LegacyBox")),
                _ => None,
            },
        );
        assert_eq!(
            fact, None,
            "non-matching CST block payload should not use {function} fallback"
        );
    }
}

#[test]
fn self_facts_prefer_cst_payload_shape() {
    let source = SourceId::new(1);
    let text = r#"
struct CstBox {}
struct LegacyBox {}

impl CstBox {
    fn id(self) {
        return self;
    }
}

fn legacy_path(legacy) {
    return legacy;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let self_method = semantic
        .script_impl_methods()
        .into_iter()
        .find(|method| method.method_name == "id")
        .expect("self method");
    let (legacy_payload, _, _) = semantic.function("legacy_path").expect("legacy function");
    let self_return = self_method
        .body
        .statement_payloads()
        .into_iter()
        .find_map(|statement| statement.return_value_expression_payload())
        .expect("self return expression");
    let legacy_return = legacy_payload
        .body
        .statement_payloads()
        .into_iter()
        .find_map(|statement| statement.return_value_expression_payload())
        .expect("legacy path return expression");
    let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
        source,
        self_return
            .syntax_expression()
            .expect("self CST expression")
            .clone(),
        legacy_return.fallback(),
    );
    assert!(mismatched_payload.syntax_is_self());

    let fact = script_types::expression_script_fact_with_payload(
        mismatched_payload.fallback(),
        Some(&mismatched_payload),
        |_| None,
        |_| None,
        |name| match name {
            "self" => Some(script_types::ScriptTypeFact::new("CstBox")),
            "legacy" => Some(script_types::ScriptTypeFact::new("LegacyBox")),
            _ => None,
        },
    )
    .expect("CST self payload should produce a script type fact");
    assert_eq!(fact, script_types::ScriptTypeFact::new("CstBox"));

    with_cst_payload_compiler(
        r#"
fn main() {
    let legacy = 1;
    self;
}
"#,
        |compiler, payload| {
            compiler.value_types.set_name(
                "self",
                Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Bool)),
            );
            compiler.value_shapes.set_name(
                "self",
                Some(record_shapes::ValueShape::Scalar("bool".to_owned())),
            );
            let statements = payload.body.statement_payloads();
            let legacy_initializer = statements[0]
                .let_initializer_expression_payload()
                .expect("legacy literal initializer");
            let cst_self = statements[1]
                .expression_payload()
                .expect("CST self expression statement");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                source,
                cst_self
                    .syntax_expression()
                    .expect("self CST expression")
                    .clone(),
                legacy_initializer.fallback(),
            );
            assert!(mismatched_payload.syntax_is_self());
            assert_eq!(
                compiler.static_type_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                value_types::StaticExprType::Exact(RuntimeTypeFact::primitive(
                    vela_common::PrimitiveTag::Bool,
                ))
            );
            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                Some(record_shapes::ValueShape::Scalar("bool".to_owned()))
            );
        },
    );
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
        payload.syntax_path_segments()
    );
    payload.syntax_path_segments()
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
