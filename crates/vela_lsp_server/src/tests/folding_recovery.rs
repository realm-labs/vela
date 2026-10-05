use super::{TestServer, notify, request, response_value};
use crate::global_state::GlobalStateSnapshot;
use crate::matrix_fixture::{Document, FixtureWorkspace, folding_recovery as oracle};
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
    fn new(disk: &FixtureWorkspace, overlay: Option<(&Document, &Value)>) -> Self {
        let owner = super::support::unique_temp_root("中文 % folding recovery");
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
            response["result"]["capabilities"]["foldingRangeProvider"],
            true
        );
        for encoding in ["%E4%B8%AD", "%20", "%25"] {
            assert!(h.uri("").contains(encoding));
        }
        if let Some((doc, case)) = overlay {
            h.open(doc, case);
        }
        h
    }
    fn uri(&self, file: &str) -> String {
        lsp_types::Url::from_file_path(self.root.join(file))
            .expect("owned URI")
            .to_string()
    }
    fn open(&mut self, doc: &Document, case: &Value) {
        let uri = self.uri("scripts/main.vela");
        let messages = notify::<n::DidOpenTextDocument>(
            &mut self.test_server,
            json!({"textDocument":{"uri":uri,"languageId":"vela","version":1,"text":doc.text}}),
        );

        self.diagnostics(messages, case);
    }
    fn change(&mut self, doc: &Document, version: i32, case: &Value) {
        let uri = self.uri("scripts/main.vela");
        let messages = notify::<n::DidChangeTextDocument>(
            &mut self.test_server,
            json!({"textDocument":{"uri":uri,"version":version},"contentChanges":[{"text":doc.text}]}),
        );

        self.diagnostics(messages, case);
    }
    fn close(&mut self, case: &Value) {
        let uri = self.uri("scripts/main.vela");
        let messages = notify::<n::DidCloseTextDocument>(
            &mut self.test_server,
            json!({"textDocument":{"uri":uri}}),
        );

        self.diagnostics(messages, case);
    }
    fn diagnostics(&self, messages: Vec<lsp_server::Message>, case: &Value) {
        let uri = self.uri("scripts/main.vela");
        let receipts = messages
            .into_iter()
            .filter_map(|message| match message {
                lsp_server::Message::Notification(n)
                    if n.method == "textDocument/publishDiagnostics" && n.params["uri"] == uri =>
                {
                    Some(n.params)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            !receipts.is_empty(),
            "each document transition publishes diagnostics"
        );
        for params in receipts {
            assert!(
                params.get("version").is_none(),
                "server publishes unversioned diagnostics"
            );
            let actual = params["diagnostics"].as_array().expect("diagnostics").iter().filter(|d| {
                let code = d["code"].as_str().unwrap_or(""); code == "E_PARSE" || code.starts_with("E_LEX")
            }).map(|d| json!({"code":d["code"],"message":d["message"],"severity":d["severity"],"source":d["source"],"range":d["range"]})).collect();
            let actual = Value::Array(actual);
            let rows = actual.as_array().expect("syntax notifications");
            assert_eq!(
                !rows.is_empty(),
                case["parseError"].as_bool().expect("parse policy"),
                "{}",
                case["id"]
            );
            let doc = oracle::document(case);
            for row in rows {
                assert_eq!(row["severity"], 1);
                assert_eq!(row["source"], "vela");
                assert!(!row["message"].as_str().expect("message").is_empty());
                for edge in ["start", "end"] {
                    let position = &row["range"][edge];
                    let line = position["line"].as_u64().expect("line") as usize;
                    let character = position["character"].as_u64().expect("UTF16 column") as usize;
                    let text = doc
                        .text
                        .split('\n')
                        .nth(line)
                        .expect("owned diagnostic line")
                        .trim_end_matches('\r');
                    assert!(character <= text.encode_utf16().count());
                }
                let start = &row["range"]["start"];
                let end = &row["range"]["end"];
                assert!(
                    (start["line"].as_u64(), start["character"].as_u64())
                        <= (end["line"].as_u64(), end["character"].as_u64())
                );
            }
        }
    }
    fn facts(
        &self,
        snapshot: &GlobalStateSnapshot,
        doc: &Document,
        disk: &FixtureWorkspace,
        case: &Value,
    ) {
        let db = snapshot.databases();
        assert_eq!(db.source_db().records().len(), 2);
        for (file, effective) in [
            ("scripts/main.vela", doc),
            ("scripts/helper.vela", &disk.disk["scripts/helper.vela"]),
        ] {
            let id = DocumentId::from(self.uri(file));
            assert_eq!(db.source_db().records()[&id].text(), effective.text);
            let diagnostics = db.parse_db().parse_diagnostics(&id).expect("parsed source");
            let actual = parse_actual(db, &id);
            let expected_error =
                file == "scripts/main.vela" && case["parseError"].as_bool().expect("parse policy");
            assert_eq!(
                !actual.as_array().expect("metadata").is_empty(),
                expected_error,
                "{}",
                case["id"]
            );
            if file == "scripts/main.vela" {
                oracle::assert_syntax(
                    &db.parse_db()
                        .syntax_parse(&id)
                        .expect("cached syntax")
                        .tree(),
                    case,
                );
            }
            for diagnostic in diagnostics {
                assert_eq!(
                    diagnostic.span.expect("source span").source,
                    db.parse_db().source_id(&id).expect("source id")
                );
                assert_eq!(format!("{:?}", diagnostic.severity), "Error");
                let code = diagnostic.code.as_deref().expect("parse code");
                assert!(code == "E_PARSE" || code.starts_with("E_LEX"));
                assert!(!diagnostic.message.is_empty());
                let span = diagnostic.span.expect("bounds");
                assert!(
                    span.start <= span.end
                        && span.end as usize <= db.source_db().records()[&id].text().len()
                );
                assert!(
                    db.source_db().records()[&id]
                        .text()
                        .is_char_boundary(span.start as usize)
                );
                assert!(
                    db.source_db().records()[&id]
                        .text()
                        .is_char_boundary(span.end as usize)
                );
            }
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
        self.facts(&before, doc, disk, case);
        let before_counts = counts(&before);
        for _ in 0..3 {
            for (file, wanted) in [
                ("scripts/main.vela", oracle::expected(case, true)),
                ("scripts/helper.vela", json!([])),
                ("scripts/missing.vela", json!([])),
            ] {
                self.id += 1;
                let uri = self.uri(file);
                assert_eq!(
                    response_value(request::<r::FoldingRangeRequest>(
                        &mut self.test_server,
                        self.id,
                        json!({"textDocument":{"uri":uri}})
                    )),
                    json!({"jsonrpc":"2.0","id":self.id,"result":wanted}),
                    "current whole folding envelope {} {file}",
                    case["id"]
                );
            }
        }
        assert_eq!(counts(&self.test_server.snapshot()), before_counts);
        self.facts(&self.test_server.snapshot(), doc, disk, case);
    }
    fn frozen(
        &mut self,
        snapshot: &GlobalStateSnapshot,
        doc: &Document,
        case: &Value,
        disk: &FixtureWorkspace,
    ) {
        self.facts(snapshot, doc, disk, case);
        let before = counts(snapshot);
        for _ in 0..3 {
            for (file, wanted) in [
                ("scripts/main.vela", oracle::expected(case, true)),
                ("scripts/helper.vela", json!([])),
                ("scripts/missing.vela", json!([])),
            ] {
                self.id += 1;
                let params = serde_json::from_value(json!({"textDocument":{"uri":self.uri(file)}}))
                    .expect("typed folding params");
                assert_eq!(
                    response_value(snapshot.clone().folding_range(self.id.into(), params)),
                    json!({"jsonrpc":"2.0","id":self.id,"result":wanted}),
                    "old complete folding set {} {file}",
                    case["id"]
                );
            }
        }
        assert_eq!(counts(snapshot), before);
        self.facts(snapshot, doc, disk, case);
    }
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
fn lsp_folding_recovery_pin_complete_utf16_damage_repair_redamage_close_and_immutable_sets() {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let spec = oracle::spec(crlf, shifted);
            let disk = FixtureWorkspace::new(&spec).expect("disk corpus");
            let cases = oracle::states(&spec);
            let mut current = Harness::new(&disk, None);
            current.check(&disk.disk["scripts/main.vela"], &cases[0], &disk);
            let mut frozen = vec![(
                current.test_server.snapshot(),
                disk.disk["scripts/main.vela"].clone(),
                cases[0].clone(),
            )];
            for (index, case) in cases.iter().enumerate() {
                let doc = oracle::document(case);
                if index == 0 {
                    current.open(&doc, case);
                } else {
                    current.change(&doc, index as i32 + 1, case);
                }
                current.check(&doc, case, &disk);
                let mut fresh = Harness::new(&disk, Some((&doc, case)));
                fresh.check(&doc, case, &disk);
                assert_eq!(
                    parse_actual(
                        current.test_server.snapshot().databases(),
                        &DocumentId::from(current.uri("scripts/main.vela"))
                    ),
                    parse_actual(
                        fresh.test_server.snapshot().databases(),
                        &DocumentId::from(fresh.uri("scripts/main.vela"))
                    ),
                    "complete current/fresh parse metadata",
                );
                for (snapshot, doc, case) in &frozen {
                    current.frozen(snapshot, doc, case, &disk);
                }
                frozen.push((current.test_server.snapshot(), doc, case.clone()));
            }
            current.close(&cases[0]);
            current.check(&disk.disk["scripts/main.vela"], &cases[0], &disk);
            let mut fresh = Harness::new(&disk, None);
            fresh.check(&disk.disk["scripts/main.vela"], &cases[0], &disk);
            for (snapshot, doc, case) in &frozen {
                current.frozen(snapshot, doc, case, &disk);
            }
        }
    }
}

fn parse_actual(
    db: &vela_language_service::LanguageServiceDatabases,
    id: &vela_language_service::DocumentId,
) -> Value {
    let diagnostics = db.parse_db().parse_diagnostics(id).expect("parsed source");
    Value::Array(diagnostics.iter().map(|d| {
        let span = d.span.expect("authored parse error span");
        json!({"code":d.code,"message":d.message,"severity":format!("{:?}",d.severity),"start":span.start,"end":span.end})
    }).collect())
}
