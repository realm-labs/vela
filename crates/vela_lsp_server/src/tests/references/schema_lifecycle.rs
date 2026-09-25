use std::{collections::BTreeMap, fs, path::Path};

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use vela_language_service::DocumentId;

use crate::matrix_fixture::{FixtureWorkspace, Spec, lifecycle_facts, load, schema_artifact};
use crate::tests::{TestServer, notification_values, notify, request, response_value};

#[test]
fn schema_reference_rename_lifecycle_retargets_clears_and_restores_owners() {
    for crlf in [false, true] {
        let mut spec = load("reference-rename-schema-lifecycle");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("schema fixture");
        let parent = crate::tests::support::unique_temp_root("schema-reference-lifecycle");
        let root = parent.join("中文 % schema references");
        fixture.materialize(&root).expect("workspace");
        fs::create_dir(root.join("target")).expect("schema directory");
        let path = root.join("target/schema.json");
        let mut driver = Driver::new(&root, &fixture);
        for step in spec.oracle["states"].as_array().expect("states") {
            let existed = path.exists();
            let kind = match step["op"].as_str().expect("operation") {
                "delete" => {
                    fs::remove_file(&path).expect("delete schema");
                    3
                }
                "invalid" => {
                    fs::write(&path, "{ broken").expect("invalid schema");
                    2
                }
                "restore" | "replace" => {
                    let snapshot = driver.endpoint.snapshot();
                    let facts = lifecycle_facts(&spec.oracle["schema"], step);
                    let artifact = schema_artifact(&facts, &fixture, |file| {
                        snapshot.databases().source_db().records()
                            [&DocumentId::from(driver.uri(file))]
                            .source_id()
                            .get()
                    });
                    fs::write(&path, artifact.to_string()).expect("schema artifact");
                    if existed { 2 } else { 1 }
                }
                _ => panic!("unknown schema operation"),
            };
            let notifications = notification_values(notify::<n::DidChangeWatchedFiles>(
                &mut driver.endpoint,
                json!({"changes":[{"uri":uri(&root,"target/schema.json"),"type":kind}]}),
            ));
            let publication = notifications
                .iter()
                .find(|value| {
                    value["method"] == "textDocument/publishDiagnostics"
                        && value["params"]["uri"] == uri(&root, "scripts/main.vela")
                })
                .expect("caller diagnostic publication");
            let errors = publication["params"]["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .filter(|value| value["code"] == "schema::unavailable")
                .collect::<Vec<_>>();
            if step["diagnostic"].is_string() {
                assert_eq!(errors.len(), 1, "{}", step["id"]);
            } else {
                assert!(errors.is_empty(), "{}: {errors:?}", step["id"]);
            }
            let mut fresh = Driver::new(&root, &fixture);
            for endpoint in [&mut driver, &mut fresh] {
                for _ in 0..2 {
                    endpoint.check(&fixture, &spec, step, crlf);
                }
            }
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
    fn new(root: &Path, fixture: &FixtureWorkspace) -> Self {
        let mut driver = Self {
            root: root.to_path_buf(),
            endpoint: TestServer::new(),
            id: 0,
            versions: BTreeMap::new(),
        };
        let root_uri = driver.uri("");
        let _ = driver.query::<r::Initialize>(json!({"processId":null,"rootUri":root_uri,
            "capabilities":{"workspace":{"workspaceEdit":{"documentChanges":true}}}}));
        for file in ["scripts/main.vela", "scripts/helpers.vela"] {
            driver.versions.insert(file.to_owned(), 1);
            let _ = notify::<n::DidOpenTextDocument>(
                &mut driver.endpoint,
                json!({"textDocument":{
                "uri":uri(root,file),"languageId":"vela","version":1,"text":fixture.disk[file].text}}),
            );
        }
        driver
    }

    fn uri(&self, file: &str) -> String {
        uri(&self.root, file)
    }

    fn query<R: r::Request>(&mut self, params: Value) -> Value {
        self.id += 1;
        let response = response_value(request::<R>(&mut self.endpoint, self.id, params));
        assert_eq!(response["id"], self.id);
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    }

    fn check(&mut self, fixture: &FixtureWorkspace, spec: &Spec, step: &Value, crlf: bool) {
        let context = format!("{} CRLF={crlf}", step["id"].as_str().expect("state"));
        let owner = step["owner"].as_str().expect("owner");
        let main = "scripts/main.vela";
        for group in ["grant", "award"] {
            let definition = &spec.oracle["groups"][group];
            let active = owner == group;
            let marker = format!("{group}-call");
            let params = self.params(fixture, main, &marker);
            let sites = if active {
                definition["sites"].as_array().expect("sites").clone()
            } else {
                Vec::new()
            };
            for include in [false, true] {
                let actual=self.query::<r::References>(json!({"textDocument":params["textDocument"],"position":params["position"],"context":{"includeDeclaration":include}}));
                let expected = sites
                    .iter()
                    .filter(|site| include || site["kind"] != "Declaration")
                    .map(|site| self.site(fixture, site))
                    .collect::<Vec<_>>();
                assert_eq!(
                    sorted(actual.as_array().expect("references").clone()),
                    sorted(expected),
                    "{group} references {context}"
                );
            }
            let highlights = self.query::<r::DocumentHighlightRequest>(params.clone());
            let expected = if active {
                vec![json!({"range":marker_range(fixture,main,&marker),"kind":1})]
            } else {
                Vec::new()
            };
            assert_eq!(
                sorted(highlights.as_array().expect("highlights").clone()),
                sorted(expected),
                "{group} highlights {context}"
            );
            let target = self.query::<r::GotoDefinition>(params.clone());
            let prepare = self.query::<r::PrepareRenameRequest>(params.clone());
            let edit=self.query::<r::Rename>(json!({"textDocument":params["textDocument"],"position":params["position"],"newName":spec.oracle["replacement"]}));
            if active {
                assert_eq!(
                    target,
                    json!({"uri":self.uri("scripts/helpers.vela"),"range":marker_range(fixture,"scripts/helpers.vela",&format!("{group}-decl"))}),
                    "{group} definition {context}"
                );
                assert_eq!(
                    prepare,
                    json!({"range":marker_range(fixture,main,&marker),"placeholder":definition["name"]}),
                    "{group} prepare {context}"
                );
                self.check_edits(fixture, &sites, &edit, &context, true);
            } else {
                assert!(target.is_null(), "inactive definition {context}");
                assert!(prepare.is_null(), "inactive prepare {context}");
                assert!(edit.is_null(), "inactive rename {context}");
            }
        }
        self.check_source_only(fixture, spec, &context);
        let params = self.params(fixture, main, "ping-call");
        let available = step["metadata"].as_bool().expect("metadata state");
        let refs=self.query::<r::References>(json!({"textDocument":params["textDocument"],"position":params["position"],"context":{"includeDeclaration":true}}));
        let expected = if available {
            vec![self.site(fixture, &spec.oracle["groups"]["metadata"]["sites"][0])]
        } else {
            Vec::new()
        };
        assert_eq!(
            sorted(refs.as_array().expect("metadata refs").clone()),
            sorted(expected),
            "metadata refs {context}"
        );
        let highlights = self.query::<r::DocumentHighlightRequest>(params.clone());
        let expected = if available {
            json!([{"range":marker_range(fixture,main,"ping-call"),"kind":1}])
        } else {
            json!([])
        };
        assert_eq!(highlights, expected, "metadata highlights {context}");
        assert!(self.query::<r::GotoDefinition>(params.clone()).is_null());
        assert!(
            self.query::<r::PrepareRenameRequest>(params.clone())
                .is_null()
        );
        assert!(self.query::<r::Rename>(json!({"textDocument":params["textDocument"],"position":params["position"],"newName":"new_ping"})).is_null());
        let unknown = self.params(fixture, main, "unknown");
        let refs=self.query::<r::References>(json!({"textDocument":unknown["textDocument"],"position":unknown["position"],"context":{"includeDeclaration":true}}));
        assert_eq!(refs, json!([]));
        assert_eq!(
            self.query::<r::DocumentHighlightRequest>(unknown.clone()),
            json!([]),
            "unknown schema module highlights {context}"
        );
        assert!(
            self.query::<r::PrepareRenameRequest>(unknown.clone())
                .is_null()
        );
        assert!(self.query::<r::Rename>(json!({"textDocument":unknown["textDocument"],"position":unknown["position"],"newName":"new_grant"})).is_null());
        let dynamic = self.params(fixture, main, "dynamic");
        let refs = self.query::<r::References>(json!({"textDocument":dynamic["textDocument"],"position":dynamic["position"],"context":{"includeDeclaration":true}}));
        assert_eq!(refs, json!([]));
        assert_eq!(
            self.query::<r::DocumentHighlightRequest>(dynamic.clone()),
            json!([])
        );
        assert!(
            self.query::<r::PrepareRenameRequest>(dynamic.clone())
                .is_null()
        );
        assert!(self.query::<r::Rename>(json!({"textDocument":dynamic["textDocument"],"position":dynamic["position"],"newName":"new_grant"})).is_null());
    }

    fn check_source_only(&mut self, fixture: &FixtureWorkspace, spec: &Spec, context: &str) {
        let main = "scripts/main.vela";
        let definition = &spec.oracle["groups"]["source"];
        let sites = definition["sites"].as_array().expect("source sites");
        let params = self.params(fixture, main, "stable-call");
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
                sorted(actual.as_array().expect("source references").clone()),
                sorted(expected),
                "source references {context}"
            );
        }
        let highlights = self.query::<r::DocumentHighlightRequest>(params.clone());
        assert_eq!(
            highlights,
            json!([{"range":marker_range(fixture,main,"stable-call"),"kind":1}]),
            "source highlights {context}"
        );
        let target = self.query::<r::GotoDefinition>(params.clone());
        assert_eq!(
            target,
            json!({"uri":self.uri("scripts/helpers.vela"),
                "range":marker_range(fixture,"scripts/helpers.vela","stable-decl")}),
            "source definition {context}"
        );
        let prepare = self.query::<r::PrepareRenameRequest>(params.clone());
        assert_eq!(
            prepare,
            json!({"range":marker_range(fixture,main,"stable-call"),"placeholder":"stable"}),
            "source prepare {context}"
        );
        let edit = self.query::<r::Rename>(json!({
            "textDocument":params["textDocument"],"position":params["position"],
            "newName":spec.oracle["replacement"]
        }));
        self.check_edits(fixture, sites, &edit, context, false);
    }

    fn check_edits(
        &self,
        fixture: &FixtureWorkspace,
        sites: &[Value],
        edit: &Value,
        context: &str,
        schema_abi: bool,
    ) {
        let actual_schema_abi = edit["changeAnnotations"]
            .as_object()
            .is_some_and(|annotations| {
                annotations.values().any(|value| {
                    value["description"] == "schemaAbi" && value["needsConfirmation"] == true
                })
            });
        assert_eq!(
            actual_schema_abi, schema_abi,
            "schema ABI warning {context}"
        );
        let mut expected = BTreeMap::<String, Vec<Value>>::new();
        for site in sites {
            let file = site["file"].as_str().expect("file");
            expected
                .entry(self.uri(file))
                .or_default()
                .push(json!({"range":self.site(fixture,site)["range"],"newText":"bestow"}));
        }
        let actual: BTreeMap<String, Vec<Value>> =
            serde_json::from_value(edit["changes"].clone()).expect("legacy changes");
        assert_eq!(
            sort_map(actual),
            sort_map(expected.clone()),
            "legacy edits {context}"
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
        let actual_changes=edit["documentChanges"].as_array().expect("versioned changes").iter().map(|change|
            json!({"textDocument":change["textDocument"],"edits":sorted(change["edits"].as_array().expect("edits").clone())})
        ).collect::<Vec<_>>();
        assert_eq!(
            sorted(actual_changes),
            sorted(expected_changes),
            "versioned edits {context}"
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

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("file URI")
        .to_string()
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
