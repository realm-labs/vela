use super::*;

#[test]
fn mismatched_let_initializer_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn take(value) {
    return value;
}

fn main() {
    let cst_value = take(1);
    let legacy_value = [1];
    return 0;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_let = statements[0]
        .syntax_statement()
        .expect("CST let statement")
        .clone();
    let legacy_array_let = statements[1].fallback();
    let mismatched =
        body_payloads::CompilerStatementPayload::syntax(source, cst_let, legacy_array_let);

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("mismatched let initializer payload must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST let initializer payload")
    ));
}

#[test]
fn mismatched_return_value_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn take(value) {
    return value;
}

fn main() {
    let value = 1;
    return take(value);
    return [value];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_return = statements[1]
        .syntax_statement()
        .expect("CST return statement")
        .clone();
    let legacy_array_return = statements[2].fallback();
    let mismatched =
        body_payloads::CompilerStatementPayload::syntax(source, cst_return, legacy_array_return);

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("mismatched return value payload must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST return value payload")
    ));
}
