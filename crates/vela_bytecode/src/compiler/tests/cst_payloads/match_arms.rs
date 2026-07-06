use super::*;

mod patterns;

fn body_fallback(
    source: SourceId,
    body: &body_payloads::CompilerBodyPayload<'_>,
) -> &'static [Stmt] {
    fallback_statements_for_body(source, body)
}

fn match_statement_payloads<'ast>(
    body: &body_payloads::CompilerBodyPayload<'ast>,
) -> Vec<body_payloads::CompilerStatementPayload<'ast>> {
    paired_statement_payloads_for_body(body.syntax_payload().source, body)
}

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
    let statement_payloads = match_statement_payloads(&payload.body);

    let statement_scrutinee = statement_payloads
        .iter()
        .find_map(|statement| {
            statement
                .expression_payload()
                .and_then(|payload| payload.match_scrutinee_payload())
        })
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
        .filter_map(|statement| {
            statement
                .expression_payload()
                .and_then(|payload| payload.assignment_value_payload())
        })
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
    let statement_payloads = match_statement_payloads(&payload.body);

    let statement_arm_payloads = statement_payloads
        .iter()
        .flat_map(|statement| {
            statement
                .expression_payload()
                .and_then(|payload| payload.match_arm_payloads())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let statement_match = first_statement_match_expr(body_fallback(source, &payload.body));
    assert_eq!(statement_arm_payloads.len(), 2);
    assert_match_guard_payload(
        &statement_arm_payloads[0],
        statement_match.arms[0].guard.as_ref(),
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
            cst_return_value_match_arms_from_expression(statement).unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let return_match = first_return_match_expr(body_fallback(source, &payload.body));
    assert_eq!(return_arm_payloads.len(), 2);
    assert_match_guard_payload(
        &return_arm_payloads[0],
        return_match.arms[0].guard.as_ref(),
        &[
            (SyntaxStatementKind::Let, "let accepted = value == 1;"),
            (SyntaxStatementKind::Expr, "accepted"),
        ],
    );
    assert_match_body_block_payload(
        &return_arm_payloads[0],
        &return_match.arms[0].body,
        &[
            (SyntaxStatementKind::Let, "let result = value + 10;"),
            (SyntaxStatementKind::Expr, "result"),
        ],
    );
    assert_match_body_block_payload(
        &return_arm_payloads[1],
        &return_match.arms[1].body,
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
    let statement_payloads = match_statement_payloads(&payload.body);
    let return_arm_payloads = statement_payloads
        .iter()
        .flat_map(|statement| {
            cst_return_value_match_arms_from_expression(statement).unwrap_or_default()
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
    let missing_source_record =
        body_payloads::CompilerPatternPayload::missing_child_payload_context(
            syntax_pattern.clone(),
        );
    let missing_source_fields = missing_source_record
        .record_field_payloads()
        .expect("source-less record pattern should expose field payloads");
    assert_eq!(missing_source_fields.len(), record_fields.len());
    assert_eq!(missing_source_fields[0].syntax_label_name(), None);
    assert_eq!(missing_source_fields[0].syntax_pattern_kind(), None);
    assert!(missing_source_fields[0].pattern_payload().is_none());
    assert_eq!(missing_source_fields[1].syntax_is_shorthand(), None);
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
    let missing_source_tuple = body_payloads::CompilerPatternPayload::missing_child_payload_context(
        tuple_pattern
            .syntax_pattern()
            .expect("tuple arm should expose CST pattern")
            .clone(),
    );
    let missing_source_tuple_fields = missing_source_tuple
        .tuple_pattern_payloads()
        .expect("source-less tuple pattern should expose field payloads");
    assert_eq!(missing_source_tuple_fields.len(), tuple_fields.len());
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
    let arm_payloads = match_statement_payloads(&payload.body)
        .iter()
        .flat_map(|statement| {
            cst_return_value_match_arms_from_expression(statement).unwrap_or_default()
        })
        .collect::<Vec<_>>();
    assert_eq!(arm_payloads.len(), 3);

    let literal_pattern = arm_payloads[0].pattern_payload();
    assert_eq!(
        literal_pattern.syntax_literal(),
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
        path_pattern.syntax_path_segments().as_deref(),
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
    assert_eq!(
        binding_pattern.syntax_binding_name().as_deref(),
        Some("value")
    );
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
fn payload_match_parameter_defaults_compile_from_cst() {
    let source = SourceId::new(1);
    let text = r#"
enum Result {
    Err { code: i64, detail: String }
    Ok(i64)
}

fn classify(result, code = match result {
    Result::Err { code, detail: _ } => code,
    Result::Ok(value) => value,
}) {
    return code;
}
"#;

    compile_program_source(source, text)
        .expect("payload match parameter defaults should compile from CST");
}

#[test]
fn mismatched_match_pattern_payloads_pair_children_by_position_not_legacy_span() {
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
    let (cst_record_payload, _, _) = semantic.function("cst_record").expect("cst record");
    let (legacy_record_payload, _, _) = semantic.function("legacy_record").expect("legacy record");

    let mismatched_tuple = first_return_match_pattern_payload(&cst_tuple_payload.body);
    let tuple_fields = mismatched_tuple
        .tuple_pattern_payloads()
        .expect("tuple pattern should expose field payloads");
    assert_eq!(tuple_fields.len(), 2);
    let tuple_field_texts = tuple_fields
        .iter()
        .map(|field| {
            field
                .syntax_pattern()
                .expect("tuple field syntax")
                .syntax()
                .text()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        tuple_field_texts,
        ["cst_left".to_owned(), "cst_right".to_owned()]
    );

    let legacy_record_pattern =
        first_return_match_fallback_pattern(body_fallback(source, &legacy_record_payload.body));
    let mismatched_record = first_return_match_pattern_payload(&cst_record_payload.body);
    let record_fields = mismatched_record
        .record_field_payloads()
        .expect("record pattern should expose field payloads");
    assert_eq!(record_fields.len(), 1);
    assert_eq!(
        record_fields[0].syntax_label_name().as_deref(),
        Some("first")
    );

    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "legacy_record");
    compiler
        .bind_pattern_locals(
            Register(0),
            legacy_record_pattern,
            Some(&mismatched_record),
            Span::new(source, 0, 1),
            crate::compiler::patterns::PatternBindingFacts::default(),
            LocalBindingKind::Pattern,
        )
        .expect("CST record field payload should bind the CST field name");
    assert!(
        compiler
            .code
            .frame
            .slot("cst_field", crate::FrameSlotKind::PatternBinding)
            .is_some(),
        "record pattern payload must bind the CST field binding"
    );
    assert!(
        compiler
            .code
            .frame
            .slot("legacy_field", crate::FrameSlotKind::PatternBinding)
            .is_none(),
        "record pattern payload must not bind the legacy fallback field"
    );
}

#[test]
fn mismatched_tuple_child_payloads_do_not_use_legacy_pattern_kind() {
    let source = SourceId::new(1);
    let text = r#"
enum Shape {
    Pair(left: i64, right: i64)
}

fn cst_tuple(value) {
    return match value {
        Shape::Pair(cst_left, cst_right) => cst_left,
        _ => 0,
    };
}

fn legacy_tuple(value) {
    return match value {
        Shape::Pair(1, _) => 1,
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_tuple_payload, _, _) = semantic.function("cst_tuple").expect("cst tuple");
    let (legacy_tuple_payload, _, _) = semantic.function("legacy_tuple").expect("legacy tuple");
    let legacy_tuple_pattern =
        first_return_match_fallback_pattern(body_fallback(source, &legacy_tuple_payload.body));
    let mismatched_tuple = first_return_match_pattern_payload(&cst_tuple_payload.body);

    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "legacy_tuple");
    compiler
        .compile_match_pattern(Register(0), legacy_tuple_pattern, Some(&mismatched_tuple))
        .expect("CST tuple child bindings should not compile legacy literal checks");
    assert!(
        compiler
            .code
            .instructions
            .iter()
            .all(|instruction| !matches!(instruction.kind, UnlinkedInstructionKind::Equal { .. })),
        "CST tuple child bindings must not compile fallback literal checks"
    );
}

#[test]
fn missing_pattern_child_payloads_do_not_use_legacy_pattern_fields() {
    let no_record_payloads: [body_payloads::CompilerRecordPatternFieldPayload; 0] = [];
    let record_error = match crate::compiler::patterns::record_pattern_field_payload_at(
        Some(&no_record_payloads),
        0,
    ) {
        Ok(_) => panic!("missing record pattern child payload must not look at legacy field"),
        Err(error) => error,
    };
    assert!(matches!(
        record_error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST record pattern field payload")
    ));

    let no_tuple_payloads: [body_payloads::CompilerPatternPayload; 0] = [];
    let tuple_error =
        match crate::compiler::patterns::tuple_pattern_payload_at(Some(&no_tuple_payloads), 0) {
            Ok(_) => panic!("missing tuple pattern child payload must not look at legacy field"),
            Err(error) => error,
        };
    assert!(matches!(
        tuple_error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST tuple pattern field payload")
    ));

    assert!(
        crate::compiler::patterns::record_pattern_field_payload_at(None, 0)
            .expect("absent record payload vector should preserve non-CST fallback path")
            .is_none()
    );
    assert!(
        crate::compiler::patterns::tuple_pattern_payload_at(None, 0)
            .expect("absent tuple payload vector should preserve non-CST fallback path")
            .is_none()
    );
}

#[test]
fn mismatched_basic_match_pattern_payloads_do_not_use_legacy_payload_data() {
    let source = SourceId::new(1);
    let text = r#"
enum State {
    Ready
    Waiting
}

fn cst_literal(value) {
    return match value {
        0 => 0,
        _ => value,
    };
}

fn legacy_literal(value) {
    return match value {
        1 => 1,
        _ => value,
    };
}

fn cst_path(value) {
    return match value {
        State::Waiting => 0,
        _ => value,
    };
}

fn cst_binding(value) {
    return match value {
        current => current,
    };
}

fn legacy_path(value) {
    return match value {
        State::Ready => 1,
        _ => value,
    };
}

fn legacy_binding(value) {
    return match value {
        fallback_binding => fallback_binding,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_literal_payload, _, _) = semantic.function("cst_literal").expect("cst literal");
    let (legacy_literal_payload, _, _) =
        semantic.function("legacy_literal").expect("legacy literal");
    let (cst_path_payload, _, _) = semantic.function("cst_path").expect("cst path");
    let (cst_binding_payload, _, _) = semantic.function("cst_binding").expect("cst binding");
    let (legacy_path_payload, _, _) = semantic.function("legacy_path").expect("legacy path");
    let (legacy_binding_payload, _, _) =
        semantic.function("legacy_binding").expect("legacy binding");

    let legacy_literal_pattern =
        first_return_match_fallback_pattern(body_fallback(source, &legacy_literal_payload.body));
    let legacy_path_pattern =
        first_return_match_fallback_pattern(body_fallback(source, &legacy_path_payload.body));
    let legacy_binding_pattern =
        first_return_match_fallback_pattern(body_fallback(source, &legacy_binding_payload.body));

    let mismatched_literal = first_return_match_pattern_payload(&cst_path_payload.body);
    let mismatched_path = first_return_match_pattern_payload(&cst_literal_payload.body);
    let mismatched_binding = first_return_match_pattern_payload(&cst_literal_payload.body);

    let (mut literal_compiler, _) = cst_payload_compiler_for_function(&semantic, "legacy_literal");
    literal_compiler
        .compile_match_pattern(
            Register(0),
            legacy_literal_pattern,
            Some(&mismatched_literal),
        )
        .expect("mismatched literal fallback should compile from CST path pattern");
    assert!(
        literal_compiler
            .code
            .instructions
            .iter()
            .any(|instruction| matches!(
                instruction.kind,
                UnlinkedInstructionKind::EnumTagEqual { .. }
            )),
        "mismatched literal fallback should not drive pattern compilation"
    );

    let (mut path_compiler, _) = cst_payload_compiler_for_function(&semantic, "legacy_path");
    path_compiler
        .compile_match_pattern(Register(0), legacy_path_pattern, Some(&mismatched_path))
        .expect("mismatched path fallback should compile from CST literal pattern");
    assert!(
        path_compiler
            .code
            .instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Equal { .. })),
        "mismatched path fallback should not drive pattern compilation"
    );

    let (mut binding_compiler, _) = cst_payload_compiler_for_function(&semantic, "legacy_binding");
    binding_compiler
        .bind_pattern_locals(
            Register(0),
            legacy_binding_pattern,
            Some(&mismatched_binding),
            Span::new(source, 0, 1),
            crate::compiler::patterns::PatternBindingFacts::default(),
            LocalBindingKind::Pattern,
        )
        .expect("CST literal pattern should not bind legacy fallback name");
    assert!(
        binding_compiler
            .code
            .instructions
            .iter()
            .all(|instruction| !matches!(instruction.kind, UnlinkedInstructionKind::Move { .. })),
        "mismatched binding fallback should not drive local binding"
    );

    let cst_binding_payload = first_return_match_pattern_payload(&cst_binding_payload.body);
    let (mut cst_binding_compiler, _) =
        cst_payload_compiler_for_function(&semantic, "legacy_binding");
    cst_binding_compiler
        .bind_pattern_locals(
            Register(0),
            legacy_literal_pattern,
            Some(&cst_binding_payload),
            Span::new(source, 0, 1),
            crate::compiler::patterns::PatternBindingFacts::default(),
            LocalBindingKind::Pattern,
        )
        .expect("CST binding pattern should bind without a binding fallback");
    assert!(
        cst_binding_compiler
            .code
            .instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Move { .. })),
        "CST binding pattern should drive local binding"
    );
}

#[test]
fn missing_statement_match_arm_payload_count_does_not_use_legacy_arm() {
    let source = SourceId::new(1);
    let text = r#"
fn main(value) {
    match value {
        1 => value,
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    let match_expr = first_statement_match_expr(body_fallback(source, &payload.body));
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "main");

    let error = compiler
        .compile_match_with_payloads(match_expr, None, Some(&[]))
        .expect_err("missing CST statement match arm payloads must not compile legacy arms");

    assert!(
        matches!(
            error.kind,
            CompileErrorKind::UnsupportedSyntax("mismatched CST match arms")
        ),
        "expected mismatched CST match arms, got {error:?}"
    );
}

#[test]
fn missing_value_match_arm_payload_count_does_not_use_legacy_arm() {
    let source = SourceId::new(1);
    let text = r#"
fn main(value) {
    return match value {
        1 => value,
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    let match_expr = first_return_match_expr(body_fallback(source, &payload.body));
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "main");

    let error = compiler
        .compile_match_value_with_payloads(match_expr, Register(0), None, &[])
        .expect_err("missing CST value match arm payloads must not compile legacy arms");

    assert!(
        matches!(
            error.kind,
            CompileErrorKind::UnsupportedSyntax("mismatched CST match arms")
        ),
        "expected mismatched CST match arms, got {error:?}"
    );
}

#[test]
fn value_match_without_arm_payloads_does_not_compile_legacy_arms() {
    let source = SourceId::new(1);
    let text = r#"
fn main(value) {
    return match value {
        1 => value,
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    let match_expr = first_return_match_expr(body_fallback(source, &payload.body));
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "main");
    let error = compiler
        .compile_match_value_with_payloads(match_expr, Register(0), None, &[])
        .expect_err("missing CST value match arm body must not compile legacy expression");

    assert!(
        matches!(
            error.kind,
            CompileErrorKind::UnsupportedSyntax("mismatched CST match arms")
        ),
        "expected mismatched CST match arms, got {error:?}"
    );
}

#[test]
fn missing_match_scrutinee_payload_does_not_use_legacy_scrutinee() {
    let source = SourceId::new(1);
    let text = r#"
fn main(value) {
    return match value {
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    let match_expr = first_return_match_expr(body_fallback(source, &payload.body));
    let missing_scrutinee =
        body_payloads::CompilerExpressionPayload::missing_syntax(source, &match_expr.scrutinee);
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "main");
    let error = compiler
        .compile_match_value_with_payloads(match_expr, Register(0), Some(&missing_scrutinee), &[])
        .expect_err("missing CST match scrutinee must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST match scrutinee payload")
    ));
}

#[test]
fn missing_match_arm_body_payload_does_not_use_legacy_body() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn cst_missing_body(value) {
    return match value {
        _ => ,
    };
}
"#;
    let legacy_text = r#"
fn statement_form(value) {
    match value {
        _ => 1,
    };
}

fn value_form(value) {
    return match value {
        _ => 1,
    };
}
"#;
    let missing_body_arm = first_return_match_arm_payload_from_cst(source, cst_text);
    assert_eq!(
        missing_body_arm
            .syntax_arm()
            .expect("CST match arm")
            .body_expression(),
        None
    );

    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (statement_payload, _, _) = semantic.function("statement_form").expect("statement form");
    let statement_match =
        first_statement_match_expr(body_fallback(source, &statement_payload.body));
    let (mut statement_compiler, _) =
        cst_payload_compiler_for_function(&semantic, "statement_form");

    let statement_error = statement_compiler
        .compile_match_with_payloads(statement_match, None, Some(&[missing_body_arm]))
        .expect_err("missing CST match statement arm body must not use legacy body");
    assert!(matches!(
        statement_error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST match arm body")
    ));

    let (value_payload, _, _) = semantic.function("value_form").expect("value form");
    let value_match = first_return_match_expr(body_fallback(source, &value_payload.body));
    let missing_value_arm = first_return_match_arm_payload_from_cst(source, cst_text);
    let (mut value_compiler, _) = cst_payload_compiler_for_function(&semantic, "value_form");

    let value_error = value_compiler
        .compile_match_value_with_payloads(value_match, Register(0), None, &[missing_value_arm])
        .expect_err("missing CST match value arm body must not use legacy body");
    assert!(matches!(
        value_error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST match arm body")
    ));
}

#[test]
fn missing_match_arm_child_payloads_do_not_use_legacy_control_flow_body() {
    let source = SourceId::new(1);
    let text = r#"
fn statement_form(value) {
    match value {
        _ => {
            let chosen = 1;
            chosen
        },
    };
}

fn value_form(value, flag) {
    return match value {
        _ => if flag {
            1
        } else {
            2
        },
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");

    let (statement_payload, _, _) = semantic.function("statement_form").expect("statement form");
    let statement_match =
        first_statement_match_expr(body_fallback(source, &statement_payload.body));
    let statement_syntax_arm = first_statement_match_syntax_arm(&statement_payload.body);
    let missing_statement_arm =
        body_payloads::CompilerMatchArmPayload::missing_child_payload_context(statement_syntax_arm);
    let (mut statement_compiler, _) =
        cst_payload_compiler_for_function(&semantic, "statement_form");

    let statement_error = statement_compiler
        .compile_match_with_payloads(statement_match, None, Some(&[missing_statement_arm]))
        .expect_err("source-less CST match arm payload must not use legacy pattern or body");
    assert!(matches!(
        statement_error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST match arm payload")
    ));

    let (value_payload, _, _) = semantic.function("value_form").expect("value form");
    let value_match = first_return_match_expr(body_fallback(source, &value_payload.body));
    let value_syntax_arm = first_return_match_syntax_arm(&value_payload.body);
    let missing_value_arm =
        body_payloads::CompilerMatchArmPayload::missing_child_payload_context(value_syntax_arm);
    let (mut value_compiler, _) = cst_payload_compiler_for_function(&semantic, "value_form");

    let value_error = value_compiler
        .compile_match_value_with_payloads(value_match, Register(0), None, &[missing_value_arm])
        .expect_err("source-less CST match arm payload must not use legacy pattern or body");
    assert!(matches!(
        value_error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST match arm payload")
    ));
}

#[test]
fn syntax_only_match_arm_block_drops_owned_body_fallback() {
    let source = SourceId::new(1);
    let text = r#"
fn main(value) {
    match value {
        _ => {
            let nested;
            return;
        },
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (_, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let arm = match_statement_payloads(&payload.body)[0]
        .expression_payload()
        .and_then(|payload| payload.match_arm_payloads())
        .expect("match arm payloads")
        .remove(0);
    let body = arm
        .body_block_payload()
        .expect("match arm body block payload");

    assert!(
        body_has_no_statement_fallbacks(&body),
        "syntax-only match arm block should not retain an owned body fallback"
    );
}

fn assert_scrutinee_block_payload(
    payload: &body_payloads::CompilerExpressionPayload<'_>,
    expected: &[(SyntaxStatementKind, &str)],
) {
    assert_eq!(payload.syntax_kind(), Some(SyntaxExpressionKind::Paren));
    let inner = payload
        .paren_inner_payload()
        .expect("match scrutinee paren should expose inner payload");
    assert_eq!(inner.syntax_kind(), Some(SyntaxExpressionKind::Block));
    let body = inner
        .block_body_payload()
        .expect("match scrutinee block should expose body payload");
    assert_eq!(
        cst_statement_texts(&body),
        expected_statement_texts(&[expected.to_vec()])[0]
    );
}

fn first_return_match_pattern_payload(
    body: &body_payloads::CompilerBodyPayload<'_>,
) -> body_payloads::CompilerPatternPayload {
    let statements = match_statement_payloads(body);
    statements[0]
        .return_value_expression_payload()
        .and_then(|payload| payload.match_arm_payloads())
        .expect("return match")[0]
        .pattern_payload()
}

fn first_return_match_pattern_syntax(
    body: &body_payloads::CompilerBodyPayload<'_>,
) -> vela_syntax::ast::SyntaxPattern {
    first_return_match_pattern_payload(body)
        .syntax_pattern()
        .expect("CST pattern")
        .clone()
}

fn first_return_match_fallback_pattern(
    statements: &[vela_syntax::ast::Stmt],
) -> &vela_syntax::ast::Pattern {
    return_match_fallback_pattern(statements, 0)
}

fn return_match_fallback_pattern(
    statements: &[vela_syntax::ast::Stmt],
    arm_index: usize,
) -> &vela_syntax::ast::Pattern {
    let statement = statements.first().expect("return statement");
    let vela_syntax::ast::StmtKind::Return(Some(value)) = &statement.kind else {
        panic!("expected return statement");
    };
    let vela_syntax::ast::ExprKind::Match(match_expr) = &value.kind else {
        panic!("expected return match expression");
    };
    &match_expr.arms.get(arm_index).expect("match arm").pattern
}

fn first_return_match_syntax_arm(
    body: &body_payloads::CompilerBodyPayload<'_>,
) -> vela_syntax::ast::SyntaxMatchArm {
    let statements = match_statement_payloads(body);
    statements[0]
        .return_value_expression_payload()
        .and_then(|payload| payload.match_arm_payloads())
        .expect("return match")[0]
        .syntax_arm()
        .expect("CST arm")
        .clone()
}

fn first_statement_match_syntax_arm(
    body: &body_payloads::CompilerBodyPayload<'_>,
) -> vela_syntax::ast::SyntaxMatchArm {
    let statements = match_statement_payloads(body);
    statements[0]
        .expression_payload()
        .and_then(|payload| payload.match_arm_payloads())
        .expect("statement match")[0]
        .syntax_arm()
        .expect("CST arm")
        .clone()
}

fn first_return_match_expr(statements: &[vela_syntax::ast::Stmt]) -> &vela_syntax::ast::MatchExpr {
    statements
        .iter()
        .find_map(|statement| {
            let vela_syntax::ast::StmtKind::Return(Some(value)) = &statement.kind else {
                return None;
            };
            let vela_syntax::ast::ExprKind::Match(match_expr) = &value.kind else {
                return None;
            };
            Some(match_expr)
        })
        .expect("return match expression")
}

fn first_statement_match_expr(
    statements: &[vela_syntax::ast::Stmt],
) -> &vela_syntax::ast::MatchExpr {
    statements
        .iter()
        .find_map(|statement| {
            let vela_syntax::ast::StmtKind::Expr(value) = &statement.kind else {
                return None;
            };
            let vela_syntax::ast::ExprKind::Match(match_expr) = &value.kind else {
                return None;
            };
            Some(match_expr)
        })
        .expect("match expression statement")
}

fn assert_match_guard_payload(
    arm: &body_payloads::CompilerMatchArmPayload,
    _fallback: Option<&vela_syntax::ast::Expr>,
    expected: &[(SyntaxStatementKind, &str)],
) {
    let guard = arm
        .guard_payload()
        .expect("match arm should expose guard payload");
    assert_eq!(guard.syntax_kind(), Some(SyntaxExpressionKind::Block));
    let body = guard
        .block_body_payload()
        .expect("guard block should expose body payload");
    assert_eq!(
        cst_statement_texts(&body),
        expected_statement_texts(&[expected.to_vec()])[0]
    );
}

fn assert_match_body_array_element_payload(
    arm: &body_payloads::CompilerMatchArmPayload,
    expected: &[(SyntaxStatementKind, &str)],
) {
    let body = arm.body_expression_payload();
    assert_eq!(body.syntax_kind(), Some(SyntaxExpressionKind::Array));
    let element_payloads = body
        .array_element_value_payloads()
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
    arm: &body_payloads::CompilerMatchArmPayload,
    fallback: &vela_syntax::ast::Expr,
    expected: &[(SyntaxStatementKind, &str)],
) {
    let vela_syntax::ast::ExprKind::Block(_) = &fallback.kind else {
        panic!("expected match arm block body");
    };
    let body = arm
        .body_block_payload()
        .expect("match arm should expose block body payload");
    assert_eq!(
        cst_statement_texts(&body),
        expected_statement_texts(&[expected.to_vec()])[0]
    );
}
