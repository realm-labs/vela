use super::matrix::assert_queries;
use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::tests::{TestServer, notify, request};
use lsp_types::{notification as n, request as r};
use serde_json::json;
use std::{collections::BTreeMap, fs, path::Path};

#[test]
fn source_navigation_lifecycle_preserves_dirty_ranges_and_drops_deleted_dependency_targets() {
    for crlf in [false, true] {
        let mut spec = load("navigation-source-lifecycle");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
            for action in &mut spec.actions {
                if let Some(source) = &mut action.source {
                    *source = source.replace('\n', "\r\n");
                }
            }
        }
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let temp = crate::tests::support::unique_temp_root("source-navigation-lifecycle");
        let root = temp.join("中文 workspace");
        fixture.materialize(&root).expect("workspace");
        let mut server = start_server(&root, &fixture);
        let mut id = 2;
        assert_queries(
            &mut server,
            &fixture,
            &spec.oracle["initial"],
            &root,
            &mut id,
            "unopened importer",
        );
        let mut versions = BTreeMap::new();
        for (index, action) in spec.actions.iter().enumerate() {
            fixture.apply(action).expect("action");
            let file_uri = uri(&root, &action.file);
            match action.op.as_str() {
                "open" => {
                    versions.insert(action.file.clone(), 1);
                    let _ = notify::<n::DidOpenTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":file_uri,"languageId":"vela","version":1,"text":fixture.open[&action.file].text}}),
                    );
                }
                "change" => {
                    let version = versions.get_mut(&action.file).expect("open version");
                    *version += 1;
                    let _ = notify::<n::DidChangeTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":file_uri,"version":version},"contentChanges":[{"text":fixture.open[&action.file].text}]}),
                    );
                }
                "close" => {
                    let _ = notify::<n::DidCloseTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":file_uri}}),
                    );
                }
                "save" => {
                    fs::write(root.join(&action.file), &fixture.disk[&action.file].text)
                        .expect("save");
                    let _ = notify::<n::DidSaveTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":file_uri}}),
                    );
                    // Save sync is not advertised; disk snapshots advance through
                    // the client's watched-file notification, including saves.
                    let _ = notify::<n::DidChangeWatchedFiles>(
                        &mut server,
                        json!({"changes":[{"uri":file_uri,"type":2}]}),
                    );
                }
                "write" | "delete" => {
                    let existed = root.join(&action.file).exists();
                    let change = if action.op == "delete" {
                        fs::remove_file(root.join(&action.file)).expect("delete");
                        3
                    } else {
                        fs::write(root.join(&action.file), &fixture.disk[&action.file].text)
                            .expect("write");
                        if existed { 2 } else { 1 }
                    };
                    let _ = notify::<n::DidChangeWatchedFiles>(
                        &mut server,
                        json!({"changes":[{"uri":file_uri,"type":change}]}),
                    );
                }
                _ => panic!("action"),
            }
            let mut fresh = start_server(&root, &fixture);
            let mut fresh_id = 2;
            for _ in 0..2 {
                let context = format!("action {index} {} CRLF={crlf}", action.op);
                let queries = &spec.oracle["afterEachAction"][index];
                assert_queries(
                    &mut server,
                    &fixture,
                    queries,
                    &root,
                    &mut id,
                    &format!("incremental {context}"),
                );
                assert_queries(
                    &mut fresh,
                    &fixture,
                    queries,
                    &root,
                    &mut fresh_id,
                    &format!("fresh {context}"),
                );
            }
        }
        fs::remove_dir_all(temp).expect("cleanup");
    }
}

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("URI")
        .to_string()
}

fn start_server(root: &Path, fixture: &FixtureWorkspace) -> TestServer {
    let mut server = TestServer::new();
    let _ = request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":uri(root,""),"capabilities":{}}),
    );
    for (file, document) in &fixture.open {
        let _ = notify::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri(root,file),"languageId":"vela","version":1,"text":document.text}}),
        );
    }
    server
}
