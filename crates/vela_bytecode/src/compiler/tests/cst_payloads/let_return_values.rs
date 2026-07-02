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
fn mismatched_path_let_initializer_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main(value) {
    let cst_value = self;
    let legacy_value = value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_self_let = statements[0]
        .syntax_statement()
        .expect("CST let statement")
        .clone();
    let legacy_path_let = statements[1].fallback();
    let mismatched =
        body_payloads::CompilerStatementPayload::syntax(source, cst_self_let, legacy_path_let);

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("mismatched path let initializer payload must not compile legacy expression");

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
fn unclassified_let_initializer_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = 1;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statement = payload.body.statement_payloads()[0].fallback();
    let vela_syntax::ast::StmtKind::Let {
        value: Some(value), ..
    } = &statement.kind
    else {
        panic!("expected let statement");
    };
    let missing_payload = body_payloads::CompilerExpressionPayload::missing_syntax(source, value);

    let error = compiler
        .compile_let_initializer_value_payload_for_test(value, Some(&missing_payload))
        .expect_err("unclassified CST let payload must not compile legacy expression");

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
fn unclassified_return_value_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    return 1;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statement = payload.body.statement_payloads()[0].fallback();
    let vela_syntax::ast::StmtKind::Return(Some(value)) = &statement.kind else {
        panic!("expected return statement");
    };
    let missing_payload = body_payloads::CompilerExpressionPayload::missing_syntax(source, value);

    let error = compiler
        .compile_return_value_payload_for_test(value, Some(&missing_payload))
        .expect_err("unclassified CST return payload must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST return value payload")
    ));
}

#[test]
fn mismatched_path_return_value_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main(value) {
    return self;
    return value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_self_return = statements[0]
        .syntax_statement()
        .expect("CST return statement")
        .clone();
    let legacy_path_return = statements[1].fallback();
    let mismatched = body_payloads::CompilerStatementPayload::syntax(
        source,
        cst_self_return,
        legacy_path_return,
    );

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("mismatched path return payload must not compile legacy expression");

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
fn missing_let_initializer_block_body_payload_does_not_use_legacy_block() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = {
        let nested = 1;
        nested
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statement = payload.body.statement_payloads()[0]
        .syntax_statement()
        .expect("CST let")
        .clone();
    let missing_child = body_payloads::CompilerStatementPayload::missing_child_payload_context(
        statement,
        payload.body.statement_payloads()[0].fallback(),
    );

    let error = compiler
        .compile_statement_payload_for_test(&missing_child)
        .expect_err("missing CST let block body must not compile legacy block");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST let initializer block body payload")
    ));
}

#[test]
fn empty_return_payload_with_array_fallback_uses_cst_empty_return() {
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

    compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect("CST empty return payload must not compile legacy array expression");

    assert_empty_return_without_i64_fallback(&compiler);
}

#[test]
fn missing_return_block_body_payload_does_not_use_legacy_block() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    return {
        let nested = 1;
        nested
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statement = payload.body.statement_payloads()[0]
        .syntax_statement()
        .expect("CST return")
        .clone();
    let missing_child = body_payloads::CompilerStatementPayload::missing_child_payload_context(
        statement,
        payload.body.statement_payloads()[0].fallback(),
    );

    let error = compiler
        .compile_statement_payload_for_test(&missing_child)
        .expect_err("missing CST return block body must not compile legacy block");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST return block body payload")
    ));
}

#[test]
fn empty_return_statement_payload_uses_cst_kind() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_body() {
    return;
}

fn fallback_body() {
    return 1;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_body")
        .expect("fallback function");
    let cst_statement = cst_payload.body.statement_payloads()[0]
        .syntax_statement()
        .expect("cst statement syntax")
        .clone();
    let fallback_statement = fallback_payload.body.statement_payloads()[0].fallback();
    let mismatched =
        body_payloads::CompilerStatementPayload::syntax(source, cst_statement, fallback_statement);
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_body");

    compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect("CST empty return payload must not compile fallback return value");

    assert_empty_return_without_i64_fallback(&compiler);
}

#[test]
fn empty_return_payload_with_literal_fallback_uses_cst_empty_return() {
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

    compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect("CST empty return payload must not compile legacy literal expression");

    assert_empty_return_without_i64_fallback(&compiler);
}

fn assert_empty_return_without_i64_fallback(compiler: &Compiler<'_, '_>) {
    assert_eq!(
        compiler
            .code
            .instructions
            .iter()
            .filter(|instruction| matches!(
                instruction.kind,
                UnlinkedInstructionKind::Return { .. }
            ))
            .count(),
        1
    );
    assert!(
        compiler
            .code
            .constants
            .iter()
            .all(|constant| *constant != Constant::i64(1)),
        "fallback return value must not be emitted"
    );
}
