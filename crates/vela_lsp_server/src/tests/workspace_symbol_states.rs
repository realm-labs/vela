use super::{TestServer, notify, request, response_value};
use crate::matrix_fixture::{
    FixtureWorkspace, schema_artifact, workspace_symbol_states as states,
    workspace_symbols as oracle,
};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::path::PathBuf;

struct Harness {
    test_server: TestServer,
    owner: PathBuf,
    root: PathBuf,
    id: i32,
}

impl Harness {
    fn new(disk: &FixtureWorkspace, authored: &Value, open_text: &str) -> Self {
        let owner = super::support::unique_temp_root("中文 % workspace state");
        let root = owner.join("workspace");
        disk.materialize(&root).expect("owned corpus");
        let mut harness = Self {
            test_server: TestServer::new(),
            owner,
            root,
            id: 10,
        };
        let root_uri = harness.uri("scripts");
        let _ = response_value(request::<r::Initialize>(
            &mut harness.test_server,
            1,
            json!({"processId":null,"rootUri":root_uri,"capabilities":{}}),
        ));
        let snapshot = harness.test_server.snapshot();
        let artifact = schema_artifact(&authored["schema"], disk, |file| {
            snapshot.databases().source_db().records()
                [&vela_language_service::DocumentId::from(harness.uri(file))]
                .source_id()
                .get()
        })
        .to_string();
        std::fs::write(harness.root.join("schema.json"), artifact).expect("bound static schema");
        let schema_uri = harness.uri("schema.json");
        let _ = notify::<n::DidChangeConfiguration>(
            &mut harness.test_server,
            json!({"settings":{"vela":{"host":{"schema":schema_uri}}}}),
        );
        let main_uri = harness.uri("scripts/main.vela");
        let _ = notify::<n::DidOpenTextDocument>(
            &mut harness.test_server,
            json!({"textDocument":{"uri":main_uri,"languageId":"vela","version":1,"text":open_text}}),
        );
        harness
    }

    fn uri(&self, file: &str) -> String {
        lsp_types::Url::from_file_path(self.root.join(file))
            .expect("URI")
            .to_string()
    }

    fn check_facts(
        &self,
        snapshot: &crate::global_state::GlobalStateSnapshot,
        fixture: &FixtureWorkspace,
    ) {
        let db = snapshot.databases();
        assert_eq!(db.source_db().records().len(), 8);
        assert!(db.schema_db().diagnostics().is_empty());
        assert!(
            db.schema_db()
                .source_locations()
                .type_span("host::Box")
                .is_some()
        );
        assert!(
            db.schema_db()
                .source_locations()
                .field_span("host::Box", "value")
                .is_some()
        );
        assert!(
            db.schema_db()
                .source_locations()
                .method_span("host::Box", "read")
                .is_some()
        );
        assert!(
            db.schema_db()
                .source_locations()
                .function_span("host::make")
                .is_some()
        );
        for (file, doc) in fixture.disk.iter().filter(|(f, _)| f.ends_with(".vela")) {
            let id = vela_language_service::DocumentId::from(self.uri(file));
            assert_eq!(db.source_db().records()[&id].text(), doc.text);
            assert!(
                db.parse_db()
                    .parse_diagnostics(&id)
                    .expect("parsed source")
                    .is_empty()
            );
        }
    }

    fn check(&mut self, fixture: &FixtureWorkspace, authored: &Value, disk: &FixtureWorkspace) {
        let snapshot = self.test_server.snapshot();
        self.check_facts(&snapshot, fixture);
        let db = snapshot.databases();
        for encoding in ["%E4%B8%AD", "%20", "%25"] {
            assert!(self.uri("").contains(encoding));
        }
        let before = db.workspace_symbols("");
        let schema_before = std::fs::read(self.root.join("schema.json")).expect("physical schema");
        for query in authored["queries"].as_array().expect("queries") {
            let wanted = oracle::expected(fixture, authored, &query["symbols"], true, &|file| {
                self.uri(file)
            });
            for _ in 0..3 {
                self.id += 1;
                let actual = response_value(request::<r::WorkspaceSymbolRequest>(
                    &mut self.test_server,
                    self.id,
                    json!({"query":query["query"]}),
                ));
                assert_eq!(
                    actual,
                    json!({"jsonrpc":"2.0","id":self.id,"result":wanted}),
                    "query {}",
                    query["id"]
                );
            }
        }
        assert_eq!(
            self.test_server.snapshot().generation(),
            snapshot.generation()
        );
        assert_eq!(
            self.test_server
                .snapshot()
                .databases()
                .workspace_symbols(""),
            before
        );
        assert_eq!(
            std::fs::read(self.root.join("schema.json")).expect("physical schema"),
            schema_before
        );
        for (file, doc) in disk.disk.iter().filter(|(f, _)| f.ends_with(".vela")) {
            assert_eq!(
                std::fs::read_to_string(self.root.join(file)).expect("physical source"),
                doc.text
            );
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.owner).expect("owned cleanup");
    }
}

fn run(group: &str) {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let cases = states::cases(group, crlf, shifted);
            let disk = &cases[0].1;
            let file = "scripts/main.vela";
            let mut current = Harness::new(disk, &cases[0].2, &disk.disk[file].text);
            let mut frozen: Vec<(
                crate::global_state::GlobalStateSnapshot,
                FixtureWorkspace,
                Value,
            )> = Vec::new();
            for (index, (_, fixture, authored)) in cases.iter().enumerate() {
                if index > 0 {
                    let uri = current.uri(file);
                    let old_generation = current.test_server.snapshot().generation();
                    let _ = notify::<n::DidChangeTextDocument>(
                        &mut current.test_server,
                        json!({"textDocument":{"uri":uri,"version":index as i32+1},"contentChanges":[{"text":fixture.disk[file].text}]}),
                    );
                    assert!(current.test_server.snapshot().generation() > old_generation);
                }
                current.check(fixture, authored, disk);
                let mut fresh = Harness::new(disk, authored, &fixture.disk[file].text);
                fresh.check(fixture, authored, disk);
                for (snapshot, old_fixture, old_authored) in &frozen {
                    current.check_facts(snapshot, old_fixture);
                    for query in old_authored["queries"].as_array().expect("frozen queries") {
                        let wanted = oracle::expected(
                            old_fixture,
                            old_authored,
                            &query["symbols"],
                            false,
                            &|file| current.uri(file),
                        );
                        assert_eq!(
                            Value::Array(
                                snapshot
                                    .databases()
                                    .workspace_symbols(query["query"].as_str().expect("query text"))
                                    .iter()
                                    .map(vela_language_service_project)
                                    .collect()
                            ),
                            wanted
                        );
                    }
                }
                frozen.push((
                    current.test_server.snapshot(),
                    fixture.clone(),
                    authored.clone(),
                ));
            }
        }
    }
}

fn vela_language_service_project(symbol: &vela_language_service::WorkspaceSymbol) -> Value {
    use vela_language_service::{SymbolRef, WorkspaceSymbolLocation};
    let mut row = json!({"name":symbol.name(),"kind":format!("{:?}",symbol.kind())});
    match (symbol.symbol(), symbol.location()) {
        (SymbolRef::Source(identity), WorkspaceSymbolLocation::Source { document_id, range }) => {
            row["identity"] = json!(identity);
            row["ownership"] = json!("Source");
            row["location"] = json!({"uri":document_id.as_str(),"range":{"start":{"line":range.start().line,"character":range.start().character},"end":{"line":range.end().line,"character":range.end().character}}});
        }
        (SymbolRef::Schema(identity), WorkspaceSymbolLocation::Schema) => {
            row["identity"] = json!(identity);
            row["ownership"] = json!("Schema");
            row["location"] = json!({"schema":true});
        }
        _ => panic!("no invented rows"),
    }
    if let Some(detail) = symbol.detail() {
        row["detail"] = json!(detail);
    }
    if let Some(container) = symbol.container_name() {
        row["containerName"] = json!(container);
    }
    row
}

#[test]
fn lsp_workspace_symbol_unresolved_states_pin_whole_utf16_sets_and_restoration() {
    run("unresolved");
}

#[test]
fn lsp_workspace_symbol_dynamic_states_pin_whole_utf16_sets_and_restoration() {
    run("dynamic");
}
