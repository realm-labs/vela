use rstest::rstest;
use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_language_service::{
    DocumentId, LanguageServiceDatabases, LineIndex, Position, SourceFileSnapshot, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use vela_vm::owned_value::OwnedValue;

fn format_source(source: &str) -> String {
    let document_id = DocumentId::from("file:///workspace/scripts/main.vela");
    let config = WorkspaceConfig::workspace([WorkspaceRoot::new("/workspace/scripts")]);
    let files = [SourceFileSnapshot::new(document_id.clone(), source)];
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let edits = databases.document_formatting(&document_id);
    if edits.is_empty() {
        return source.to_owned();
    }
    assert_eq!(
        edits.len(),
        1,
        "document formatting replaces the whole source"
    );
    assert_eq!(edits[0].range().start(), Position::new(0, 0));
    assert_eq!(
        edits[0].range().end(),
        LineIndex::new(source).position(source.len())
    );
    edits[0].new_text().to_owned()
}

#[rstest]
#[case::local_operators(
    include_str!("../../../tests/fixtures/conformance/assignments/local_operators.vela"),
    &[("main", Some(20))]
)]
#[case::index_reads(
    include_str!("../../../tests/fixtures/conformance/assignments/index_reads.vela"),
    &[("main", Some(10))]
)]
#[case::block_tail(
    include_str!("../../../tests/fixtures/conformance/control_flow/block_tail.vela"),
    &[("terminated", None), ("nonterminated", Some(9)), ("defaulted", None)]
)]
#[case::break_continue(
    include_str!("../../../tests/fixtures/conformance/control_flow/break_continue.vela"),
    &[("main", Some(8))]
)]
fn formatting_preserves_engine_execution(
    #[case] source: &str,
    #[case] entries: &[(&str, Option<i64>)],
) {
    let formatted = format_source(source);
    assert_eq!(
        format_source(&formatted),
        formatted,
        "formatter is idempotent"
    );

    for (route, source) in [("original", source), ("formatted", formatted.as_str())] {
        let engine = Engine::builder().build().expect("fresh engine");
        let program = engine
            .compile_source(source)
            .expect("retained-valid source");
        let mut runtime = Runtime::new(engine, program).expect("fresh runtime");
        for &(entry, expected) in entries {
            let value = runtime
                .call(
                    entry,
                    CallArgs::new(),
                    CallOptions::new(100_000, 1 << 20, 32),
                )
                .unwrap_or_else(|error| panic!("{route} {entry}: {error}"));
            assert_eq!(
                runtime.value_to_owned(&value),
                Ok(expected.map_or(OwnedValue::Unit, OwnedValue::i64)),
                "{route} {entry} must independently match the typed oracle"
            );
        }
    }
}
