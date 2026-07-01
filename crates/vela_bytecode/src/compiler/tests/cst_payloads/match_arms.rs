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
fn pattern_fact_payloads_read_simple_facts_from_cst_without_fallback_kind() {
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

fn legacy_literal(value) {
    return match value {
        1 => 1,
        _ => value,
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
    let (cst_path_payload, _, _) = semantic.function("cst_path").expect("cst path");
    let (cst_binding_payload, _, _) = semantic.function("cst_binding").expect("cst binding");
    let (legacy_literal_payload, _, _) =
        semantic.function("legacy_literal").expect("legacy literal");
    let (legacy_path_payload, _, _) = semantic.function("legacy_path").expect("legacy path");
    let (legacy_binding_payload, _, _) =
        semantic.function("legacy_binding").expect("legacy binding");

    let literal_payload = body_payloads::CompilerPatternPayload::syntax(
        first_return_match_pattern_syntax(&cst_literal_payload.body),
        first_return_match_fallback_pattern(legacy_path_payload.body.fallback()),
    );
    assert_eq!(
        literal_payload.syntax_literal(),
        Some(vela_syntax::ast::Literal::integer("0"))
    );

    let path_payload = body_payloads::CompilerPatternPayload::syntax(
        first_return_match_pattern_syntax(&cst_path_payload.body),
        first_return_match_fallback_pattern(legacy_binding_payload.body.fallback()),
    );
    assert_eq!(
        path_payload.syntax_path_segments().as_deref(),
        Some(&["State".to_owned(), "Waiting".to_owned()][..])
    );

    let binding_payload = body_payloads::CompilerPatternPayload::syntax(
        first_return_match_pattern_syntax(&cst_binding_payload.body),
        first_return_match_fallback_pattern(legacy_literal_payload.body.fallback()),
    );
    assert_eq!(
        binding_payload.syntax_binding_name().as_deref(),
        Some("current")
    );
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

    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "legacy_record");
    let err = compiler
        .bind_pattern_locals(
            Register(0),
            legacy_record_pattern,
            Some(&mismatched_record),
            Span::new(source, 0, 1),
            crate::compiler::patterns::PatternBindingFacts::default(),
            LocalBindingKind::Pattern,
        )
        .expect_err("mismatched record field payload should not use legacy field name");
    assert!(matches!(
        err.kind,
        CompileErrorKind::UnsupportedSyntax("record pattern field")
    ));
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
    let cst_tuple_syntax = first_return_match_pattern_syntax(&cst_tuple_payload.body);
    let legacy_tuple_pattern =
        first_return_match_fallback_pattern(legacy_tuple_payload.body.fallback());
    let mismatched_tuple =
        body_payloads::CompilerPatternPayload::syntax(cst_tuple_syntax, legacy_tuple_pattern);

    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "legacy_tuple");
    let err = compiler
        .compile_match_pattern(Register(0), legacy_tuple_pattern, Some(&mismatched_tuple))
        .expect_err("mismatched tuple child should not use legacy pattern kind");

    assert!(matches!(
        err.kind,
        CompileErrorKind::UnsupportedSyntax("tuple pattern field")
    ));
}

#[test]
fn missing_pattern_child_payloads_do_not_use_legacy_pattern_fields() {
    let no_record_payloads: [body_payloads::CompilerRecordPatternFieldPayload<'_>; 0] = [];
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

    let no_tuple_payloads: [body_payloads::CompilerPatternPayload<'_>; 0] = [];
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

    let cst_literal_syntax = first_return_match_pattern_syntax(&cst_literal_payload.body);
    let cst_path_syntax = first_return_match_pattern_syntax(&cst_path_payload.body);
    let cst_binding_syntax = first_return_match_pattern_syntax(&cst_binding_payload.body);
    let legacy_literal_pattern =
        first_return_match_fallback_pattern(legacy_literal_payload.body.fallback());
    let legacy_path_pattern =
        first_return_match_fallback_pattern(legacy_path_payload.body.fallback());
    let legacy_binding_pattern =
        first_return_match_fallback_pattern(legacy_binding_payload.body.fallback());

    let mismatched_literal =
        body_payloads::CompilerPatternPayload::syntax(cst_path_syntax, legacy_literal_pattern);
    let mismatched_path = body_payloads::CompilerPatternPayload::syntax(
        cst_literal_syntax.clone(),
        legacy_path_pattern,
    );
    let mismatched_binding =
        body_payloads::CompilerPatternPayload::syntax(cst_literal_syntax, legacy_binding_pattern);

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

    let cst_binding_payload =
        body_payloads::CompilerPatternPayload::syntax(cst_binding_syntax, legacy_literal_pattern);
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
fn missing_statement_match_arm_child_payload_does_not_use_legacy_arm() {
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
    let match_expr = first_statement_match_expr(payload.body.fallback());
    let arm_payloads: [body_payloads::CompilerMatchArmPayload<'_>; 0] = [];
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "main");

    let error = compiler
        .compile_match_with_payloads(match_expr, None, Some(&arm_payloads))
        .expect_err("missing CST statement match arm payload must not compile legacy arm");

    assert!(
        matches!(
            error.kind,
            CompileErrorKind::UnsupportedSyntax("missing CST match arm payload")
        ),
        "expected missing CST match arm payload, got {error:?}"
    );
}

#[test]
fn missing_value_match_arm_child_payload_does_not_use_legacy_arm() {
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
    let match_expr = first_return_match_expr(payload.body.fallback());
    let arm_payloads: [body_payloads::CompilerMatchArmPayload<'_>; 0] = [];
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "main");

    let error = compiler
        .compile_match_value_with_payloads(match_expr, Register(0), None, Some(&arm_payloads))
        .expect_err("missing CST value match arm payload must not compile legacy arm");

    assert!(
        matches!(
            error.kind,
            CompileErrorKind::UnsupportedSyntax("missing CST match arm payload")
        ),
        "expected missing CST match arm payload, got {error:?}"
    );
}

#[test]
fn mismatched_match_guard_payloads_do_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_guard(value, cst_flag) {
    return match value {
        _ if {
            let allowed = cst_flag;
            allowed
        } => 1,
        _ => 0,
    };
}

fn legacy_guard(value, legacy_flag) {
    return match value {
        _ if legacy_flag => 1,
        _ => 0,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_guard").expect("cst guard");
    let (legacy_payload, _, _) = semantic.function("legacy_guard").expect("legacy guard");

    let cst_arm = first_return_match_syntax_arm(&cst_payload.body);
    let legacy_match = first_return_match_expr(legacy_payload.body.fallback());
    let mismatched_arm =
        body_payloads::CompilerMatchArmPayload::syntax(source, cst_arm, &legacy_match.arms[0]);
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "legacy_guard");

    let err = compiler
        .compile_match_value_with_payloads(legacy_match, Register(0), None, Some(&[mismatched_arm]))
        .expect_err("mismatched guard payload should not use legacy expression");
    assert!(matches!(
        err.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST match guard")
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
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_arm = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body")
        .statements()
        .next()
        .expect("CST return statement")
        .as_return()
        .expect("CST return")
        .expression()
        .expect("CST return expression")
        .as_match()
        .expect("CST match expression")
        .arms()
        .into_iter()
        .next()
        .expect("CST match arm");
    assert_eq!(cst_arm.body_expression(), None);

    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (statement_payload, _, _) = semantic.function("statement_form").expect("statement form");
    let statement_match = first_statement_match_expr(statement_payload.body.fallback());
    let missing_statement_arm = body_payloads::CompilerMatchArmPayload::syntax(
        source,
        cst_arm.clone(),
        &statement_match.arms[0],
    );
    let (mut statement_compiler, _) =
        cst_payload_compiler_for_function(&semantic, "statement_form");

    let statement_error = statement_compiler
        .compile_match_with_payloads(statement_match, None, Some(&[missing_statement_arm]))
        .expect_err("missing CST match statement arm body must not use legacy body");
    assert!(matches!(
        statement_error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST match arm body")
    ));

    let (value_payload, _, _) = semantic.function("value_form").expect("value form");
    let value_match = first_return_match_expr(value_payload.body.fallback());
    let missing_value_arm =
        body_payloads::CompilerMatchArmPayload::syntax(source, cst_arm, &value_match.arms[0]);
    let (mut value_compiler, _) = cst_payload_compiler_for_function(&semantic, "value_form");

    let value_error = value_compiler
        .compile_match_value_with_payloads(
            value_match,
            Register(0),
            None,
            Some(&[missing_value_arm]),
        )
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
    let statement_match = first_statement_match_expr(statement_payload.body.fallback());
    let statement_syntax_arm = first_statement_match_syntax_arm(&statement_payload.body);
    let missing_statement_arm =
        body_payloads::CompilerMatchArmPayload::missing_child_payload_context(
            statement_syntax_arm,
            &statement_match.arms[0],
        );
    let (mut statement_compiler, _) =
        cst_payload_compiler_for_function(&semantic, "statement_form");

    let statement_error = statement_compiler
        .compile_match_with_payloads(statement_match, None, Some(&[missing_statement_arm]))
        .expect_err("missing CST match block body payload must not use legacy block body");
    assert!(matches!(
        statement_error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST match arm block body payload")
    ));

    let (value_payload, _, _) = semantic.function("value_form").expect("value form");
    let value_match = first_return_match_expr(value_payload.body.fallback());
    let value_syntax_arm = first_return_match_syntax_arm(&value_payload.body);
    let missing_value_arm = body_payloads::CompilerMatchArmPayload::missing_child_payload_context(
        value_syntax_arm,
        &value_match.arms[0],
    );
    let (mut value_compiler, _) = cst_payload_compiler_for_function(&semantic, "value_form");

    let value_error = value_compiler
        .compile_match_value_with_payloads(
            value_match,
            Register(0),
            None,
            Some(&[missing_value_arm]),
        )
        .expect_err("missing CST match if payload must not use legacy if body");
    assert!(matches!(
        value_error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST match arm if payload")
    ));
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

fn first_return_match_syntax_arm(
    body: &body_payloads::CompilerBodyPayload<'_>,
) -> vela_syntax::ast::SyntaxMatchArm {
    let statements = body.statement_payloads();
    statements[0]
        .return_value_match_arm_payloads()
        .expect("return match")[0]
        .syntax_arm()
        .expect("CST arm")
        .clone()
}

fn first_statement_match_syntax_arm(
    body: &body_payloads::CompilerBodyPayload<'_>,
) -> vela_syntax::ast::SyntaxMatchArm {
    let statements = body.statement_payloads();
    statements[0].match_arm_payloads().expect("statement match")[0]
        .syntax_arm()
        .expect("CST arm")
        .clone()
}

fn first_return_match_expr(body: &vela_syntax::ast::Block) -> &vela_syntax::ast::MatchExpr {
    let statement = body.statements.first().expect("return statement");
    let vela_syntax::ast::StmtKind::Return(Some(value)) = &statement.kind else {
        panic!("expected return statement");
    };
    let vela_syntax::ast::ExprKind::Match(match_expr) = &value.kind else {
        panic!("expected return match expression");
    };
    match_expr
}

fn first_statement_match_expr(body: &vela_syntax::ast::Block) -> &vela_syntax::ast::MatchExpr {
    let statement = body.statements.first().expect("match statement");
    let vela_syntax::ast::StmtKind::Expr(value) = &statement.kind else {
        panic!("expected expression statement");
    };
    let vela_syntax::ast::ExprKind::Match(match_expr) = &value.kind else {
        panic!("expected match expression");
    };
    match_expr
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
