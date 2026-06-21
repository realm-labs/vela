use super::*;

#[test]
fn semantic_match_scrutinees_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn classify(input) {
    let local = 0;
    match ({
        let statement_value = input;
        statement_value
    }) {
        _ => 0,
    };
    let initialized = match ({
        let initializer_value = input;
        initializer_value
    }) {
        _ => 1,
    };
    local = match ({
        let assignment_value = initialized;
        assignment_value
    }) {
        _ => 2,
    };
    return match ({
        let return_value = local;
        return_value
    }) {
        _ => 3,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("classify").expect("classify function");
    let statement_payloads = payload.body.statement_payloads();

    let statement_scrutinee = statement_payloads
        .iter()
        .find_map(body_payloads::CompilerStatementPayload::match_scrutinee_payload)
        .expect("match statement should expose scrutinee payload");
    assert_scrutinee_block_payload(
        &statement_scrutinee,
        &[
            (SyntaxStatementKind::Let, "let statement_value = input;"),
            (SyntaxStatementKind::Expr, "statement_value"),
        ],
    );

    let initializer_scrutinee = statement_payloads
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::let_initializer_expression_payload)
        .find_map(|payload| payload.match_scrutinee_payload())
        .expect("match initializer should expose scrutinee payload");
    assert_scrutinee_block_payload(
        &initializer_scrutinee,
        &[
            (SyntaxStatementKind::Let, "let initializer_value = input;"),
            (SyntaxStatementKind::Expr, "initializer_value"),
        ],
    );

    let assignment_scrutinee = statement_payloads
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::assignment_value_expression_payload)
        .find_map(|payload| payload.match_scrutinee_payload())
        .expect("match assignment value should expose scrutinee payload");
    assert_scrutinee_block_payload(
        &assignment_scrutinee,
        &[
            (
                SyntaxStatementKind::Let,
                "let assignment_value = initialized;",
            ),
            (SyntaxStatementKind::Expr, "assignment_value"),
        ],
    );

    let return_scrutinee = statement_payloads
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::return_value_expression_payload)
        .find_map(|payload| payload.match_scrutinee_payload())
        .expect("match return value should expose scrutinee payload");
    assert_scrutinee_block_payload(
        &return_scrutinee,
        &[
            (SyntaxStatementKind::Let, "let return_value = local;"),
            (SyntaxStatementKind::Expr, "return_value"),
        ],
    );

    compile_program_source(source, text).expect("CST-backed match scrutinees should compile");
}

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

fn assert_scrutinee_block_payload(
    payload: &body_payloads::CompilerExpressionPayload<'_>,
    expected: &[(SyntaxStatementKind, &str)],
) {
    assert_eq!(payload.kind(), Some(SyntaxExpressionKind::Paren));
    let inner = payload
        .paren_inner_payload()
        .expect("match scrutinee paren should expose inner payload");
    assert_eq!(inner.kind(), Some(SyntaxExpressionKind::Block));
    let body = inner
        .block_body_payload()
        .expect("match scrutinee block should expose body payload");
    assert_eq!(
        cst_statement_texts(&body),
        expected_statement_texts(&[expected.to_vec()])[0]
    );
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
