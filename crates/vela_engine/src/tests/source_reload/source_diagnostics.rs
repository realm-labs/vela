use super::*;

fn frontend_diagnostics(
    error: crate::reload::EngineHotReloadSourceError,
) -> Vec<vela_common::Diagnostic> {
    let EngineHotReloadSourceErrorKind::Source(crate::source::EngineSourceError {
        kind: EngineSourceErrorKind::Frontend(error),
    }) = error.kind
    else {
        panic!("expected front-end source error, got {error:?}");
    };
    error.into_diagnostics()
}

#[test]
fn duplicate_script_field_ids_are_engine_source_diagnostics() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            r#"
struct Reward {
    #[id(101)]
    item_id: String
}
fn main() { return 1; }
"#,
        )
        .expect("initial script schema should compile");
    let error = engine
        .compile_hot_reload_update(
            &initial,
            r#"
struct Reward {
    #[id(101)]
    item_id: String
    #[id(101)]
    count: i64
}
fn main() { return 1; }
"#,
        )
        .expect_err("duplicate stable ids should fail source ingestion");

    assert!(
        frontend_diagnostics(error)
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_field_id"))
    );
}

#[test]
fn syntax_rejection_preserves_source_span_message_and_label_in_engine_error() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial("fn main(value) { return value; }")
        .expect("initial source should compile");
    let error = engine
        .compile_hot_reload_update(&initial, "fn main(value: Player<i64>) { return value; }")
        .expect_err("generic script type hint should fail source ingestion");
    let diagnostics = frontend_diagnostics(error);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("syntax::generic_type_hint"))
        .expect("generic type-hint diagnostic");

    assert_eq!(
        diagnostic.span.expect("source span").source,
        SourceId::new(1)
    );
    assert!(
        diagnostic.message.contains(
            "only builtin container, Option, and Result type hints support type arguments"
        )
    );
    assert!(diagnostic.labels.iter().any(|label| label.message
        == "use a builtin parameterized type hint or remove these type arguments"));
}
