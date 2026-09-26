use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::matrix_fixture::{
    FixtureWorkspace, Spec, load, parse_markers, semantic_tokens as oracle,
};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
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
    pub fn new(crlf: bool) -> Self {
        let mut spec = load("semantic-token-incremental");
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let temp = crate::tests::support::unique_temp_root("semantic-incremental");
        let root = temp.join("中文 % token workspace");
        fixture.materialize(&root).expect("workspace");
        let (live, legend) = start(&root, &fixture);
        let versions = fixture
            .disk
            .keys()
            .filter(|file| file.ends_with(".vela"))
            .map(|file| (file.clone(), 1))
            .collect();
        Self {
            live,
            fixture,
            spec,
            root,
            crlf,
            legend,
            temp,
            versions,
        }
    }

    pub fn change(&mut self, index: usize) {
        let step = &self.spec.oracle["steps"][index];
        let file = step["file"].as_str().expect("file");
        let id = DocumentId::from(self.uri(file));
        let before = self.live.snapshot();
        let before = before.databases();
        let key = before.source_db().records()[&id].module_key();
        let fingerprint = before
            .parse_db()
            .module_fingerprint(key)
            .expect("fingerprint");
        let source = step["source"]
            .as_str()
            .expect("source")
            .replace('\n', if self.crlf { "\r\n" } else { "\n" });
        self.fixture
            .disk
            .insert(file.to_owned(), parse_markers(&source).expect("source"));
        let version = i32::try_from(index + 2).expect("version");
        self.versions.insert(file.to_owned(), version);
        let _ = notify::<n::DidChangeTextDocument>(
            &mut self.live,
            json!({"textDocument":{"uri":uri(&self.root,file),"version":version},"contentChanges":[{"text":self.fixture.disk[file].text}]}),
        );
        let after = self.live.snapshot();
        let after = after.databases();
        assert_eq!(
            after.parse_db().parse_count(),
            before.parse_db().parse_count() + 1,
            "{step}"
        );
        assert_eq!(
            after.hir_db().rebuild_count(),
            before.hir_db().rebuild_count() + 1
        );
        assert_eq!(
            after.project_db().rebuild_count() - before.project_db().rebuild_count(),
            usize::from(step["declarationChanged"] == true || step["importChanged"] == true)
        );
        let current = after
            .parse_db()
            .module_fingerprint(key)
            .expect("fingerprint");
        assert_eq!(
            current.declaration() != fingerprint.declaration(),
            step["declarationChanged"] == true,
            "{step}"
        );
        assert_eq!(
            current.import() != fingerprint.import(),
            step["importChanged"] == true,
            "{step}"
        );
        let invalidated = after
            .analysis_db()
            .invalidated_modules()
            .iter()
            .map(|key| key.path.join())
            .collect::<Vec<_>>();
        assert_eq!(json!(invalidated), step["invalidated"], "{step}");
        assert!(after.generation() > before.generation());
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
            assert_eq!(document.text, self.fixture.disk[file].text);
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

    pub fn params(&self, method: &str, previous: &Value) -> Value {
        let uri = self.uri("scripts/main.vela");
        let mut params = json!({"textDocument":{"uri":uri}});
        match method {
            "textDocument/semanticTokens/full/delta" => {
                params["previousResultId"] = previous["resultId"].clone()
            }
            "textDocument/semanticTokens/range" => {
                params["range"] = json!({"start":{"line":0,"character":0},"end":{"line":self.fixture.disk["scripts/main.vela"].text.lines().count(),"character":0}})
            }
            "textDocument/semanticTokens/full" => (),
            _ => panic!("unexpected method"),
        }
        params
    }

    pub fn assert_current_response(
        &mut self,
        method: &str,
        previous: &Value,
        result: &Value,
        queries: &Value,
    ) {
        let query = &queries["scripts/main.vela"];
        let source = query["source"]
            .as_str()
            .expect("query source")
            .replace('\n', if self.crlf { "\r\n" } else { "\n" });
        let document = parse_markers(&source).expect("query document");
        assert_eq!(document.text, self.fixture.disk["scripts/main.vela"].text);
        let uri = self.uri("scripts/main.vela");
        let counts = counters(self.live.snapshot().databases());
        let full = super::support::full(&mut self.live, &uri);
        oracle::assert_stream(
            &super::support::rows(&document, &full["data"], &self.legend),
            &oracle::expected(&document, &query["tokens"], true),
        );
        let (mut fresh, _) = start(&self.root, &self.fixture);
        assert_eq!(super::support::full(&mut fresh, &uri), full);
        if method == "textDocument/semanticTokens/full/delta" {
            assert_eq!(
                super::support::apply_delta(&previous["data"], result),
                full["data"]
            );
            assert_eq!(result["resultId"], full["resultId"]);
        } else {
            assert_eq!(*result, full);
        }
        assert_eq!(counters(self.live.snapshot().databases()), counts);
    }

    pub fn reject_old_versions(&mut self, file: &str) {
        let version = self.versions[file];
        let uri = self.uri(file);
        let counts = counters(self.live.snapshot().databases());
        let original = super::support::full(&mut self.live, &uri);
        for version in [version - 1, version] {
            for ranged in [false, true] {
                let change = if ranged {
                    json!({"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"text":"fn poison() {}\n"})
                } else {
                    json!({"text":"fn poison() {}\n"})
                };
                assert!(notify::<n::DidChangeTextDocument>(&mut self.live, json!({"textDocument":{"uri":uri,"version":version},"contentChanges":[change]})).is_empty());
                assert_eq!(super::support::full(&mut self.live, &uri), original);
                assert_eq!(counters(self.live.snapshot().databases()), counts);
            }
        }
    }

    pub fn uri(&self, file: &str) -> String {
        uri(&self.root, file)
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
    for (file, source) in fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
    {
        let _ = notify::<n::DidOpenTextDocument>(
            &mut live,
            json!({"textDocument":{"uri":uri(root,file),"languageId":"vela","version":1,"text":source.text}}),
        );
    }
    (live, legend)
}
