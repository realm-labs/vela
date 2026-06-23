use super::*;

#[test]
fn semantic_function_block_statement_body_is_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn scoped() {
    let total = 0;
    {
        total += 1;
    }
    return total;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("scoped").expect("scoped function");
    assert_cst_statements(
        &payload.body,
        &[
            (SyntaxStatementKind::Let, "let total = 0;"),
            (SyntaxStatementKind::Block, "{\n        total += 1;\n    }"),
            (SyntaxStatementKind::Return, "return total;"),
        ],
    );
    assert_cst_block_statement_payloads(
        &payload.body,
        &[vec![(SyntaxStatementKind::Expr, "total += 1;")]],
    );

    compile_program_source(source, text).expect("CST-backed block statement body should compile");
}

#[test]
fn semantic_function_control_flow_statements_are_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn flow() {
    let total = 0;
    if total == 0 {
        return 1;
    }
    match total {
        0 => { return 0; },
        _ => { return total; },
    }
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("flow").expect("flow function");
    assert_cst_statements(
        &payload.body,
        &[
            (SyntaxStatementKind::Let, "let total = 0;"),
            (
                SyntaxStatementKind::If,
                "if total == 0 {\n        return 1;\n    }",
            ),
            (
                SyntaxStatementKind::Match,
                "match total {\n        0 => { return 0; },\n        _ => { return total; },\n    }",
            ),
        ],
    );
    assert_cst_match_arm_body_payloads(
        &payload.body,
        &[
            vec![(SyntaxStatementKind::Return, "return 0;")],
            vec![(SyntaxStatementKind::Return, "return total;")],
        ],
    );

    compile_program_source(source, text).expect("CST-backed control-flow body should compile");
}
