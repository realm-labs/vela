use super::{TestServer, notify, request, response_value};
use crate::global_state::GlobalStateSnapshot;
use crate::matrix_fixture::{Action, FixtureWorkspace, Spec, selection_lifecycle as oracle};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};
use vela_language_service::DocumentId;

struct Harness {
    test_server: TestServer,
    owner: PathBuf,
    root: PathBuf,
    id: i32,
    versions: BTreeMap<String, i32>,
}
impl Harness {
    fn new(fixture: &FixtureWorkspace) -> Self {
        let owner = super::support::unique_temp_root("中文 % selection lifecycle");
        let root = owner.join("workspace");
        fixture.materialize(&root).expect("owned disk");
        let mut h = Self {
            test_server: TestServer::new(),
            owner,
            root,
            id: 10,
            versions: BTreeMap::new(),
        };
        let root = h.uri("scripts");
        let response = response_value(request::<r::Initialize>(
            &mut h.test_server,
            1,
            json!({"processId":null,"rootUri":root,"capabilities":{}}),
        ));
        assert_eq!(
            response["result"]["capabilities"]["selectionRangeProvider"],
            true
        );
        for part in ["%E4%B8%AD", "%20", "%25"] {
            assert!(h.uri("").contains(part));
        }
        for (file, doc) in &fixture.open {
            h.versions.insert(file.clone(), 1);
            let uri = h.uri(file);
            let _ = notify::<n::DidOpenTextDocument>(
                &mut h.test_server,
                json!({"textDocument":{"uri":uri,"languageId":"vela","version":1,"text":doc.text}}),
            );
        }
        h
    }
    fn uri(&self, file: &str) -> String {
        lsp_types::Url::from_file_path(self.root.join(file))
            .expect("owned encoded URI")
            .to_string()
    }
    fn apply(
        &mut self,
        fixture: &FixtureWorkspace,
        action: &Action,
        spec: &Spec,
        publication: bool,
    ) {
        let uri = self.uri(&action.file);
        let messages = match action.op.as_str() {
            "open" => {
                self.versions.insert(action.file.clone(), 1);
                notify::<n::DidOpenTextDocument>(
                    &mut self.test_server,
                    json!({"textDocument":{"uri":uri,"languageId":"vela","version":1,"text":fixture.open[&action.file].text}}),
                )
            }
            "change" => {
                let version = self.versions.get_mut(&action.file).expect("open version");
                *version += 1;
                notify::<n::DidChangeTextDocument>(
                    &mut self.test_server,
                    json!({"textDocument":{"uri":uri,"version":version},"contentChanges":[{"text":fixture.open[&action.file].text}]}),
                )
            }
            "close" => {
                self.versions.remove(&action.file).expect("close version");
                notify::<n::DidCloseTextDocument>(
                    &mut self.test_server,
                    json!({"textDocument":{"uri":uri}}),
                )
            }
            "save" => {
                std::fs::write(
                    self.root.join(&action.file),
                    &fixture.disk[&action.file].text,
                )
                .expect("owned save");
                notify::<n::DidSaveTextDocument>(
                    &mut self.test_server,
                    json!({"textDocument":{"uri":uri},"text":fixture.disk[&action.file].text}),
                )
            }
            op @ ("write" | "delete") => {
                let path = self.root.join(&action.file);
                let existed = path.exists();
                let event = if op == "delete" {
                    let target = path.canonicalize().expect("owned source exists");
                    assert!(target.starts_with(self.root.canonicalize().expect("owned workspace")));
                    std::fs::remove_file(&target).expect("owned delete");
                    3
                } else {
                    std::fs::write(&path, &fixture.disk[&action.file].text)
                        .expect("owned replacement");
                    if existed { 2 } else { 1 }
                };
                notify::<n::DidChangeWatchedFiles>(
                    &mut self.test_server,
                    json!({"changes":[{"uri":uri,"type":event}]}),
                )
            }
            op => panic!("unknown finite action {op}"),
        };
        self.publications(messages, fixture, spec, &action.file, publication);
    }
    fn publications(
        &self,
        messages: Vec<lsp_server::Message>,
        fixture: &FixtureWorkspace,
        spec: &Spec,
        affected: &str,
        publication: bool,
    ) {
        let mut affected_count = 0;
        let mut owners = std::collections::BTreeSet::new();
        for message in messages {
            let lsp_server::Message::Notification(message) = message else {
                continue;
            };
            if message.method != "textDocument/publishDiagnostics" {
                continue;
            }
            let params = &message.params;
            assert!(
                owners.insert(params["uri"].as_str().expect("owned URI").to_owned()),
                "one publication per URI"
            );
            assert!(params.get("version").is_none());
            let file = spec.oracle["phases"][0]["views"]
                .as_object()
                .expect("owned files")
                .keys()
                .find(|file| params["uri"] == self.uri(file))
                .expect("source-owned diagnostic publication");
            if file == affected {
                affected_count += 1;
            }
            let expected = fixture.document(file).map_or_else(
                || json!([]),
                |doc| {
                    let variant = spec.oracle["variants"]
                        .as_object()
                        .expect("variants")
                        .values()
                        .find(|v| oracle::document(v) == *doc)
                        .expect("literal current source variant");
                    oracle::syntax_diagnostics(variant, true)
                },
            );
            let actual=params["diagnostics"].as_array().expect("diagnostics").iter().filter(|d|{
                let code=d["code"].as_str().unwrap_or("");code=="E_PARSE"||code.starts_with("E_LEX")||code.starts_with("syntax::")
            }).map(|d|json!({"code":d["code"],"message":d["message"],"severity":d["severity"],"source":d["source"],"range":d["range"]})).collect::<Vec<_>>();
            assert_eq!(
                json!(actual),
                expected,
                "exact syntax publication {affected}: {file}"
            );
        }
        assert_eq!(
            affected_count,
            usize::from(publication),
            "exact transition publication cardinality: {affected}"
        );
    }
    fn check(&mut self, fixture: &FixtureWorkspace, spec: &Spec, phase: &Value) {
        let before = counts(&self.test_server.snapshot());
        oracle::assert_state(fixture, spec, phase);
        for (file, id) in phase["views"].as_object().expect("views") {
            let path = self.root.join(file);
            if let Some(doc) = fixture.disk.get(file) {
                assert_eq!(
                    std::fs::read_to_string(path).expect("literal disk"),
                    doc.text
                );
            } else {
                assert!(!path.exists());
            }
            let variant = id.as_str().map(|id| &spec.oracle["variants"][id]);
            let (positions, expected) = oracle::queries(variant, true);
            for _ in 0..3 {
                self.id += 1;
                let uri = self.uri(file);
                let result = response_value(request::<r::SelectionRangeRequest>(
                    &mut self.test_server,
                    self.id,
                    json!({"textDocument":{"uri":uri},"positions":positions}),
                ));
                assert_eq!(
                    result,
                    json!({"jsonrpc":"2.0","id":self.id,"result":expected}),
                    "whole current vector {}: {file}",
                    phase["id"]
                );
                self.id += 1;
                assert_eq!(
                    response_value(request::<r::SelectionRangeRequest>(
                        &mut self.test_server,
                        self.id,
                        json!({"textDocument":{"uri":uri},"positions":[]})
                    )),
                    json!({"jsonrpc":"2.0","id":self.id,"result":[]})
                );
            }
        }
        self.frozen(&self.test_server.snapshot(), spec, phase);
        assert_eq!(counts(&self.test_server.snapshot()), before);
    }
    fn frozen(&self, snapshot: &GlobalStateSnapshot, spec: &Spec, phase: &Value) {
        let db = snapshot.databases();
        let before = counts(snapshot);
        assert_eq!(
            db.source_db().records().len(),
            phase["views"]
                .as_object()
                .expect("views")
                .values()
                .filter(|v| !v.is_null())
                .count()
        );
        assert_eq!(db.schema_db().facts().types().count(), 0);
        assert_eq!(db.schema_db().facts().functions().count(), 0);
        for (file, id) in phase["views"].as_object().expect("views") {
            let uri = self.uri(file);
            let key = DocumentId::from(uri.clone());
            let variant = id.as_str().map(|id| &spec.oracle["variants"][id]);
            if let Some(variant) = variant {
                let doc = oracle::document(variant);
                assert_eq!(db.source_db().records()[&key].text(), doc.text);
                oracle::assert_cst(
                    &db.parse_db()
                        .syntax_parse(&key)
                        .expect("cached owned CST")
                        .syntax_node(),
                    &doc,
                    variant,
                );
                let actual = db
                    .parse_db()
                    .parse_diagnostics(&key)
                    .expect("cached parse errors")
                    .iter()
                    .map(|d| {
                        assert_eq!(format!("{:?}", d.severity), "Error");
                        let span = d.span.expect("owned diagnostic span");
                        assert_eq!(
                            span.source,
                            db.parse_db().source_id(&key).expect("source id")
                        );
                        json!({"code":d.code,"message":d.message,"start":span.start,"end":span.end})
                    })
                    .collect::<Vec<_>>();
                let expected=variant["diagnostics"].as_array().expect("literal errors").iter().map(|d|{let m=doc.markers[d["range"].as_str().expect("range")];json!({"code":d["code"],"message":d["message"],"start":m.start.byte,"end":m.end.byte})}).collect::<Vec<_>>();
                assert_eq!(
                    actual, expected,
                    "exact cached syntax {}: {file}",
                    phase["id"]
                );
            } else {
                assert!(!db.source_db().records().contains_key(&key));
                assert!(db.parse_db().syntax_parse(&key).is_none());
                assert!(db.parse_db().parse_diagnostics(&key).is_none());
            }
            let (positions, expected) = oracle::queries(variant, true);
            for _ in 0..3 {
                let params = serde_json::from_value(
                    json!({"textDocument":{"uri":uri},"positions":positions}),
                )
                .expect("typed params");
                assert_eq!(
                    response_value(snapshot.clone().selection_range(999.into(), params)),
                    json!({"jsonrpc":"2.0","id":999,"result":expected}),
                    "whole frozen vector {}: {file}",
                    phase["id"]
                );
                let params =
                    serde_json::from_value(json!({"textDocument":{"uri":uri},"positions":[]}))
                        .expect("typed empty params");
                assert_eq!(
                    response_value(snapshot.clone().selection_range(1000.into(), params)),
                    json!({"jsonrpc":"2.0","id":1000,"result":[]})
                );
            }
        }
        assert_eq!(counts(snapshot), before);
    }
}
fn counts(snapshot: &GlobalStateSnapshot) -> (u64, usize, usize, usize) {
    let db = snapshot.databases();
    (
        db.generation().get(),
        db.parse_db().parse_count(),
        db.project_db().rebuild_count(),
        db.hir_db().rebuild_count(),
    )
}
impl Drop for Harness {
    fn drop(&mut self) {
        let owner = self.owner.canonicalize().expect("owned root");
        assert!(
            owner.starts_with(
                std::env::temp_dir()
                    .canonicalize()
                    .expect("temporary roots")
            )
        );
        assert!(
            self.root
                .canonicalize()
                .expect("owned workspace")
                .starts_with(&owner)
        );
        std::fs::remove_dir_all(owner).expect("owned cleanup");
    }
}

#[test]
fn watched_repair_clears_syntax_diagnostic_previously_published_on_close() {
    let spec = oracle::spec(false, false);
    let fixture = FixtureWorkspace::new(&spec).expect("disk sources");
    let mut h = Harness::new(&fixture);
    let file = "scripts/helper.vela";
    let uri = h.uri(file);
    let original = fixture.disk[file].text.clone();
    let damaged = "// source 中😀 \n/*中😀*/ fn broken(items:) {}\npub fn apply(items: Array<i64>) { return 1; }\n";
    let _ = notify::<n::DidOpenTextDocument>(
        &mut h.test_server,
        json!({"textDocument":{"uri":uri,"languageId":"vela","version":1,"text":original}}),
    );
    std::fs::write(h.root.join(file), damaged).expect("owned damaged disk");
    let _ = notify::<n::DidChangeWatchedFiles>(
        &mut h.test_server,
        json!({"changes":[{"uri":uri,"type":2}]}),
    );
    let publications = |messages: Vec<lsp_server::Message>| {
        messages
            .into_iter()
            .filter_map(|m| {
                let lsp_server::Message::Notification(n) = m else {
                    return None;
                };
                (n.method == "textDocument/publishDiagnostics" && n.params["uri"] == uri)
                    .then_some(n.params)
            })
            .collect::<Vec<_>>()
    };
    let closed = publications(notify::<n::DidCloseTextDocument>(
        &mut h.test_server,
        json!({"textDocument":{"uri":uri}}),
    ));
    assert_eq!(closed.len(), 1);
    let errors = closed[0]["diagnostics"]
        .as_array()
        .expect("typed diagnostics")
        .iter()
        .filter(|d| d["code"] == "E_PARSE")
        .map(|d| {
            json!({
                "code":d["code"],"message":d["message"],"severity":d["severity"],
                "source":d["source"],"range":d["range"]
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        errors,
        vec![
            json!({"code":"E_PARSE","message":"expected type annotation",
        "severity":1,"source":"vela","range":{"start":{"line":1,"character":18},"end":{"line":1,"character":24}}})
        ]
    );
    std::fs::write(h.root.join(file), original).expect("owned repaired disk");
    let repaired = publications(notify::<n::DidChangeWatchedFiles>(
        &mut h.test_server,
        json!({"changes":[{"uri":uri,"type":2}]}),
    ));
    assert_eq!(
        repaired,
        vec![json!({"uri":uri,"diagnostics":[]})],
        "a repaired closed source must clear its previously published diagnostics exactly once"
    );
}

#[test]
fn lsp_selection_lifecycle_preserves_exact_utf16_cst_vectors_and_diagnostic_publications_through_source_dependency_recovery()
 {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let spec = oracle::spec(crlf, shifted);
            let mut fixture = FixtureWorkspace::new(&spec).expect("disk corpus");
            let mut current = Harness::new(&fixture);
            let mut retained = Vec::new();
            for phase in spec.oracle["phases"].as_array().expect("phases") {
                for (action, row) in oracle::actions(&spec, phase)
                    .iter()
                    .zip(phase["actions"].as_array().expect("actions"))
                {
                    fixture.apply(action).expect("finite action");
                    current.apply(
                        &fixture,
                        action,
                        &spec,
                        row["publication"]
                            .as_bool()
                            .expect("literal publication policy"),
                    );
                }
                current.check(&fixture, &spec, phase);
                let fresh_fixture = oracle::fresh(&spec, phase);
                let mut fresh = Harness::new(&fresh_fixture);
                fresh.check(&fresh_fixture, &spec, phase);
                for (snapshot, old_phase) in &retained {
                    current.frozen(snapshot, &spec, old_phase);
                }
                retained.push((current.test_server.snapshot(), phase.clone()));
            }
        }
    }
}
