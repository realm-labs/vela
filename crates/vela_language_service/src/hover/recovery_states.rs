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
}

fn schema(db: &mut LanguageServiceDatabases, fixture: &FixtureWorkspace, layout: &Layout) {
    if let Some(source) = fixture.disk.get("schema.json") {
        db.load_schema_artifact_json(&layout.path("schema.json"), &source.text);
        assert!(db.schema_db().diagnostics().is_empty());
    } else {
        db.mark_schema_missing(layout.path("schema.json"));
    }
}

pub(super) fn verify(expected_positions: usize) {
    let mut positions = 0;
    for crlf in [false, true] {
        for missing_schema in [false, true] {
            let spec = oracle::spec("hover-s9-recovery", crlf, missing_schema);
            let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
            let disk = fixture.disk.clone();
            let layout = Layout::new(&fixture, false);
            let mut db = LanguageServiceDatabases::new();
            let mut workspace = Workspace::new();
            update(&mut db, &fixture, &layout, &workspace);
            schema(&mut db, &fixture, &layout);
            let mut version = 1;
            let mut initial = None;
            let mut last = Vec::new();
            for phase in spec.oracle["phases"].as_array().expect("phases") {
                let actions: Vec<Action> =
                    serde_json::from_value(phase["actions"].clone()).expect("actions");
                for action in actions {
                    fixture.apply(&action).expect("overlay action");
                    version += 1;
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
                        other => panic!("unexpected recovery action {other}"),
                    }
                }
                update(&mut db, &fixture, &layout, &workspace);
                let queries = oracle::cases(&spec, phase);
                let (count, actual) = super::matrix_tests::verify_queries(
                    &db,
                    &fixture,
                    &layout,
                    &queries,
                    missing_schema,
                    crlf,
                );
                positions += count;
                last = actual;
                for (file, expected) in phase["parseFiles"].as_object().expect("parse files") {
                    assert_eq!(
                        !db.parse_db()
                            .parse_diagnostics(&layout.uri(file))
                            .expect("current parse")
                            .is_empty(),
                        expected.as_bool().expect("error presence"),
                        "{} {file} parser recovery",
                        phase["id"]
                    );
                }
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
                schema(&mut fresh, &fixture, &layout);
                assert_eq!(
                    last,
                    super::matrix_tests::verify_queries(
                        &fresh,
                        &fixture,
                        &layout,
                        &queries,
                        missing_schema,
                        crlf
                    )
                    .1,
                    "{} incremental/fresh CRLF={crlf} missing={missing_schema}",
                    phase["id"]
                );
                initial.get_or_insert_with(|| last.clone());
                assert_eq!(
                    fixture.disk, disk,
                    "unsaved changes preserve disk at {}",
                    phase["id"]
                );
            }
            assert_eq!(
                last,
                initial.expect("baseline"),
                "close restores disk hover"
            );
            assert!(fixture.open.is_empty(), "all overlays closed");
        }
    }
    assert_eq!(
        positions, expected_positions,
        "all phases/newlines/schema modes"
    );
}
