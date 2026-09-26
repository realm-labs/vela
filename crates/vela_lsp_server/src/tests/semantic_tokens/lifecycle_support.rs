use crate::matrix_fixture::{
    Action, FixtureWorkspace, Spec, load, parse_markers, semantic_tokens as oracle,
};
use crate::tests::{TestServer, notification_values, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use vela_language_service::{DocumentId, LanguageServiceDatabases};

pub(super) struct Driver {
    pub live: TestServer,
    pub fixture: FixtureWorkspace,
    pub spec: Spec,
    pub root: PathBuf,
    pub crlf: bool,
    legend: Value,
    temp: PathBuf,
    versions: BTreeMap<String, i32>,
}
impl Driver {
    pub fn new(name: &str, crlf: bool) -> Self {
        let mut spec = load(name);
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let temp = crate::tests::support::unique_temp_root(name);
        let root = temp.join("中文 % token workspace");
        fixture.materialize(&root).expect("workspace");
        let (live, legend) = start(&root, &fixture);
        Self {
            live,
            fixture,
            spec,
            root,
            crlf,
            legend,
            temp,
            versions: BTreeMap::new(),
        }
    }
    pub fn uri(&self, file: &str) -> String {
        uri(&self.root, file)
    }
    pub fn apply(&mut self, state: &Value) {
        if state["action"].is_null() {
            return;
        }
        let mut action: Action = serde_json::from_value(state["action"].clone()).expect("action");
        if self.crlf
            && let Some(source) = &mut action.source
        {
            *source = source.replace('\n', "\r\n");
        }
        self.fixture.apply(&action).expect("fixture action");
        let file_uri = self.uri(&action.file);
        let messages = match action.op.as_str() {
            "open" => {
                self.versions.insert(action.file.clone(), 1);
                notify::<n::DidOpenTextDocument>(
                    &mut self.live,
                    json!({"textDocument":{"uri":file_uri,"languageId":"vela","version":1,"text":self.fixture.open[&action.file].text}}),
                )
            }
            "change" => {
                let version = self.versions.get_mut(&action.file).expect("open version");
                *version += 1;
                notify::<n::DidChangeTextDocument>(
                    &mut self.live,
                    json!({"textDocument":{"uri":file_uri,"version":version},"contentChanges":[{"text":self.fixture.open[&action.file].text}]}),
                )
            }
            "close" => {
                self.versions.remove(&action.file).expect("open version");
                notify::<n::DidCloseTextDocument>(
                    &mut self.live,
                    json!({"textDocument":{"uri":file_uri}}),
                )
            }
            "save" => {
                std::fs::write(
                    self.root.join(&action.file),
                    &self.fixture.disk[&action.file].text,
                )
                .expect("save");
                let mut messages = notify::<n::DidSaveTextDocument>(
                    &mut self.live,
                    json!({"textDocument":{"uri":file_uri}}),
                );
                // Save sync is not advertised; notify the disk watcher as the client does.
                messages.extend(notify::<n::DidChangeWatchedFiles>(
                    &mut self.live,
                    json!({"changes":[{"uri":file_uri,"type":2}]}),
                ));
                messages
            }
            "write" | "delete" => {
                let path = self.root.join(&action.file);
                let existed = path.exists();
                let change = if action.op == "delete" {
                    std::fs::remove_file(path).expect("delete");
                    3
                } else {
                    std::fs::write(path, &self.fixture.disk[&action.file].text).expect("write");
                    if existed { 2 } else { 1 }
                };
                notify::<n::DidChangeWatchedFiles>(
                    &mut self.live,
                    json!({"changes":[{"uri":file_uri,"type":change}]}),
                )
            }
            _ => panic!("unknown action"),
        };
        let messages = notification_values(messages);
        for message in &messages {
            assert!(message.get("error").is_none(), "{state}: {message}");
        }
        if action.file == "scripts/main.vela" && matches!(action.op.as_str(), "open" | "change") {
            let publications: Vec<_> = messages
                .iter()
                .filter(|message| {
                    message["method"] == "textDocument/publishDiagnostics"
                        && message["params"]["uri"] == file_uri
                })
                .collect();
            assert_eq!(publications.len(), 1, "{state}");
            assert_eq!(
                publications[0]["params"]["diagnostics"]
                    .as_array()
                    .expect("diagnostics")
                    .iter()
                    .any(|diagnostic| diagnostic["code"] == "E_PARSE"),
                state["parseErrors"] == true,
                "{}",
                state["id"]
            );
        }
    }
    pub fn assert_sources(&self, state: &Value) {
        for (key, actual) in [
            ("diskSources", &self.fixture.disk),
            ("openSources", &self.fixture.open),
        ] {
            for (file, expected) in state[key].as_object().expect("sources") {
                let expected = expected.as_str().map(|source| {
                    parse_markers(&source.replace('\n', if self.crlf { "\r\n" } else { "\n" }))
                        .expect("expected source")
                        .text
                });
                assert_eq!(
                    actual.get(file).map(|document| &document.text),
                    expected.as_ref(),
                    "{} {key} {file}",
                    state["id"]
                );
                if key == "diskSources" {
                    assert_eq!(
                        std::fs::read_to_string(self.root.join(file)).ok(),
                        expected,
                        "actual disk {file}"
                    );
                }
            }
        }
        let snapshot = self.live.snapshot();
        let records = snapshot.databases().source_db().records();
        for file in state["diskSources"]
            .as_object()
            .expect("disk sources")
            .keys()
        {
            assert_eq!(
                records
                    .get(&DocumentId::from(self.uri(file)))
                    .map(|record| record.text()),
                self.fixture
                    .document(file)
                    .map(|document| document.text.as_str()),
                "{} effective {file}",
                state["id"]
            );
        }
    }
    pub fn check(
        &mut self,
        queries: &Value,
        previous: &BTreeMap<String, Value>,
    ) -> BTreeMap<String, Value> {
        let (mut fresh, legend) = start(&self.root, &self.fixture);
        assert_eq!(legend, self.legend);
        let mut results = BTreeMap::new();
        for (file, query) in queries.as_object().expect("queries") {
            let source = query["source"]
                .as_str()
                .expect("query source")
                .replace('\n', if self.crlf { "\r\n" } else { "\n" });
            let document = parse_markers(&source).expect("query document");
            assert_eq!(
                document.text,
                self.fixture
                    .document(file)
                    .map_or("", |document| document.text.as_str())
            );
            let expected = oracle::expected(&document, &query["tokens"], true);
            let uri = self.uri(file);
            let counts = counters(self.live.snapshot().databases());
            let full = super::support::full(&mut self.live, &uri);
            oracle::assert_stream(
                &super::support::rows(&document, &full["data"], &self.legend),
                &expected,
            );
            assert_eq!(super::support::full(&mut self.live, &uri), full);
            assert_eq!(super::support::full(&mut fresh, &uri), full, "fresh {file}");
            assert_eq!(
                super::support::delta(&mut self.live, &uri, &full["resultId"])["edits"],
                json!([])
            );
            if let Some(prior) = previous.get(file) {
                self.check_delta(file, prior, &full);
            }
            for token in &expected {
                let start = json!({"line":token.line,"character":token.column});
                let end = json!({"line":token.line,"character":token.column+token.length});
                let params = json!({"textDocument":{"uri":uri},"range":{"start":start,"end":end}});
                let selected = range(&mut self.live, &params);
                oracle::assert_stream(
                    &super::support::rows(&document, &selected["data"], &self.legend),
                    std::slice::from_ref(token),
                );
                assert_eq!(range(&mut self.live, &params), selected);
                assert_eq!(range(&mut fresh, &params), selected);
                let empty = range(
                    &mut self.live,
                    &json!({"textDocument":{"uri":uri},"range":{"start":end,"end":end}}),
                );
                assert_eq!(empty["data"], json!([]));
            }
            for line in 0..document.text.lines().count() {
                let selected = range(
                    &mut self.live,
                    &json!({"textDocument":{"uri":uri},"range":{"start":{"line":line,"character":0},"end":{"line":line+1,"character":0}}}),
                );
                let expected = expected
                    .iter()
                    .filter(|row| row.line == line)
                    .cloned()
                    .collect::<Vec<_>>();
                oracle::assert_stream(
                    &super::support::rows(&document, &selected["data"], &self.legend),
                    &expected,
                );
            }
            let empty = range(
                &mut self.live,
                &json!({"textDocument":{"uri":uri},"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}}}),
            );
            assert_eq!(empty["data"], json!([]));
            assert_eq!(
                counters(self.live.snapshot().databases()),
                counts,
                "queries cannot rebuild"
            );
            results.insert(file.clone(), full);
        }
        results
    }

    pub fn check_delta(&mut self, file: &str, previous: &Value, current: &Value) {
        let uri = self.uri(file);
        let delta = super::support::delta(&mut self.live, &uri, &previous["resultId"]);
        assert_eq!(
            super::support::apply_delta(&previous["data"], &delta),
            current["data"],
            "actual applied delta {file}"
        );
        assert_eq!(delta["resultId"], current["resultId"]);
    }
}
impl Drop for Driver {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.temp).expect("cleanup own fixture");
    }
}
pub(super) fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("URI")
        .to_string()
}
fn counters(db: &LanguageServiceDatabases) -> (usize, usize, usize, u64) {
    (
        db.parse_db().parse_count(),
        db.project_db().rebuild_count(),
        db.hir_db().rebuild_count(),
        db.generation().get(),
    )
}
fn range(server: &mut TestServer, params: &Value) -> Value {
    let response = response_value(request::<r::SemanticTokensRangeRequest>(
        server,
        4,
        params.clone(),
    ));
    assert!(response.get("error").is_none(), "{response}");
    response["result"].clone()
}
fn start(root: &Path, fixture: &FixtureWorkspace) -> (TestServer, Value) {
    let (mut live, legend) = super::support::start(root);
    for (file, source) in &fixture.open {
        let _ = notify::<n::DidOpenTextDocument>(
            &mut live,
            json!({"textDocument":{"uri":uri(root,file),"languageId":"vela","version":1,"text":source.text}}),
        );
    }
    (live, legend)
}
