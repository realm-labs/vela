use crate::matrix_fixture::{FixtureWorkspace, hover_signature as oracle, load};
use crate::tests::{TestServer, notify, request, response_value, sync_diagnostics};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::{fs, path::Path};

pub(super) fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("encoded URI")
        .to_string()
}

fn hover(server: &mut TestServer, uri: &str, position: &Value) -> Value {
    let response = response_value(request::<r::HoverRequest>(
        server,
        2,
        json!({"textDocument":{"uri":uri},"position":position}),
    ));
    assert!(response.get("error").is_none(), "{response}");
    assert!(response.get("result").is_some(), "explicit JSON null");
    response["result"].clone()
}

pub(super) fn verify_fixture(name: &str, expected_queries: usize, expected_positions: usize) {
    let mut positions = 0;
    for crlf in [false, true] {
        for missing_schema in [false, true] {
            let mut spec = load(name);
            if crlf {
                for source in spec.files.values_mut() {
                    *source = source.replace('\n', "\r\n");
                }
            }
            if missing_schema {
                spec.files.remove("schema.json");
            }
            let fixture = FixtureWorkspace::new(&spec).expect("fixture");
            let parent = crate::tests::support::unique_temp_root(name);
            let root = parent.join("中文 % hover");
            fixture.materialize(&root).expect("isolated files");
            let mut server = initialize(&root, &fixture);
            let queries = spec.oracle["queries"].as_array().expect("queries");
            assert_eq!(queries.len(), expected_queries);
            positions += verify_queries(
                &mut server,
                &fixture,
                &root,
                queries,
                missing_schema,
                crlf,
                true,
            )
            .0;
            for (file, document) in &fixture.disk {
                assert_eq!(
                    fs::read_to_string(root.join(file)).expect("all physical inputs"),
                    document.text
                );
            }
            fs::remove_dir_all(parent).expect("remove owned isolated workspace");
        }
    }
    assert_eq!(
        positions, expected_positions,
        "every authored token position and schema variant"
    );
}

pub(super) fn initialize(root: &Path, fixture: &FixtureWorkspace) -> TestServer {
    let mut server = TestServer::new();
    let response = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":uri(root,""),"capabilities":{}}),
    ));
    assert!(response.get("error").is_none());
    assert_eq!(response["result"]["capabilities"]["hoverProvider"], true);
    notify::<n::Initialized>(&mut server, json!({}));
    for (file, source) in &fixture.open {
        let _ = sync_diagnostics::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri(root,file),"languageId":"vela","version":1,"text":source.text}}),
        );
    }
    server
}

pub(super) fn verify_queries(
    server: &mut TestServer,
    fixture: &FixtureWorkspace,
    root: &Path,
    queries: &[Value],
    missing_schema: bool,
    crlf: bool,
    open_queries: bool,
) -> (usize, Vec<Value>) {
    let mut positions = 0;
    let mut results = Vec::new();
    let mut opened = std::collections::BTreeMap::new();
    for authored in queries {
        let mut case = authored.clone();
        if missing_schema && case.get("missingResult").is_some() {
            case["result"] = case["missingResult"].clone();
        }
        if missing_schema && case.get("missingDefinition").is_some() {
            case["definition"] = case["missingDefinition"].clone();
        }
        let file = case["file"].as_str().expect("file");
        let document = fixture.document(file).expect("current query document");
        let target = uri(root, file);
        if open_queries && !opened.contains_key(file) {
            let publication = sync_diagnostics::<n::DidOpenTextDocument>(
                server,
                json!({"textDocument":{"uri":target,"languageId":"vela","version":1,"text":document.text}}),
            );
            opened.insert(file.to_owned(), publication);
        }
        assert_recovery(server, &target, &case, opened.get(file));
        for offset in 0..if case["result"].is_null() { 1 } else { 2 } {
            positions += 1;
            let point = oracle::position(
                document,
                case["marker"].as_str().expect("marker"),
                true,
                offset,
            );
            let actual = hover(server, &target, &point);
            assert_eq!(
                actual,
                oracle::hover_result(document, &case, true),
                "{}, missing={missing_schema}, CRLF={crlf}",
                case["id"]
            );
            assert_eq!(hover(server, &target, &point), actual, "repeat hover");
            if let Some(owner) = case.get("definition") {
                let params = json!({"textDocument":{"uri":target},"position":point});
                let response =
                    response_value(request::<r::GotoDefinition>(server, 3, params.clone()));
                assert!(response.get("error").is_none(), "{response}");
                let expected = if owner.is_null() {
                    Value::Null
                } else {
                    let file = owner["file"].as_str().expect("owner file");
                    let marker = owner["marker"].as_str().expect("owner marker");
                    json!({"uri":uri(root,file),"range":oracle::marker_range(fixture.document(file).expect("current definition"),marker,true)})
                };
                assert_eq!(
                    response.get("result"),
                    Some(&expected),
                    "{} physical owner",
                    case["id"]
                );
                let repeated = response_value(request::<r::GotoDefinition>(server, 3, params));
                assert_eq!(repeated.get("result"), Some(&expected), "repeat owner");
            }
            results.push(actual);
        }
    }
    (positions, results)
}

fn assert_recovery(server: &TestServer, target: &str, case: &Value, publication: Option<&Value>) {
    if let Some(expected) = case["parseErrors"].as_bool() {
        let snapshot = server.snapshot();
        let id = vela_language_service::DocumentId::from(target);
        assert_eq!(
            !snapshot
                .databases()
                .parse_db()
                .parse_diagnostics(&id)
                .expect("parsed query")
                .is_empty(),
            expected,
            "{} parser recovery",
            case["id"]
        );
    }
    if let Some(candidate) = case["diagnosticCandidate"].as_object() {
        let publication = publication.expect("real diagnostic publication");
        assert!(
            publication["params"]["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|d| d["code"] == candidate["code"]
                    && d["data"]["candidates"]
                        .as_array()
                        .expect("candidates")
                        .iter()
                        .any(|c| c["replacement"] == candidate["replacement"])),
            "{} real repair candidate: {publication}",
            case["id"]
        );
    }
}
