use super::*;

#[test]
fn recovered_cst_tail_without_expression_does_not_use_legacy_block_tail() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    ;
}
"#;
    let legacy_text = r#"
fn main() {
    1
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_body = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body");
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let mismatched = body_payloads::CompilerBodyPayload::syntax(
        source,
        cst_body,
        legacy_payload.body.fallback(),
    );

    let statements = mismatched.statement_payloads();
    assert_eq!(statements.len(), 1);
    assert_eq!(
        statements[0].statement_kind(),
        Some(SyntaxStatementKind::Expr)
    );
    assert_eq!(statements[0].value_expression_kind(), None);

    let error = compiler
        .compile_block_payload_value_to(&mismatched, Register(0))
        .expect_err("recovered CST tail must not compile the legacy tail expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST block tail expression")
    ));
}

#[test]
fn missing_block_tail_body_payload_does_not_use_legacy_body() {
    with_cst_payload_compiler(
        r#"
fn main(flag) {
    if flag {
        1
    } else {
        2
    }
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let tail = statements.last().expect("tail statement");
            let syntax = tail.syntax_statement().expect("tail CST statement").clone();
            let missing_children =
                body_payloads::CompilerStatementPayload::missing_child_payload_context(
                    syntax,
                    tail.fallback(),
                );
            let vela_syntax::ast::StmtKind::Expr(expr) = &tail.fallback().kind else {
                panic!("expected legacy expression tail");
            };

            let error = compiler
                .compile_block_tail_expr_to_for_test(expr, Some(&missing_children), Register(0))
                .expect_err("missing CST if child payload must not use legacy tail expression");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST block tail if payload")
            ));
        },
    );
}
