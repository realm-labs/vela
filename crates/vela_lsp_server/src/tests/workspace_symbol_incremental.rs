use super::workspace_symbol_states::vela_language_service_project;
use super::{TestServer, notify, request, response_value};
use crate::global_state::GlobalStateSnapshot;
use crate::matrix_fixture::{
    FixtureWorkspace, workspace_symbol_incremental as states, workspace_symbols as oracle,
};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use vela_language_service::{DocumentId, LanguageServiceDatabases};

struct Harness {
    test_server: TestServer,
    owner: PathBuf,
    root: PathBuf,
    id: i32,
}

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("owned URI")
        .to_string()
}

fn start(root: &Path, fixture: &FixtureWorkspace) -> TestServer {
    let mut server = TestServer::new();
    let _ = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":uri(root,"scripts"),"capabilities":{}}),
    ));
    for (file, doc) in &fixture.disk {
        let _ = notify::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri(root,file),"languageId":"vela","version":1,"text":doc.text}}),
        );
    }
    server
}

fn counters(db: &LanguageServiceDatabases) -> (usize, usize, usize, u64) {
    (
        db.parse_db().parse_count(),
        db.project_db().rebuild_count(),
        db.hir_db().rebuild_count(),
        db.generation().get(),
    )
}

fn facts(snapshot: &GlobalStateSnapshot, root: &Path, fixture: &FixtureWorkspace) {
    let db = snapshot.databases();
    assert_eq!(db.source_db().records().len(), 5);
    assert!(db.schema_db().diagnostics().is_empty());
    assert!(
        db.schema_db()
            .source_locations()
            .type_span("host::Box")
            .is_none()
    );
    for (file, doc) in &fixture.disk {
        let id = DocumentId::from(uri(root, file));
        assert_eq!(db.source_db().records()[&id].text(), doc.text);
        assert!(
            db.parse_db()
                .parse_diagnostics(&id)
                .expect("parsed source")
                .is_empty()
        );
    }
}

impl Harness {
    fn new(disk: &FixtureWorkspace) -> Self {
        let owner = super::support::unique_temp_root("中文 % workspace incremental");
        let root = owner.join("workspace");
        disk.materialize(&root).expect("physical corpus");
        Self {
            test_server: start(&root, disk),
            owner,
            root,
            id: 10,
        }
    }

    fn check(&mut self, fixture: &FixtureWorkspace, phase: &Value, disk: &FixtureWorkspace) {
        let snapshot = self.test_server.snapshot();
        facts(&snapshot, &self.root, fixture);
        let before = counters(snapshot.databases());
        for encoding in ["%E4%B8%AD", "%20", "%25"] {
            assert!(uri(&self.root, "").contains(encoding));
        }
        for query in phase["workspace"]["queries"].as_array().expect("queries") {
            let wanted = oracle::expected(
                fixture,
                &phase["workspace"],
                &query["symbols"],
                true,
                &|file| uri(&self.root, file),
            );
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
                    "phase {} query {}",
                    phase["id"],
                    query["id"]
                );
            }
        }
        assert_eq!(
            counters(self.test_server.snapshot().databases()),
            before,
            "read-only requests"
        );
        for (file, doc) in &disk.disk {
            assert_eq!(
                std::fs::read_to_string(self.root.join(file)).expect("physical source"),
                doc.text
            );
        }
        let mut fresh = start(&self.root, fixture);
        facts(&fresh.snapshot(), &self.root, fixture);
        for query in phase["workspace"]["queries"]
            .as_array()
            .expect("fresh queries")
        {
            let wanted = oracle::expected(
                fixture,
                &phase["workspace"],
                &query["symbols"],
                true,
                &|file| uri(&self.root, file),
            );
            self.id += 1;
            assert_eq!(
                response_value(request::<r::WorkspaceSymbolRequest>(
                    &mut fresh,
                    self.id,
                    json!({"query":query["query"]})
                )),
                json!({"jsonrpc":"2.0","id":self.id,"result":wanted})
            );
        }
    }

    fn frozen(&self, snapshot: &GlobalStateSnapshot, fixture: &FixtureWorkspace, phase: &Value) {
        facts(snapshot, &self.root, fixture);
        let before = counters(snapshot.databases());
        for query in phase["workspace"]["queries"]
            .as_array()
            .expect("frozen queries")
        {
            let wanted = oracle::expected(
                fixture,
                &phase["workspace"],
                &query["symbols"],
                false,
                &|file| uri(&self.root, file),
            );
            assert_eq!(
                Value::Array(
                    snapshot
                        .databases()
                        .workspace_symbols(query["query"].as_str().expect("query"))
                        .iter()
                        .map(vela_language_service_project)
                        .collect()
                ),
                wanted
            );
        }
        assert_eq!(counters(snapshot.databases()), before);
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.owner).expect("owned cleanup");
    }
}

#[test]
fn lsp_workspace_symbol_incremental_body_import_and_declaration_edits_pin_cache_ownership_and_all_old_snapshots()
 {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let cases = states::cases(crlf, shifted);
            let disk = &cases[0].0;
            let mut live = Harness::new(disk);
            let mut frozen = Vec::new();
            for (index, (fixture, phase)) in cases.iter().enumerate() {
                if index > 0 {
                    let file = phase["file"].as_str().expect("changed file");
                    let id = DocumentId::from(uri(&live.root, file));
                    let snapshot = live.test_server.snapshot();
                    let before = snapshot.databases();
                    let key = before.source_db().records()[&id].module_key();
                    let fingerprint = before
                        .parse_db()
                        .module_fingerprint(key)
                        .expect("old fingerprint");
                    let _ = notify::<n::DidChangeTextDocument>(
                        &mut live.test_server,
                        json!({"textDocument":{"uri":uri(&live.root,file),"version":index as i32+1},"contentChanges":[{"text":fixture.disk[file].text}]}),
                    );
                    let snapshot = live.test_server.snapshot();
                    let after = snapshot.databases();
                    assert_eq!(
                        after.parse_db().parse_count(),
                        before.parse_db().parse_count() + 1
                    );
                    assert_eq!(
                        after.hir_db().rebuild_count(),
                        before.hir_db().rebuild_count() + 1
                    );
                    assert_eq!(
                        after.project_db().rebuild_count() - before.project_db().rebuild_count(),
                        usize::from(
                            phase["declarationChanged"] == true || phase["importChanged"] == true
                        )
                    );
                    let current = after
                        .parse_db()
                        .module_fingerprint(key)
                        .expect("new fingerprint");
                    assert_eq!(
                        current.declaration() != fingerprint.declaration(),
                        phase["declarationChanged"] == true
                    );
                    assert_eq!(
                        current.import() != fingerprint.import(),
                        phase["importChanged"] == true
                    );
                    let invalidated = after
                        .analysis_db()
                        .invalidated_modules()
                        .iter()
                        .map(|key| key.path.join())
                        .collect::<Vec<_>>();
                    assert_eq!(json!(invalidated), phase["invalidated"], "{phase}");
                    assert!(after.generation() > before.generation());
                }
                live.check(fixture, phase, disk);
                for (snapshot, old_fixture, old_phase) in &frozen {
                    live.frozen(snapshot, old_fixture, old_phase);
                }
                frozen.push((live.test_server.snapshot(), fixture.clone(), phase.clone()));
            }
        }
    }
}
