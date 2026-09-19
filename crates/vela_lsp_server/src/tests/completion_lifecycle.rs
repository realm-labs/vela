use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

use super::{TestServer, notify, request, response_value};
use crate::task::TaskOutcome;

struct Fixture {
    live: TestServer,
    uri: lsp_types::Url,
    schema: std::path::PathBuf,
    crlf: bool,
}

impl Fixture {
    fn new(crlf: bool, version: i32) -> Self {
        let root = super::support::unique_temp_root("completion-request-lifecycle");
        std::fs::create_dir_all(root.join("scripts")).expect("workspace");
        std::fs::write(root.join("vela.toml"),
            "[package]\nid = \"dev.vela.request_lifecycle\"\nname = \"request_lifecycle\"\nversion = \"0.1.0\"\n[source]\nroots = [\"scripts\"]\n[host]\nschema = \"schema.json\"\n")
            .expect("manifest");
        let schema = root.join("schema.json");
        write_schema(&schema, false);
        let uri = lsp_types::Url::from_file_path(root.join("scripts/main.vela")).expect("URI");
        let mut server = TestServer::new();
        let initialized = response_value(request::<r::Initialize>(
            &mut server,
            0,
            json!({
                "processId":null,"rootUri":lsp_types::Url::from_file_path(&root).expect("root URI"),
                "capabilities":{"workspace":{"workspaceEdit":{"documentChanges":true}}}
            }),
        ));
        assert!(initialized.get("error").is_none());
        let mut fixture = Self {
            live: server,
            uri,
            schema,
            crlf,
        };
        fixture.open(version, "Pla");
        fixture
    }

    fn source(&self, prefix: &str) -> String {
        let eol = if self.crlf { "\r\n" } else { "\n" };
        format!("fn main() {{ let text = \"中😀\"; {prefix} }}{eol}")
    }

    fn open(&mut self, version: i32, prefix: &str) {
        let source = self.source(prefix);
        let messages = notify::<n::DidOpenTextDocument>(
            &mut self.live,
            json!({
                "textDocument":{"uri":self.uri,"languageId":"vela","version":version,"text":source}
            }),
        );
        assert!(!messages.is_empty());
    }

    fn change(&mut self, version: i32, changes: Value) -> Vec<lsp_server::Message> {
        notify::<n::DidChangeTextDocument>(
            &mut self.live,
            json!({
                "textDocument":{"uri":self.uri,"version":version},"contentChanges":changes
            }),
        )
    }

    fn params(&self) -> Value {
        json!({"textDocument":{"uri":self.uri},"position":{"line":0,"character":33}})
    }

    fn completion(&mut self, expected: &[&str]) -> Value {
        let params = self.params();
        let response = response_value(request::<r::Completion>(&mut self.live, 10, params));
        assert!(response.get("error").is_none(), "{response}");
        assert_candidates(&response["result"], expected);
        response["result"].clone()
    }

    fn assert_versioned_rename(&mut self, version: i32) {
        let response = response_value(request::<r::Rename>(
            &mut self.live,
            11,
            json!({
                "textDocument":{"uri":self.uri},"position":{"line":0,"character":17},"newName":"renamed"
            }),
        ));
        assert!(response.get("error").is_none(), "{response}");
        let changes = response["result"]["documentChanges"]
            .as_array()
            .expect("versioned edits");
        assert_eq!(changes.len(), 1, "{response}");
        assert_eq!(
            changes[0]["textDocument"],
            json!({"uri":self.uri,"version":version})
        );
        assert_eq!(
            changes[0]["edits"],
            json!([{
                "range":{"start":{"line":0,"character":16},"end":{"line":0,"character":20}},"newText":"renamed"
            }])
        );
    }

    fn update_schema(&mut self, updated: bool) {
        write_schema(&self.schema, updated);
        let _ = notify::<n::DidChangeWatchedFiles>(
            &mut self.live,
            json!({"changes":[{
                "uri":lsp_types::Url::from_file_path(&self.schema).expect("schema URI"),"type":2
            }]}),
        );
    }

    fn candidate(&mut self) -> Value {
        self.completion(&["Player"])["items"][0].clone()
    }

    fn assert_current_request(&mut self, method: &str, params: &Value, updated: bool) -> Value {
        self.live.queue_request(90, method, params.clone());
        let task = self.live.receive_task();
        let (outcome, messages) = self.live.publish_task(task);
        assert_eq!(outcome, TaskOutcome::Completed);
        let result = successful_response(messages, 90);
        assert_current(&result, method, params, updated);
        result
    }
}

fn write_schema(path: &std::path::Path, updated: bool) {
    let mut types = vec![
        json!({"name":"Player","fact":{"kind":"host","name":"Player"},"docs":if updated {"Current Player docs."} else {"Original Player docs."}}),
        json!({"name":"Photon","fact":{"kind":"host","name":"Photon"},"docs":"Photon docs."}),
    ];
    if updated {
        types.push(
            json!({"name":"Planet","fact":{"kind":"host","name":"Planet"},"docs":"Planet docs."}),
        );
    }
    std::fs::write(
        path,
        json!({"formatVersion":1,"facts":{"types":types}}).to_string(),
    )
    .expect("schema");
}

fn assert_candidates(result: &Value, expected: &[&str]) {
    let items = result["items"].as_array().expect("completion items");
    let mut labels: Vec<_> = items
        .iter()
        .map(|item| item["label"].as_str().expect("label"))
        .collect();
    labels.sort_unstable();
    assert_eq!(labels, expected, "{result}");
    for item in items {
        assert_eq!(item["kind"], 22);
        assert_eq!(item["detail"], item["label"]);
        assert_eq!(
            item["textEdit"],
            json!({
                "range":{"start":{"line":0,"character":30},"end":{"line":0,"character":33}},
                "newText":item["label"]
            })
        );
        assert!(item.get("documentation").is_none());
    }
}

#[test]
fn old_and_duplicate_versions_preserve_completion_and_signed_edit_versions() {
    for crlf in [false, true] {
        let mut fixture = Fixture::new(crlf, i32::MIN);
        fixture.completion(&["Player"]);
        fixture.assert_versioned_rename(i32::MIN);
        for version in [i32::MIN + 2, -1, 0, 2, i32::MAX] {
            let source = fixture.source("Pho");
            assert!(!fixture.change(version, json!([{"text":source}])).is_empty());
            let expected = fixture.completion(&["Photon"]);
            fixture.assert_versioned_rename(version);
            let before = fixture.live.snapshot();
            for stale in [version, version - 1, i32::MIN] {
                for changes in [
                    json!([{"text":fixture.source("Pla")}]),
                    json!([{"range":{"start":{"line":0,"character":30},"end":{"line":0,"character":33}},"text":"Pla"}]),
                    json!([{"range":{"start":{"line":999,"character":0},"end":{"line":999,"character":1}},"text":"bad"}]),
                    json!([]),
                ] {
                    assert!(
                        fixture.change(stale, changes).is_empty(),
                        "ignored version {stale}"
                    );
                    let after = fixture.live.snapshot();
                    assert_eq!(
                        after.databases().generation(),
                        before.databases().generation()
                    );
                    assert_eq!(
                        after.databases().parse_db().parse_count(),
                        before.databases().parse_db().parse_count()
                    );
                    assert_eq!(
                        after.databases().hir_db().rebuild_count(),
                        before.databases().hir_db().rebuild_count()
                    );
                    assert_eq!(fixture.completion(&["Photon"]), expected);
                }
            }
        }
        let _ = notify::<n::DidCloseTextDocument>(
            &mut fixture.live,
            json!({"textDocument":{"uri":fixture.uri}}),
        );
        fixture.open(-7, "Pla");
        fixture.completion(&["Player"]);
        fixture.assert_versioned_rename(-7);
        assert!(
            !fixture
                .change(-3, json!([{"text":fixture.source("Pho")}]))
                .is_empty()
        );
        fixture.completion(&["Photon"]);
        fixture.assert_versioned_rename(-3);
    }
}

fn successful_response(messages: Vec<lsp_server::Message>, id: i32) -> Value {
    let response = response_value(messages);
    assert_eq!(response["id"], id);
    assert!(response.get("error").is_none(), "{response}");
    response["result"].clone()
}

fn assert_current(result: &Value, method: &str, params: &Value, updated: bool) {
    if method == "textDocument/completion" {
        assert_candidates(
            result,
            if updated {
                &["Planet", "Player"]
            } else {
                &["Player"]
            },
        );
    } else {
        let mut expected = params.clone();
        expected["documentation"] = json!({"kind":"markdown","value":if updated {
            "Current Player docs."
        } else {
            "Original Player docs."
        }});
        assert_eq!(*result, expected);
    }
}

fn request_params(fixture: &mut Fixture, method: &str) -> Value {
    if method == "textDocument/completion" {
        fixture.params()
    } else {
        fixture.candidate()
    }
}

#[test]
fn cancelled_completion_and_resolve_publish_only_errors_and_allow_later_requests() {
    for crlf in [false, true] {
        for method in ["textDocument/completion", "completionItem/resolve"] {
            // Cover cancellation before collection and after computation. Hold
            // the result before publication to remove scheduler race dependence.
            for collect_first in [false, true] {
                let mut fixture = Fixture::new(crlf, 1);
                let params = request_params(&mut fixture, method);
                fixture.live.queue_request(20, method, params.clone());
                let held = collect_first.then(|| fixture.live.receive_task());
                assert!(notify::<n::Cancel>(&mut fixture.live, json!({"id":20})).is_empty());
                fixture.update_schema(true);
                let task = held.unwrap_or_else(|| fixture.live.receive_task());
                let (outcome, messages) = fixture.live.publish_task(task);
                assert_eq!(outcome, TaskOutcome::Cancelled);
                let response = response_value(messages);
                assert_eq!(response["id"], 20);
                assert_eq!(response["error"]["code"], -32800);
                assert!(response.get("result").is_none());
                let expected = fixture.assert_current_request(method, &params, true);
                // Late/unknown cancellation must not poison a subsequent ID.
                assert!(notify::<n::Cancel>(&mut fixture.live, json!({"id":20})).is_empty());
                assert!(notify::<n::Cancel>(&mut fixture.live, json!({"id":91})).is_empty());
                fixture.live.queue_request(91, method, params.clone());
                let task = fixture.live.receive_task();
                let (outcome, messages) = fixture.live.publish_task(task);
                assert_eq!(outcome, TaskOutcome::Completed);
                assert_eq!(successful_response(messages, 91), expected);
            }
        }
    }
}

#[test]
fn stale_completion_and_resolve_retry_current_facts_and_bound_repeated_changes() {
    for crlf in [false, true] {
        for method in ["textDocument/completion", "completionItem/resolve"] {
            for invalidate_retry in [false, true] {
                let mut fixture = Fixture::new(crlf, 1);
                let params = request_params(&mut fixture, method);
                fixture.live.queue_request(30, method, params.clone());
                let stale = fixture.live.receive_task();
                fixture.update_schema(true);
                let (outcome, messages) = fixture.live.publish_task(stale);
                assert_eq!(outcome, TaskOutcome::Retried);
                assert!(messages.is_empty(), "stale facts must not be published");
                let retry = fixture.live.receive_task();
                if invalidate_retry {
                    fixture.update_schema(false);
                }
                let (outcome, messages) = fixture.live.publish_task(retry);
                if invalidate_retry {
                    assert_eq!(outcome, TaskOutcome::StaleDiscarded);
                    let response = response_value(messages);
                    assert_eq!(response["id"], 30);
                    assert_eq!(response["error"]["code"], -32801);
                    assert!(response.get("result").is_none());
                } else {
                    assert_eq!(outcome, TaskOutcome::Completed);
                    let result = successful_response(messages, 30);
                    assert_current(&result, method, &params, true);
                    let current = fixture.assert_current_request(method, &params, true);
                    assert_eq!(result, current);
                }
                fixture.assert_current_request(method, &params, !invalidate_retry);
            }
        }
    }
}
