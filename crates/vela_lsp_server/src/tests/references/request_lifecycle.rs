use std::{collections::BTreeMap, fs, path::PathBuf};

use lsp_server::{Message, Request, RequestId};
use lsp_types::notification as n;
use serde_json::{Value, json};

use crate::matrix_fixture::{Action, FixtureWorkspace, Spec, load};
use crate::task::TaskOutcome;
use crate::tests::{TestServer, notify, response_value};

const METHODS: [&str; 4] = [
    "textDocument/references",
    "textDocument/documentHighlight",
    "textDocument/prepareRename",
    "textDocument/rename",
];

#[test]
fn stale_document_versions_leave_reference_and_rename_facts_unchanged() {
    for crlf in [false, true] {
        let mut fixture = Fixture::new(crlf);
        fixture.change(2, 4);
        fixture.check_all();
        let before = fixture.live.snapshot();
        for version in [4, 3, i32::MIN] {
            for changes in [
                json!([{"text":fixture.spec.files["scripts/helper.vela"]}]),
                json!([{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"text":"// stale\n"}]),
                json!([{"range":{"start":{"line":999,"character":0},"end":{"line":999,"character":1}},"text":"bad"}]),
                json!([]),
            ] {
                assert!(fixture.change_raw(version, changes).is_empty());
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
                fixture.check_all();
            }
        }
        let helper = fixture.uri("scripts/helper.vela");
        let _ = notify::<n::DidCloseTextDocument>(
            &mut fixture.live,
            json!({"textDocument":{"uri":helper}}),
        );
        fixture.version = None;
        let source = fixture.workspace.disk["scripts/helper.vela"].text.clone();
        let _ = notify::<n::DidOpenTextDocument>(
            &mut fixture.live,
            json!({"textDocument":{"uri":helper,"languageId":"vela","version":-7,"text":source}}),
        );
        fixture.version = Some(-7);
        fixture
            .workspace
            .apply(&Action {
                op: "close".into(),
                file: "scripts/helper.vela".into(),
                source: None,
            })
            .expect("close overlay");
        fixture
            .workspace
            .apply(&Action {
                op: "open".into(),
                file: "scripts/helper.vela".into(),
                source: None,
            })
            .expect("reopen overlay");
        fixture.check_all();
        fixture.change(8, -3);
        fixture.check_all();
    }
}

#[test]
fn cancellation_notifications_do_not_poison_reference_and_rename_requests() {
    for crlf in [false, true] {
        for method in METHODS {
            for cancel_before_change in [false, true] {
                let mut fixture = Fixture::new(crlf);
                if cancel_before_change {
                    assert!(notify::<n::Cancel>(&mut fixture.live, json!({"id":20})).is_empty());
                }
                fixture.change(2, 2);
                if !cancel_before_change {
                    assert!(notify::<n::Cancel>(&mut fixture.live, json!({"id":20})).is_empty());
                }
                let response = fixture.request(method, 20);
                fixture.assert_response(method, response, 20);
                fixture.assert_current(method);
                assert!(notify::<n::Cancel>(&mut fixture.live, json!({"id":20})).is_empty());
                assert!(notify::<n::Cancel>(&mut fixture.live, json!({"id":91})).is_empty());
                let response = fixture.request(method, 91);
                fixture.assert_response(method, response, 91);
            }
        }
    }
}

#[test]
fn cancelled_queued_rename_discards_edits_and_preserves_later_requests() {
    for crlf in [false, true] {
        let mut fixture = Fixture::new(crlf);
        fixture.change(2, 4);
        let params = fixture.params("textDocument/rename");
        fixture
            .live
            .queue_request(120, "textDocument/rename", params);
        let task = fixture.live.receive_task();
        assert!(notify::<n::Cancel>(&mut fixture.live, json!({"id":120})).is_empty());
        let (outcome, messages) = fixture.live.publish_task(task);
        assert_eq!(outcome, TaskOutcome::Cancelled);
        let response = response_value(messages);
        assert_eq!(response["id"], 120);
        assert_eq!(response["error"]["code"], -32800);
        assert!(response.get("result").is_none_or(Value::is_null));
        fixture.assert_current("textDocument/rename");
        fixture.check_all();
    }
}

#[test]
fn stale_queued_rename_discards_old_edits_and_uses_current_generation() {
    for crlf in [false, true] {
        let mut fixture = Fixture::new(crlf);
        let params = fixture.params("textDocument/rename");
        fixture
            .live
            .queue_request(121, "textDocument/rename", params);
        let task = fixture.live.receive_task();
        fixture.change(2, 4);
        let (outcome, messages) = fixture.live.publish_task(task);
        assert_eq!(outcome, TaskOutcome::StaleDiscarded);
        let response = response_value(messages);
        assert_eq!(response["id"], 121);
        assert_eq!(response["error"]["code"], -32801);
        assert!(response.get("result").is_none_or(Value::is_null));
        fixture.assert_current("textDocument/rename");
        fixture.check_all();
    }
}

#[test]
fn cancelled_queued_references_discard_locations_and_preserve_later_requests() {
    for crlf in [false, true] {
        let mut fixture = Fixture::new(crlf);
        fixture.change(2, 4);
        let params = fixture.params("textDocument/references");
        fixture
            .live
            .queue_request(122, "textDocument/references", params);
        let task = fixture.live.receive_task();
        assert!(notify::<n::Cancel>(&mut fixture.live, json!({"id":122})).is_empty());
        let (outcome, messages) = fixture.live.publish_task(task);
        assert_eq!(outcome, TaskOutcome::Cancelled);
        let response = response_value(messages);
        assert_eq!(response["id"], 122);
        assert_eq!(response["error"]["code"], -32800);
        assert!(response.get("result").is_none_or(Value::is_null));
        fixture.assert_current("textDocument/references");
        fixture.check_all();
    }
}

#[test]
fn stale_queued_references_discard_old_locations_and_use_current_generation() {
    for crlf in [false, true] {
        let mut fixture = Fixture::new(crlf);
        let params = fixture.params("textDocument/references");
        fixture
            .live
            .queue_request(123, "textDocument/references", params);
        let task = fixture.live.receive_task();
        fixture.change(2, 4);
        let (outcome, messages) = fixture.live.publish_task(task);
        assert_eq!(outcome, TaskOutcome::StaleDiscarded);
        let response = response_value(messages);
        assert_eq!(response["id"], 123);
        assert_eq!(response["error"]["code"], -32801);
        assert!(response.get("result").is_none_or(Value::is_null));
        fixture.assert_current("textDocument/references");
        fixture.check_all();
    }
}

#[test]
fn body_only_edit_preserves_exact_reference_and_rename_locations() {
    for crlf in [false, true] {
        let mut fixture = Fixture::new(crlf);
        let original_range = fixture.range("scripts/helper.vela", "definition");
        let original_generation = fixture.live.snapshot().databases().generation();
        let source = fixture.spec.files["scripts/helper.vela"].replace("value + 1", "value + 9");
        fixture
            .workspace
            .apply(&Action {
                op: "change".into(),
                file: "scripts/helper.vela".into(),
                source: Some(source),
            })
            .expect("body-only fixture edit");
        let text = fixture.workspace.open["scripts/helper.vela"].text.clone();
        assert!(!fixture.change_raw(2, json!([{"text":text}])).is_empty());
        fixture.version = Some(2);
        assert_ne!(
            fixture.live.snapshot().databases().generation(),
            original_generation
        );
        assert_eq!(
            fixture.range("scripts/helper.vela", "definition"),
            original_range
        );
        fixture.check_all();
    }
}

#[test]
fn successive_reference_and_rename_requests_use_current_generation() {
    for crlf in [false, true] {
        for method in METHODS {
            let mut fixture = Fixture::new(crlf);
            fixture.assert_current(method);
            fixture.change(2, 2);
            fixture.assert_current(method);
            fixture.change(8, 3);
            fixture.assert_current(method);
        }
    }
}

struct Fixture {
    spec: Spec,
    workspace: FixtureWorkspace,
    root: PathBuf,
    live: TestServer,
    version: Option<i32>,
    id: i32,
}

impl Fixture {
    fn new(crlf: bool) -> Self {
        let mut spec = load("reference-rename-lifecycle");
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
        let workspace = FixtureWorkspace::new(&spec).expect("reference fixture");
        let parent = crate::tests::support::unique_temp_root("reference-request-lifecycle");
        let root = parent.join("中文 % reference requests");
        workspace.materialize(&root).expect("workspace");
        let mut fixture = Self {
            spec,
            workspace,
            root,
            live: TestServer::new(),
            version: Some(i32::MIN),
            id: 0,
        };
        let root_uri = fixture.uri("");
        let response = response_value(crate::tests::request::<lsp_types::request::Initialize>(
            &mut fixture.live,
            0,
            json!({"processId":null,"rootUri":root_uri,"capabilities":{"workspace":{"workspaceEdit":{"documentChanges":true}}}}),
        ));
        assert!(response.get("error").is_none(), "{response}");
        for index in [0, 1] {
            let action = fixture.spec.actions[index].clone();
            fixture.workspace.apply(&action).expect("open fixture");
            let file = &action.file;
            let file_uri = fixture.uri(file);
            let _ = notify::<n::DidOpenTextDocument>(
                &mut fixture.live,
                json!({"textDocument":{
                    "uri":file_uri,"languageId":"vela","version":i32::MIN,
                    "text":fixture.workspace.open[file].text
                }}),
            );
        }
        fixture
    }

    fn uri(&self, file: &str) -> String {
        lsp_types::Url::from_file_path(self.root.join(file))
            .expect("file URI")
            .to_string()
    }

    fn change(&mut self, action_index: usize, version: i32) {
        let action = self.spec.actions[action_index].clone();
        assert_eq!(action.file, "scripts/helper.vela");
        self.workspace.apply(&action).expect("change fixture");
        let text = self.workspace.open[&action.file].text.clone();
        assert!(!self.change_raw(version, json!([{"text":text}])).is_empty());
        self.version = Some(version);
    }

    fn change_raw(&mut self, version: i32, changes: Value) -> Vec<lsp_server::Message> {
        let uri = self.uri("scripts/helper.vela");
        notify::<n::DidChangeTextDocument>(
            &mut self.live,
            json!({
                "textDocument":{"uri":uri,"version":version},"contentChanges":changes
            }),
        )
    }

    fn params(&self, method: &str) -> Value {
        let marker = self
            .workspace
            .document("scripts/main.vela")
            .expect("main")
            .markers["call"];
        let mut params = json!({"textDocument":{"uri":self.uri("scripts/main.vela")},
            "position":{"line":marker.start.line,"character":marker.start.character+1}});
        if method == "textDocument/references" {
            params["context"] = json!({"includeDeclaration":true});
        }
        if method == "textDocument/rename" {
            params["newName"] = self.spec.oracle["replacement"].clone();
        }
        params
    }

    fn assert_current(&mut self, method: &str) {
        self.id += 1;
        let response = self.request(method, self.id);
        self.assert_response(method, response, self.id);
    }

    fn request(&mut self, method: &str, id: i32) -> Value {
        let params = self.params(method);
        response_value(self.live.send_protocol_message(Message::Request(Request {
            id: RequestId::from(id),
            method: method.to_owned(),
            params,
        })))
    }

    fn check_all(&mut self) {
        for method in METHODS {
            self.assert_current(method);
        }
    }

    fn assert_response(&self, method: &str, response: Value, id: i32) {
        assert_eq!(response["id"], id);
        assert!(response.get("error").is_none(), "{response}");
        let result = &response["result"];
        match method {
            "textDocument/references" => {
                let actual = result.as_array().expect("references").clone();
                let expected = self.sites().iter().map(|site| json!({
                    "uri":self.uri(site["file"].as_str().expect("file")),
                    "range":self.range(site["file"].as_str().expect("file"), site["marker"].as_str().expect("marker"))
                })).collect::<Vec<_>>();
                assert_eq!(sorted(actual), sorted(expected));
            }
            "textDocument/documentHighlight" => {
                let expected = ["import", "call"].map(|marker| {
                    json!({
                        "range":self.range("scripts/main.vela",marker),"kind":1
                    })
                });
                assert_eq!(
                    sorted(result.as_array().expect("highlights").clone()),
                    sorted(expected.to_vec())
                );
            }
            "textDocument/prepareRename" => {
                assert_eq!(
                    *result,
                    json!({"range":self.range("scripts/main.vela","call"),"placeholder":"increment"})
                );
            }
            "textDocument/rename" => self.assert_edits(result),
            _ => panic!("unknown method"),
        }
    }

    fn sites(&self) -> &[Value] {
        self.spec.oracle["sites"].as_array().expect("site oracle")
    }

    fn assert_edits(&self, edit: &Value) {
        let mut expected = BTreeMap::<String, Vec<Value>>::new();
        for site in self.sites() {
            let file = site["file"].as_str().expect("file");
            let marker = site["marker"].as_str().expect("marker");
            expected.entry(self.uri(file)).or_default().push(json!({
                "range":self.range(file,marker),"newText":self.spec.oracle["replacement"]
            }));
        }
        let actual: BTreeMap<String, Vec<Value>> =
            serde_json::from_value(edit["changes"].clone()).expect("legacy edits");
        assert_eq!(sort_map(actual), sort_map(expected.clone()));
        let expected_changes = expected
            .into_iter()
            .map(|(uri, edits)| {
                let version = (uri == self.uri("scripts/helper.vela"))
                    .then_some(self.version)
                    .flatten()
                    .or_else(|| (uri == self.uri("scripts/main.vela")).then_some(i32::MIN));
                json!({"textDocument":{"uri":uri,"version":version},"edits":sorted(edits)})
            })
            .collect::<Vec<_>>();
        let actual_changes = edit["documentChanges"].as_array().expect("versioned edits").iter().map(|change| {
            json!({"textDocument":change["textDocument"],"edits":sorted(change["edits"].as_array().expect("edits").clone())})
        }).collect::<Vec<_>>();
        assert_eq!(sorted(actual_changes), sorted(expected_changes));
    }

    fn range(&self, file: &str, marker: &str) -> Value {
        let marker = self.workspace.document(file).expect("document").markers[marker];
        json!({"start":{"line":marker.start.line,"character":marker.start.character},
            "end":{"line":marker.end.line,"character":marker.end.character}})
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(self.root.parent().expect("fixture parent")).expect("remove fixture");
    }
}

fn sorted(mut values: Vec<Value>) -> Vec<Value> {
    values.sort_by_key(Value::to_string);
    values
}

fn sort_map(map: BTreeMap<String, Vec<Value>>) -> BTreeMap<String, Vec<Value>> {
    map.into_iter()
        .map(|(key, values)| (key, sorted(values)))
        .collect()
}
