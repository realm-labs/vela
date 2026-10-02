use super::{TestServer, notify, request, response_value};
use crate::matrix_fixture::{
    Action, FixtureWorkspace, Spec, document_symbol_lifecycle as oracle, document_symbols,
    schema_artifact,
};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::path::PathBuf;
use vela_language_service::DocumentId;

struct Harness {
    owner: PathBuf,
    root: PathBuf,
    test_server: TestServer,
    id: i32,
}

impl Harness {
    fn new(fixture: &FixtureWorkspace, spec: &Spec, phase: &Value) -> Self {
        let owner = super::support::unique_temp_root("中文 % outline lifecycle");
        let root = owner.join("workspace");
        fixture.materialize(&root).expect("owned fixture root");
        match phase["schema"]["mode"].as_str().expect("mode") {
            "missing" => {
                std::fs::remove_file(root.join("schema.json")).expect("missing schema setup")
            }
            "invalid" => std::fs::write(
                root.join("schema.json"),
                phase["schema"]["text"].as_str().expect("invalid schema"),
            )
            .expect("invalid schema setup"),
            "valid" => {}
            mode => panic!("unknown schema mode {mode}"),
        }
        let mut current = Self {
            owner,
            root,
            test_server: TestServer::new(),
            id: 1,
        };
        let root_uri = current.uri("");
        let _ = response_value(request::<r::Initialize>(
            &mut current.test_server,
            1,
            json!({"processId":null,"rootUri":root_uri,"capabilities":{"textDocument":{"documentSymbol":{"hierarchicalDocumentSymbolSupport":true}}}}),
        ));
        current.schema(fixture, spec, &phase["schema"]);
        for (file, doc) in &fixture.open {
            let uri = current.uri(file);
            let _ = notify::<n::DidOpenTextDocument>(
                &mut current.test_server,
                json!({"textDocument":{"uri":uri,"languageId":"vela","version":1,"text":doc.text}}),
            );
        }
        current
    }

    fn uri(&self, file: &str) -> String {
        lsp_types::Url::from_file_path(self.root.join(file))
            .expect("encoded URI")
            .to_string()
    }

    fn schema(&mut self, fixture: &FixtureWorkspace, spec: &Spec, state: &Value) {
        let path = self.root.join("schema.json");
        let existed = path.exists();
        match state["mode"].as_str().expect("mode") {
            "valid" => {
                let snapshot = self.test_server.snapshot();
                let artifact = schema_artifact(
                    &spec.oracle["schemas"][state["id"].as_str().expect("schema id")],
                    fixture,
                    |file| {
                        snapshot.databases().source_db().records()
                            [&DocumentId::from(self.uri(file))]
                            .source_id()
                            .get()
                    },
                );
                std::fs::write(&path, artifact.to_string()).expect("bound artifact");
            }
            "invalid" => std::fs::write(&path, state["text"].as_str().expect("invalid artifact"))
                .expect("invalid artifact write"),
            "missing" => {
                if existed {
                    std::fs::remove_file(&path).expect("schema delete");
                }
            }
            mode => panic!("unknown schema mode {mode}"),
        }
        let uri = self.uri("schema.json");
        let kind = if state["mode"] == "missing" {
            3
        } else if existed {
            2
        } else {
            1
        };
        let _ = notify::<n::DidChangeWatchedFiles>(
            &mut self.test_server,
            json!({"changes":[{"uri":uri,"type":kind}]}),
        );
    }

    fn apply(&mut self, fixture: &FixtureWorkspace, action: &Action, version: i32) {
        let uri = self.uri(&action.file);
        match action.op.as_str() {
            "open" => {
                let _ = notify::<n::DidOpenTextDocument>(
                    &mut self.test_server,
                    json!({"textDocument":{"uri":uri,"languageId":"vela","version":version,"text":fixture.open[&action.file].text}}),
                );
            }
            "change" => {
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut self.test_server,
                    json!({"textDocument":{"uri":uri,"version":version},"contentChanges":[{"text":fixture.open[&action.file].text}]}),
                );
            }
            "close" => {
                let _ = notify::<n::DidCloseTextDocument>(
                    &mut self.test_server,
                    json!({"textDocument":{"uri":uri}}),
                );
            }
            op @ ("write" | "delete") => {
                let path = self.root.join(&action.file);
                let existed = path.exists();
                if op == "delete" {
                    std::fs::remove_file(&path).expect("owned source deletion");
                } else {
                    std::fs::write(&path, &fixture.disk[&action.file].text)
                        .expect("owned source replacement");
                }
                let _ = notify::<n::DidChangeWatchedFiles>(
                    &mut self.test_server,
                    json!({"changes":[{"uri":uri,"type":if op=="delete" {3} else if existed {2} else {1}}]}),
                );
            }
            op => panic!("unknown source action {op}"),
        }
    }

    fn check(&mut self, fixture: &FixtureWorkspace, spec: &Spec, phase: &Value) {
        oracle::assert_state(fixture, spec, phase);
        for (file, variant) in phase["views"].as_object().expect("views") {
            let uri = self.uri(file);
            let expected = expected(spec, variant);
            for _ in 0..3 {
                self.id += 1;
                let response = response_value(request::<r::DocumentSymbolRequest>(
                    &mut self.test_server,
                    self.id,
                    json!({"textDocument":{"uri":uri}}),
                ));
                assert_eq!(
                    response,
                    json!({"jsonrpc":"2.0","id":self.id,"result":expected}),
                    "whole current tree {}: {file}",
                    phase["id"]
                );
            }
            let disk = self.root.join(file);
            if let Some(doc) = fixture.disk.get(file) {
                assert_eq!(
                    std::fs::read_to_string(disk).expect("disk source"),
                    doc.text,
                    "disk bytes {file}"
                );
            } else {
                assert!(!disk.exists());
            }
        }
        check_snapshot(&self.test_server.snapshot(), self, spec, phase);
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.owner).expect("owned lifecycle root cleanup");
    }
}

fn expected(spec: &Spec, variant: &Value) -> Value {
    variant.as_str().map_or_else(
        || Value::Array(Vec::new()),
        |v| {
            let doc = oracle::document(spec, v);
            let expected =
                document_symbols::expected(&doc, &spec.oracle["variants"][v]["symbols"], true);
            document_symbols::assert_ancestry(&expected, None);
            expected
        },
    )
}

fn check_snapshot(
    snapshot: &crate::global_state::GlobalStateSnapshot,
    harness: &Harness,
    spec: &Spec,
    phase: &Value,
) {
    let db = snapshot.databases();
    for (file, variant) in phase["views"].as_object().expect("views") {
        let id = DocumentId::from(harness.uri(file));
        let actual = db.document_symbols(&id);
        let text = db.source_db().records().get(&id).map_or("", |r| r.text());
        if let Some(v) = variant.as_str() {
            assert_eq!(
                text,
                oracle::document(spec, v).text,
                "snapshot text {}: {file}",
                phase["id"]
            );
        } else {
            assert!(!db.source_db().records().contains_key(&id));
        }
        assert_eq!(
            serde_json::to_value(
                crate::lsp::to_proto::document_symbols(&actual, text).expect("snapshot projection")
            )
            .expect("snapshot JSON"),
            expected(spec, variant),
            "frozen snapshot {}: {file}",
            phase["id"]
        );
    }
    let schema = &phase["schema"];
    let facts = db.schema_db().facts();
    if schema["mode"] == "valid" {
        assert!(db.schema_db().diagnostics().is_empty());
        assert_eq!(facts.types().count(), 1);
        let row = &spec.oracle["schemas"][schema["id"].as_str().expect("schema id")]["fields"][0];
        let fields = facts.fields().collect::<Vec<_>>();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, row["name"].as_str().expect("field"));
        assert_eq!(
            fields[0].fact.display_name(),
            spec.oracle["schemaFieldDisplays"][schema["id"].as_str().expect("schema id")]
                .as_str()
                .expect("authored type display")
        );
        assert!(
            db.schema_db()
                .source_locations()
                .field_span("host::Box", &fields[0].name)
                .is_some()
        );
    } else {
        assert_eq!(facts.types().count(), 0);
        assert_eq!(facts.fields().count(), 0);
        assert_eq!(db.schema_db().diagnostics().len(), 1);
        let suffix = if schema["mode"] == "invalid" {
            "is invalid: unsupported schema artifact format version 99; expected 1; host facts degrade to Any"
        } else {
            "is unavailable; host facts degrade to Any"
        };
        assert!(db.schema_db().diagnostics()[0].message().ends_with(suffix));
        assert!(
            db.schema_db()
                .source_locations()
                .type_span("host::Box")
                .is_none()
        );
    }
}

#[test]
fn lsp_document_symbol_lifecycle_pins_current_and_frozen_whole_utf16_trees_and_fresh_parity() {
    for crlf in [false, true] {
        let spec = oracle::spec(crlf);
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let phases = spec.oracle["phases"].as_array().expect("phases");
        let mut current = Harness::new(&fixture, &spec, &phases[0]);
        let mut frozen = Vec::new();
        for (index, phase) in phases.iter().enumerate() {
            for action in oracle::actions(&spec, phase) {
                fixture.apply(&action).expect("finite source action");
                current.apply(&fixture, &action, index as i32 + 1);
            }
            if !phase["schemaAction"].is_null() {
                current.schema(&fixture, &spec, &phase["schema"]);
            }
            current.check(&fixture, &spec, phase);
            let fresh_fixture = oracle::fresh_fixture(&spec, phase);
            let mut fresh = Harness::new(&fresh_fixture, &spec, phase);
            fresh.check(&fresh_fixture, &spec, phase);
            for (snapshot, old_phase) in &frozen {
                check_snapshot(snapshot, &current, &spec, old_phase);
            }
            frozen.push((current.test_server.snapshot(), phase.clone()));
        }
    }
}
