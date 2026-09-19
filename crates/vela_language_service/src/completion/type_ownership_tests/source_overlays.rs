use super::{Layout, update_with_workspace};
use crate::matrix_fixture::{FixtureWorkspace, Spec, source_lifecycle_action};
use crate::{LanguageServiceDatabases, SourceVersion, Workspace};
use serde_json::Value;

pub(super) struct State {
    pub fixture: FixtureWorkspace,
    workspace: Workspace,
    version: u64,
}

impl State {
    pub fn new(spec: &Spec, layout: &Layout) -> Self {
        let mut fixture = FixtureWorkspace::new(spec).expect("overlay fixture");
        let mut workspace = Workspace::new();
        let file = spec.oracle["sourceDiagnosticFile"]
            .as_str()
            .expect("control");
        fixture
            .open
            .insert(file.to_owned(), fixture.disk[file].clone());
        workspace.open_document(
            layout.uri(file),
            fixture.disk[file].text.as_str(),
            SourceVersion::new(1),
        );
        Self {
            fixture,
            workspace,
            version: 1,
        }
    }

    pub fn apply(
        &mut self,
        db: &mut LanguageServiceDatabases,
        layout: &Layout,
        phase: &Value,
        crlf: bool,
    ) {
        if let Some(action) = source_lifecycle_action(phase, crlf) {
            self.fixture.apply(&action).expect("overlay action");
            self.version += 1;
            let uri = layout.uri(&action.file);
            let version = SourceVersion::new(self.version);
            match action.op.as_str() {
                "open" => self.workspace.open_document(
                    uri,
                    self.fixture.open[&action.file].text.as_str(),
                    version,
                ),
                "change" => self.workspace.change_document(
                    uri,
                    self.fixture.open[&action.file].text.as_str(),
                    version,
                ),
                "close" => self.workspace.close_document(&uri),
                "save" | "write" | "delete" => {}
                _ => panic!("unsupported source action"),
            }
        }
        update_with_workspace(db, &self.fixture, layout, &self.workspace);
        if let Some(expected) = phase["overlayParseErrors"].as_bool() {
            let file = phase["overlayFile"].as_str().expect("diagnostic file");
            let diagnostics = db.diagnostics_for_document(&layout.uri(file));
            assert_eq!(
                diagnostics
                    .diagnostics()
                    .iter()
                    .any(|d| d.code() == Some("E_PARSE")),
                expected,
                "{phase}: {diagnostics:?}"
            );
            let syntax = db
                .parse_db()
                .parse_diagnostics(&layout.uri(file))
                .expect("parsed overlay");
            assert_eq!(!syntax.is_empty(), expected, "recovery syntax: {syntax:?}");
        }
    }
}
