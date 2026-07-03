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
fn missing_path_expression_payload_does_not_use_legacy_path() {
    let source = SourceId::new(1);
    let text = r#"
fn main(value) {
    let output = value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_path = payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("legacy path payload");
    let missing_path =
        body_payloads::CompilerExpressionPayload::missing_syntax(source, legacy_path.fallback());

    let error = compiler
        .compile_expr_with_payload(legacy_path.fallback(), Some(&missing_path))
        .expect_err("missing CST path payload must not compile legacy path");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST expression payload")
    ));
}

#[test]
fn path_expression_without_cst_segments_does_not_use_legacy_path() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let legacy_value = 1;
    let selected = self;
}
"#;
    let cst_semantic = parse_semantic_source(source, cst_text).expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("CST function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
fn main() {
    let legacy_value = 1;
    let selected = legacy_value;
}
"#,
        |compiler, payload| {
            let mismatched_body = body_payloads::CompilerBodyPayload::syntax(
                source,
                cst_body,
                payload.body.fallback(),
            );
            let statements = mismatched_body.statement_payloads();
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("legacy local should compile");
            let legacy_path = statements[1]
                .let_initializer_expression_payload()
                .expect("path initializer payload");

            let error = compiler
                .compile_expr_with_payload(legacy_path.fallback(), Some(&legacy_path))
                .expect_err("CST path without segments must not compile legacy path");

            assert_eq!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST expression payload"),
                "{:?}",
                error.kind
            );
        },
    );
}

#[test]
fn source_less_path_payload_does_not_expose_cst_segments() {
    with_cst_payload_compiler(
        r#"
fn main(value) {
    let selected = value;
}
"#,
        |_, payload| {
            let path = payload.body.statement_payloads()[0]
                .let_initializer_expression_payload()
                .expect("path initializer payload");
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    path.syntax_expression().expect("path syntax").clone(),
                    path.fallback(),
                );

            assert_eq!(missing_source.syntax_path_segments(), None);
        },
    );
}

#[test]
fn normal_path_payload_does_not_compile_legacy_self() {
    with_cst_payload_compiler(
        r#"
fn main(value) {
    value;
    self;
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let path = statements[0]
                .expression_payload()
                .expect("path expression payload");
            let self_value = statements[1]
                .expression_payload()
                .expect("self expression payload");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                path.syntax_expression()
                    .expect("path CST expression")
                    .clone(),
                self_value.fallback(),
            );
            assert_eq!(
                mismatched_payload.syntax_path_segments(),
                Some(vec!["value".to_owned()])
            );
            assert!(!mismatched_payload.syntax_is_self());

            let error = compiler
                .compile_expr_with_payload(self_value.fallback(), Some(&mismatched_payload))
                .expect_err("normal path payload must not compile legacy self fallback");

            assert_eq!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST expression payload")
            );
        },
    );
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
fn static_value_type_facts_with_overlapping_child_cst_payload_do_not_use_child_fact() {
    with_cst_payload_compiler(
        r#"
fn main(cst) {
    let value = {
        let selected = cst;
        selected
    };
}
"#,
        |compiler, payload| {
            compiler.value_types.set_name(
                "selected",
                Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Bool)),
            );
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

            assert_eq!(
                compiler.static_type_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                value_types::StaticExprType::Dynamic,
                "overlapping child CST path payload must not type the enclosing block fallback"
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

    let missing_source_self_payload =
        body_payloads::CompilerExpressionPayload::missing_child_payload_context(
            self_return
                .syntax_expression()
                .expect("self CST expression")
                .clone(),
            legacy_return.fallback(),
        );
    assert!(!missing_source_self_payload.syntax_is_self());
    let fact = script_types::expression_script_fact_with_payload(
        missing_source_self_payload.fallback(),
        Some(&missing_source_self_payload),
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
        "source-less CST self payload must not produce a script type fact"
    );

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
fn main(input) {
    let legacy = input;
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
            assert_eq!(
                compiler.static_type_for_expr_with_payload(cst_self.fallback(), Some(&cst_self)),
                value_types::StaticExprType::Exact(RuntimeTypeFact::primitive(
                    vela_common::PrimitiveTag::Bool
                )),
                "aligned CST self payload should infer the self value type"
            );
            assert_eq!(
                compiler.value_shape_for_expr_with_payload(cst_self.fallback(), Some(&cst_self)),
                Some(record_shapes::ValueShape::Scalar("bool".to_owned())),
                "aligned CST self payload should infer the self value shape"
            );
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
                None,
                "mismatched CST self payload must not infer the self value shape"
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
