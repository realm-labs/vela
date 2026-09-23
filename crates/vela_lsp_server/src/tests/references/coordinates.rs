use crate::matrix_fixture::{Edit, FixtureWorkspace, Spec, apply_edits, references as oracle};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};

#[test]
fn reference_rename_coordinate_matrix_projects_exact_sets_and_applied_utf16_edits() {
    run_matrix(oracle::spec);
}

#[test]
fn named_parameter_matrix_projects_exact_sets_and_applied_utf16_edits() {
    run_matrix(oracle::named_spec);
}

#[test]
fn method_parameter_matrix_projects_exact_sets_and_applied_utf16_edits() {
    run_matrix(oracle::method_parameter_spec);
}

#[test]
fn required_parameter_matrix_projects_exact_sets_and_applied_utf16_edits() {
    run_matrix(oracle::required_parameter_spec);
}

#[test]
fn record_field_matrix_projects_exact_sets_and_applied_utf16_edits() {
    run_matrix(oracle::field_spec);
}

#[test]
fn variant_field_matrix_projects_exact_sets_and_applied_utf16_edits() {
    run_matrix(oracle::variant_field_spec);
}

#[test]
fn tuple_field_matrix_projects_exact_sets_and_applied_utf16_edits() {
    run_matrix(oracle::tuple_field_spec);
}

#[test]
fn default_binding_matrix_projects_exact_sets_and_applied_utf16_edits() {
    run_matrix(oracle::default_binding_spec);
}

#[test]
fn schema_method_matrix_projects_exact_sets_and_applied_utf16_edits() {
    run_matrix(oracle::schema_method_spec);
}

#[test]
fn schema_function_matrix_projects_exact_sets_and_applied_utf16_edits() {
    run_matrix(oracle::schema_function_spec);
}

#[test]
fn schema_import_matrix_preserves_aliases_and_applied_utf16_edits() {
    run_matrix(oracle::schema_import_spec);
}

#[test]
fn schema_capture_matrix_rejects_changed_owners_and_applies_safe_utf16_edits() {
    run_matrix(oracle::schema_capture_spec);
}

#[test]
fn schema_lookup_matrix_preserves_unknown_and_exact_owners_after_utf16_edits() {
    run_matrix(oracle::schema_lookup_spec);
}

#[test]
fn schema_variant_matrix_preserves_expression_and_pattern_utf16_edits() {
    run_matrix(oracle::schema_variant_spec);
}

#[test]
fn schema_variant_import_matrix_preserves_aliases_and_applied_utf16_edits() {
    run_matrix(oracle::schema_variant_import_spec);
}

#[test]
fn schema_variant_capture_matrix_rejects_changed_owners_and_applies_safe_utf16_edits() {
    run_matrix(oracle::schema_variant_capture_spec);
}

#[test]
fn schema_variant_lookup_matrix_preserves_unknown_and_short_name_utf16_edits() {
    run_matrix(oracle::schema_variant_lookup_spec);
}

#[test]
fn source_variant_import_matrix_preserves_source_and_schema_utf16_edits() {
    run_matrix(oracle::source_variant_import_spec);
}

#[test]
fn private_variant_import_matrix_preserves_aliases_and_applied_utf16_edits() {
    run_matrix(oracle::private_variant_import_spec);
}

#[test]
fn schema_field_matrix_projects_exact_sets_and_applied_utf16_edits() {
    run_matrix(oracle::schema_field_spec);
}

fn run_matrix(spec_for: fn(bool) -> Spec) {
    for crlf in [false, true] {
        let spec = spec_for(crlf);
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let mut driver = Driver::new(&fixture);
        driver.load_schema(&spec, &fixture);
        driver.check(&spec, &fixture);
        for (group, definition) in spec.oracle["groups"].as_object().expect("groups") {
            if definition["readonly"] == true {
                continue;
            }
            let site = &definition["sites"][0];
            let new_name = definition["rename"].as_str().expect("new name");
            let params = driver.params(&fixture, site);
            let edit = driver.query::<r::Rename>(json!({"textDocument":params["textDocument"],"position":params["position"],"newName":new_name}));
            let expected = oracle::renamed(&spec, group, new_name);
            let mut applied = FixtureWorkspace::new(&expected).expect("expected applied source");
            for (file, document) in &mut applied.disk {
                let original = &fixture.disk[file].text;
                let edits = edit["changes"][driver.uri(file)].as_array();
                let actual = edits.map_or_else(
                    || original.clone(),
                    |edits| {
                        let edits = edits
                            .iter()
                            .map(|edit| Edit {
                                start: wire_point(&edit["range"]["start"]),
                                end: wire_point(&edit["range"]["end"]),
                                text: edit["newText"].as_str().expect("replacement"),
                            })
                            .collect::<Vec<_>>();
                        apply_edits(original, &edits).expect("valid nonoverlapping UTF-16 edits")
                    },
                );
                assert_eq!(actual, document.text, "applied {group}/{file}");
                document.text = actual;
            }
            oracle::assert_parsed(&applied);
            driver.change(&applied);
            driver.load_schema(&expected, &applied);
            driver.check(&expected, &applied);
            driver.change(&fixture);
            driver.load_schema(&spec, &fixture);
            driver.check(&spec, &fixture);
        }
        // A disk snapshot retained after close has an internal service version,
        // but must not be presented as an open client's document version.
        let closed = "scripts/consumer.vela";
        let uri = driver.uri(closed);
        let _ = notify::<n::DidCloseTextDocument>(
            &mut driver.endpoint,
            json!({"textDocument":{"uri":uri}}),
        );
        driver.versions.remove(closed);
        driver.check(&spec, &fixture);
    }
}

struct Driver {
    root: PathBuf,
    endpoint: TestServer,
    id: i32,
    versions: BTreeMap<String, i32>,
}
impl Driver {
    fn new(fixture: &FixtureWorkspace) -> Self {
        let root = crate::tests::support::unique_temp_root("references").join("中文 % references");
        fixture.materialize(&root).expect("isolated fixture");
        let mut driver = Self {
            root,
            endpoint: TestServer::new(),
            id: 0,
            versions: BTreeMap::new(),
        };
        let initialized = driver
            .query::<r::Initialize>(json!({"processId":null,"rootUri":driver.uri(""),
            "capabilities":{"workspace":{"workspaceEdit":{"documentChanges":true}}}}));
        assert_eq!(initialized["capabilities"]["referencesProvider"], true);
        for file in ["scripts/main.vela", "scripts/helpers.vela"] {
            driver.open(file, &fixture.disk[file].text);
        }
        driver
    }
    fn load_schema(&mut self, spec: &Spec, fixture: &FixtureWorkspace) {
        if spec.oracle["schema"].is_null() {
            return;
        }
        let path = self.root.join("target/schema.json");
        let existed = path.exists();
        std::fs::create_dir_all(path.parent().expect("schema parent")).expect("schema directory");
        let snapshot = self.endpoint.snapshot();
        let artifact =
            crate::matrix_fixture::schema_artifact(&spec.oracle["schema"], fixture, |file| {
                snapshot.databases().source_db().records()
                    [&vela_language_service::DocumentId::from(self.uri(file))]
                    .source_id()
                    .get()
            });
        std::fs::write(&path, artifact.to_string()).expect("regenerated schema");
        let schema_uri = self.uri("target/schema.json");
        let _ = notify::<n::DidChangeWatchedFiles>(
            &mut self.endpoint,
            json!({"changes":[{"uri":schema_uri,"type":if existed {2} else {1}}]}),
        );
    }
    fn uri(&self, file: &str) -> String {
        lsp_types::Url::from_file_path(self.root.join(file))
            .expect("valid fixture value")
            .to_string()
    }
    fn query<R: r::Request>(&mut self, params: Value) -> Value {
        self.id += 1;
        let response = response_value(request::<R>(&mut self.endpoint, self.id, params));
        assert_eq!(response["id"], self.id);
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    }
    fn open(&mut self, file: &str, text: &str) {
        self.versions.insert(file.into(), 1);
        let _ = notify::<n::DidOpenTextDocument>(
            &mut self.endpoint,
            json!({"textDocument":{
            "uri":lsp_types::Url::from_file_path(self.root.join(file)).expect("valid fixture value"),"languageId":"vela","version":1,"text":text}}),
        );
    }
    fn change(&mut self, fixture: &FixtureWorkspace) {
        for (file, doc) in fixture
            .disk
            .iter()
            .filter(|(file, _)| file.ends_with(".vela"))
        {
            if let Some(version) = self.versions.get_mut(file) {
                *version += 1;
                let _ = notify::<n::DidChangeTextDocument>(
                    &mut self.endpoint,
                    json!({"textDocument":{
                    "uri":lsp_types::Url::from_file_path(self.root.join(file)).expect("valid fixture value"),"version":version},"contentChanges":[{"text":doc.text}]}),
                );
            } else {
                self.open(file, &doc.text);
            }
        }
    }
    fn params(&self, fixture: &FixtureWorkspace, query: &Value) -> Value {
        let file = query["file"].as_str().expect("fixture string");
        let marker = query["marker"].as_str().expect("fixture string");
        let point = fixture.disk[file].markers[marker].start;
        json!({"textDocument":{"uri":self.uri(file)},"position":{"line":point.line,"character":point.character+1}})
    }
    fn check(&mut self, spec: &Spec, fixture: &FixtureWorkspace) {
        for check in spec.oracle["localChecks"].as_array().into_iter().flatten() {
            let file = check["file"].as_str().expect("file");
            let query = oracle::local_marker(spec, fixture, check, false);
            let target = oracle::local_marker(spec, fixture, check, true);
            let definition = self.query::<r::GotoDefinition>(json!({"textDocument":{"uri":self.uri(file)},"position":{"line":query.start.line,"character":query.start.character+1}}));
            assert_eq!(
                definition,
                json!({"uri":self.uri(file),"range":{"start":{"line":target.start.line,"character":target.start.character},"end":{"line":target.end.line,"character":target.end.character}}})
            );
        }
        for query in spec.oracle["queries"].as_array().expect("fixture array") {
            let file = query["file"].as_str().expect("fixture string");
            let marker = query["marker"].as_str().expect("fixture string");
            let sites = oracle::sites(spec, query);
            let params = self.params(fixture, query);
            for _ in 0..2 {
                for include in [false, true] {
                    let result=self.query::<r::References>(json!({"textDocument":params["textDocument"],"position":params["position"],"context":{"includeDeclaration":include}}));
                    let expected=sites.iter().filter(|site| include || site["kind"]!="Declaration").map(|site|
                        json!({"uri":self.uri(site["file"].as_str().expect("fixture string")),"range":site_range(fixture,site)})).collect();
                    assert_eq!(
                        sorted(result.as_array().expect("reference array").clone()),
                        sorted(expected),
                        "references {marker}, include={include}"
                    );
                }
                let result = self.query::<r::DocumentHighlightRequest>(params.clone());
                let expected = sites
                    .iter()
                    .filter(|site| site["file"] == file)
                    .map(|site| {
                        let kind = match site["kind"].as_str().expect("fixture string") {
                            "Read" => 2,
                            "Write" => 3,
                            _ => 1,
                        };
                        json!({"range":site_range(fixture,site),"kind":kind})
                    })
                    .collect();
                assert_eq!(
                    sorted(result.as_array().expect("highlight array").clone()),
                    sorted(expected),
                    "highlights {marker}"
                );
                let prepare = self.query::<r::PrepareRenameRequest>(params.clone());
                let edit=self.query::<r::Rename>(json!({"textDocument":params["textDocument"],"position":params["position"],"newName":"renamed_symbol"}));
                if let Some(group) = query["group"].as_str() {
                    if let Some(target) = spec.oracle["groups"][group]["typeTarget"].as_object() {
                        let site = &json!(target);
                        let definition = self.query::<r::GotoTypeDefinition>(params.clone());
                        assert_eq!(
                            definition,
                            json!({"uri":self.uri(site["file"].as_str().expect("file")),"range":site_range(fixture,site)})
                        );
                    }
                    if spec.oracle["groups"][group]["readonly"] == true {
                        assert!(prepare.is_null());
                        assert!(edit.is_null());
                        let definition = self.query::<r::GotoDefinition>(params.clone());
                        if spec.oracle["groups"][group]["sourceDefinition"] == true {
                            let site = &sites[0];
                            assert_eq!(
                                definition,
                                json!({"uri":self.uri(site["file"].as_str().expect("file")),"range":site_range(fixture,site)})
                            );
                        } else {
                            assert!(definition.is_null());
                        }
                        continue;
                    }
                    if spec.oracle["groups"][group]["origin"] == "schema" {
                        assert!(
                            edit["changeAnnotations"]
                                .as_object()
                                .expect("schema ABI warnings")
                                .values()
                                .any(|value| value["description"] == "schemaAbi"
                                    && value["needsConfirmation"] == true)
                        );
                    }
                    if spec.oracle["checkDefinition"] == true {
                        let site = &sites[0];
                        let definition = self.query::<r::GotoDefinition>(params.clone());
                        assert_eq!(
                            definition,
                            json!({"uri":self.uri(site["file"].as_str().expect("file")),"range":site_range(fixture,site)})
                        );
                    }
                    for rejected in oracle::rejections(spec, group) {
                        let result=self.query::<r::Rename>(json!({"textDocument":params["textDocument"],"position":params["position"],"newName":rejected}));
                        assert!(result.is_null(), "collision {marker}: {rejected}");
                    }
                    assert_eq!(
                        prepare,
                        json!({"range":site_range(fixture,query),"placeholder":spec.oracle["groups"][group]["name"]}),
                        "prepare {marker}"
                    );
                    let mut expected = BTreeMap::<String, Vec<Value>>::new();
                    for site in oracle::edits(spec, group) {
                        expected.entry(self.uri(site["file"].as_str().expect("fixture string"))).or_default().push(json!({"range":site_range(fixture,site),"newText":oracle::replacement(site,"renamed_symbol")}));
                    }
                    let actual: BTreeMap<String, Vec<Value>> =
                        serde_json::from_value(edit["changes"].clone()).expect("changes");
                    let sort_map = |map: BTreeMap<String, Vec<Value>>| {
                        map.into_iter()
                            .map(|(file, edits)| (file, sorted(edits)))
                            .collect::<BTreeMap<_, _>>()
                    };
                    assert_eq!(
                        sort_map(actual),
                        sort_map(expected.clone()),
                        "rename {marker}"
                    );
                    let changes = edit["documentChanges"]
                        .as_array()
                        .expect("versioned changes");
                    let expected=expected.into_iter().map(|(uri,edits)| {
                        let version=self.versions.iter().find_map(|(file,version)|(self.uri(file)==uri).then_some(*version));
                        json!({"textDocument":{"uri":uri,"version":version},"edits":sorted(edits)})
                    }).collect();
                    let actual=changes.iter().map(|change|json!({"textDocument":change["textDocument"],"edits":sorted(change["edits"].as_array().expect("fixture array").clone())})).collect();
                    assert_eq!(
                        sorted(actual),
                        sorted(expected),
                        "versioned rename {marker}"
                    );
                } else {
                    assert_eq!(prepare, Value::Null, "negative prepare {marker}");
                    assert_eq!(edit, Value::Null, "negative rename {marker}");
                }
            }
        }
    }
}
impl Drop for Driver {
    fn drop(&mut self) {
        let allocation = self.root.parent().expect("isolated allocation");
        assert_eq!(allocation.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            allocation
                .file_name()
                .expect("valid fixture value")
                .to_string_lossy()
                .starts_with("vela-lsp-references-")
        );
        std::fs::remove_dir_all(allocation).expect("fixture cleanup");
    }
}
fn wire_point(point: &Value) -> (usize, usize) {
    (
        point["line"].as_u64().expect("wire coordinate") as usize,
        point["character"].as_u64().expect("wire coordinate") as usize,
    )
}
fn site_range(fixture: &FixtureWorkspace, site: &Value) -> Value {
    let range = fixture.disk[site["file"].as_str().expect("fixture string")].markers
        [site["marker"].as_str().expect("fixture string")];
    json!({"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}})
}
fn sorted(mut values: Vec<Value>) -> Vec<Value> {
    values.sort_by_key(Value::to_string);
    values
}
