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
fn missing_array_element_payloads_do_not_compile_fallback_items() {
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

    let error = compiler
        .compile_expr_with_payload(fallback_array.fallback(), Some(&mismatched))
        .expect_err("missing CST array elements must not compile fallback items");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST array elements")
    ));
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
