use super::fixture_layout::Layout;
use crate::matrix_fixture::{Action, FixtureWorkspace, hover_recovery as oracle};
use crate::{
    LanguageServiceDatabases, SourceFileSnapshot, SourceVersion, Workspace, WorkspaceConfig,
    assemble_project_sources,
};

fn update(
    db: &mut LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    layout: &Layout,
    workspace: &Workspace,
) {
    let config = WorkspaceConfig::from_vela_toml(layout.path(""), &fixture.disk["vela.toml"].text);
    assert!(config.diagnostics.is_empty());
    let sources = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, source)| SourceFileSnapshot::new(layout.uri(file), source.text.as_str()))
        .collect::<Vec<_>>();
    db.update(&assemble_project_sources(
        &config.config,
        &sources,
        &workspace.snapshot(),
    ));
    if let Some(source) = fixture.disk.get("schema.json") {
        db.load_schema_artifact_json(&layout.path("schema.json"), &source.text);
    } else {
        db.mark_schema_missing(layout.path("schema.json"));
    }
}

fn apply(
    workspace: &mut Workspace,
    fixture: &mut FixtureWorkspace,
    layout: &Layout,
    action: &Action,
    version: u64,
) {
    fixture.apply(action).expect("authored lifecycle action");
    let id = layout.uri(&action.file);
    match action.op.as_str() {
        "open" => workspace.open_document(
            id,
            fixture.open[&action.file].text.as_str(),
            SourceVersion::new(version),
        ),
        "change" => workspace.change_document(
            id,
            fixture.open[&action.file].text.as_str(),
            SourceVersion::new(version),
        ),
        "close" => workspace.close_document(&id),
        "write" | "delete" => {}
        other => panic!("unexpected lifecycle action {other}"),
    }
}

fn assert_state(
    db: &LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    layout: &Layout,
    phase: &serde_json::Value,
) {
    for (file, expected) in phase["parseFiles"].as_object().expect("parse files") {
        assert_eq!(
            !db.parse_db()
                .parse_diagnostics(&layout.uri(file))
                .expect("current parse")
                .is_empty(),
            expected.as_bool().expect("errors"),
            "{} {file}",
            phase["id"]
        );
    }
    for file in phase["absentFiles"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .filter(|file| file.ends_with(".vela"))
    {
        assert!(!fixture.disk.contains_key(file));
        assert!(
            !db.source_db().records().contains_key(&layout.uri(file)),
            "{} removed source {file}",
            phase["id"]
        );
    }
    let unavailable = phase["schemaUnavailable"].as_bool().expect("schema state");
    assert_eq!(
        !db.schema_db().diagnostics().is_empty(),
        unavailable,
        "{} schema diagnostics",
        phase["id"]
    );
    let diagnostics = db.diagnostics_for_document(&layout.uri("scripts/main.vela"));
    assert_eq!(
        diagnostics
            .diagnostics()
            .iter()
            .filter(|d| d.code() == Some("schema::unavailable"))
            .count(),
        usize::from(unavailable),
        "{} schema publication model",
        phase["id"]
    );
}

#[test]
fn hover_state_matrix_preserves_current_facts_through_source_and_schema_lifecycle() {
    let mut positions = 0;
    for crlf in [false, true] {
        let spec = oracle::spec("hover-lifecycle", crlf, false);
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let original = fixture.disk.clone();
        let layout = Layout::new(&fixture, false);
        let mut db = LanguageServiceDatabases::new();
        let mut workspace = Workspace::new();
        let mut baseline = None;
        let mut last = Vec::new();
        let mut version = 1;
        assert_eq!(
            spec.oracle["queries"].as_array().expect("queries").len(),
            34
        );
        assert_eq!(spec.oracle["phases"].as_array().expect("phases").len(), 30);
        for phase in spec.oracle["phases"].as_array().expect("phases") {
            let actions: Vec<Action> =
                serde_json::from_value(phase["actions"].clone()).expect("actions");
            for action in actions {
                version += 1;
                apply(&mut workspace, &mut fixture, &layout, &action, version);
            }
            update(&mut db, &fixture, &layout, &workspace);
            assert_state(&db, &fixture, &layout, phase);
            let disk = fixture.disk.clone();
            let queries = oracle::cases(&spec, phase);
            let (count, actual) =
                super::matrix_tests::verify_queries(&db, &fixture, &layout, &queries, false, crlf);
            positions += count;
            last = actual;
            let mut fresh_workspace = Workspace::new();
            for (file, source) in &fixture.open {
                fresh_workspace.open_document(
                    layout.uri(file),
                    source.text.as_str(),
                    SourceVersion::new(1),
                );
            }
            let mut fresh = LanguageServiceDatabases::new();
            update(&mut fresh, &fixture, &layout, &fresh_workspace);
            assert_state(&fresh, &fixture, &layout, phase);
            assert_eq!(
                last,
                super::matrix_tests::verify_queries(
                    &fresh, &fixture, &layout, &queries, false, crlf
                )
                .1,
                "{} incremental/fresh CRLF={crlf}",
                phase["id"]
            );
            assert_eq!(fixture.disk, disk, "queries preserve current disk model");
            baseline.get_or_insert_with(|| last.clone());
        }
        assert_eq!(last, baseline.expect("baseline"), "complete restoration");
        assert_eq!(
            fixture.disk, original,
            "authored disk replacements restore original inputs"
        );
        assert!(fixture.open.is_empty(), "all overlays closed");
    }
    assert_eq!(positions, 3748, "all authored phases and newline variants");
}
