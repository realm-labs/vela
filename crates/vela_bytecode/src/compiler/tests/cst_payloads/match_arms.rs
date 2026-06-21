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

#[test]
fn semantic_function_match_arm_patterns_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
enum Result {
    Err { code: i64, message: String }
    Ok(i64)
}

fn classify(result) {
    return match result {
        Result::Err { code: status, message } => status,
        Result::Ok(value) => value,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("classify").expect("classify function");
    let statement_payloads = payload.body.statement_payloads();
    let return_arm_payloads = statement_payloads
        .iter()
        .flat_map(|statement| {
            statement
                .return_value_match_arm_payloads()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    assert_eq!(return_arm_payloads.len(), 2);

    let record_pattern = return_arm_payloads[0].pattern_payload();
    let syntax_pattern = record_pattern
        .syntax_pattern()
        .expect("record arm should expose CST pattern");
    assert_eq!(
        syntax_pattern.pattern_kind(),
        Some(vela_syntax::ast::SyntaxPatternKind::RecordVariant)
    );
    let record_fields = record_pattern
        .record_field_payloads()
        .expect("record pattern should expose field payloads");
    let field_labels = record_fields
        .iter()
        .filter_map(|field| field.syntax_label_name())
        .collect::<Vec<_>>();
    assert_eq!(field_labels, ["code", "message"]);
    let nested_pattern = record_fields[0]
        .pattern_payload()
        .expect("explicit record pattern field should expose nested payload");
    assert_eq!(
        nested_pattern
            .syntax_pattern()
            .and_then(|pattern| pattern.binding_name())
            .as_deref(),
        Some("status")
    );

    let tuple_pattern = return_arm_payloads[1].pattern_payload();
    let tuple_fields = tuple_pattern
        .tuple_pattern_payloads()
        .expect("tuple pattern should expose field payloads");
    assert_eq!(
        tuple_fields[0]
            .syntax_pattern()
            .and_then(|pattern| pattern.binding_name())
            .as_deref(),
        Some("value")
    );

    compile_program_source(source, text).expect("CST-backed match arm patterns should compile");
}

#[test]
fn semantic_function_basic_match_arm_patterns_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
enum State {
    Ready
    Waiting
}

fn classify(state) {
    return match state {
        0 => 0,
        State::Ready => 1,
        value => value,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("classify").expect("classify function");
    let arm_payloads = payload
        .body
        .statement_payloads()
        .iter()
        .flat_map(|statement| {
            statement
                .return_value_match_arm_payloads()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    assert_eq!(arm_payloads.len(), 3);

    let literal_pattern = arm_payloads[0].pattern_payload();
    assert_eq!(
        literal_pattern.literal(),
        Some(vela_syntax::ast::Literal::integer("0"))
    );
    assert_eq!(
        literal_pattern
            .syntax_pattern()
            .and_then(|pattern| pattern.literal_text())
            .as_deref(),
        Some("0")
    );

    let path_pattern = arm_payloads[1].pattern_payload();
    assert_eq!(
        path_pattern.path_segments().as_deref(),
        Some(&["State".to_owned(), "Ready".to_owned()][..])
    );
    assert_eq!(
        path_pattern
            .syntax_pattern()
            .and_then(|pattern| pattern.path_text())
            .as_deref(),
        Some("State::Ready")
    );

    let binding_pattern = arm_payloads[2].pattern_payload();
    assert_eq!(binding_pattern.binding_name().as_deref(), Some("value"));
    assert_eq!(
        binding_pattern
            .syntax_pattern()
            .and_then(|pattern| pattern.binding_name())
            .as_deref(),
        Some("value")
    );

    compile_program_source(source, text)
        .expect("CST-backed basic match arm patterns should compile");
}

#[test]
fn mismatched_match_pattern_payloads_do_not_pair_children_by_index_or_label() {
    let source = SourceId::new(1);
    let text = r#"
enum Shape {
    Pair(left: i64, right: i64)
    Named { first: i64, second: i64 }
}

fn cst_tuple(value) {
    return match value {
        Shape::Pair(cst_left, cst_right) => cst_left,
        _ => value,
    };
}

fn legacy_tuple(value) {
    return match value {
        Shape::Pair(legacy_left, legacy_right) => legacy_left,
        _ => value,
    };
}

fn cst_record(value) {
    return match value {
        Shape::Named { first: cst_field } => cst_field,
        _ => value,
    };
}

fn legacy_record(value) {
    return match value {
        Shape::Named { second: legacy_field } => legacy_field,
        _ => value,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_tuple_payload, _, _) = semantic.function("cst_tuple").expect("cst tuple");
    let (legacy_tuple_payload, _, _) = semantic.function("legacy_tuple").expect("legacy tuple");
    let (cst_record_payload, _, _) = semantic.function("cst_record").expect("cst record");
    let (legacy_record_payload, _, _) = semantic.function("legacy_record").expect("legacy record");

    let cst_tuple_syntax = first_return_match_pattern_syntax(&cst_tuple_payload.body);
    let legacy_tuple_pattern =
        first_return_match_fallback_pattern(legacy_tuple_payload.body.fallback());
    let mismatched_tuple =
        body_payloads::CompilerPatternPayload::syntax(cst_tuple_syntax, legacy_tuple_pattern);
    let tuple_fields = mismatched_tuple
        .tuple_pattern_payloads()
        .expect("tuple pattern should expose field payloads");
    assert_eq!(tuple_fields.len(), 2);
    assert!(
        tuple_fields
            .iter()
            .all(|field| field.syntax_pattern().is_none()),
        "mismatched tuple fields must not receive index-based CST patterns"
    );

    let cst_record_syntax = first_return_match_pattern_syntax(&cst_record_payload.body);
    let legacy_record_pattern =
        first_return_match_fallback_pattern(legacy_record_payload.body.fallback());
    let mismatched_record =
        body_payloads::CompilerPatternPayload::syntax(cst_record_syntax, legacy_record_pattern);
    let record_fields = mismatched_record
        .record_field_payloads()
        .expect("record pattern should expose field payloads");
    assert_eq!(record_fields.len(), 1);
    assert!(
        record_fields
            .iter()
            .all(|field| field.syntax_label_name().is_none()),
        "mismatched record fields must not receive label or index fallback CST fields"
    );
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

fn first_return_match_pattern_syntax(
    body: &body_payloads::CompilerBodyPayload<'_>,
) -> vela_syntax::ast::SyntaxPattern {
    let statements = body.statement_payloads();
    statements[0]
        .return_value_match_arm_payloads()
        .expect("return match")[0]
        .pattern_payload()
        .syntax_pattern()
        .expect("CST pattern")
        .clone()
}

fn first_return_match_fallback_pattern(
    body: &vela_syntax::ast::Block,
) -> &vela_syntax::ast::Pattern {
    let statement = body.statements.first().expect("return statement");
    let vela_syntax::ast::StmtKind::Return(Some(value)) = &statement.kind else {
        panic!("expected return statement");
    };
    let vela_syntax::ast::ExprKind::Match(match_expr) = &value.kind else {
        panic!("expected return match expression");
    };
    &match_expr.arms[0].pattern
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
