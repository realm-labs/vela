use super::{TestServer, notify, request, response_value};
use crate::matrix_fixture::{FixtureWorkspace, parse_markers, workspace_symbols as oracle};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::path::Path;

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("URI")
        .to_string()
}

fn server(root: &Path, text: &str, version: i32) -> TestServer {
    let mut server = TestServer::new();
    let _ = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":uri(root,"scripts"),"capabilities":{}}),
    ));
    let _ = notify::<n::DidChangeConfiguration>(
        &mut server,
        json!({"settings":{"vela":{"host":{"schema":uri(root,"schema.json")}}}}),
    );
    let _ = notify::<n::DidOpenTextDocument>(
        &mut server,
        json!({"textDocument":{"uri":uri(root,"scripts/main.vela"),"languageId":"vela","version":version,"text":text}}),
    );
    server
}

fn check(
    server: &mut TestServer,
    root: &Path,
    fixture: &FixtureWorkspace,
    expected: &Value,
    parse_error: bool,
    case: &str,
    request_id: &mut i32,
) {
    let file = "scripts/main.vela";
    let snapshot = server.snapshot();
    assert!(
        snapshot.databases().schema_db().diagnostics().is_empty(),
        "healthy static schema"
    );
    assert_eq!(snapshot.databases().source_db().records().len(), 2);
    assert_eq!(
        snapshot
            .databases()
            .parse_db()
            .parse_diagnostics(&vela_language_service::DocumentId::from(uri(root, file)))
            .expect("parsed")
            .iter()
            .any(|d| d.code.as_deref() == Some("E_PARSE")),
        parse_error,
        "parse policy {case}"
    );
    let before = snapshot.databases().workspace_symbols("");
    for query in expected["queries"].as_array().expect("queries") {
        let wanted = oracle::expected(fixture, expected, &query["symbols"], true, &|file| {
            uri(root, file)
        });
        for repeat in 0..3 {
            let response = response_value(request::<r::WorkspaceSymbolRequest>(
                server,
                *request_id,
                json!({"query":query["query"]}),
            ));
            assert_eq!(
                response,
                json!({"jsonrpc":"2.0","id":*request_id,"result":wanted}),
                "case={case} query={} repeat={repeat}",
                query["id"]
            );
            *request_id += 1;
        }
    }
    assert_eq!(
        server.snapshot().databases().workspace_symbols(""),
        before,
        "query cannot mutate ownership"
    );
    for (file, source) in fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
    {
        assert_eq!(
            server.snapshot().databases().source_db().records()
                [&vela_language_service::DocumentId::from(uri(root, file))]
                .text(),
            source.text
        );
    }
}

#[test]
fn lsp_workspace_symbol_recovery_pins_complete_utf16_sets_after_damage_repair_and_repeat() {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let (fixture, authored) = oracle::recovery(crlf, shifted);
            let file = authored["file"].as_str().expect("file");
            let owner = super::support::unique_temp_root("中文 % workspace recovery");
            let root = owner.join("workspace");
            fixture.materialize(&root).expect("owned fixture");
            let schema = json!({"formatVersion":1,"facts":authored["schema"]}).to_string();
            std::fs::write(root.join("schema.json"), &schema).expect("static schema");
            for encoding in ["%E4%B8%AD", "%25", "%20"] {
                assert!(uri(&root, "").contains(encoding));
            }
            let mut current_server = server(&root, &fixture.disk[file].text, 1);
            let mut version = 1;
            let mut request_id = 2;
            for case in authored["cases"].as_array().expect("cases") {
                let damage = parse_markers(case["source"].as_str().expect("source"))
                    .expect("damage markers");
                for (doc, expected, parse_error) in [
                    (
                        &damage,
                        case,
                        case["parseError"].as_bool().expect("parse policy"),
                    ),
                    (&fixture.disk[file], &authored, false),
                    (
                        &damage,
                        case,
                        case["parseError"].as_bool().expect("parse policy"),
                    ),
                ] {
                    version += 1;
                    let _ = notify::<n::DidChangeTextDocument>(
                        &mut current_server,
                        json!({"textDocument":{"uri":uri(&root,file),"version":version},"contentChanges":[{"text":doc.text}]}),
                    );
                    let mut current = fixture.clone();
                    current.disk.insert(file.to_owned(), doc.clone());
                    check(
                        &mut current_server,
                        &root,
                        &current,
                        expected,
                        parse_error,
                        case["id"].as_str().expect("case"),
                        &mut request_id,
                    );
                    let mut fresh = server(&root, &doc.text, version);
                    check(
                        &mut fresh,
                        &root,
                        &current,
                        expected,
                        parse_error,
                        case["id"].as_str().expect("case"),
                        &mut request_id,
                    );
                }
            }
            for (file, source) in fixture
                .disk
                .iter()
                .filter(|(file, _)| file.ends_with(".vela"))
            {
                assert_eq!(
                    std::fs::read_to_string(root.join(file)).expect("physical source"),
                    source.text
                );
            }
            assert_eq!(
                std::fs::read_to_string(root.join("schema.json")).expect("physical schema"),
                schema
            );
            std::fs::remove_dir_all(&owner).expect("owned cleanup");
        }
    }
}
