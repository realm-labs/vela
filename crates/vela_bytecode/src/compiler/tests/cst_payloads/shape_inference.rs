use super::*;

fn shape_statement_payloads<'ast>(
    body: &body_payloads::CompilerBodyPayload<'ast>,
) -> Vec<body_payloads::CompilerStatementPayload<'ast>> {
    paired_statement_payloads_for_body(body.syntax_payload().source, body)
}

#[test]
fn path_shape_inference_uses_typed_cst_parameter_value_fact() {
    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(text: String) {
    return text;
}
"#,
    )
    .expect("source should parse");
    let (compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = shape_statement_payloads(&payload.body);
    let (source, expression, _) = statements[0]
        .return_value_syntax_expression_and_span()
        .expect("CST return expression");

    assert_eq!(
        compiler.value_shape_for_syntax_expression(Some(source), &expression),
        Some(record_shapes::ValueShape::Scalar("String".to_owned())),
        "typed CST parameter path should derive shape from HIR value facts"
    );
}

#[test]
fn field_shape_inference_with_non_field_cst_payload_does_not_use_legacy_field() {
    with_cst_payload_compiler(
        r#"
struct LegacyBox {
    amount: i64,
}

fn main() {
    let legacy = LegacyBox { amount: 1 };
    let cst_block = {
        let selected = legacy;
        selected;
        selected && true
    };
    let legacy_amount = make(legacy).amount;
}

fn make(value) {
    return value;
}
"#,
        |compiler, payload| {
            let statements = paired_statement_payloads_for_body(SourceId::new(1), &payload.body);
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("legacy local should compile");
            let cst_block = statements[1]
                .let_initializer_expression_payload()
                .expect("CST block initializer");
            let legacy_field = statements[2]
                .let_initializer_expression_payload()
                .expect("legacy field fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_block
                    .syntax_expression()
                    .expect("CST block syntax")
                    .clone(),
                legacy_field.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                None,
                "non-field CST payload must not use the legacy field shape"
            );
            assert_eq!(
                compiler.record_field_value_type_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                None,
                "non-field CST payload must not use the legacy field value type"
            );
        },
    );
}

#[test]
fn shape_inference_with_unsupported_cst_payload_does_not_use_legacy_shape() {
    with_cst_payload_compiler(
        r#"
struct LegacyBox {
    amount: i64,
}

fn main(input) {
    let cst_binary = input && make(input);
    let legacy = LegacyBox { amount: 1 };
}

fn make(value) {
    return value;
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let cst_binary = statements[0]
                .let_initializer_expression_payload()
                .expect("CST binary initializer");
            let legacy_record = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy record initializer");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_binary
                    .syntax_expression()
                    .expect("CST binary syntax")
                    .clone(),
                legacy_record.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                None,
                "unsupported CST payload must not use the legacy record shape"
            );
        },
    );
}

#[test]
fn missing_shape_expression_payload_does_not_use_legacy_shape() {
    with_cst_payload_compiler(
        r#"
struct LegacyBox {
    amount: i64,
}

fn main() {
    let legacy = LegacyBox { amount: 1 };
}
"#,
        |compiler, payload| {
            let record = shape_statement_payloads(&payload.body)[0]
                .let_initializer_expression_payload()
                .expect("record initializer payload");
            let missing_payload =
                body_payloads::CompilerExpressionPayload::missing_syntax(SourceId::new(1));

            assert_eq!(
                compiler
                    .value_shape_for_expr_with_payload(record.fallback(), Some(&missing_payload),),
                None,
                "missing source-backed CST payload must not use the legacy record shape"
            );
        },
    );
}

#[test]
fn shape_inference_with_unshaped_cst_record_does_not_use_legacy_fields() {
    let source = SourceId::new(1);
    let cst_text = r#"
struct LegacyBox {
    amount: i64,
}

fn main() {
    let selected = LegacyBox {};
}
"#;
    let cst_semantic = parse_semantic_source(source, cst_text).expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("CST function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
struct LegacyBox {
    amount: i64,
}

fn main() {
    let selected = LegacyBox { amount: 1 };
}
"#,
        |compiler, payload| {
            let statements = body_payloads::CompilerBodyPayload::paired_statement_payloads_for_test(
                source,
                cst_body,
                fallback_statements_for_body(source, &payload.body),
            );
            let record = statements[0]
                .let_initializer_expression_payload()
                .expect("record initializer payload");

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(record.fallback(), Some(&record)),
                None,
                "same-kind CST record payload with no shape must not use legacy fields"
            );
        },
    );
}

#[test]
fn binary_shape_inference_prefers_cst_operator_shape() {
    with_cst_payload_compiler(
        r#"
fn main(input) {
    let cst_range = 1..3;
    let cst_compare = input < (3 + 1);
    let legacy_bool = input == false;
    let legacy_arithmetic = input && make(input);
}

fn make(value) {
    return value;
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let cst_range = statements[0]
                .let_initializer_syntax_expression_and_span()
                .expect("CST range initializer")
                .1;
            let cst_compare = statements[1]
                .let_initializer_expression_payload()
                .expect("CST comparison initializer");
            let legacy_bool = statements[2]
                .let_initializer_expression_payload()
                .expect("legacy boolean fallback");
            let legacy_arithmetic = statements[3]
                .let_initializer_expression_payload()
                .expect("legacy arithmetic fallback");

            let range_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_range,
                legacy_bool.fallback(),
            );
            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    range_payload.fallback(),
                    Some(&range_payload)
                ),
                Some(record_shapes::ValueShape::Scalar("Range".to_owned())),
                "range CST payload must not use the old fallback boolean shape"
            );

            let compare_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_compare
                    .syntax_expression()
                    .expect("CST comparison syntax")
                    .clone(),
                legacy_arithmetic.fallback(),
            );
            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    compare_payload.fallback(),
                    Some(&compare_payload),
                ),
                Some(record_shapes::ValueShape::Scalar("bool".to_owned())),
                "comparison CST payload must not use the old fallback arithmetic shape"
            );
        },
    );
}

#[test]
fn arithmetic_shape_inference_prefers_cst_literal_operands() {
    with_cst_payload_compiler(
        r#"
fn main(input) {
    let cst_float = 1.0 + 2;
    let legacy_bool = input == false;
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let cst_float = statements[0]
                .let_initializer_expression_payload()
                .expect("CST float arithmetic initializer");
            let legacy_bool = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy boolean fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_float
                    .syntax_expression()
                    .expect("CST float arithmetic syntax")
                    .clone(),
                legacy_bool.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                Some(record_shapes::ValueShape::Scalar("f64".to_owned())),
                "arithmetic shape must use CST float operands instead of legacy boolean operands"
            );
        },
    );
}

#[test]
fn fallback_dynamic_call_shape_does_not_invent_numeric_literal_shape() {
    with_cst_payload_compiler(
        r#"
fn make(value) {
    return value;
}

fn main(input) {
    let dynamic = make(input);
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let dynamic = statements[0]
                .let_initializer_expression_payload()
                .expect("dynamic call initializer");

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(dynamic.fallback(), None),
                None,
                "fallback dynamic call shape must not invent a numeric type for dynamic operands"
            );
        },
    );
}

#[test]
fn unsupported_binary_shape_payload_does_not_use_legacy_binary_shape() {
    with_cst_payload_compiler(
        r#"
struct LegacyBox {
    amount: i64,
}

fn main(input) {
    let cst_logical = input && make(input);
    let legacy_record = LegacyBox { amount: 1 };
}

fn make(value) {
    return value;
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let cst_logical = statements[0]
                .let_initializer_expression_payload()
                .expect("CST logical initializer");
            let legacy_record = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy record initializer");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_logical
                    .syntax_expression()
                    .expect("CST logical syntax")
                    .clone(),
                legacy_record.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                None,
                "unsupported CST binary shape must not use the legacy record shape"
            );
        },
    );
}

#[test]
fn paren_shape_inference_prefers_inner_cst_payload_shape() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_paren = (["cst"]);
    let legacy_array = [true];
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let cst_paren = statements[0]
                .let_initializer_expression_payload()
                .expect("CST parenthesized initializer");
            let legacy_array = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy array fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_paren
                    .syntax_expression()
                    .expect("CST parenthesized syntax")
                    .clone(),
                legacy_array.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                Some(record_shapes::ValueShape::Array(Box::new(
                    record_shapes::ValueShape::Scalar("String".to_owned())
                ))),
                "parenthesized CST payload must not use the old fallback array shape"
            );
        },
    );
}

#[test]
fn native_call_shape_inference_prefers_cst_payload_shape() {
    with_cst_payload_compiler(
        r#"
struct LegacyBox {
    amount: i64,
}

fn main() {
    let cst_call = option::some(["cst"]);
    let legacy_call = result::ok(LegacyBox { amount: 1 });
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let cst_call = statements[0]
                .let_initializer_expression_payload()
                .expect("CST call initializer");
            let legacy_call = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy call fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_call
                    .syntax_expression()
                    .expect("CST call syntax")
                    .clone(),
                legacy_call.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                Some(record_shapes::ValueShape::Option(Box::new(
                    record_shapes::ValueShape::Array(Box::new(record_shapes::ValueShape::Scalar(
                        "String".to_owned()
                    )))
                ))),
                "call-shaped CST payload must not use the old fallback call shape"
            );
        },
    );
}

#[test]
fn unsupported_call_shape_payload_does_not_use_legacy_call_shape() {
    with_cst_payload_compiler(
        r#"
struct LegacyBox {
    amount: i64,
}

fn main() {
    let unsupported = unknown_shape();
    let legacy_call = result::ok(LegacyBox { amount: 1 });
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let unsupported = statements[0]
                .let_initializer_expression_payload()
                .expect("unsupported CST call initializer");
            let legacy_call = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy call fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                unsupported
                    .syntax_expression()
                    .expect("unsupported CST call syntax")
                    .clone(),
                legacy_call.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                None,
                "unsupported CST call payload must not use the old fallback call shape"
            );
        },
    );
}

#[test]
fn method_call_shape_inference_prefers_cst_payload_shape() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_method = ["cst"].len();
    let legacy_call = option::some(true);
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let cst_method = statements[0]
                .let_initializer_expression_payload()
                .expect("CST method-call initializer");
            let legacy_call = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy call fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_method
                    .syntax_expression()
                    .expect("CST method-call syntax")
                    .clone(),
                legacy_call.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                Some(record_shapes::ValueShape::Scalar("i64".to_owned())),
                "method-call CST payload must not use the old fallback call shape"
            );
        },
    );
}

#[test]
fn group_by_shape_inference_preserves_array_values_with_unshaped_key() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let groups = [21, 10].group_by(|value| if value % 2 == 0 {
        "even"
    } else {
        "odd"
    });
    let odd = groups["odd"];
    let odd_count = odd.count(|value| value > 12);
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("CST group_by local should compile");
            let odd = statements[1]
                .let_initializer_expression_payload()
                .expect("CST map index initializer");

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(odd.fallback(), Some(&odd)),
                Some(record_shapes::ValueShape::Array(Box::new(
                    record_shapes::ValueShape::Scalar("i64".to_owned())
                ))),
                "group_by CST payload should preserve array values even when key shape is unknown"
            );

            compiler
                .compile_statement_payload_for_test(&statements[1])
                .expect("CST indexed group local should compile");
            let odd_count = statements[2]
                .let_initializer_expression_payload()
                .expect("CST count initializer");
            assert_eq!(
                compiler.value_shape_for_expr_with_payload(odd_count.fallback(), Some(&odd_count)),
                Some(record_shapes::ValueShape::Scalar("i64".to_owned())),
                "indexed group array should keep method-call shape for count"
            );
        },
    );
}

#[test]
fn map_shape_inference_preserves_array_receiver_with_unknown_callback_result() {
    with_cst_payload_compiler(
        r#"
fn main(tick) {
    let mapped = [7, 3].map(|value| value * 2 + tick);
    let filtered = mapped.filter(|value| value != 0);
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("CST map local should compile");
            let filtered = statements[1]
                .let_initializer_expression_payload()
                .expect("CST filter initializer");

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(filtered.fallback(), Some(&filtered)),
                Some(record_shapes::ValueShape::Array(Box::new(
                    record_shapes::ValueShape::Unknown
                ))),
                "map CST payload should preserve array receiver shape when callback result is unknown"
            );
        },
    );
}

#[test]
fn method_unwrap_or_shape_inference_prefers_cst_payload_shape() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_method = option::some(["cst"]).unwrap_or([true]);
    let legacy_call = result::ok(true);
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let cst_method = statements[0]
                .let_initializer_expression_payload()
                .expect("CST unwrap_or method initializer");
            let legacy_call = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy call fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_method
                    .syntax_expression()
                    .expect("CST unwrap_or method syntax")
                    .clone(),
                legacy_call.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                Some(record_shapes::ValueShape::Array(Box::new(
                    record_shapes::ValueShape::Scalar("String".to_owned())
                ))),
                "unwrap_or CST payload must not use the old fallback call shape"
            );
        },
    );
}

#[test]
fn map_method_shape_inference_prefers_cst_payload_shape() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_map = { "key": ["cst"] };
    let cst_get = cst_map.get("key");
    let legacy_call = option::some(true);
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("CST map local should compile");
            let cst_get = statements[1]
                .let_initializer_expression_payload()
                .expect("CST map get initializer");
            let legacy_call = statements[2]
                .let_initializer_expression_payload()
                .expect("legacy call fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_get
                    .syntax_expression()
                    .expect("CST map get syntax")
                    .clone(),
                legacy_call.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                Some(record_shapes::ValueShape::Option(Box::new(
                    record_shapes::ValueShape::Array(Box::new(record_shapes::ValueShape::Scalar(
                        "String".to_owned()
                    )))
                ))),
                "map get CST payload must not use the old fallback call shape"
            );
        },
    );
}

#[test]
fn iterator_method_shape_inference_prefers_cst_payload_shape() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_collect = ["cst"].values().collect_array();
    let legacy_call = option::some(true);
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let cst_collect = statements[0]
                .let_initializer_expression_payload()
                .expect("CST collect_array initializer");
            let legacy_call = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy call fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_collect
                    .syntax_expression()
                    .expect("CST collect_array syntax")
                    .clone(),
                legacy_call.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                Some(record_shapes::ValueShape::Array(Box::new(
                    record_shapes::ValueShape::Scalar("String".to_owned())
                ))),
                "iterator collect_array CST payload must not use the old fallback call shape"
            );
        },
    );
}

#[test]
fn option_ok_or_shape_inference_prefers_cst_argument_shape() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_ok_or = option::some(["cst"]).ok_or(true);
    let legacy_call = result::ok(false);
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let cst_ok_or = statements[0]
                .let_initializer_expression_payload()
                .expect("CST ok_or initializer");
            let legacy_call = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy call fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_ok_or
                    .syntax_expression()
                    .expect("CST ok_or syntax")
                    .clone(),
                legacy_call.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                Some(record_shapes::ValueShape::Result {
                    ok: Some(Box::new(record_shapes::ValueShape::Array(Box::new(
                        record_shapes::ValueShape::Scalar("String".to_owned())
                    )))),
                    err: Some(Box::new(record_shapes::ValueShape::Scalar(
                        "bool".to_owned()
                    ))),
                }),
                "ok_or CST payload must not use the old fallback call shape"
            );
        },
    );
}

#[test]
fn callback_map_shape_inference_prefers_cst_lambda_body_shape() {
    with_cst_payload_compiler(
        r#"
struct Payload {
    label: String,
    score: i64,
}

fn main() {
    let cst_map = [Payload { label: "cst", score: 1 }].map(|item| item.label);
    let legacy_call = option::some(true);
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let cst_map = statements[0]
                .let_initializer_expression_payload()
                .expect("CST map initializer");
            let legacy_call = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy call fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_map.syntax_expression().expect("CST map syntax").clone(),
                legacy_call.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                Some(record_shapes::ValueShape::Array(Box::new(
                    record_shapes::ValueShape::Scalar("String".to_owned())
                ))),
                "callback map CST payload must not use the old fallback call shape"
            );
        },
    );
}

#[test]
fn fallback_callback_shape_does_not_infer_legacy_lambda_body_shape() {
    with_cst_payload_compiler(
        r#"
struct Payload {
    label: String,
    score: i64,
}

fn main() {
    let legacy_callback = [Payload { label: "legacy", score: 1 }].map(|item| item.label);
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let legacy_callback = statements[0]
                .let_initializer_expression_payload()
                .expect("legacy callback initializer");

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(legacy_callback.fallback(), None),
                Some(record_shapes::ValueShape::Array(Box::new(
                    record_shapes::ValueShape::Unknown
                ))),
                "owned fallback shape inference must not inspect legacy lambda bodies"
            );
        },
    );
}

#[test]
fn callback_map_values_shape_inference_prefers_cst_lambda_body_shape() {
    with_cst_payload_compiler(
        r#"
struct Payload {
    label: String,
    score: i64,
}

fn main() {
    let cst_map = { "one": Payload { label: "cst", score: 1 } };
    let cst_values = cst_map.map_values(|value| value.score);
    let legacy_call = option::some(true);
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("CST map local should compile");
            let cst_values = statements[1]
                .let_initializer_expression_payload()
                .expect("CST map_values initializer");
            let legacy_call = statements[2]
                .let_initializer_expression_payload()
                .expect("legacy call fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_values
                    .syntax_expression()
                    .expect("CST map_values syntax")
                    .clone(),
                legacy_call.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                Some(record_shapes::ValueShape::Map {
                    key: Box::new(record_shapes::ValueShape::Scalar("String".to_owned())),
                    value: Box::new(record_shapes::ValueShape::Scalar("i64".to_owned())),
                }),
                "map_values CST payload must not use the old fallback call shape"
            );
        },
    );
}

#[test]
fn callback_map_err_shape_inference_prefers_cst_lambda_body_shape() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_map_err = result::err(["cst"]).map_err(|errors| errors[0]);
    let legacy_call = option::some(true);
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let cst_map_err = statements[0]
                .let_initializer_expression_payload()
                .expect("CST map_err initializer");
            let legacy_call = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy call fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_map_err
                    .syntax_expression()
                    .expect("CST map_err syntax")
                    .clone(),
                legacy_call.fallback(),
            );

            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                Some(record_shapes::ValueShape::Result {
                    ok: None,
                    err: Some(Box::new(record_shapes::ValueShape::Scalar(
                        "String".to_owned()
                    ))),
                }),
                "map_err CST payload must not use the old fallback call shape"
            );
        },
    );
}

#[test]
fn index_shape_inference_prefers_cst_payload_shape() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_array_index = ["cst"][0];
    let legacy_array_index = [true][0];
}
"#,
        |compiler, payload| {
            let statements = shape_statement_payloads(&payload.body);
            let cst_array_index = statements[0]
                .let_initializer_expression_payload()
                .expect("CST array index initializer");
            let legacy_array_index = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy array index fallback");

            let mismatched_array_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_array_index
                    .syntax_expression()
                    .expect("CST array index syntax")
                    .clone(),
                legacy_array_index.fallback(),
            );
            assert_eq!(
                compiler.value_shape_for_expr_with_payload(
                    mismatched_array_payload.fallback(),
                    Some(&mismatched_array_payload),
                ),
                Some(record_shapes::ValueShape::Scalar("String".to_owned())),
                "array-index CST payload must not use the old fallback index shape"
            );
        },
    );
}
