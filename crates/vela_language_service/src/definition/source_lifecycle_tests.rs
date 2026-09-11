use super::matrix_tests::{assert_queries, uri};
use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    LanguageServiceDatabases, SourceFileSnapshot, SourceVersion, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};

#[test]
fn source_navigation_lifecycle_matches_markers_and_fresh_disk_overlay_analysis() {
    for crlf in [false, true] {
        let mut spec = load("navigation-source-lifecycle");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
            for action in &mut spec.actions {
                if let Some(source) = &mut action.source {
                    *source = source.replace('\n', "\r\n");
                }
            }
        }
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let mut workspace = Workspace::new();
        let mut databases = LanguageServiceDatabases::new();
        check(
            &mut databases,
            &workspace,
            &fixture,
            &spec.oracle["initial"],
            "initial",
        );
        for (index, action) in spec.actions.iter().enumerate() {
            fixture.apply(action).expect("action");
            let version = SourceVersion::new(u64::try_from(index + 1).expect("version"));
            match action.op.as_str() {
                "open" => workspace.open_document(
                    uri(&action.file),
                    fixture.open[&action.file].text.as_str(),
                    version,
                ),
                "change" => workspace.change_document(
                    uri(&action.file),
                    fixture.open[&action.file].text.as_str(),
                    version,
                ),
                "close" => workspace.close_document(&uri(&action.file)),
                "write" | "delete" | "save" => {}
                _ => panic!("action"),
            }
            let queries = &spec.oracle["afterEachAction"][index];
            let context = format!("action {index} {} CRLF={crlf}", action.op);
            check(&mut databases, &workspace, &fixture, queries, &context);
            let mut fresh_workspace = Workspace::new();
            for (file, document) in &fixture.open {
                fresh_workspace.open_document(uri(file), document.text.as_str(), version);
            }
            check(
                &mut LanguageServiceDatabases::new(),
                &fresh_workspace,
                &fixture,
                queries,
                &format!("fresh {context}"),
            );
        }
    }
}

fn check(
    databases: &mut LanguageServiceDatabases,
    workspace: &Workspace,
    fixture: &FixtureWorkspace,
    queries: &serde_json::Value,
    context: &str,
) {
    let sources = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, document)| SourceFileSnapshot::new(uri(file), document.text.as_str()))
        .collect::<Vec<_>>();
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    databases.update(&assemble_project_sources(
        &config,
        &sources,
        &workspace.snapshot(),
    ));
    for _ in 0..2 {
        assert_queries(databases, fixture, queries, context);
    }
}
