use super::*;

#[test]
fn extra_body_statement_payloads_do_not_compile_fallback_body() {
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
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_body");
    let mismatched = body_payloads::CompilerBodyPayload::syntax(
        source,
        cst_payload.body.syntax_payload().body.clone(),
        fallback_payload.body.fallback_statements(),
    );

    let error = compiler
        .compile_body_payload_statements_for_test(&mismatched)
        .expect_err("extra CST body statements must not compile fallback body");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST body statements")
    ));
}
