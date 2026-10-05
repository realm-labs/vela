use super::workspace_symbol_states::vela_language_service_project;
use super::{TestServer, notify, request, response_value};
use crate::global_state::GlobalStateSnapshot;
use crate::matrix_fixture::{
    FixtureWorkspace, Spec, workspace_symbol_sources as states, workspace_symbols as oracle,
};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::path::PathBuf;
use vela_language_service::DocumentId;

struct Harness {
    test_server: TestServer,
    owner: PathBuf,
    root: PathBuf,
    roots: Vec<String>,
    id: i32,
}

impl Harness {
    fn new(fixture: &FixtureWorkspace, phase: &Value) -> Self {
        let owner = super::support::unique_temp_root("中文 % workspace source ownership");
        let root = owner.join("workspace");
        fixture.materialize(&root).expect("owned physical corpus");
        let roots = phase["roots"]
            .as_array()
            .expect("root list")
            .iter()
            .map(|r| r.as_str().expect("root").to_owned())
            .collect::<Vec<_>>();
        let mut h = Self {
            test_server: TestServer::new(),
            owner,
            root,
            roots,
            id: 10,
        };
        let folders = h
            .roots
            .iter()
            .map(|root| json!({"uri":h.uri(root),"name":root}))
            .collect::<Vec<_>>();
        let _ = response_value(request::<r::Initialize>(
            &mut h.test_server,
            1,
            json!({"processId":null,"rootUri":null,"workspaceFolders":folders,"capabilities":{}}),
        ));
        for (file, doc) in &fixture.open {
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
            .expect("encoded owned URI")
            .to_string()
    }

    fn watched(&mut self, changes: Value) {
        let _ =
            notify::<n::DidChangeWatchedFiles>(&mut self.test_server, json!({"changes":changes}));
    }

    fn apply(&mut self, spec: &Spec, action: &Value, version: i32) {
        if action["op"] == "roots" {
            let roots = action["roots"]
                .as_array()
                .expect("new roots")
                .iter()
                .map(|r| r.as_str().expect("root").to_owned())
                .collect::<Vec<_>>();
            let removed = self
                .roots
                .iter()
                .filter(|r| !roots.contains(r))
                .map(|root| json!({"uri":self.uri(root),"name":root}))
                .collect::<Vec<_>>();
            let added = roots
                .iter()
                .filter(|r| !self.roots.contains(r))
                .map(|root| json!({"uri":self.uri(root),"name":root}))
                .collect::<Vec<_>>();
            let _ = notify::<n::DidChangeWorkspaceFolders>(
                &mut self.test_server,
                json!({"event":{"added":added,"removed":removed}}),
            );
            self.roots = roots;
            return;
        }
        let file = action["file"].as_str().expect("file");
        let uri = self.uri(file);
        match action["op"].as_str().expect("finite action") {
            "open" => {
                let doc =
                    states::document(spec, action["variant"].as_str().expect("source variant"));
                let _ = notify::<n::DidOpenTextDocument>(
                    &mut self.test_server,
                    json!({"textDocument":{"uri":uri,"languageId":"vela","version":version,"text":doc.text}}),
                );
            }
            "change" => {
                let doc =
                    states::document(spec, action["variant"].as_str().expect("source variant"));
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut self.test_server,
                    json!({"textDocument":{"uri":uri,"version":version},"contentChanges":[{"text":doc.text}]}),
                );
            }
            "close" => {
                let _ = notify::<n::DidCloseTextDocument>(
                    &mut self.test_server,
                    json!({"textDocument":{"uri":uri}}),
                );
            }
            "write" => {
                let path = self.root.join(file);
                let existed = path.exists();
                let doc =
                    states::document(spec, action["variant"].as_str().expect("source variant"));
                std::fs::write(path, doc.text).expect("owned physical write");
                self.watched(json!([{"uri":uri,"type":if existed {2} else {1}}]));
            }
            "delete" => {
                std::fs::remove_file(self.root.join(file)).expect("owned physical delete");
                self.watched(json!([{"uri":uri,"type":3}]));
            }
            "rename" => {
                let to = action["to"].as_str().expect("new owner");
                let old = self.root.join(file);
                let new = self.root.join(to);
                assert!(old.starts_with(&self.root) && new.starts_with(&self.root));
                std::fs::rename(old, new).expect("owned physical rename");
                self.watched(json!([{"uri":uri,"type":3},{"uri":self.uri(to),"type":1}]));
            }
            op => panic!("unsupported source action {op}"),
        }
    }

    fn snapshot(&self, snapshot: &GlobalStateSnapshot, fixture: &FixtureWorkspace, phase: &Value) {
        let db = snapshot.databases();
        let owned = states::owned_files(phase);
        assert_eq!(
            db.source_db()
                .records()
                .keys()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            owned
                .iter()
                .map(|file| DocumentId::from(self.uri(file)))
                .collect()
        );
        let effective = states::effective(fixture);
        for file in &owned {
            let id = DocumentId::from(self.uri(file));
            assert_eq!(
                db.source_db().records()[&id].text(),
                effective.disk[file].text
            );
            assert!(
                db.parse_db()
                    .parse_diagnostics(&id)
                    .expect("parsed source")
                    .is_empty()
            );
        }
        assert!(db.schema_db().diagnostics().is_empty());
        assert_eq!(db.schema_db().facts().types().count(), 0);
        assert_eq!(db.schema_db().facts().functions().count(), 0);
        for query in phase["workspace"]["queries"]
            .as_array()
            .expect("frozen queries")
        {
            let wanted = oracle::expected(
                &effective,
                &phase["workspace"],
                &query["symbols"],
                false,
                &|file| self.uri(file),
            );
            assert_eq!(
                Value::Array(
                    db.workspace_symbols(query["query"].as_str().expect("query"))
                        .iter()
                        .map(vela_language_service_project)
                        .collect()
                ),
                wanted,
                "snapshot {} query {}",
                phase["id"],
                query["id"]
            );
        }
    }

    fn check(&mut self, fixture: &FixtureWorkspace, phase: &Value) {
        let snapshot = self.test_server.snapshot();
        self.snapshot(&snapshot, fixture, phase);
        let db = snapshot.databases();
        let before = (
            db.generation(),
            db.parse_db().parse_count(),
            db.project_db().rebuild_count(),
            db.hir_db().rebuild_count(),
        );
        let effective = states::effective(fixture);
        for query in phase["workspace"]["queries"].as_array().expect("queries") {
            let wanted = oracle::expected(
                &effective,
                &phase["workspace"],
                &query["symbols"],
                true,
                &|file| self.uri(file),
            );
            for _ in 0..3 {
                self.id += 1;
                assert_eq!(
                    response_value(request::<r::WorkspaceSymbolRequest>(
                        &mut self.test_server,
                        self.id,
                        json!({"query":query["query"]})
                    )),
                    json!({"jsonrpc":"2.0","id":self.id,"result":wanted})
                );
            }
        }
        let snapshot = self.test_server.snapshot();
        let db = snapshot.databases();
        assert_eq!(
            (
                db.generation(),
                db.parse_db().parse_count(),
                db.project_db().rebuild_count(),
                db.hir_db().rebuild_count()
            ),
            before
        );
        for encoding in ["%E4%B8%AD", "%20", "%25"] {
            assert!(self.uri("").contains(encoding));
        }
        for file in [
            "scripts/api.vela",
            "scripts/main.vela",
            "scripts/renamed.vela",
            "shared/helper.vela",
            "outside/scratch.vela",
            "outside/secret.vela",
        ] {
            match fixture.disk.get(file) {
                Some(doc) => assert_eq!(
                    std::fs::read_to_string(self.root.join(file)).expect("physical source"),
                    doc.text
                ),
                None => assert!(!self.root.join(file).exists(), "physical absence {file}"),
            }
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.owner).expect("owned cleanup");
    }
}

fn run(sequence: &str) {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let spec = states::spec(crlf, shifted);
            let phases = spec.oracle["sequences"][sequence]
                .as_array()
                .expect("finite phases");
            let initial = states::fixture(&spec, &phases[0]);
            let mut current = Harness::new(&initial, &phases[0]);
            let mut frozen = Vec::new();
            for (index, phase) in phases.iter().enumerate() {
                if index > 0 {
                    for action in phase["actions"].as_array().expect("actions") {
                        current.apply(&spec, action, index as i32 + 1);
                    }
                }
                let fixture = states::fixture(&spec, phase);
                current.check(&fixture, phase);
                let mut fresh = Harness::new(&fixture, phase);
                fresh.check(&fixture, phase);
                for (snapshot, old_fixture, old_phase) in &frozen {
                    current.snapshot(snapshot, old_fixture, old_phase);
                }
                frozen.push((current.test_server.snapshot(), fixture, phase.clone()));
            }
        }
    }
}

#[test]
fn lsp_workspace_symbol_configured_roots_disk_rename_and_missing_scratch_pin_whole_owned_sets() {
    run("workspace");
}

#[test]
fn lsp_workspace_symbol_no_root_scratch_open_dirty_close_reopen_pin_whole_owned_sets() {
    run("scratch");
}
