use super::source_lifecycle;
use crate::matrix_fixture::{FixtureWorkspace, Spec, source_lifecycle_action};
use crate::tests::{TestServer, notification_values, notify};
use lsp_types::notification as n;
use serde_json::{Value, json};
use std::path::Path;

pub(super) struct State {
    pub fixture: FixtureWorkspace,
    version: i32,
    diagnostic_file: String,
    publication: Value,
}

impl State {
    pub fn new(spec: &Spec, root: &Path, messages: &[Value]) -> Self {
        let mut fixture = FixtureWorkspace::new(spec).expect("overlay fixture");
        let file = spec.oracle["sourceDiagnosticFile"]
            .as_str()
            .expect("control");
        fixture
            .open
            .insert(file.to_owned(), fixture.disk[file].clone());
        let uri = lsp_types::Url::from_file_path(root.join(file)).expect("control URI");
        let publications = messages
            .iter()
            .filter(|m| {
                m["method"] == "textDocument/publishDiagnostics"
                    && m["params"]["uri"] == uri.as_str()
            })
            .collect::<Vec<_>>();
        assert_eq!(publications.len(), 1, "initial control publication");
        Self {
            fixture,
            version: 1,
            diagnostic_file: file.to_owned(),
            publication: publications[0].clone(),
        }
    }

    pub fn apply(&mut self, server: &mut TestServer, root: &Path, phase: &Value, crlf: bool) {
        let Some(action) = source_lifecycle_action(phase, crlf) else {
            source_lifecycle::assert_diagnostics(
                std::slice::from_ref(&self.publication),
                root,
                phase,
                &self.diagnostic_file,
            );
            return;
        };
        let path = root.join(&action.file);
        let existed = path.exists();
        self.fixture.apply(&action).expect("overlay action");
        self.version += 1;
        let uri = lsp_types::Url::from_file_path(&path).expect("URI");
        let messages = match action.op.as_str() {
            "open" => notify::<n::DidOpenTextDocument>(
                server,
                json!({"textDocument":{"uri":uri,"languageId":"vela","version":self.version,"text":self.fixture.open[&action.file].text}}),
            ),
            "change" => notify::<n::DidChangeTextDocument>(
                server,
                json!({"textDocument":{"uri":uri,"version":self.version},"contentChanges":[{"text":self.fixture.open[&action.file].text}]}),
            ),
            "close" => {
                notify::<n::DidCloseTextDocument>(server, json!({"textDocument":{"uri":uri}}))
            }
            "write" | "save" | "delete" => {
                let kind = if action.op == "delete" {
                    std::fs::remove_file(&path).expect("delete dependency");
                    3
                } else {
                    std::fs::write(&path, &self.fixture.disk[&action.file].text)
                        .expect("write dependency");
                    if existed { 2 } else { 1 }
                };
                if action.op == "save" {
                    // Save sync is not advertised; the watcher advances the disk snapshot.
                    let _ = notify::<n::DidSaveTextDocument>(
                        server,
                        json!({"textDocument":{"uri":uri}}),
                    );
                }
                notify::<n::DidChangeWatchedFiles>(
                    server,
                    json!({"changes":[{"uri":uri,"type":kind}]}),
                )
            }
            _ => panic!("unsupported source action"),
        };
        let messages = notification_values(messages);
        if let Some(expected) = phase["overlayParseErrors"].as_bool() {
            let publications: Vec<_> = messages
                .iter()
                .filter(|m| {
                    m["method"] == "textDocument/publishDiagnostics"
                        && m["params"]["uri"] == uri.as_str()
                })
                .collect();
            assert_eq!(publications.len(), 1, "overlay publication: {phase}");
            let diagnostics = publications[0]["params"]["diagnostics"]
                .as_array()
                .expect("overlay diagnostics");
            assert_eq!(
                diagnostics.iter().any(|d| d["code"] == "E_PARSE"),
                expected,
                "{phase}: {diagnostics:?}"
            );
        }
        let uri =
            lsp_types::Url::from_file_path(root.join(&self.diagnostic_file)).expect("control URI");
        let publications = messages
            .iter()
            .filter(|m| {
                m["method"] == "textDocument/publishDiagnostics"
                    && m["params"]["uri"] == uri.as_str()
            })
            .collect::<Vec<_>>();
        assert!(
            publications.len() <= 1,
            "one control publication per transition"
        );
        if let Some(publication) = publications.first() {
            self.publication = (*publication).clone();
        }
        // Unchanged imports need not be republished. Retain the actual client
        // publication so a missing loss/recovery update still fails the oracle.
        source_lifecycle::assert_diagnostics(
            std::slice::from_ref(&self.publication),
            root,
            phase,
            &self.diagnostic_file,
        );
        assert_eq!(
            std::fs::read_to_string(&path).ok(),
            self.fixture
                .disk
                .get(&action.file)
                .map(|document| document.text.clone()),
            "disk after {}",
            action.op
        );
    }

    pub fn fresh_server(&self, root: &Path, capabilities: &Value) -> TestServer {
        let mut server = source_lifecycle::fresh_server(root, capabilities);
        for (file, document) in &self.fixture.open {
            let _ = notify::<n::DidOpenTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":lsp_types::Url::from_file_path(root.join(file)).expect("URI"),"languageId":"vela","version":1,"text":document.text}}),
            );
        }
        server
    }
}
