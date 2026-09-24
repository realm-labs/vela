use std::{collections::BTreeMap, fs, path::Path};

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

use crate::matrix_fixture::{FixtureWorkspace, Spec, load};
use crate::tests::{TestServer, notification_values, notify, request, response_value};

#[test]
fn reference_rename_recovery_publishes_current_facts_and_parse_diagnostics() {
    for crlf in [false, true] {
        let mut spec = load("reference-rename-recovery");
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
        let mut fixture = FixtureWorkspace::new(&spec).expect("recovery fixture");
        let parent = crate::tests::support::unique_temp_root("reference-recovery");
        let root = parent.join("中文 % recovery");
        fixture.materialize(&root).expect("workspace");
        let mut driver = Driver::new(&root);
        for (index, action) in spec.actions.iter().enumerate() {
            fixture.apply(action).expect("recovery action");
            let file = &action.file;
            let file_uri = driver.uri(file);
            let publications = match action.op.as_str() {
                "open" => {
                    driver.versions.insert(file.clone(), 1);
                    notification_values(notify::<n::DidOpenTextDocument>(
                        &mut driver.endpoint,
                        json!({"textDocument":{"uri":file_uri,"languageId":"vela","version":1,"text":fixture.open[file].text}}),
                    ))
                }
                "change" => {
                    let version = driver.versions.get_mut(file).expect("open version");
                    *version += 1;
                    notification_values(notify::<n::DidChangeTextDocument>(
                        &mut driver.endpoint,
                        json!({"textDocument":{"uri":file_uri,"version":version},"contentChanges":[{"text":fixture.open[file].text}]}),
                    ))
                }
                _ => panic!("unknown recovery action"),
            };
            let state = &spec.oracle["states"][index];
            if file == "scripts/main.vela" {
                let publication = publications
                    .iter()
                    .find(|value| {
                        value["method"] == "textDocument/publishDiagnostics"
                            && value["params"]["uri"] == file_uri
                    })
                    .expect("main diagnostic publication");
                assert_eq!(
                    publication["params"]["diagnostics"]
                        .as_array()
                        .expect("diagnostics")
                        .iter()
                        .any(|diagnostic| diagnostic["code"] == "E_PARSE"),
                    state["syntaxError"] == true,
                    "{} CRLF={crlf}",
                    state["id"]
                );
            }
            let mut fresh = Driver::new(&root);
            for (open_file, source) in &fixture.open {
                let version = driver.versions[open_file];
                fresh.versions.insert(open_file.clone(), version);
                let open_uri = fresh.uri(open_file);
                let _ = notify::<n::DidOpenTextDocument>(
                    &mut fresh.endpoint,
                    json!({"textDocument":{"uri":open_uri,"languageId":"vela","version":version,"text":source.text}}),
                );
            }
            for endpoint in [&mut driver, &mut fresh] {
                for _ in 0..2 {
                    endpoint.check(&fixture, &spec, state, crlf);
                }
            }
        }
        fs::remove_dir_all(parent).expect("remove fixture");
    }
}

struct Driver {
    root: std::path::PathBuf,
    endpoint: TestServer,
    versions: BTreeMap<String, i32>,
    id: i32,
}

impl Driver {
    fn new(root: &Path) -> Self {
        let mut driver = Self {
            root: root.to_path_buf(),
            endpoint: TestServer::new(),
            versions: BTreeMap::new(),
            id: 0,
        };
        let root_uri = driver.uri("");
        let response = response_value(request::<r::Initialize>(
            &mut driver.endpoint,
            0,
            json!({"processId":null,"rootUri":root_uri,"capabilities":{"workspace":{"workspaceEdit":{"documentChanges":true}}}}),
        ));
        assert!(response.get("error").is_none(), "{response}");
        driver
    }
    fn uri(&self, file: &str) -> String {
        lsp_types::Url::from_file_path(self.root.join(file))
            .expect("URI")
            .to_string()
    }
    fn query<R: r::Request>(&mut self, params: Value) -> Value {
        self.id += 1;
        let response = response_value(request::<R>(&mut self.endpoint, self.id, params));
        assert_eq!(response["id"], self.id);
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    }
    fn params(&self, fixture: &FixtureWorkspace, file: &str, marker: &str) -> Value {
        let point = fixture.document(file).expect("document").markers[marker].start;
        json!({"textDocument":{"uri":self.uri(file)},"position":{"line":point.line,"character":point.character+1}})
    }
    fn check(&mut self, fixture: &FixtureWorkspace, spec: &Spec, state: &Value, crlf: bool) {
        let context = format!("{} CRLF={crlf}", state["id"].as_str().expect("state"));
        let main = "scripts/main.vela";
        let params = self.params(fixture, main, "call");
        let sites = spec.oracle["sites"].as_array().expect("sites");
        for include in [false, true] {
            let actual = self.query::<r::References>(json!({"textDocument":params["textDocument"],
                "position":params["position"],"context":{"includeDeclaration":include}}));
            let expected = sites
                .iter()
                .filter(|site| include || site["kind"] != "Declaration")
                .map(|site| self.site(fixture, site))
                .collect::<Vec<_>>();
            assert_eq!(
                sorted(actual.as_array().expect("references").clone()),
                sorted(expected),
                "refs {context}"
            );
        }
        let highlights = self.query::<r::DocumentHighlightRequest>(params.clone());
        assert_eq!(
            sorted(highlights.as_array().expect("highlights").clone()),
            sorted(vec![
                json!({"range":marker_range(fixture,main,"import"),"kind":1}),
                json!({"range":marker_range(fixture,main,"call"),"kind":1})
            ]),
            "highlights {context}"
        );
        let target = self.query::<r::GotoDefinition>(params.clone());
        assert_eq!(
            target,
            json!({"uri":self.uri("scripts/helper.vela"),
            "range":marker_range(fixture,"scripts/helper.vela","definition")}),
            "definition {context}"
        );
        let prepare = self.query::<r::PrepareRenameRequest>(params.clone());
        assert_eq!(
            prepare,
            json!({"range":marker_range(fixture,main,"call"),"placeholder":"increment"}),
            "prepare {context}"
        );
        let edit = self.query::<r::Rename>(json!({"textDocument":params["textDocument"],
            "position":params["position"],"newName":spec.oracle["replacement"]}));
        self.check_edits(fixture, sites, &edit, &context);
        let decoy = "scripts/decoy.vela";
        let params = self.params(fixture, decoy, "decoy-call");
        let actual = self.query::<r::References>(json!({"textDocument":params["textDocument"],
            "position":params["position"],"context":{"includeDeclaration":true}}));
        let expected = spec.oracle["decoy"]
            .as_array()
            .expect("decoy")
            .iter()
            .map(|site| self.site(fixture, site))
            .collect::<Vec<_>>();
        assert_eq!(
            sorted(actual.as_array().expect("decoy refs").clone()),
            sorted(expected),
            "decoy {context}"
        );
        if state["bad"] == true {
            let bad = self.params(fixture, main, "bad");
            let refs = self.query::<r::References>(json!({"textDocument":bad["textDocument"],
                "position":bad["position"],"context":{"includeDeclaration":true}}));
            assert_eq!(refs, json!([]), "bad refs {context}");
            assert_eq!(
                self.query::<r::DocumentHighlightRequest>(bad.clone()),
                json!([]),
                "bad highlights {context}"
            );
            assert!(
                self.query::<r::PrepareRenameRequest>(bad.clone()).is_null(),
                "bad prepare {context}"
            );
            assert!(
                self.query::<r::Rename>(json!({"textDocument":bad["textDocument"],
                "position":bad["position"],"newName":"advance"}))
                    .is_null(),
                "bad rename {context}"
            );
        }
    }
    fn site(&self, fixture: &FixtureWorkspace, site: &Value) -> Value {
        let file = site["file"].as_str().expect("file");
        json!({"uri":self.uri(file),"range":marker_range(fixture,file,site["marker"].as_str().expect("marker"))})
    }
    fn check_edits(
        &self,
        fixture: &FixtureWorkspace,
        sites: &[Value],
        edit: &Value,
        context: &str,
    ) {
        let mut expected = BTreeMap::<String, Vec<Value>>::new();
        for site in sites {
            let file = site["file"].as_str().expect("file");
            expected.entry(self.uri(file)).or_default().push(json!({
                "range":marker_range(fixture,file,site["marker"].as_str().expect("marker")),
                "newText":"advance"
            }));
        }
        let actual: BTreeMap<String, Vec<Value>> =
            serde_json::from_value(edit["changes"].clone()).expect("legacy edits");
        assert_eq!(
            sort_map(actual),
            sort_map(expected.clone()),
            "legacy {context}"
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
        let actual_changes=edit["documentChanges"].as_array().expect("document changes").iter().map(|change|
            json!({"textDocument":change["textDocument"],"edits":sorted(change["edits"].as_array().expect("edits").clone())})
        ).collect::<Vec<_>>();
        assert_eq!(
            sorted(actual_changes),
            sorted(expected_changes),
            "versioned {context}"
        );
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
