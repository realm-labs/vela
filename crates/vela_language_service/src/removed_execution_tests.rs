use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    DocumentId, LanguageServiceDatabases, LineIndex, ServiceDiagnosticSeverity, SourceFileSnapshot,
    Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

pub(crate) fn verify_fixture(name: &str) {
    for crlf in [false, true] {
        let mut spec = load(name);
        assert_eq!(spec.oracle["removedSurface"], true);
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("removed surface fixture");
        let files = fixture
            .disk
            .iter()
            .filter(|(path, _)| path.ends_with(".vela"))
            .map(|(path, doc)| {
                SourceFileSnapshot::new(format!("/workspace/{path}"), doc.text.as_str())
            })
            .collect::<Vec<_>>();
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut db = LanguageServiceDatabases::new();
        db.update(&project);
        for case in spec.oracle["cases"].as_array().expect("cases") {
            let file = case["file"].as_str().expect("file");
            let source = fixture.document(file).expect("source");
            let document = DocumentId::from(format!("/workspace/{file}"));
            let diagnostics = db.diagnostics_for_document(&document);
            let code = if case["syntaxError"] == true {
                "syntax::removed_async"
            } else {
                "syntax::removed_task"
            };
            assert!(
                diagnostics
                    .diagnostics()
                    .iter()
                    .any(|d| d.severity() == ServiceDiagnosticSeverity::Error
                        && d.code() == Some(code)),
                "{name}: {case}: {diagnostics:?}"
            );
            if case["syntaxError"] == true {
                assert!(
                    !vela_syntax::parse::parse_source(&source.text)
                        .diagnostics()
                        .is_empty(),
                    "removed syntax must not parse successfully: {file}"
                );
            }
            if case["signatureNull"] == true {
                let index = LineIndex::new(&source.text);
                let position = index.position(source.markers["cursor"].start.byte);
                assert!(
                    db.signature_help(&document, position).is_none(),
                    "removed operation must have no signature: {file}"
                );
                assert!(
                    db.hover(
                        &document,
                        index.position(source.markers["removed"].start.byte)
                    )
                    .is_none(),
                    "removed operation must have no builtin hover: {file}"
                );
                let completions = db.completion_items(
                    &document,
                    index.position(source.markers["removed"].start.byte),
                );
                assert!(
                    completions
                        .items()
                        .iter()
                        .all(|item| !matches!(item.label(), "spawn_scoped" | "spawn_scoped_then")),
                    "removed task completions: {file}"
                );
            }
        }
    }
}
