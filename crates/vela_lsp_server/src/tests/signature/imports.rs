use crate::matrix_fixture::{
    Action, FixtureWorkspace, Spec, hover_signature, signature_imports as oracle,
};
use crate::tests::{TestServer, notify, request, response_value, sync_diagnostics};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

mod recovery;

fn uri(layout: &oracle::Layout, file: &str) -> String {
    lsp_types::Url::from_file_path(layout.root.join(file))
        .expect("encoded URI")
        .to_string()
}

fn open(server: &mut TestServer, layout: &oracle::Layout, file: &str, text: &str, version: i32) {
    let _ = sync_diagnostics::<n::DidOpenTextDocument>(
        server,
        json!({"textDocument":{
            "uri":uri(layout,file),"languageId":"vela","version":version,"text":text
        }}),
    );
}

fn initialize(fixture: &FixtureWorkspace, layout: &oracle::Layout) -> TestServer {
    let mut server = TestServer::new();
    let response = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":uri(layout,""),"capabilities":{}}),
    ));
    assert!(response.get("error").is_none(), "{response}");
    if let Some(markers) = fixture.disk.get("schema-markers.json") {
        let snapshot = server.snapshot();
        let artifact = crate::matrix_fixture::schema_artifact(
            &serde_json::from_str::<Value>(&markers.text).expect("schema markers"),
            fixture,
            |file| {
                snapshot.databases().source_db().records()
                    [&vela_language_service::DocumentId::from(uri(layout, file))]
                    .source_id()
                    .get()
            },
        );
        std::fs::write(layout.root.join("schema.json"), artifact.to_string())
            .expect("bind schema spans");
        let _ = notify::<n::DidChangeWatchedFiles>(
            &mut server,
            json!({"changes":[{"uri":uri(layout,"schema.json"),"type":2}]}),
        );
        assert!(
            server
                .snapshot()
                .databases()
                .schema_db()
                .diagnostics()
                .is_empty()
        );
    }
    for (file, source) in &fixture.open {
        open(&mut server, layout, file, &source.text, 1);
    }
    server
}

fn apply(
    server: &mut TestServer,
    fixture: &FixtureWorkspace,
    layout: &oracle::Layout,
    action: &Action,
    version: i32,
) {
    let target = uri(layout, &action.file);
    let existed = layout.root.join(&action.file).exists();
    layout.apply(fixture, action);
    match action.op.as_str() {
        "open" => open(
            server,
            layout,
            &action.file,
            &fixture.open[&action.file].text,
            version,
        ),
        "change" => {
            let _ = sync_diagnostics::<n::DidChangeTextDocument>(
                server,
                json!({"textDocument":{"uri":target,"version":version},"contentChanges":[{"text":fixture.open[&action.file].text}]}),
            );
        }
        "close" => {
            let _ = sync_diagnostics::<n::DidCloseTextDocument>(
                server,
                json!({"textDocument":{"uri":target}}),
            );
        }
        "write" | "delete" => {
            let _ = notify::<n::DidChangeWatchedFiles>(
                server,
                json!({"changes":[{"uri":target,"type":if action.op == "delete" {3} else if existed {2} else {1}}]}),
            );
        }
        _ => panic!("unsupported signature action"),
    }
}

fn result<R>(server: &mut TestServer, target: &str, position: &Value) -> Value
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned + serde::Serialize,
{
    let response = response_value(request::<R>(
        server,
        2,
        json!({"textDocument":{"uri":target},"position":position}),
    ));
    assert!(response.get("error").is_none(), "{response}");
    assert!(response.get("result").is_some(), "explicit JSON null");
    response["result"].clone()
}

fn verify(
    server: &mut TestServer,
    fixture: &FixtureWorkspace,
    layout: &oracle::Layout,
    spec: &Spec,
    phase: &Value,
) -> Vec<Value> {
    oracle::cases(spec, phase)
        .iter()
        .map(|case| {
            let file = case["file"].as_str().expect("query file");
            let source = fixture.document(file).expect("query source");
            let target = uri(layout, file);
            let point = hover_signature::position(
                source,
                case["marker"].as_str().expect("argument marker"),
                true,
                0,
            );
            let actual = result::<r::SignatureHelpRequest>(server, &target, &point);
            assert_eq!(
                actual,
                hover_signature::signature_result(case, true),
                "{} {}",
                phase["id"],
                case["id"]
            );
            assert_eq!(
                result::<r::SignatureHelpRequest>(server, &target, &point),
                actual,
                "repeat signature"
            );
            recovery::assert_query(server, layout, case, source);
            let Some(callee_marker) = case["callee"].as_str() else {
                return json!({"signature":actual});
            };
            let callee = hover_signature::position(source, callee_marker, true, 1);
            let mut definition = result::<r::GotoDefinition>(server, &target, &callee);
            assert_eq!(
                definition,
                oracle::definition(fixture, case, true, |file| uri(layout, file)),
                "{} {} target",
                phase["id"],
                case["id"]
            );
            assert_eq!(
                result::<r::GotoDefinition>(server, &target, &callee),
                definition,
                "repeat definition"
            );
            // Each server's URI was checked in full first. Remove only the unique
            // owned root for incremental/fresh comparisons across separate roots.
            if !definition.is_null() {
                definition["uri"] = case["definition"]["file"].clone();
            }
            json!({"signature":actual,"definition":definition})
        })
        .collect()
}

#[test]
fn signature_package_matrix_preserves_import_ownership_and_lifecycle_facts() {
    verify_fixture("signature-s8", 544);
}

#[test]
fn signature_recovery_matrix_preserves_known_facts_and_clears_unavailable_schema() {
    verify_fixture("signature-s9", 572);
}

fn verify_fixture(name: &str, expected_count: usize) {
    for crlf in [false, true] {
        let spec = oracle::spec(name, crlf);
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let original_disk = fixture.disk.clone();
        fixture.open.insert(
            "scripts/main.vela".into(),
            fixture.disk["scripts/main.vela"].clone(),
        );
        let layout = oracle::Layout::new(&fixture);
        let mut server = initialize(&fixture, &layout);
        let mut version = 1;
        let mut initial = None;
        let mut last = Vec::new();
        let mut total = 0;
        for phase in spec.oracle["phases"].as_array().expect("phases") {
            for action in oracle::actions(phase) {
                fixture.apply(&action).expect("action");
                version += 1;
                apply(&mut server, &fixture, &layout, &action, version);
            }
            recovery::assert_schema(&server, phase);
            last = verify(&mut server, &fixture, &layout, &spec, phase);
            let fresh_layout = oracle::Layout::new(&fixture);
            let mut fresh = initialize(&fixture, &fresh_layout);
            recovery::assert_schema(&fresh, phase);
            assert_eq!(
                last,
                verify(&mut fresh, &fixture, &fresh_layout, &spec, phase),
                "incremental / fresh: {} CRLF={crlf}",
                phase["id"]
            );
            total += last.len();
            initial.get_or_insert_with(|| last.clone());
            layout.assert_disk(&fixture, &spec);
            fresh_layout.assert_disk(&fixture, &spec);
        }
        assert_eq!(total, expected_count, "all queries at all phases");
        assert_eq!(
            last,
            initial.expect("disk baseline"),
            "restored facts and targets"
        );
        assert_eq!(fixture.disk, original_disk, "original disk restored");
        assert!(fixture.open.is_empty(), "overlays closed");
    }
}
