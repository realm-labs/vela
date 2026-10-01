use super::{TestServer, message_value, support::unique_temp_root};
use crate::{
    LaunchConfiguration,
    matrix_fixture::{FixtureWorkspace, Spec},
};
use lsp_server::Message;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use vela_language_service::DocumentId;

pub(super) struct Layout {
    parent: PathBuf,
    root: PathBuf,
    disk: FixtureWorkspace,
}
impl Layout {
    pub(super) fn new(spec: &Spec) -> Self {
        let parent = unique_temp_root("document-open");
        let root = parent.join("中文 % open");
        let mut disk = FixtureWorkspace::new(spec).expect("inputs");
        for file in spec.oracle["missingDisk"].as_array().expect("missing disk") {
            disk.disk.remove(file.as_str().expect("file"));
        }
        disk.materialize(&root).expect("owned fixture");
        Self { parent, root, disk }
    }
    pub(super) fn uri(&self, file: &str) -> String {
        lsp_types::Url::from_file_path(self.root.join(file))
            .expect("encoded file URI")
            .to_string()
    }
    pub(super) fn id(&self, file: &str) -> DocumentId {
        DocumentId::from(self.uri(file))
    }
    pub(super) fn server(&self, capabilities: Value) -> TestServer {
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
    pub(super) fn check_disk(&self, spec: &Spec) {
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
pub(super) fn send(server: &mut TestServer, value: Value) -> Vec<Value> {
    let message: Message = serde_json::from_value(value).expect("message envelope");
    server
        .send_protocol_message(message)
        .iter()
        .map(message_value)
        .collect()
}
pub(super) fn open(server: &mut TestServer, uri: &str, text: &str, version: i32) -> Vec<Value> {
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
pub(super) fn published(messages: &[Value], uri: &str) -> Vec<Value> {
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
pub(super) fn profiles() -> [Value; 3] {
    [
        json!({}),
        json!({"textDocument":{"publishDiagnostics":{"versionSupport":true,"relatedInformation":false}}}),
        json!({"textDocument":{"publishDiagnostics":{"versionSupport":true,"relatedInformation":true,"tagSupport":{"valueSet":[1,2]}}},
            "workspace":{"didChangeWatchedFiles":{"dynamicRegistration":true}},"window":{"workDoneProgress":true}}),
    ]
}
