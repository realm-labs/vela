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
    let cst_return = cst_payload
        .body
        .statement_payloads()
        .into_iter()
        .find_map(|statement| statement.return_value_expression_payload())
        .expect("CST record return expression");
    let fact = script_types::expression_script_fact_with_payload(
        cst_return.fallback(),
        Some(&cst_return),
        |_| None,
        |_| None,
        |_| None,
    )
    .expect("aligned CST record payload should produce a script type fact");
    assert_eq!(fact, script_types::ScriptTypeFact::new("CstBox"));

    let (legacy_payload, _, _) = semantic.function("legacy_path").expect("legacy function");
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
    );
    assert_eq!(
        fact, None,
        "non-overlapping CST record payload must not produce a script type fact"
    );
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
fn script_type_facts_with_overlapping_child_cst_payload_do_not_use_child_shape() {
    with_cst_payload_compiler(
        r#"
fn main(cst) {
    let value = {
        let selected = cst;
        selected
    };
}
"#,
        |_, payload| {
            let statements = payload.body.statement_payloads();
            let block = statements[0]
                .let_initializer_expression_payload()
                .expect("block initializer");
            assert_eq!(block.kind(), Some(SyntaxExpressionKind::Block));
            let block_body = block.block_body_payload().expect("block body");
            let block_statements = block_body.statement_payloads();
            let child_path = block_statements[1]
                .expression_payload()
                .expect("block tail path");
            assert_eq!(child_path.kind(), Some(SyntaxExpressionKind::Path));

            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                child_path
                    .syntax_expression()
                    .expect("child path CST expression")
                    .clone(),
                block.fallback(),
            );

            let fact = script_types::expression_script_fact_with_payload(
                mismatched_payload.fallback(),
                Some(&mismatched_payload),
                |_| None,
                |_| None,
                |name| match name {
                    "selected" => Some(script_types::ScriptTypeFact::new("ChildBox")),
                    _ => None,
                },
            );

            assert_eq!(
                fact, None,
                "overlapping child CST path payload must not type the enclosing block fallback"
            );
        },
    );
}

#[test]
fn script_type_facts_with_cst_path_payload_do_not_use_legacy_path_fact() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_path(cst) {
    return cst;
}

fn legacy_path(legacy) {
    return legacy;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_path").expect("cst function");
    let (legacy_payload, _, _) = semantic.function("legacy_path").expect("legacy function");
    let cst_return = cst_payload
        .body
        .statement_payloads()
        .into_iter()
        .find_map(|statement| statement.return_value_expression_payload())
        .expect("CST path return expression");
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
            .expect("path CST expression")
            .clone(),
        legacy_return.fallback(),
    );

    let fact = script_types::expression_script_fact_with_payload(
        mismatched_payload.fallback(),
        Some(&mismatched_payload),
        |_| None,
        |_| None,
        |name| match name {
            "legacy" => Some(script_types::ScriptTypeFact::new("LegacyBox")),
            _ => None,
        },
    );

    assert_eq!(
        fact, None,
        "CST path payload without a fact must not use the legacy fallback path"
    );
}

#[test]
fn static_value_type_facts_prefer_cst_path_payloads_and_reject_mismatch() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_value = true;
    let legacy_value = 1;
    cst_value;
    legacy_value;
    let cst_block = {
        let selected = cst_value;
        selected
    };
}
"#,
        |compiler, payload| {
            compiler.value_types.set_name(
                "cst_value",
                Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Bool)),
            );
            compiler.value_types.set_name(
                "legacy_value",
                Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::I64)),
            );
            compiler.value_shapes.set_name(
                "legacy_value",
                Some(record_shapes::ValueShape::Scalar("i64".to_owned())),
            );
            let statements = payload.body.statement_payloads();
            let cst_path = statements[2]
                .expression_payload()
                .expect("CST path expression");
            let legacy_path = statements[3]
                .expression_payload()
                .expect("legacy path fallback");
            let cst_block = statements[4]
                .let_initializer_expression_payload()
                .expect("CST block initializer");

            let mismatched_path = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_path
                    .syntax_expression()
                    .expect("path CST expression")
                    .clone(),
                legacy_path.fallback(),
            );
            assert_eq!(
                compiler.static_type_for_expr_with_payload(
                    mismatched_path.fallback(),
                    Some(&mismatched_path),
                ),
                value_types::StaticExprType::Dynamic
            );

            compiler
                .value_types
                .set_name("cst_value", None::<RuntimeTypeFact>);
            assert_eq!(
                compiler.static_type_for_expr_with_payload(
                    mismatched_path.fallback(),
                    Some(&mismatched_path),
                ),
                value_types::StaticExprType::Dynamic,
                "CST path payload without a fact must not use the legacy fallback path"
            );

            let mismatched_block = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_block
                    .syntax_expression()
                    .expect("block CST expression")
                    .clone(),
                legacy_path.fallback(),
            );
            assert_eq!(
                compiler.static_type_for_expr_with_payload(
                    mismatched_block.fallback(),
                    Some(&mismatched_block),
                ),
                value_types::StaticExprType::Dynamic
            );
            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_block.fallback(),
                    Some(&mismatched_block),
                ),
                None,
                "non-path CST payload must not use the legacy fallback path shape"
            );
        },
    );
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
    let self_return = self_method
        .body
        .statement_payloads()
        .into_iter()
        .find_map(|statement| statement.return_value_expression_payload())
        .expect("self return expression");
    let fact = script_types::expression_script_fact_with_payload(
        self_return.fallback(),
        Some(&self_return),
        |_| None,
        |_| None,
        |name| match name {
            "self" => Some(script_types::ScriptTypeFact::new("CstBox")),
            "legacy" => Some(script_types::ScriptTypeFact::new("LegacyBox")),
            _ => None,
        },
    )
    .expect("aligned CST self payload should produce a script type fact");
    assert_eq!(fact, script_types::ScriptTypeFact::new("CstBox"));

    let (legacy_payload, _, _) = semantic.function("legacy_path").expect("legacy function");
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
    );
    assert_eq!(
        fact, None,
        "non-overlapping CST self payload must not produce a script type fact"
    );

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
                value_types::StaticExprType::Dynamic
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
