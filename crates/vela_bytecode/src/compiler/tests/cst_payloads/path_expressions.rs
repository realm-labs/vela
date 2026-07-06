use super::*;

fn path_statement_payloads<'ast>(
    body: &body_payloads::CompilerBodyPayload<'ast>,
) -> Vec<body_payloads::CompilerStatementPayload<'ast>> {
    paired_statement_payloads_for_body(body.syntax_payload().source, body)
}

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
fn take(value) {
    return value;
}

fn main(value) {
    take(value);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let call = path_statement_payloads(&payload.body)[0]
        .expression_payload()
        .expect("call expression payload");
    let _legacy_path = call
        .call_argument_value_payloads()
        .expect("call argument payloads")
        .remove(0);
    let legacy_expr = call_argument_fallback(&call, 0);
    let missing_path = body_payloads::CompilerExpressionPayload::missing_syntax(source);

    let error = compiler
        .compile_expr_with_payload(legacy_expr, Some(&missing_path))
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
fn main(cst_value) {
    let legacy_value = 1;
    let selected = cst_value;
}
"#;
    let cst_semantic = parse_semantic_source(source, cst_text).expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("CST function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
fn main() {
    let legacy_value = 1;
    let selected = legacy_value && make(legacy_value);
}

fn make(value) {
    return value;
}
"#,
        |compiler, legacy_payload| {
            let legacy_statements = path_statement_payloads(&legacy_payload.body);
            let legacy_initializer = legacy_statements[1]
                .let_initializer_fallback_for_test()
                .expect("legacy initializer fallback");
            let cst_statement = cst_body
                .statements()
                .nth(1)
                .expect("CST selected statement");
            let mismatched_statement =
                body_payloads::CompilerStatementPayload::missing_let_child_payload_context(
                    cst_statement,
                    legacy_initializer,
                );
            let mismatched_payload = mismatched_statement
                .let_initializer_expression_payload()
                .expect("CST path initializer");
            assert_eq!(mismatched_payload.syntax_path_segments(), None);

            let fact = script_types::expression_script_fact_with_payload(
                mismatched_payload.fallback(),
                Some(&mismatched_payload),
                |_| None,
                |_| None,
                |name| match name {
                    "legacy_value" => Some(script_types::ScriptTypeFact::new("LegacyBox")),
                    _ => None,
                },
            );
            assert_eq!(
                fact, None,
                "path facts must not use legacy fallback segments when CST segments differ"
            );
            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                None,
                "value shapes must not use legacy fallback segments when CST segments differ"
            );
        },
    );
}

#[test]
fn source_less_path_payload_does_not_expose_cst_segments() {
    with_cst_payload_compiler(
        r#"
fn take(value) {
    return value;
}

fn main(value) {
    take(value);
}
"#,
        |_, payload| {
            let call = path_statement_payloads(&payload.body)[0]
                .expression_payload()
                .expect("call expression payload");
            let path = call
                .call_argument_value_payloads()
                .expect("call argument payloads")
                .remove(0);
            let missing_source =
                body_payloads::CompilerExpressionPayload::missing_child_payload_context(
                    path.syntax_expression().expect("path syntax").clone(),
                );

            assert_eq!(missing_source.syntax_path_segments(), None);
        },
    );
}

#[test]
fn normal_path_payload_does_not_compile_legacy_self() {
    with_cst_payload_compiler(
        r#"
fn take(value) {
    return value;
}

fn main(value) {
    take(value);
    take(self);
}
"#,
        |compiler, payload| {
            let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
            let path_call = statements[0]
                .expression_payload()
                .expect("path call payload");
            let path = path_call
                .call_argument_value_payloads()
                .expect("path call argument payloads")
                .remove(0);
            let self_call = statements[1]
                .expression_payload()
                .expect("self call payload");
            let _self_value = self_call
                .call_argument_value_payloads()
                .expect("self call argument payloads")
                .remove(0);
            let self_fallback = call_argument_fallback(&self_call, 0);
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                path.syntax_expression()
                    .expect("path CST expression")
                    .clone(),
                self_fallback,
            );
            assert_eq!(
                mismatched_payload.syntax_path_segments(),
                Some(vec!["value".to_owned()])
            );
            assert!(!mismatched_payload.syntax_is_self());

            let error = compiler
                .compile_expr_with_payload(self_fallback, Some(&mismatched_payload))
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
    return legacy && make(legacy);
}

fn make(value) {
    return value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_record").expect("cst function");
    let cst_return = path_statement_payloads(&cst_payload.body)
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
    let legacy_return = path_statement_payloads(&legacy_payload.body)
        .into_iter()
        .find_map(|statement| statement.return_value_expression_payload())
        .expect("legacy path return expression");
    let mismatched_payload = expression_payload_with_fallback(
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
fn script_type_facts_require_cst_payload_not_owned_fallback() {
    let source = SourceId::new(1);
    let text = r#"
struct LegacyBox {}

enum LegacyResult {
    Ok(value),
}

impl LegacyBox {
    fn id(self, consumer) {
        consumer(self);
    }
}

fn legacy_record() {
    return LegacyBox {};
}

fn legacy_call(legacy) {
    return LegacyResult::Ok(legacy);
}

fn legacy_path(consumer, legacy) {
    consumer(legacy);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");

    let (record_payload, _, _) = semantic.function("legacy_record").expect("record function");
    let record_return = path_statement_payloads(&record_payload.body)
        .into_iter()
        .find_map(|statement| statement.return_value_expression_payload())
        .expect("record return expression");
    let fact = script_types::expression_script_fact_with_payload(
        record_return.fallback(),
        None,
        |_| Some("LegacyBox".to_owned()),
        |_| None,
        |_| None,
    );
    assert_eq!(
        fact, None,
        "owned record fallback must not provide script type facts"
    );

    let (call_payload, _, _) = semantic.function("legacy_call").expect("call function");
    let call_return = path_statement_payloads(&call_payload.body)
        .into_iter()
        .find_map(|statement| statement.return_value_expression_payload())
        .expect("enum call return expression");
    let fact = script_types::expression_script_fact_with_payload(
        call_return.fallback(),
        None,
        |_| Some("LegacyResult".to_owned()),
        |_| None,
        |_| None,
    );
    assert_eq!(
        fact, None,
        "owned enum-call fallback must not provide script type facts"
    );

    let (path_payload, _, _) = semantic.function("legacy_path").expect("path function");
    let path_call = path_statement_payloads(&path_payload.body)[0]
        .expression_payload()
        .expect("path call expression");
    let path_arg = call_argument_fallback(&path_call, 0);
    let fact = script_types::expression_script_fact_with_payload(
        path_arg,
        None,
        |_| None,
        |_| None,
        |name| match name {
            "legacy" => Some(script_types::ScriptTypeFact::new("LegacyBox")),
            _ => None,
        },
    );
    assert_eq!(
        fact, None,
        "owned path fallback must not provide local script type facts"
    );

    let self_method = semantic
        .script_impl_methods()
        .into_iter()
        .find(|method| method.method_name == "id")
        .expect("self method");
    let self_call = path_statement_payloads(&self_method.body)[0]
        .expression_payload()
        .expect("self call expression");
    let self_arg = call_argument_fallback(&self_call, 0);
    let fact = script_types::expression_script_fact_with_payload(
        self_arg,
        None,
        |_| None,
        |_| None,
        |name| match name {
            "self" => Some(script_types::ScriptTypeFact::new("LegacyBox")),
            _ => None,
        },
    );
    assert_eq!(
        fact, None,
        "owned self fallback must not provide script type facts"
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
        selected;
        selected && true
    };
}

fn legacy_record() {
    return LegacyBox {};
}

fn legacy_path(legacy) {
    return legacy && make(legacy);
}

fn legacy_call(legacy) {
    return LegacyResult::Ok(legacy);
}

fn make(value) {
    return value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_block").expect("cst function");
    let cst_statements = paired_statement_payloads_for_body(source, &cst_payload.body);
    let cst_block = cst_statements
        .into_iter()
        .find_map(|statement| statement.return_value_expression_payload())
        .expect("CST block return expression");
    assert_eq!(cst_block.syntax_kind(), Some(SyntaxExpressionKind::Block));

    for function in ["legacy_record", "legacy_path", "legacy_call"] {
        let (legacy_payload, _, _) = semantic.function(function).expect("legacy function");
        let legacy_statements = paired_statement_payloads_for_body(source, &legacy_payload.body);
        let legacy_return = legacy_statements
            .into_iter()
            .find_map(|statement| statement.return_value_expression_payload())
            .expect("legacy return expression");
        let mismatched_payload = expression_payload_with_fallback(
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
        selected;
        selected && true
    };
}
"#,
        |_, payload| {
            let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
            let block = statements[0]
                .let_initializer_expression_payload()
                .expect("block initializer");
            assert_eq!(block.syntax_kind(), Some(SyntaxExpressionKind::Block));
            let block_body = block.block_body_payload().expect("block body");
            let block_statements = path_statement_payloads(&block_body);
            let (_, child_path) = block_statements[1]
                .expression_statement_syntax_expression()
                .expect("block child path syntax");
            assert_eq!(child_path.expression_kind(), SyntaxExpressionKind::Path);

            let mismatched_payload =
                expression_payload_with_fallback(SourceId::new(1), child_path, block.fallback());

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
fn cst_path(consumer, cst) {
    consumer(cst);
}

fn legacy_path(consumer, legacy) {
    consumer(legacy);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_path").expect("cst function");
    let (legacy_payload, _, _) = semantic.function("legacy_path").expect("legacy function");
    let cst_call = path_statement_payloads(&cst_payload.body)[0]
        .expression_payload()
        .expect("CST call expression");
    let cst_return = cst_call
        .call_argument_value_payloads()
        .expect("CST call argument payloads")
        .remove(0);
    let legacy_call = path_statement_payloads(&legacy_payload.body)[0]
        .expression_payload()
        .expect("legacy call expression");
    let legacy_expr = call_argument_fallback(&legacy_call, 0);
    let mismatched_payload = expression_payload_with_fallback(
        source,
        cst_return
            .syntax_expression()
            .expect("path CST expression")
            .clone(),
        legacy_expr,
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
    take(cst_value);
    take(legacy_value);
    let cst_block = {
        let selected = cst_value;
        selected;
        selected && true
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
            let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
            let cst_path = statements[2]
                .expression_payload()
                .and_then(|payload| payload.call_argument_value_payloads())
                .expect("CST path call argument payloads")
                .remove(0);
            let legacy_call = statements[3]
                .expression_payload()
                .expect("legacy path call payload");
            let _legacy_path = legacy_call
                .call_argument_value_payloads()
                .expect("legacy path call argument payloads")
                .remove(0);
            let legacy_expr = call_argument_fallback(&legacy_call, 0);
            assert_eq!(
                compiler.static_type_for_expr_with_payload(legacy_expr, None),
                value_types::StaticExprType::Dynamic,
                "missing CST path payload must not use the legacy path value type"
            );
            let cst_block = statements[4]
                .let_initializer_expression_payload()
                .expect("CST block initializer");

            let mismatched_path = expression_payload_with_fallback(
                SourceId::new(1),
                cst_path
                    .syntax_expression()
                    .expect("path CST expression")
                    .clone(),
                legacy_expr,
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

            let mismatched_block = expression_payload_with_fallback(
                SourceId::new(1),
                cst_block
                    .syntax_expression()
                    .expect("block CST expression")
                    .clone(),
                legacy_expr,
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
        selected;
        selected && true
    };
}
"#,
        |compiler, payload| {
            compiler.value_types.set_name(
                "selected",
                Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Bool)),
            );
            let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
            let block = statements[0]
                .let_initializer_expression_payload()
                .expect("block initializer");
            assert_eq!(block.syntax_kind(), Some(SyntaxExpressionKind::Block));
            let block_body = block.block_body_payload().expect("block body");
            let block_statements = path_statement_payloads(&block_body);
            let (_, child_path) = block_statements[1]
                .expression_statement_syntax_expression()
                .expect("block child path syntax");
            assert_eq!(child_path.expression_kind(), SyntaxExpressionKind::Path);

            let mismatched_payload =
                expression_payload_with_fallback(SourceId::new(1), child_path, block.fallback());

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
    fn id(self, consumer) {
        consumer(self);
    }
}

fn legacy_path(consumer, legacy) {
    consumer(legacy);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let self_method = semantic
        .script_impl_methods()
        .into_iter()
        .find(|method| method.method_name == "id")
        .expect("self method");
    let self_call = path_statement_payloads(&self_method.body)[0]
        .expression_payload()
        .expect("self call expression");
    let self_return = self_call
        .call_argument_value_payloads()
        .expect("self call argument payloads")
        .remove(0);
    let self_expr = call_argument_fallback(&self_call, 0);
    let fact = script_types::expression_script_fact_with_payload(
        self_expr,
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
    let legacy_call = path_statement_payloads(&legacy_payload.body)[0]
        .expression_payload()
        .expect("legacy call expression");
    let legacy_expr = call_argument_fallback(&legacy_call, 0);
    let mismatched_payload = expression_payload_with_fallback(
        source,
        self_return
            .syntax_expression()
            .expect("self CST expression")
            .clone(),
        legacy_expr,
    );
    assert!(mismatched_payload.syntax_is_self());

    let missing_source_self_payload =
        body_payloads::CompilerExpressionPayload::missing_child_payload_context(
            self_return
                .syntax_expression()
                .expect("self CST expression")
                .clone(),
        );
    assert!(!missing_source_self_payload.syntax_is_self());
    let fact = script_types::expression_script_fact_with_payload(
        legacy_expr,
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
fn make(value) {
    return value;
}

fn main(input) {
    let legacy = input && make(input);
    consumer(self);
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
            let statements = path_statement_payloads(&payload.body);
            let legacy_initializer = statements[0]
                .let_initializer_expression_payload()
                .expect("legacy path initializer");
            let self_call = statements[1]
                .expression_payload()
                .expect("self call payload");
            let cst_self = self_call
                .call_argument_value_payloads()
                .expect("self call argument payloads")
                .remove(0);
            let self_expr = call_argument_fallback(&self_call, 0);
            assert_eq!(
                compiler.static_type_for_expr_with_payload(self_expr, Some(&cst_self)),
                value_types::StaticExprType::Exact(RuntimeTypeFact::primitive(
                    vela_common::PrimitiveTag::Bool
                )),
                "aligned CST self payload should infer the self value type"
            );
            assert_eq!(
                compiler.value_shape_for_expr_with_payload(self_expr, Some(&cst_self)),
                Some(record_shapes::ValueShape::Scalar("bool".to_owned())),
                "aligned CST self payload should infer the self value shape"
            );
            let mismatched_payload = expression_payload_with_fallback(
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
    let actual = path_statement_payloads(body)
        .iter()
        .filter_map(|statement| {
            statement
                .let_initializer_syntax_path_and_span()
                .map(|(path, _)| path)
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_segments(expected));
}

fn assert_cst_call_argument_path_segments(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&[&str]],
) {
    let actual = path_statement_payloads(body)
        .iter()
        .flat_map(|statement| {
            statement
                .expression_payload()
                .and_then(|payload| payload.call_argument_value_payloads())
                .unwrap_or_default()
        })
        .filter_map(path_payload_segments)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_segments(expected));
}

fn assert_cst_return_value_path_segments(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&[&str]],
) {
    let actual = path_statement_payloads(body)
        .iter()
        .filter_map(|statement| {
            statement
                .return_value_syntax_path_and_span()
                .map(|(path, _)| path)
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_segments(expected));
}

fn path_payload_segments(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Option<Vec<String>> {
    assert_eq!(payload.syntax_kind(), Some(SyntaxExpressionKind::Path));
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
