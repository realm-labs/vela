use super::*;

#[test]
fn missing_map_entry_payloads_do_not_compile_fallback_entries() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_body() {
    let value = {
        first: 1,
    };
}

fn fallback_body() {
    let value = {
        first: 1,
        second: 2,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_body")
        .expect("fallback function");
    let cst_map = cst_statement_payloads(&cst_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("cst map payload");
    let fallback_map = cst_statement_payloads(&fallback_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("fallback map payload");
    let mismatched = expression_payload_with_fallback(
        source,
        cst_map.syntax_expression().expect("cst map syntax").clone(),
        fallback_map.fallback(),
    );
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_body");

    let error = compiler
        .compile_expr_with_payload(fallback_map.fallback(), Some(&mismatched))
        .expect_err("missing CST map entries must not compile fallback entries");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST map entries")
    ));
}

#[test]
fn array_payload_compiles_cst_elements_not_fallback_items() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_body() {
    let value = [1];
}

fn fallback_body() {
    let value = [1, 2];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_body")
        .expect("fallback function");
    let cst_array = cst_statement_payloads(&cst_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("cst array payload");
    let fallback_array = cst_statement_payloads(&fallback_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("fallback array payload");
    let mismatched = expression_payload_with_fallback(
        source,
        cst_array
            .syntax_expression()
            .expect("cst array syntax")
            .clone(),
        fallback_array.fallback(),
    );
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_body");

    compiler
        .compile_expr_with_payload(fallback_array.fallback(), Some(&mismatched))
        .expect("CST array elements should compile instead of fallback items");

    assert!(
        compiler
            .code
            .instructions
            .iter()
            .any(|instruction| matches!(
                instruction.kind,
                UnlinkedInstructionKind::MakeArray { ref elements, .. } if elements.len() == 1
            ))
    );
}

#[test]
fn mismatched_container_payload_counts_do_not_infer_fallback_value_types() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_short() {
    let values = [true];
    let table = { value: true };
}

fn cst_long() {
    let values = [true, false];
    let table = { value: true, other: false };
}

fn fallback_short() {
    let values = [1];
    let table = { value: 1 };
}

fn fallback_long() {
    let values = [1, 2];
    let table = { value: 1, other: 2 };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_short, _, _) = semantic.function("cst_short").expect("short CST function");
    let (cst_long, _, _) = semantic.function("cst_long").expect("long CST function");
    let (fallback_short, _, _) = semantic
        .function("fallback_short")
        .expect("short fallback function");
    let (fallback_long, _, _) = semantic
        .function("fallback_long")
        .expect("long fallback function");
    let cst_short_statements = cst_statement_payloads(&cst_short.body);
    let cst_long_statements = cst_statement_payloads(&cst_long.body);
    let fallback_short_statements = cst_statement_payloads(&fallback_short.body);
    let fallback_long_statements = cst_statement_payloads(&fallback_long.body);
    let (compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_long");

    let cases = [
        (
            cst_short_statements[0]
                .let_initializer_expression_payload()
                .expect("short CST array"),
            fallback_long_statements[0]
                .let_initializer_expression_payload()
                .expect("long fallback array"),
        ),
        (
            cst_long_statements[0]
                .let_initializer_expression_payload()
                .expect("long CST array"),
            fallback_short_statements[0]
                .let_initializer_expression_payload()
                .expect("short fallback array"),
        ),
        (
            cst_short_statements[1]
                .let_initializer_expression_payload()
                .expect("short CST map"),
            fallback_long_statements[1]
                .let_initializer_expression_payload()
                .expect("long fallback map"),
        ),
        (
            cst_long_statements[1]
                .let_initializer_expression_payload()
                .expect("long CST map"),
            fallback_short_statements[1]
                .let_initializer_expression_payload()
                .expect("short fallback map"),
        ),
    ];

    for (cst_payload, fallback_payload) in cases {
        let mismatched = expression_payload_with_fallback(
            source,
            cst_payload
                .syntax_expression()
                .expect("CST expression")
                .clone(),
            fallback_payload.fallback(),
        );

        assert_eq!(
            compiler.static_type_for_expr_with_payload(mismatched.fallback(), Some(&mismatched)),
            value_types::StaticExprType::Dynamic,
            "value-type inference must reject mismatched CST child payload counts"
        );
    }
}

#[test]
fn missing_record_field_payloads_do_not_compile_fallback_fields() {
    let source = SourceId::new(1);
    let text = r#"
struct Pair {
    first
    second
}

fn cst_body() {
    let value = Pair {
        first: 1,
    };
}

fn fallback_body() {
    let value = Pair {
        first: 1,
        second: 2,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_body")
        .expect("fallback function");
    let cst_record = cst_statement_payloads(&cst_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("cst record payload");
    let fallback_record = cst_statement_payloads(&fallback_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("fallback record payload");
    let mismatched = expression_payload_with_fallback(
        source,
        cst_record
            .syntax_expression()
            .expect("cst record syntax")
            .clone(),
        fallback_record.fallback(),
    );
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_body");

    let error = compiler
        .compile_expr_with_payload(fallback_record.fallback(), Some(&mismatched))
        .expect_err("missing CST record fields must not compile fallback fields");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST record fields")
    ));
}
