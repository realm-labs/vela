use super::*;

#[test]
fn semantic_function_match_arm_guards_and_bodies_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn classify(input) {
    let total = 0;
    match input {
        value if {
            let allowed = value > 0;
            allowed
        } => [
            {
                let item = value + 1;
                item
            },
        ],
        _ => [
            {
                let fallback = 0;
                fallback
            },
        ],
    };
    return match input {
        value if {
            let accepted = value == 1;
            accepted
        } => {
            let result = value + 10;
            result
        },
        _ => {
            let other = total;
            other
        },
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("classify").expect("classify function");
    let statement_payloads = payload.body.statement_payloads();

    let statement_arm_payloads = statement_payloads
        .iter()
        .flat_map(|statement| statement.match_arm_payloads().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(statement_arm_payloads.len(), 2);
    assert_match_guard_payload(
        &statement_arm_payloads[0],
        &[
            (SyntaxStatementKind::Let, "let allowed = value > 0;"),
            (SyntaxStatementKind::Expr, "allowed"),
        ],
    );
    assert_match_body_array_element_payload(
        &statement_arm_payloads[0],
        &[
            (SyntaxStatementKind::Let, "let item = value + 1;"),
            (SyntaxStatementKind::Expr, "item"),
        ],
    );
    assert_match_body_array_element_payload(
        &statement_arm_payloads[1],
        &[
            (SyntaxStatementKind::Let, "let fallback = 0;"),
            (SyntaxStatementKind::Expr, "fallback"),
        ],
    );

    let return_arm_payloads = statement_payloads
        .iter()
        .flat_map(|statement| {
            statement
                .return_value_match_arm_payloads()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    assert_eq!(return_arm_payloads.len(), 2);
    assert_match_guard_payload(
        &return_arm_payloads[0],
        &[
            (SyntaxStatementKind::Let, "let accepted = value == 1;"),
            (SyntaxStatementKind::Expr, "accepted"),
        ],
    );
    assert_match_body_block_payload(
        &return_arm_payloads[0],
        &[
            (SyntaxStatementKind::Let, "let result = value + 10;"),
            (SyntaxStatementKind::Expr, "result"),
        ],
    );
    assert_match_body_block_payload(
        &return_arm_payloads[1],
        &[
            (SyntaxStatementKind::Let, "let other = total;"),
            (SyntaxStatementKind::Expr, "other"),
        ],
    );

    compile_program_source(source, text)
        .expect("CST-backed match arm guards and bodies should compile");
}

fn assert_match_guard_payload(
    arm: &body_payloads::CompilerMatchArmPayload<'_>,
    expected: &[(SyntaxStatementKind, &str)],
) {
    let guard = arm
        .guard_payload()
        .expect("match arm should expose guard payload");
    assert_eq!(guard.kind(), Some(SyntaxExpressionKind::Block));
    let body = guard
        .block_body_payload()
        .expect("guard block should expose body payload");
    assert_eq!(
        cst_statement_texts(&body),
        expected_statement_texts(&[expected.to_vec()])[0]
    );
}

fn assert_match_body_array_element_payload(
    arm: &body_payloads::CompilerMatchArmPayload<'_>,
    expected: &[(SyntaxStatementKind, &str)],
) {
    let body = arm.body_expression_payload();
    assert_eq!(body.kind(), Some(SyntaxExpressionKind::Array));
    let element_payloads = body
        .array_element_payloads()
        .expect("array arm body should expose element payloads");
    let element_body = element_payloads[0]
        .block_body_payload()
        .expect("array arm body element should expose block body payload");
    assert_eq!(
        cst_statement_texts(&element_body),
        expected_statement_texts(&[expected.to_vec()])[0]
    );
}

fn assert_match_body_block_payload(
    arm: &body_payloads::CompilerMatchArmPayload<'_>,
    expected: &[(SyntaxStatementKind, &str)],
) {
    let body = arm
        .body_block_payload()
        .expect("match arm should expose block body payload");
    assert_eq!(
        cst_statement_texts(&body),
        expected_statement_texts(&[expected.to_vec()])[0]
    );
}
