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
    let guard = 0;
    guard && true
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
    let recovered_statement = cst_body
        .statements()
        .next()
        .expect("recovered CST statement");
    let fallback_statement = &fallback_statements_for_body(source, &legacy_payload.body)[1];
    let mismatched = body_payloads::CompilerStatementPayload::syntax(
        source,
        recovered_statement,
        fallback_statement,
    );

    assert_eq!(mismatched.statement_kind(), Some(SyntaxStatementKind::Expr));
    assert_eq!(mismatched.value_expression_kind(), None);
    let vela_syntax::ast::StmtKind::Expr(expr) = &fallback_statement.kind else {
        panic!("expected legacy expression tail");
    };

    let error = compiler
        .compile_block_tail_expr_to_for_test(expr, Some(&mismatched), Register(0))
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
    if ({
        let selected = flag;
        selected
    }) {
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

#[test]
fn missing_block_expression_body_payload_does_not_use_legacy_body() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let value = {
        1
    };
}
"#;
    let legacy_text = r#"
fn main() {
    let value = {
        let guard = 2;
        guard && true
    };
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_block = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body")
        .statements()
        .next()
        .expect("CST let statement")
        .as_let()
        .expect("CST let")
        .initializer()
        .expect("CST initializer");
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_block = legacy_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("legacy block initializer");
    let missing = body_payloads::CompilerExpressionPayload::missing_child_payload_context(
        cst_block,
        legacy_block.fallback(),
    );

    let error = compiler
        .compile_expr_with_payload(legacy_block.fallback(), Some(&missing))
        .expect_err("missing CST block body must not compile legacy block body");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST block expression body payload")
    ));
}

#[test]
fn empty_block_expression_payload_uses_cst_empty_body() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let value = {};
}
"#;
    let legacy_text = r#"
fn main() {
    let value = {
        let guard = 1;
        guard && true
    };
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_block = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body")
        .statements()
        .next()
        .expect("CST let statement")
        .as_let()
        .expect("CST let")
        .initializer()
        .expect("CST initializer");
    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_block = legacy_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("legacy block initializer");
    let mismatched = body_payloads::CompilerExpressionPayload::syntax(
        source,
        cst_block,
        legacy_block.fallback(),
    );

    compiler
        .compile_expr_with_payload(legacy_block.fallback(), Some(&mismatched))
        .expect("empty CST block must not compile legacy block body");

    assert!(compiler.code.constants.contains(&Constant::Null));
    assert!(
        compiler
            .code
            .constants
            .iter()
            .all(|constant| *constant != Constant::i64(1)),
        "fallback block expression body must not be emitted"
    );
}
