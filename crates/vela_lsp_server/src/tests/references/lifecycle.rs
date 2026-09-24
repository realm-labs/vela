use std::{collections::BTreeMap, fs};

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

use super::{TestServer, request, response_value};
use crate::matrix_fixture::{FixtureWorkspace, Spec, load};
use crate::tests::notify;

#[test]
fn closing_a_saved_renamed_source_restores_current_disk_declaration() {
    let parent = crate::tests::support::unique_temp_root("rename-close-source");
    let root = parent.join("中文 % rename close");
    let scripts = root.join("scripts");
    fs::create_dir_all(&scripts).expect("scripts directory");
    fs::write(root.join("vela.toml"), "[package]\nid='dev.vela.rename_close'\nname='rename_close'\nversion='0.1.0'\n[source]\nroots=['scripts']\n").expect("package config");
    let origin = scripts.join("origin.vela");
    let caller = scripts.join("caller.vela");
    let old_origin = "/* 中😀 */ pub fn grant(value: i64) -> i64 { value }\n";
    let new_origin = "/* 中😀 */ pub fn award(value: i64) -> i64 { value }\n";
    let old_caller = "/* 中😀 */ use origin::grant;\nfn call() { grant(1); }\n";
    let new_caller = "/* 中😀 */ use origin::award;\nfn call() { award(1); }\n";
    fs::write(&origin, old_origin).expect("old origin");
    fs::write(&caller, old_caller).expect("old caller");
    let root_uri = lsp_types::Url::from_file_path(&root).expect("root URI");
    let origin_uri = lsp_types::Url::from_file_path(&origin).expect("origin URI");
    let caller_uri = lsp_types::Url::from_file_path(&caller).expect("caller URI");
    let mut server = TestServer::new();
    let _ = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({
            "processId":null,"rootUri":root_uri,"capabilities":{"workspace":{"workspaceEdit":{"documentChanges":true}}}
        }),
    ));
    for (uri, text) in [(&origin_uri, old_origin), (&caller_uri, old_caller)] {
        let _ = notify::<n::DidOpenTextDocument>(
            &mut server,
            json!({
                "textDocument":{"uri":uri,"languageId":"vela","version":1,"text":text}
            }),
        );
    }
    fs::write(&origin, new_origin).expect("renamed origin on disk");
    fs::write(&caller, new_caller).expect("renamed caller on disk");
    for (uri, text) in [(&caller_uri, new_caller), (&origin_uri, new_origin)] {
        let _ = notify::<n::DidChangeTextDocument>(
            &mut server,
            json!({
                "textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":text}]
            }),
        );
    }
    let _ = notify::<n::DidCloseTextDocument>(
        &mut server,
        json!({
            "textDocument":{"uri":origin_uri}
        }),
    );
    let response = response_value(request::<r::References>(
        &mut server,
        2,
        json!({
            "textDocument":{"uri":caller_uri},"position":{"line":1,"character":13},
            "context":{"includeDeclaration":true}
        }),
    ));
    let refs = response["result"].as_array().expect("resolved references");
    assert_eq!(
        refs.len(),
        3,
        "source declaration, import and call must retain one owner: {response}"
    );
    assert!(
        refs.iter()
            .any(|location| location["uri"] == origin_uri.as_str())
    );
    fs::remove_dir_all(parent).expect("remove fixture");
}

#[test]
fn reference_rename_lifecycle_projects_overlay_close_and_dependency_states() {
    for crlf in [false, true] {
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
        let mut fixture = FixtureWorkspace::new(&spec).expect("lifecycle fixture");
        let parent = crate::tests::support::unique_temp_root("reference-lifecycle");
        let root = parent.join("中文 % lifecycle");
        fixture.materialize(&root).expect("isolated root");
        let mut driver = Driver {
            root,
            endpoint: TestServer::new(),
            id: 0,
            versions: BTreeMap::new(),
        };
        let root_uri = driver.uri("");
        let _ = driver.query::<r::Initialize>(
            json!({"processId":null,"rootUri":root_uri,"capabilities":{"workspace":{"workspaceEdit":{"documentChanges":true}}}}),
        );
        for (index, action) in spec.actions.iter().enumerate() {
            fixture.apply(action).expect("lifecycle action");
            driver.apply(action, &fixture);
            let owner = spec.oracle["ownerAfterEachAction"][index]
                .as_str()
                .expect("explicit owner state");
            driver.check(&fixture, &spec, owner, index, crlf);
        }
        fs::remove_dir_all(parent).expect("remove isolated fixture");
    }
}

struct Driver {
    root: std::path::PathBuf,
    endpoint: TestServer,
    id: i32,
    versions: BTreeMap<String, i32>,
}

impl Driver {
    fn uri(&self, file: &str) -> String {
        lsp_types::Url::from_file_path(self.root.join(file))
            .expect("file URI")
            .to_string()
    }

    fn query<R: r::Request>(&mut self, params: Value) -> Value {
        self.id += 1;
        let response = response_value(request::<R>(&mut self.endpoint, self.id, params));
        assert_eq!(response["id"], self.id);
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    }

    fn apply(&mut self, action: &crate::matrix_fixture::Action, fixture: &FixtureWorkspace) {
        let file_uri = self.uri(&action.file);
        match action.op.as_str() {
            "open" => {
                self.versions.insert(action.file.clone(), 1);
                let _ = notify::<n::DidOpenTextDocument>(
                    &mut self.endpoint,
                    json!({"textDocument":{"uri":file_uri,"languageId":"vela","version":1,"text":fixture.open[&action.file].text}}),
                );
            }
            "change" => {
                let version = self.versions.get_mut(&action.file).expect("open version");
                *version += 1;
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut self.endpoint,
                    json!({"textDocument":{"uri":file_uri,"version":version},"contentChanges":[{"text":fixture.open[&action.file].text}]}),
                );
            }
            "close" => {
                self.versions.remove(&action.file).expect("open file");
                let _ = notify::<n::DidCloseTextDocument>(
                    &mut self.endpoint,
                    json!({"textDocument":{"uri":file_uri}}),
                );
            }
            "save" => {
                fs::write(
                    self.root.join(&action.file),
                    &fixture.disk[&action.file].text,
                )
                .expect("save fixture");
                let _ = notify::<n::DidSaveTextDocument>(
                    &mut self.endpoint,
                    json!({"textDocument":{"uri":file_uri}}),
                );
            }
            "write" | "delete" => {
                let path = self.root.join(&action.file);
                let existed = path.exists();
                let kind = if action.op == "delete" {
                    fs::remove_file(&path).expect("delete fixture");
                    3
                } else {
                    fs::write(&path, &fixture.disk[&action.file].text).expect("write fixture");
                    if existed { 2 } else { 1 }
                };
                let _ = notify::<n::DidChangeWatchedFiles>(
                    &mut self.endpoint,
                    json!({"changes":[{"uri":file_uri,"type":kind}]}),
                );
            }
            _ => panic!("unsupported action"),
        }
    }

    fn check(
        &mut self,
        fixture: &FixtureWorkspace,
        spec: &Spec,
        owner: &str,
        index: usize,
        crlf: bool,
    ) {
        let context = format!("state {index}, CRLF={crlf}");
        let main = "scripts/main.vela";
        let params = self.params(fixture, main, "call");
        let sites = match owner {
            "helper" => spec.oracle["sites"]
                .as_array()
                .expect("helper sites")
                .clone(),
            "other" => spec.oracle["rebound"]
                .as_array()
                .expect("other sites")
                .clone(),
            "unresolved" => Vec::new(),
            _ => panic!("unknown owner state"),
        };
        for _ in 0..2 {
            for include in [false, true] {
                let actual = self.query::<r::References>(json!({
                    "textDocument":params["textDocument"],"position":params["position"],
                    "context":{"includeDeclaration":include}
                }));
                let expected = sites
                    .iter()
                    .filter(|site| include || site["kind"] != "Declaration")
                    .map(|site| self.site(fixture, site))
                    .collect::<Vec<_>>();
                assert_eq!(
                    sorted(actual.as_array().expect("reference array").clone()),
                    sorted(expected),
                    "references {context}"
                );
            }
            let highlights = self.query::<r::DocumentHighlightRequest>(params.clone());
            let expected = if owner != "unresolved" {
                vec![
                    json!({"range":marker_range(fixture,main,"import"),"kind":1}),
                    json!({"range":marker_range(fixture,main,"call"),"kind":1}),
                ]
            } else {
                Vec::new()
            };
            assert_eq!(
                sorted(highlights.as_array().expect("highlight array").clone()),
                sorted(expected),
                "highlights {context}"
            );
            let prepare = self.query::<r::PrepareRenameRequest>(params.clone());
            let edit = self.query::<r::Rename>(json!({
                "textDocument":params["textDocument"],"position":params["position"],
                "newName":spec.oracle["replacement"]
            }));
            if owner != "unresolved" {
                assert_eq!(
                    prepare,
                    json!({"range":marker_range(fixture,main,"call"),"placeholder":"increment"}),
                    "prepare {context}"
                );
                self.check_edits(fixture, spec, &edit, &context, owner);
            } else {
                assert!(prepare.is_null(), "unresolved prepare {context}");
                assert!(edit.is_null(), "unresolved rename {context}");
            }
        }
        let other = "scripts/other.vela";
        let other_params = self.params(fixture, other, "other-decl");
        let actual = self.query::<r::References>(json!({
            "textDocument":other_params["textDocument"],"position":other_params["position"],
            "context":{"includeDeclaration":true}
        }));
        let other_sites = if owner == "other" {
            &spec.oracle["rebound"]
        } else {
            &spec.oracle["independent"]
        };
        let expected = other_sites
            .as_array()
            .expect("other sites")
            .iter()
            .map(|site| self.site(fixture, site))
            .collect::<Vec<_>>();
        assert_eq!(
            sorted(actual.as_array().expect("other array").clone()),
            sorted(expected),
            "other owner {context}"
        );
        if owner == "other" {
            let helper = "scripts/helper.vela";
            let helper_params = self.params(fixture, helper, "definition");
            let actual = self.query::<r::References>(json!({
                "textDocument":helper_params["textDocument"],"position":helper_params["position"],
                "context":{"includeDeclaration":true}
            }));
            let expected = spec.oracle["helperDuringRebind"]
                .as_array()
                .expect("helper sites")
                .iter()
                .map(|site| self.site(fixture, site))
                .collect::<Vec<_>>();
            assert_eq!(
                sorted(actual.as_array().expect("helper array").clone()),
                sorted(expected),
                "helper during rebind {context}"
            );
        }
    }

    fn check_edits(
        &self,
        fixture: &FixtureWorkspace,
        spec: &Spec,
        edit: &Value,
        context: &str,
        owner: &str,
    ) {
        let mut expected = BTreeMap::<String, Vec<Value>>::new();
        let sites = if owner == "helper" {
            &spec.oracle["sites"]
        } else {
            &spec.oracle["rebound"]
        };
        for site in sites.as_array().expect("sites") {
            let file = site["file"].as_str().expect("file");
            expected.entry(self.uri(file)).or_default().push(json!({
                "range":self.site(fixture,site)["range"],"newText":spec.oracle["replacement"]
            }));
        }
        let actual: BTreeMap<String, Vec<Value>> =
            serde_json::from_value(edit["changes"].clone()).expect("legacy edits");
        assert_eq!(
            sort_map(actual),
            sort_map(expected.clone()),
            "rename changes {context}"
        );
        let expected_changes = expected
            .into_iter()
            .map(|(uri, edits)| {
                let version = self
                    .versions
                    .iter()
                    .find_map(|(file, version)| (self.uri(file) == uri).then_some(*version));
                json!({"textDocument":{"uri":uri,"version":version},"edits":sorted(edits)})
            })
            .collect::<Vec<_>>();
        let actual_changes = edit["documentChanges"].as_array().expect("versioned changes")
            .iter().map(|change|json!({"textDocument":change["textDocument"],"edits":sorted(change["edits"].as_array().expect("edits").clone())}))
            .collect::<Vec<_>>();
        assert_eq!(
            sorted(actual_changes),
            sorted(expected_changes),
            "versioned rename {context}"
        );
    }

    fn params(&self, fixture: &FixtureWorkspace, file: &str, name: &str) -> Value {
        let marker = fixture.document(file).expect("document").markers[name];
        json!({"textDocument":{"uri":self.uri(file)},"position":{"line":marker.start.line,"character":marker.start.character+1}})
    }

    fn site(&self, fixture: &FixtureWorkspace, site: &Value) -> Value {
        let file = site["file"].as_str().expect("file");
        let name = site["marker"].as_str().expect("marker");
        json!({"uri":self.uri(file),"range":marker_range(fixture,file,name)})
    }
}

fn marker_range(fixture: &FixtureWorkspace, file: &str, name: &str) -> Value {
    let marker = fixture.document(file).expect("document").markers[name];
    json!({"start":{"line":marker.start.line,"character":marker.start.character},
           "end":{"line":marker.end.line,"character":marker.end.character}})
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
