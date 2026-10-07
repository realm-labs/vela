use super::{TestServer, notify, request, response_value};
use crate::global_state::GlobalStateSnapshot;
use crate::matrix_fixture::{Document, FixtureWorkspace, selection_bodies as oracle};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::path::PathBuf;
use vela_language_service::DocumentId;

struct Harness {
    test_server: TestServer,
    owner: PathBuf,
    root: PathBuf,
    id: i32,
}
impl Harness {
    fn new(disk: &FixtureWorkspace, overlay: Option<&Document>) -> Self {
        let owner = super::support::unique_temp_root("中文 % selection bodies");
        let root = owner.join("workspace");
        disk.materialize(&root).expect("owned disk corpus");
        let mut h = Self {
            test_server: TestServer::new(),
            owner,
            root,
            id: 10,
        };
        let root_uri = h.uri("scripts");
        let response = response_value(request::<r::Initialize>(
            &mut h.test_server,
            1,
            json!({"processId":null,"rootUri":root_uri,"capabilities":{}}),
        ));
        assert_eq!(
            response["result"]["capabilities"]["selectionRangeProvider"],
            true
        );
        for encoding in ["%E4%B8%AD", "%20", "%25"] {
            assert!(h.uri("").contains(encoding));
        }
        if let Some(doc) = overlay {
            h.open(doc);
        }
        h
    }
    fn uri(&self, file: &str) -> String {
        lsp_types::Url::from_file_path(self.root.join(file))
            .expect("owned URI")
            .to_string()
    }
    fn open(&mut self, doc: &Document) {
        let uri = self.uri("scripts/main.vela");
        let _ = notify::<n::DidOpenTextDocument>(
            &mut self.test_server,
            json!({"textDocument":{"uri":uri,"languageId":"vela","version":1,"text":doc.text}}),
        );
    }
    fn change(&mut self, doc: &Document, version: i32) {
        let uri = self.uri("scripts/main.vela");
        let _ = notify::<n::DidChangeTextDocument>(
            &mut self.test_server,
            json!({"textDocument":{"uri":uri,"version":version},"contentChanges":[{"text":doc.text}]}),
        );
    }
    fn close(&mut self) {
        let uri = self.uri("scripts/main.vela");
        let _ = notify::<n::DidCloseTextDocument>(
            &mut self.test_server,
            json!({"textDocument":{"uri":uri}}),
        );
    }
    fn reject_invalid_positions(&mut self, doc: &Document, disk: &FixtureWorkspace) {
        let before = self.test_server.snapshot();
        let before_counts = counts(&before);
        let line = doc.markers["item"].start.line;
        let uri = self.uri("scripts/main.vela");
        for (positions, reason) in [
            (
                json!([{ "line":line,"character":4 }]),
                "LSP position splits a UTF-16 character",
            ),
            (
                json!([{ "line":0,"character":0 },{ "line":line,"character":4 }]),
                "LSP position splits a UTF-16 character",
            ),
            (
                json!([{ "line":line,"character":99999 }]),
                "LSP position character is outside the line",
            ),
            (
                json!([{ "line":doc.markers["file"].end.line + 1,"character":0 }]),
                "LSP position line is outside the document",
            ),
        ] {
            self.id += 1;
            assert_eq!(
                response_value(request::<r::SelectionRangeRequest>(
                    &mut self.test_server,
                    self.id,
                    json!({"textDocument":{"uri":uri},"positions":positions})
                )),
                json!({"jsonrpc":"2.0","id":self.id,"error":{"code":-32600,
                "message":format!("invalid selectionRange params: {reason}")}})
            );
        }
        assert_eq!(counts(&self.test_server.snapshot()), before_counts);
        self.facts(&self.test_server.snapshot(), doc, disk);
    }
    fn facts(&self, snapshot: &GlobalStateSnapshot, doc: &Document, disk: &FixtureWorkspace) {
        let db = snapshot.databases();
        assert_eq!(db.source_db().records().len(), 2);
        for (file, effective) in [
            ("scripts/main.vela", doc),
            ("scripts/helper.vela", &disk.disk["scripts/helper.vela"]),
        ] {
            let id = DocumentId::from(self.uri(file));
            assert_eq!(db.source_db().records()[&id].text(), effective.text);
            assert_eq!(
                db.parse_db()
                    .syntax_parse(&id)
                    .expect("cached owned CST")
                    .syntax_node()
                    .text()
                    .to_string(),
                effective.text
            );
            assert!(
                db.parse_db()
                    .parse_diagnostics(&id)
                    .expect("parsed source")
                    .is_empty()
            );
        }
        assert_eq!(db.schema_db().facts().types().count(), 0);
        assert_eq!(db.schema_db().facts().functions().count(), 0);
        for (file, doc) in &disk.disk {
            assert_eq!(
                std::fs::read_to_string(self.root.join(file)).expect("unchanged actual disk"),
                doc.text
            );
        }
        assert!(!self.root.join("scripts/missing.vela").exists());
    }
    fn check(&mut self, doc: &Document, case: &Value, disk: &FixtureWorkspace) {
        let before = self.test_server.snapshot();
        self.facts(&before, doc, disk);
        let before_counts = counts(&before);
        for _ in 0..3 {
            for (file, positions, wanted) in queries(doc, case, disk) {
                self.id += 1;
                let uri = self.uri(file);
                assert_eq!(
                    response_value(request::<r::SelectionRangeRequest>(
                        &mut self.test_server,
                        self.id,
                        json!({"textDocument":{"uri":uri},"positions":positions})
                    )),
                    json!({"jsonrpc":"2.0","id":self.id,"result":wanted}),
                    "current whole selection envelope {} {file}",
                    case["id"]
                );
            }
        }
        assert_eq!(counts(&self.test_server.snapshot()), before_counts);
        self.facts(&self.test_server.snapshot(), doc, disk);
    }
    fn frozen(
        &mut self,
        snapshot: &GlobalStateSnapshot,
        doc: &Document,
        case: &Value,
        disk: &FixtureWorkspace,
    ) {
        self.facts(snapshot, doc, disk);
        let before = counts(snapshot);
        for _ in 0..3 {
            for (file, positions, wanted) in queries(doc, case, disk) {
                self.id += 1;
                let params = serde_json::from_value(
                    json!({"textDocument":{"uri":self.uri(file)},"positions":positions}),
                )
                .expect("typed selection params");
                assert_eq!(
                    response_value(snapshot.clone().selection_range(self.id.into(), params)),
                    json!({"jsonrpc":"2.0","id":self.id,"result":wanted}),
                    "old complete selection vector {} {file}",
                    case["id"]
                );
            }
        }
        assert_eq!(counts(snapshot), before);
        self.facts(snapshot, doc, disk);
    }
}
fn queries(
    doc: &Document,
    case: &Value,
    disk: &FixtureWorkspace,
) -> Vec<(&'static str, Value, Value)> {
    let helper = &disk.disk["scripts/helper.vela"];
    let helper_queries = oracle::spec(false, false).oracle["helperQueries"].clone();
    vec![
        (
            "scripts/main.vela",
            oracle::positions(doc, &case["queries"], true),
            oracle::expected(doc, &case["queries"], true),
        ),
        (
            "scripts/helper.vela",
            oracle::positions(helper, &helper_queries, true),
            oracle::expected(helper, &helper_queries, true),
        ),
        ("scripts/main.vela", json!([]), json!([])),
        ("scripts/helper.vela", json!([]), json!([])),
        (
            "scripts/missing.vela",
            json!([{ "line":0,"character":0 }]),
            json!([{ "range": {"start":{"line":0,"character":0},"end":{"line":0,"character":0}} }]),
        ),
        ("scripts/missing.vela", json!([]), json!([])),
    ]
}

fn counts(s: &GlobalStateSnapshot) -> (u64, usize, usize, usize) {
    let db = s.databases();
    (
        db.generation().get(),
        db.parse_db().parse_count(),
        db.project_db().rebuild_count(),
        db.hir_db().rebuild_count(),
    )
}
impl Drop for Harness {
    fn drop(&mut self) {
        let owner = self.owner.canonicalize().expect("owned temporary root");
        assert!(
            owner.starts_with(
                std::env::temp_dir()
                    .canonicalize()
                    .expect("temporary directory")
            )
        );
        assert!(
            self.root
                .canonicalize()
                .expect("workspace")
                .starts_with(&owner)
        );
        std::fs::remove_dir_all(&owner).expect("owned cleanup");
    }
}

#[test]
fn lsp_selection_bodies_pin_complete_utf16_closures_callbacks_and_retained_snapshots() {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let spec = oracle::spec(crlf, shifted);
            let disk = FixtureWorkspace::new(&spec).expect("disk corpus");
            let cases = spec.oracle["cases"].as_array().expect("fifty nine cases");
            let mut current = Harness::new(&disk, None);
            current.check(&disk.disk["scripts/main.vela"], &cases[0], &disk);
            current.reject_invalid_positions(&disk.disk["scripts/main.vela"], &disk);
            let mut frozen = vec![(
                current.test_server.snapshot(),
                disk.disk["scripts/main.vela"].clone(),
                cases[0].clone(),
            )];
            for (index, case) in cases.iter().enumerate() {
                let doc = oracle::document(case);
                if index == 0 {
                    current.open(&doc);
                } else {
                    current.change(&doc, index as i32 + 1);
                }
                current.check(&doc, case, &disk);
                let mut fresh = Harness::new(&disk, Some(&doc));
                fresh.check(&doc, case, &disk);
                for (snapshot, doc, case) in &frozen {
                    current.frozen(snapshot, doc, case, &disk);
                }
                frozen.push((current.test_server.snapshot(), doc, case.clone()));
            }
            current.close();
            current.check(&disk.disk["scripts/main.vela"], &cases[0], &disk);
            current.reject_invalid_positions(&disk.disk["scripts/main.vela"], &disk);
            let mut fresh = Harness::new(&disk, None);
            fresh.check(&disk.disk["scripts/main.vela"], &cases[0], &disk);
            for (snapshot, doc, case) in &frozen {
                current.frozen(snapshot, doc, case, &disk);
            }
        }
    }
}
