use super::{TestServer, message_value, support::unique_temp_root};
use crate::{
    LaunchConfiguration,
    matrix_fixture::{FixtureWorkspace, Spec, document_open as oracle, parse_markers},
};
use lsp_server::Message;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use vela_language_service::{DocumentId, SourceVersion};

struct Layout {
    parent: PathBuf,
    root: PathBuf,
    disk: FixtureWorkspace,
}
impl Layout {
    fn new(spec: &Spec) -> Self {
        let parent = unique_temp_root("document-open");
        let root = parent.join("中文 % open");
        let mut disk = FixtureWorkspace::new(spec).expect("inputs");
        for file in spec.oracle["missingDisk"].as_array().expect("missing disk") {
            disk.disk.remove(file.as_str().expect("file"));
        }
        disk.materialize(&root).expect("owned fixture");
        Self { parent, root, disk }
    }
    fn uri(&self, file: &str) -> String {
        lsp_types::Url::from_file_path(self.root.join(file))
            .expect("encoded file URI")
            .to_string()
    }
    fn id(&self, file: &str) -> DocumentId {
        DocumentId::from(self.uri(file))
    }
    fn server(&self, capabilities: Value) -> TestServer {
        let mut config = LaunchConfiguration::new();
        config.set_watch_files_enabled(false);
        let mut server = TestServer::with_launch_configuration(config);
        let messages = send(
            &mut server,
            json!({"jsonrpc":"2.0","id":"open-init 中😀","method":"initialize","params":{
            "processId":null,"rootUri":self.uri(""),"capabilities":capabilities}}),
        );
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["id"], "open-init 中😀");
        assert_eq!(
            messages[0]["result"]["capabilities"]["textDocumentSync"],
            json!({"openClose":true,"change":2,"save":false})
        );
        assert!(
            send(
                &mut server,
                json!({"jsonrpc":"2.0","method":"initialized","params":{}})
            )
            .is_empty()
        );
        server
    }
    fn check_disk(&self, spec: &Spec) {
        for (file, doc) in &self.disk.disk {
            assert_eq!(
                fs::read_to_string(self.root.join(file)).expect("disk"),
                doc.text
            );
        }
        for file in spec.oracle["missingDisk"].as_array().expect("missing disk") {
            assert!(!self.root.join(file.as_str().expect("file")).exists());
        }
    }
}
impl Drop for Layout {
    fn drop(&mut self) {
        let parent = self.parent.canonicalize().expect("owned parent");
        assert_eq!(
            parent.parent(),
            Some(
                std::env::temp_dir()
                    .canonicalize()
                    .expect("temporary root")
                    .as_path()
            )
        );
        assert!(
            parent
                .file_name()
                .expect("name")
                .to_string_lossy()
                .starts_with("vela-lsp-document-open-")
        );
        fs::remove_dir_all(parent).expect("remove owned package");
    }
}
fn send(server: &mut TestServer, value: Value) -> Vec<Value> {
    let message: Message = serde_json::from_value(value).expect("message envelope");
    server
        .send_protocol_message(message)
        .iter()
        .map(message_value)
        .collect()
}
fn open(server: &mut TestServer, uri: &str, text: &str, version: i32) -> Vec<Value> {
    send(
        server,
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{
        "uri":uri,"languageId":"vela","version":version,"text":text}}}),
    )
}
fn core(d: &Value) -> Value {
    assert_eq!(d["source"], "vela");
    json!({"code":d["code"],"message":d["message"],"severity":d["severity"],"range":d["range"],
        "labels":d["data"]["labels"],"candidates":d["data"]["candidates"].as_array().expect("candidate array").iter()
            .map(|c|c["replacement"].clone()).collect::<Vec<_>>(),
        "repairHints":d["data"]["repairHints"].as_array().expect("repair array").len()})
}
fn published(messages: &[Value], uri: &str) -> Vec<Value> {
    let same = messages
        .iter()
        .filter(|m| m["params"]["uri"] == uri)
        .collect::<Vec<_>>();
    assert_eq!(same.len(), 1, "exactly one current publication for {uri}");
    assert_eq!(same[0]["jsonrpc"], "2.0");
    assert_eq!(same[0]["method"], "textDocument/publishDiagnostics");
    assert!(same[0]["params"].get("version").is_none());
    same[0]["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .map(core)
        .collect()
}
fn profiles() -> [Value; 3] {
    [
        json!({}),
        json!({"textDocument":{"publishDiagnostics":{"versionSupport":true,"relatedInformation":false}}}),
        json!({"textDocument":{"publishDiagnostics":{"versionSupport":true,"relatedInformation":true,"tagSupport":{"valueSet":[1,2]}}},
            "workspace":{"didChangeWatchedFiles":{"dynamicRegistration":true}},"window":{"workDoneProgress":true}}),
    ]
}

#[test]
fn document_open_matrix_preserves_complete_source_and_diagnostics_across_client_profiles() {
    for crlf in [false, true] {
        let spec = oracle::spec(crlf);
        let layout = Layout::new(&spec);
        for capabilities in profiles() {
            for case in spec.oracle["cases"].as_array().expect("cases") {
                let file = case["file"].as_str().expect("file");
                let uri = layout.uri(file);
                let id = layout.id(file);
                let mut server = layout.server(capabilities.clone());
                assert!(
                    server.snapshot().workspace().document(&id).is_none(),
                    "not opened yet"
                );
                let disk_record = server
                    .snapshot()
                    .databases()
                    .source_db()
                    .records()
                    .get(&id)
                    .map(|r| r.text().to_owned());
                assert_eq!(
                    disk_record.is_some(),
                    file.ends_with(".vela") && !oracle::missing_disk(&spec, file)
                );
                for (index, version) in [0_i32, -1, i32::MAX, i32::MIN].into_iter().enumerate() {
                    let doc = oracle::document(&spec, file, index % 2 == 1);
                    let old = server.snapshot();
                    let previous = old
                        .workspace()
                        .document(&id)
                        .map(|d| (d.text().to_owned(), d.version()));
                    let messages = open(&mut server, &uri, &doc.text, version);
                    assert_eq!(messages.len(), 1, "only opened document changes; {file}");
                    assert_eq!(
                        published(&messages, &uri),
                        oracle::expected(&doc, case, &uri),
                        "{file}, CRLF={crlf}, version={version}"
                    );
                    let current = server.snapshot();
                    let text = current.workspace().document(&id).expect("open document");
                    let bits =
                        SourceVersion::new(u64::from(u32::from_ne_bytes(version.to_ne_bytes())));
                    assert_eq!((text.text(), text.version()), (doc.text.as_str(), bits));
                    assert_eq!(
                        current.workspace().open_document_ids().collect::<Vec<_>>(),
                        vec![id.clone()]
                    );
                    assert_ne!(current.generation(), old.generation());
                    assert_eq!(
                        old.workspace()
                            .document(&id)
                            .map(|d| (d.text().to_owned(), d.version())),
                        previous
                    );
                    if file.ends_with(".vela") {
                        let record = &current.databases().source_db().records()[&id];
                        assert_eq!((record.text(), record.version()), (doc.text.as_str(), bits));
                    } else {
                        assert!(!current.databases().source_db().records().contains_key(&id));
                    }
                    let mut fresh = layout.server(capabilities.clone());
                    assert_eq!(
                        published(&open(&mut fresh, &uri, &doc.text, version), &uri),
                        oracle::expected(&doc, case, &uri),
                        "fresh"
                    );
                    layout.check_disk(&spec);
                }
            }
        }
    }
}

#[test]
fn document_open_dependency_changes_republish_current_importer_and_preserve_old_snapshots() {
    for crlf in [false, true] {
        let spec = oracle::spec(crlf);
        let layout = Layout::new(&spec);
        let mut server = layout.server(json!({}));
        let caller = oracle::document(&spec, "scripts/open_caller.vela", false);
        let caller_uri = layout.uri("scripts/open_caller.vela");
        assert!(
            published(
                &open(&mut server, &caller_uri, &caller.text, 1),
                &caller_uri
            )
            .is_empty()
        );
        let frozen = server.snapshot();
        let frozen_generation = frozen.generation();
        for (index, phase) in spec.oracle["dependency"]
            .as_array()
            .expect("phases")
            .iter()
            .enumerate()
        {
            let source = parse_markers(
                &phase["text"]
                    .as_str()
                    .expect("text")
                    .replace('\n', if crlf { "\r\n" } else { "\n" }),
            )
            .expect("markers");
            let api_uri = layout.uri("scripts/open_api.vela");
            let point = caller.markers["call"].start;
            server.queue_request(
                20,
                "textDocument/references",
                json!({"textDocument":{"uri":caller_uri},"context":{"includeDeclaration":true},
                "position":{"line":point.line,"character":point.character}}),
            );
            let old_task = server.receive_task();
            let messages = open(&mut server, &api_uri, &source.text, index as i32 + 2);
            assert!(published(&messages, &api_uri).is_empty());
            if phase["id"] == "signature" {
                assert_eq!(messages.len(), 2);
                assert!(published(&messages, &caller_uri).is_empty());
            }
            let (outcome, stale) = server.publish_task(old_task);
            assert_eq!(outcome, crate::task::TaskOutcome::StaleDiscarded);
            assert_eq!(
                stale.iter().map(message_value).collect::<Vec<_>>(),
                vec![json!({
                "jsonrpc":"2.0","id":20,"error":{"code":-32801,
                    "message":"request result is stale because the document was modified"}})]
            );
            assert_eq!(frozen.generation(), frozen_generation);
            assert!(
                frozen
                    .workspace()
                    .document(&layout.id("scripts/open_api.vela"))
                    .is_none()
            );
            let marker = caller.markers["call"];
            for request_id in [10, 11] {
                assert_eq!(
                    send(
                        &mut server,
                        json!({"jsonrpc":"2.0","id":request_id,"method":"textDocument/hover","params":{
                    "textDocument":{"uri":caller_uri},"position":{"line":marker.start.line,"character":marker.start.character}}})
                    ),
                    vec![
                        json!({"jsonrpc":"2.0","id":request_id,"result":{"contents":{"kind":"markdown","value":phase["hover"]},
                        "range":oracle::span(&caller,"call")}})
                    ]
                );
            }
            layout.check_disk(&spec);
        }
    }
}

#[test]
fn document_open_invalid_notifications_and_lifecycle_states_cannot_replace_current_sources() {
    let spec = oracle::spec(false);
    let layout = Layout::new(&spec);
    let uri = layout.uri("scripts/scratch.vela");
    let text = oracle::document(&spec, "scripts/scratch.vela", false).text;
    let mut cold = TestServer::new();
    let generation = cold.snapshot().generation();
    assert!(open(&mut cold, &uri, &text, 1).is_empty());
    assert_eq!(cold.snapshot().generation(), generation);
    assert!(
        cold.snapshot()
            .workspace()
            .document(&layout.id("scripts/scratch.vela"))
            .is_none()
    );
    let notifications = [
        (
            "textDocument/didChange",
            json!({"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":"fn incomplete("}]}),
        ),
        ("textDocument/didClose", json!({"textDocument":{"uri":uri}})),
        (
            "textDocument/didSave",
            json!({"textDocument":{"uri":uri},"text":"fn incomplete("}),
        ),
        ("workspace/didChangeConfiguration", json!({"settings":{}})),
        (
            "workspace/didChangeWatchedFiles",
            json!({"changes":[{"uri":uri,"type":2}]}),
        ),
        (
            "workspace/didChangeWorkspaceFolders",
            json!({"event":{"added":[{"uri":layout.uri(""),"name":"late root"}],"removed":[]}}),
        ),
    ];
    for (method, params) in &notifications {
        assert!(
            send(
                &mut cold,
                json!({"jsonrpc":"2.0","method":method,"params":params})
            )
            .is_empty()
        );
        assert_eq!(cold.snapshot().generation(), generation);
        assert!(cold.snapshot().databases().source_db().records().is_empty());
    }
    let mut server = layout.server(json!({}));
    assert!(published(&open(&mut server, &uri, &text, 1), &uri).is_empty());
    let old = server.snapshot();
    for params in [
        Value::Null,
        json!({}),
        json!({"textDocument":[]}),
        json!({"textDocument":{"uri":"not a URI","languageId":"vela","version":1,"text":"broken"}}),
        json!({"textDocument":{"uri":uri,"languageId":12,"version":1,"text":"broken"}}),
        json!({"textDocument":{"uri":uri,"languageId":"vela","version":"2","text":"broken"}}),
        json!({"textDocument":{"uri":uri,"languageId":"vela","version":2,"text":false}}),
    ] {
        assert!(
            send(
                &mut server,
                json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":params})
            )
            .is_empty()
        );
        assert_eq!(server.snapshot().generation(), old.generation());
        assert_eq!(
            server
                .snapshot()
                .workspace()
                .document(&layout.id("scripts/scratch.vela"))
                .expect("current")
                .text(),
            text
        );
    }
    assert_eq!(
        send(
            &mut server,
            json!({"jsonrpc":"2.0","id":2,"method":"shutdown","params":null})
        ),
        vec![json!({"jsonrpc":"2.0","id":2,"result":null})]
    );
    assert!(open(&mut server, &uri, "fn incomplete(", 2).is_empty());
    for (method, params) in &notifications {
        assert!(
            send(
                &mut server,
                json!({"jsonrpc":"2.0","method":method,"params":params})
            )
            .is_empty()
        );
        assert_eq!(
            server
                .snapshot()
                .workspace()
                .document(&layout.id("scripts/scratch.vela"))
                .expect("still open")
                .text(),
            text
        );
        assert_eq!(server.snapshot().generation(), old.generation());
    }
    assert_eq!(server.snapshot().generation(), old.generation());
    layout.check_disk(&spec);
}
