use super::*;

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
    let missing_source_literal_payload =
        body_payloads::CompilerPatternPayload::missing_child_payload_context(
            first_return_match_pattern_syntax(&cst_literal_payload.body),
            first_return_match_fallback_pattern(legacy_path_payload.body.fallback()),
        );
    assert_eq!(missing_source_literal_payload.syntax_pattern_kind(), None);
    assert_eq!(missing_source_literal_payload.syntax_literal(), None);

    let path_payload = body_payloads::CompilerPatternPayload::syntax(
        first_return_match_pattern_syntax(&cst_path_payload.body),
        first_return_match_fallback_pattern(legacy_binding_payload.body.fallback()),
    );
    assert_eq!(
        path_payload.syntax_path_segments().as_deref(),
        Some(&["State".to_owned(), "Waiting".to_owned()][..])
    );
    let missing_source_path_payload =
        body_payloads::CompilerPatternPayload::missing_child_payload_context(
            first_return_match_pattern_syntax(&cst_path_payload.body),
            first_return_match_fallback_pattern(legacy_binding_payload.body.fallback()),
        );
    assert_eq!(missing_source_path_payload.syntax_path_segments(), None);

    let binding_payload = body_payloads::CompilerPatternPayload::syntax(
        first_return_match_pattern_syntax(&cst_binding_payload.body),
        first_return_match_fallback_pattern(legacy_literal_payload.body.fallback()),
    );
    assert_eq!(
        binding_payload.syntax_binding_name().as_deref(),
        Some("current")
    );
    let missing_source_binding_payload =
        body_payloads::CompilerPatternPayload::missing_child_payload_context(
            first_return_match_pattern_syntax(&cst_binding_payload.body),
            first_return_match_fallback_pattern(legacy_literal_payload.body.fallback()),
        );
    assert_eq!(missing_source_binding_payload.syntax_binding_name(), None);
}
