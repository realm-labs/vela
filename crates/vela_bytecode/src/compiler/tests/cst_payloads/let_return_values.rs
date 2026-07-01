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
fn missing_let_initializer_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let cst_value;
    let legacy_value = [1];
    return 0;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_let_without_initializer = statements[0]
        .syntax_statement()
        .expect("CST let statement")
        .clone();
    let legacy_array_let = statements[1].fallback();
    let mismatched = body_payloads::CompilerStatementPayload::syntax(
        source,
        cst_let_without_initializer,
        legacy_array_let,
    );

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("missing CST let initializer must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST let initializer payload")
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

#[test]
fn missing_simple_let_initializer_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let cst_value;
    let legacy_value = 1;
    return 0;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_let_without_initializer = statements[0]
        .syntax_statement()
        .expect("CST let statement")
        .clone();
    let legacy_literal_let = statements[1].fallback();
    let mismatched = body_payloads::CompilerStatementPayload::syntax(
        source,
        cst_let_without_initializer,
        legacy_literal_let,
    );

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("missing CST simple let initializer must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST let initializer payload")
    ));
}

#[test]
fn missing_return_value_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = 1;
    return;
    return [value];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_return_without_value = statements[1]
        .syntax_statement()
        .expect("CST return statement")
        .clone();
    let legacy_array_return = statements[2].fallback();
    let mismatched = body_payloads::CompilerStatementPayload::syntax(
        source,
        cst_return_without_value,
        legacy_array_return,
    );

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("missing CST return value must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST return value payload")
    ));
}

#[test]
fn missing_simple_return_value_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    return;
    return 1;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_return_without_value = statements[0]
        .syntax_statement()
        .expect("CST return statement")
        .clone();
    let legacy_literal_return = statements[1].fallback();
    let mismatched = body_payloads::CompilerStatementPayload::syntax(
        source,
        cst_return_without_value,
        legacy_literal_return,
    );

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("missing CST simple return value must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST return value payload")
    ));
}
