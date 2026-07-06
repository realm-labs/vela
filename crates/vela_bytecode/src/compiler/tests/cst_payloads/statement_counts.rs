use super::*;

#[test]
fn extra_body_statement_payloads_are_detected() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_body() {
    let extra = 1;
    return extra;
}

fn fallback_body() {
    let value = [1];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_body")
        .expect("fallback function");

    assert_ne!(
        cst_payload.body.statement_payloads().len(),
        fallback_statements_for_body(source, &fallback_payload.body).len(),
    );
}
