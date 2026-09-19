mod schema_lifecycle;
mod source_lifecycle;
mod source_overlays;

use crate::matrix_fixture::{Edit, FixtureWorkspace, apply_edits, load};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

#[test]
fn type_ownership_matrix_projects_owned_edits_definitions_and_restored_candidates() {
    assert_type_ownership("completion-type-ownership");
}

#[test]
fn visible_binding_matrix_projects_loop_pattern_and_initializer_scopes() {
    assert_type_ownership("completion-visible-bindings");
}

#[test]
fn declaration_context_matrix_projects_initializer_and_parameter_owners() {
    assert_type_ownership("completion-declaration-contexts");
}

#[test]
fn signature_scope_matrix_projects_nested_binding_owners() {
    assert_type_ownership("completion-signature-scopes");
}

#[test]
fn package_type_ownership_projects_exact_dependency_targets_and_edits() {
    assert_type_ownership("completion-package-type-ownership");
}

#[test]
fn package_callable_ownership_projects_sets_signatures_and_applied_targets() {
    assert_type_ownership("completion-package-callables");
}

#[test]
fn package_member_ownership_projects_sets_signatures_returns_and_targets() {
    assert_type_ownership("completion-package-members");
}

#[test]
fn returned_receiver_flow_projects_owned_sets_signatures_and_targets() {
    assert_type_ownership("completion-return-flow");
}

#[test]
fn callback_contract_results_project_direct_lambda_facts_and_erased_boundaries() {
    assert_type_ownership("completion-callback-results");
}

#[test]
fn callback_slot_matrix_projects_registered_slots_and_reordered_result_facts() {
    assert_type_ownership("completion-callback-slots");
}

#[test]
fn async_callable_matrix_projects_owner_metadata_and_applied_await_contracts() {
    assert_type_ownership("completion-async-callables");
}

#[test]
fn await_context_matrix_projects_owned_choices_and_exact_syntax_diagnostics() {
    assert_type_ownership("completion-await-contexts");
}

#[test]
fn receiver_assignment_flow_projects_possible_owners_and_rejects_stale_members() {
    assert_type_ownership("completion-receiver-assignments");
}

#[test]
fn shared_method_matrix_projects_merged_details_signatures_and_ambiguous_targets() {
    assert_type_ownership("completion-shared-methods");
}

#[test]
fn abrupt_flow_matrix_projects_reachable_receivers_and_explicit_lambda_returns() {
    assert_type_ownership("completion-abrupt-flow");
}

#[test]
fn local_exit_matrix_projects_reachable_assignment_owners() {
    assert_type_ownership("completion-local-exits");
}

#[test]
fn loop_match_matrix_projects_backedge_owners_and_reachable_results() {
    assert_type_ownership("completion-loop-match-flow");
}

#[test]
fn schema_callable_lifecycle_projects_current_contracts_and_stale_resolve() {
    assert_type_ownership("completion-schema-callable-lifecycle");
}

#[test]
fn unavailable_schema_lifecycle_projects_source_authoring_and_clears_stale_host_facts() {
    assert_type_ownership("completion-schema-unavailable");
}

#[test]
fn source_callable_lifecycle_projects_package_owners_after_edits_deletion_and_recreation() {
    assert_type_ownership("completion-source-callable-lifecycle");
}

#[test]
fn source_overlay_lifecycle_projects_dirty_owners_save_and_close_restoration() {
    assert_type_ownership("completion-source-overlay-lifecycle");
}

#[test]
fn source_recovery_preserves_neighbor_candidates_and_repaired_fresh_facts() {
    assert_type_ownership("completion-source-recovery");
}

#[test]
fn builtin_callable_resolve_preserves_owner_edits_and_excludes_schema_docs() {
    assert_type_ownership("completion-builtin-callable-resolve");
}

#[test]
fn resolve_identity_matrix_preserves_local_values_and_dynamic_boundaries() {
    assert_type_ownership("completion-resolve-identities");
}

#[test]
fn import_site_matrix_preserves_paths_schema_spans_and_recovery_boundaries() {
    assert_type_ownership("completion-import-sites");
}

fn assert_type_ownership(fixture_id: &str) {
    for crlf in [false, true] {
        let mut spec = load(fixture_id);
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let temp = crate::tests::support::unique_temp_root("import-aliases");
        let root = temp.join("中文 % workspace");
        fixture.materialize(&root).expect("materialize");
        let uri = |file: &str| {
            lsp_types::Url::from_file_path(root.join(file))
                .expect("URI")
                .to_string()
        };
        let capabilities = json!({"textDocument":{"completion":{"completionItem":{"snippetSupport":true,"labelDetailsSupport":true,"resolveSupport":{"properties":["documentation"]}}}}});
        let mut server = TestServer::new();
        let _ = request::<r::Initialize>(
            &mut server,
            1,
            json!({"processId":null,"rootUri":uri(""),"capabilities":capabilities}),
        );
        let mut control_messages = Vec::new();
        if let Some(markers) = fixture.disk.get("schema-markers.json") {
            let snapshot = server.snapshot();
            let artifact = crate::matrix_fixture::schema_artifact(
                &serde_json::from_str::<Value>(&markers.text).expect("schema markers"),
                &fixture,
                |file| {
                    snapshot.databases().source_db().records()
                        [&vela_language_service::DocumentId::from(uri(file))]
                        .source_id()
                        .get()
                },
            );
            std::fs::write(root.join("schema.json"), artifact.to_string())
                .expect("schema artifact");
            let _ = notify::<n::DidChangeWatchedFiles>(
                &mut server,
                json!({"changes":[{"uri":uri("schema.json"),"type":2}]}),
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
        if let Some(file) = spec.oracle["schemaDiagnosticFile"]
            .as_str()
            .or_else(|| spec.oracle["sourceDiagnosticFile"].as_str())
        {
            let source = fixture.document(file).expect("diagnostic control");
            control_messages = crate::tests::notification_values(notify::<n::DidOpenTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":uri(file),"languageId":"vela","version":1,"text":source.text}}),
            ));
        }
        let mut id = 2;
        let mut overlays = (spec.oracle["sourceOverlays"] == true)
            .then(|| source_overlays::State::new(&spec, &root, &control_messages));
        let source_lifecycle = spec.oracle["sourceLifecycle"].is_array();
        let phases = spec
            .oracle
            .get("sourceLifecycle")
            .unwrap_or(&spec.oracle["schemaLifecycle"])
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![Value::Null]);
        let mut saved = std::collections::BTreeMap::<String, Value>::new();
        for phase in phases {
            let mut current = spec.clone();
            if source_lifecycle {
                current = crate::matrix_fixture::source_lifecycle_spec(&spec, &phase, crlf);
                let fixture = FixtureWorkspace::new(&current).expect("source phase");
                if let Some(state) = &mut overlays {
                    state.apply(&mut server, &root, &phase, crlf);
                    crate::matrix_fixture::assert_source_overlay_state(
                        &state.fixture,
                        &fixture,
                        &phase,
                        crlf,
                    );
                } else {
                    source_lifecycle::apply_phase(
                        &mut server,
                        &root,
                        &fixture,
                        &phase,
                        string(&spec.oracle, "sourceDiagnosticFile"),
                    );
                }
                for item in saved.values() {
                    let resolved = response_value(request::<r::ResolveCompletionItem>(
                        &mut server,
                        id,
                        item.clone(),
                    ));
                    id += 1;
                    assert!(resolved["error"].is_null(), "{resolved}");
                    assert_eq!(
                        &resolved["result"], item,
                        "source resolve must preserve the original item without schema docs: {phase}"
                    );
                }
            } else if !phase.is_null() {
                match crate::matrix_fixture::schema_lifecycle_source(&phase) {
                    Some(text) => {
                        current.files.insert("schema.json".to_owned(), text);
                    }
                    None => {
                        current.files.remove("schema.json");
                    }
                }
                current.oracle["queries"] = phase["queries"].clone();
                schema_lifecycle::apply_phase(
                    &mut server,
                    &root,
                    spec.oracle["schemaDiagnosticFile"].as_str(),
                    &phase,
                );
                for (name, item) in &saved {
                    let resolved = response_value(request::<r::ResolveCompletionItem>(
                        &mut server,
                        id,
                        item.clone(),
                    ));
                    id += 1;
                    assert!(resolved["error"].is_null(), "{resolved}");
                    assert_eq!(resolved["result"]["data"], item["data"]);
                    assert_eq!(resolved["result"]["textEdit"], item["textEdit"]);
                    assert_eq!(
                        resolved["result"]["documentation"]["value"], phase["resolveDocs"][name],
                        "stale resolve {name}: {phase}"
                    );
                }
            }
            let spec = current;
            let fixture = FixtureWorkspace::new(&spec).expect("phase fixture");
            let mut fresh = source_lifecycle.then(|| {
                overlays.as_ref().map_or_else(
                    || source_lifecycle::fresh_server(&root, &capabilities),
                    |state| state.fresh_server(&root, &capabilities),
                )
            });
            for query in phase["signatureQueries"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
            {
                let file = string(query, "file");
                let source = fixture.document(file).expect("signature document");
                let point = source.markers["cursor"].start;
                let result = response_value(request::<r::SignatureHelpRequest>(
                    &mut server,
                    id,
                    json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}}),
                ));
                id += 1;
                assert!(result["error"].is_null(), "{result}");
                let labels = result["result"]["signatures"].as_array().map(|signatures| {
                    signatures
                        .iter()
                        .map(|signature| &signature["label"])
                        .collect::<Vec<_>>()
                });
                assert_eq!(json!(labels), query["labels"], "cached signature: {phase}");
                if let Some(fresh) = &mut fresh {
                    let independent = response_value(request::<r::SignatureHelpRequest>(
                        fresh,
                        id,
                        json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}}),
                    ));
                    id += 1;
                    assert!(independent["error"].is_null(), "{independent}");
                    assert_eq!(
                        result["result"], independent["result"],
                        "fresh signature: {phase}"
                    );
                    source_lifecycle::assert_definition(&mut server, &root, &fixture, query, id);
                    id += 1;
                    source_lifecycle::assert_definition(fresh, &root, &fixture, query, id);
                    id += 1;
                }
            }
            for case in spec.oracle["queries"].as_array().expect("queries") {
                let file = string(case, "file");
                let source = fixture.document(file).expect("document");
                let point = source.markers["cursor"].start;
                let range = source.markers["replace"];
                if case["parseErrors"] == true {
                    assert!(
                        !vela_syntax::parse::parse_source(&source.text)
                            .diagnostics()
                            .is_empty()
                    );
                }
                let params = json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}});
                let unopened = fresh.as_mut().map(|fresh| {
                    let live =
                        response_value(request::<r::Completion>(&mut server, id, params.clone()));
                    id += 1;
                    let independent =
                        response_value(request::<r::Completion>(fresh, id, params.clone()));
                    id += 1;
                    assert!(
                        live["error"].is_null() && independent["error"].is_null(),
                        "{live} {independent}"
                    );
                    assert_eq!(
                        live["result"], independent["result"],
                        "fresh unopened importer: {phase} {case}"
                    );
                    live["result"].clone()
                });
                let _ = notify::<n::DidOpenTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri(file),"languageId":"vela","version":1,"text":source.text}}),
                );
                if let Some(signature) = case["callSignature"].as_str() {
                    let point = source.markers["call-cursor"].start;
                    let help = response_value(request::<r::SignatureHelpRequest>(
                        &mut server,
                        id,
                        json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}}),
                    ));
                    id += 1;
                    assert!(help["error"].is_null(), "{help}");
                    let signatures = help["result"]["signatures"]
                        .as_array()
                        .expect("known dynamic-return callable");
                    assert_eq!(signatures.len(), 1, "{case}");
                    assert_eq!(signatures[0]["label"], signature, "{case}");
                }
                let response =
                    response_value(request::<r::Completion>(&mut server, id, params.clone()));
                id += 1;
                assert!(response["error"].is_null(), "{case} {response}");
                if let Some(unopened) = unopened {
                    assert_eq!(
                        response["result"], unopened,
                        "opening unchanged source: {case}"
                    );
                }
                let items = response["result"]["items"].as_array().expect("items");
                if !phase.is_null() {
                    for item in items {
                        let symbol = &item["data"]["resolve"]["symbol"];
                        if symbol["kind"] == "schema" && !source_lifecycle {
                            saved.insert(string(symbol, "name").to_owned(), item.clone());
                        } else if symbol["kind"] == "source" && source_lifecycle {
                            saved.insert(
                                format!("{}:{}", string(case, "id"), string(item, "label")),
                                item.clone(),
                            );
                        }
                    }
                }
                let expected = case["items"].as_array().expect("items");
                let mut actual = items
                    .iter()
                    .map(|i| (string(i, "label"), completion_text(i)))
                    .collect::<Vec<_>>();
                let mut keys = expected
                    .iter()
                    .map(|i| (string(i, "label"), string(i, "insert")))
                    .collect::<Vec<_>>();
                actual.sort();
                keys.sort();
                assert_eq!(actual, keys, "{case}");
                let mut version = 1;
                for expected in expected {
                    let item = items
                        .iter()
                        .find(|i| {
                            i["label"] == expected["label"]
                                && completion_text(i) == expected["insert"]
                        })
                        .expect("item");
                    let kind = match string(expected, "kind") {
                        "Function" => 3,
                        "Method" => 2,
                        "Field" => 5,
                        "Const" => 21,
                        "Type" => 22,
                        "Trait" => 8,
                        "Binding" => 6,
                        "Parameter" => 6,
                        "Value" => 12,
                        "Module" => 9,
                        other => panic!("{other}"),
                    };
                    assert_eq!(item["kind"], kind);
                    assert_eq!(item["detail"], expected["detail"]);
                    if let Some(format) = expected["insertFormat"].as_str() {
                        assert_eq!(
                            item["insertTextFormat"],
                            if format == "Snippet" {
                                json!(2)
                            } else {
                                Value::Null
                            },
                            "{case}"
                        );
                    }
                    assert!(
                        item.get("documentation").is_none(),
                        "initial list stays lightweight: {case}"
                    );
                    if expected["origin"] == "none" {
                        assert!(
                            item["data"].get("resolve").is_none(),
                            "no invented identity: {case}"
                        );
                    } else if expected["origin"] != "local" {
                        let symbol_name = expected["protocolSymbol"]
                            .as_str()
                            .unwrap_or(string(expected, "symbol"));
                        assert_eq!(
                            item["data"]["resolve"],
                            json!({"kind":"documentation","symbol":{"kind":expected["origin"],"name":symbol_name}}),
                            "{case}"
                        );
                    }
                    let resolved = response_value(request::<r::ResolveCompletionItem>(
                        &mut server,
                        id,
                        item.clone(),
                    ));
                    id += 1;
                    assert!(resolved["error"].is_null(), "{case}: {resolved}");
                    let mut expected_resolved = item.clone();
                    if let Some(docs) = expected["docs"].as_str() {
                        expected_resolved["documentation"] =
                            json!({"kind":"markdown","value":docs});
                    }
                    assert_eq!(resolved["result"], expected_resolved, "{case}");
                    if expected["implicitEdit"] == true {
                        assert!(item.get("textEdit").is_none(), "{case}");
                        assert!(item.get("insertText").is_none(), "{case}");
                        assert_eq!(item["label"], expected["insert"]);
                    } else {
                        assert_eq!(
                            item["textEdit"],
                            json!({"range":{"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}},"newText":expected["insert"]})
                        );
                    }
                    let insertion = format!(
                        "{}{}",
                        string(expected, "insert")
                            .replace("$0", expected["value"].as_str().unwrap_or("1")),
                        string(case, "applySuffix")
                    );
                    let edited = apply_edits(
                        &source.text,
                        &[Edit {
                            start: (range.start.line, range.start.character),
                            end: (range.end.line, range.end.character),
                            text: &insertion,
                        }],
                    )
                    .expect("edit");
                    assert_eq!(
                        edited,
                        format!(
                            "{}{}{}",
                            &source.text[..range.start.byte],
                            insertion,
                            &source.text[range.end.byte..]
                        )
                    );
                    assert_eq!(
                        vela_syntax::parse::parse_source(&edited)
                            .diagnostics()
                            .iter()
                            .map(|d| d.code.clone().expect("syntax diagnostic code"))
                            .collect::<Vec<_>>(),
                        case["syntaxCodes"]
                            .as_array()
                            .map_or_else(Vec::new, |codes| codes
                                .iter()
                                .map(|code| code.as_str().expect("syntax code").to_owned())
                                .collect()),
                        "{case}: {edited}"
                    );
                    version += 1;
                    let changes = notify::<n::DidChangeTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":edited}]}),
                    );
                    if let Some(applied_file) = expected["appliedFile"].as_str() {
                        let applied = fixture.document(applied_file).expect("applied oracle");
                        assert_eq!(edited, applied.text, "{case}");
                        let notifications = crate::tests::notification_values(changes.clone());
                        let published = notifications
                            .iter()
                            .find(|n| {
                                n["method"] == "textDocument/publishDiagnostics"
                                    && n["params"]["uri"] == uri(file)
                            })
                            .expect("applied diagnostics");
                        let actual = published["params"]["diagnostics"]
                            .as_array()
                            .expect("diagnostics")
                            .iter()
                            .filter(|d| {
                                d["code"]
                                    .as_str()
                                    .is_some_and(|code| code.contains("await"))
                            })
                            .collect::<Vec<_>>();
                        let oracle = expected["diagnostics"]
                            .as_array()
                            .expect("await diagnostics");
                        assert_eq!(actual.len(), oracle.len(), "{case}: {actual:?}");
                        for (actual, expected) in actual.iter().zip(oracle) {
                            assert_eq!(actual["code"], expected["code"]);
                            assert_eq!(actual["message"], expected["message"]);
                            assert_eq!(actual["severity"], 1);
                            let range = applied.markers[string(expected, "marker")];
                            assert_eq!(
                                actual["range"],
                                json!({"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}})
                            );
                            assert_eq!(
                                actual["data"]["labels"],
                                json!([{"uri":uri(file),"range":actual["range"],"message":expected["label"]}])
                            );
                        }
                    }
                    if case["checkAwait"] == true {
                        let notifications = crate::tests::notification_values(changes);
                        let published = notifications
                            .iter()
                            .find(|notification| {
                                notification["method"] == "textDocument/publishDiagnostics"
                                    && notification["params"]["uri"] == uri(file)
                            })
                            .expect("applied diagnostics");
                        let missing = published["params"]["diagnostics"]
                            .as_array()
                            .expect("diagnostics")
                            .iter()
                            .filter(|diagnostic| {
                                diagnostic["code"] == "analysis::async_call_requires_await"
                            })
                            .collect::<Vec<_>>();
                        let required =
                            expected["async"] == true && string(case, "applySuffix").is_empty();
                        assert_eq!(
                            missing.len(),
                            usize::from(required),
                            "{case} {expected}: {missing:?}"
                        );
                        if let Some(diagnostic) = missing.first() {
                            let call = source.markers["call"];
                            let end = call.end.character + insertion.encode_utf16().count()
                                - (range.end.character - range.start.character);
                            assert_eq!(diagnostic["severity"], 1);
                            let expected_range = json!({"start":{"line":call.start.line,"character":call.start.character},"end":{"line":call.end.line,"character":end}});
                            assert_eq!(diagnostic["range"], expected_range);
                            let labels = diagnostic["data"]["labels"]
                                .as_array()
                                .expect("diagnostic labels");
                            assert_eq!(labels.len(), 1);
                            assert_eq!(labels[0]["uri"], uri(file));
                            assert_eq!(labels[0]["range"], expected_range);
                        }
                    }
                    if let Some(signatures) = expected["signatures"].as_array() {
                        let open = insertion.find('(').expect("call");
                        let help = response_value(request::<r::SignatureHelpRequest>(
                            &mut server,
                            id,
                            json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":range.start.character+open+1}}),
                        ));
                        id += 1;
                        assert!(help["error"].is_null());
                        let mut actual = help["result"]["signatures"]
                            .as_array()
                            .expect("alternative signatures")
                            .iter()
                            .map(|signature| string(signature, "label"))
                            .collect::<Vec<_>>();
                        let mut expected = signatures
                            .iter()
                            .map(|signature| signature.as_str().expect("signature label"))
                            .collect::<Vec<_>>();
                        actual.sort();
                        expected.sort();
                        assert_eq!(actual, expected, "{case}");
                    }
                    if let Some(signature) = expected["signature"].as_str() {
                        let open = insertion.find('(').expect("call");
                        let help = response_value(request::<r::SignatureHelpRequest>(
                            &mut server,
                            id,
                            json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":range.start.character+open+1}}),
                        ));
                        id += 1;
                        assert!(help["error"].is_null());
                        assert_eq!(
                            help["result"]["signatures"]
                                .as_array()
                                .expect("signatures")
                                .len(),
                            1,
                            "{case}"
                        );
                        assert_eq!(
                            help["result"]["signatures"][0]["label"], signature,
                            "{case} {expected}"
                        );
                    }
                    if let Some(argument) = expected.get("argument") {
                        if let Some(label) = expected["inlay"].as_str() {
                            let character =
                                range.start.character + insertion.find('(').expect("call") + 1;
                            let point = json!({"line":range.start.line,"character":character});
                            let hints = response_value(request::<r::InlayHintRequest>(
                                &mut server,
                                id,
                                json!({"textDocument":{"uri":uri(file)},"range":{"start":point,"end":point}}),
                            ));
                            id += 1;
                            assert!(hints["error"].is_null());
                            let hints = hints["result"].as_array().expect("hints");
                            assert_eq!(hints.len(), 1, "{case}");
                            assert_eq!(hints[0]["label"], label, "{case}");
                            assert_eq!(hints[0]["kind"], 2);
                            assert_eq!(hints[0]["position"], point);
                        }
                        let text = source.text[..range.start.byte].to_owned()
                            + &string(expected, "insert").replace("$0", "zz_")
                            + &source.text[range.end.byte..];
                        let start = range.start.character
                            + string(expected, "insert").find('(').expect("call")
                            + 1;
                        version += 1;
                        let _ = notify::<n::DidChangeTextDocument>(
                            &mut server,
                            json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":text}]}),
                        );
                        let response = response_value(request::<r::Completion>(
                            &mut server,
                            id,
                            json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":start+3}}),
                        ));
                        id += 1;
                        assert!(response["error"].is_null());
                        let parameters =
                            response["result"]["items"].as_array().expect("parameters");
                        assert_eq!(
                            parameters
                                .iter()
                                .map(|i| string(i, "label"))
                                .collect::<Vec<_>>(),
                            [string(argument, "label")],
                            "{case}"
                        );
                        assert_eq!(parameters[0]["detail"], argument["detail"]);
                        assert_eq!(parameters[0]["kind"], 6);
                        assert_eq!(
                            parameters[0]["textEdit"],
                            json!({"range":{"start":{"line":range.start.line,"character":start},"end":{"line":range.start.line,"character":start+3}},"newText":format!("{} = ",string(argument,"label"))})
                        );
                        version += 1;
                        let _ = notify::<n::DidChangeTextDocument>(
                            &mut server,
                            json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":edited}]}),
                        );
                    }
                    if let Some(target) = expected["target"].as_str() {
                        let target = if target == "self" { file } else { target };
                        let marker = fixture.document(target).expect("target").markers
                            [string(expected, "marker")];
                        let start = insertion
                            .split('(')
                            .next()
                            .expect("callee path")
                            .rfind("::")
                            .map_or(0, |i| i + 2);
                        let definition = response_value(request::<r::GotoDefinition>(
                            &mut server,
                            id,
                            json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":range.start.character+start+1}}),
                        ));
                        id += 1;
                        assert_eq!(
                            definition["result"],
                            json!({"uri":uri(target),"range":{"start":{"line":marker.start.line,"character":marker.start.character},"end":{"line":marker.end.line,"character":marker.end.character}}}),
                            "{case} {expected}"
                        );
                    } else {
                        let start = insertion
                            .split('(')
                            .next()
                            .expect("callee path")
                            .rfind("::")
                            .map_or(0, |i| i + 2);
                        let definition = response_value(request::<r::GotoDefinition>(
                            &mut server,
                            id,
                            json!({"textDocument":{"uri":uri(file)},"position":{"line":range.start.line,"character":range.start.character+start+1}}),
                        ));
                        id += 1;
                        assert!(definition["error"].is_null(), "{case} {expected}");
                        assert!(
                            definition["result"].is_null(),
                            "{case} {expected}: {definition}"
                        );
                    }
                    if case["member"] == true {
                        let member = source.markers["member"].start;
                        let response = response_value(request::<r::Completion>(
                            &mut server,
                            id,
                            json!({"textDocument":{"uri":uri(file)},"position":{"line":member.line,"character":member.character+insertion.encode_utf16().count()-(range.end.character-range.start.character)}}),
                        ));
                        id += 1;
                        let members = response["result"]["items"].as_array().expect("members");
                        assert_eq!(
                            members
                                .iter()
                                .map(|i| string(i, "label"))
                                .collect::<Vec<_>>(),
                            ["tag"],
                            "{case}"
                        );
                        assert_eq!(
                            members[0]["data"]["resolve"]["symbol"],
                            json!({"kind":expected["origin"],"name":format!("{}.tag",string(expected,"symbol"))})
                        );
                        if let Some(detail) = expected["fieldDetail"].as_str() {
                            assert_eq!(members[0]["detail"], detail, "{case} {expected}");
                        }
                    }
                    version += 1;
                    let _ = notify::<n::DidChangeTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":uri(file),"version":version},"contentChanges":[{"text":source.text}]}),
                    );
                    let restored =
                        response_value(request::<r::Completion>(&mut server, id, params.clone()));
                    id += 1;
                    assert_eq!(restored["result"], response["result"], "{case}");
                }
                if !phase.is_null() {
                    let _ = notify::<n::DidCloseTextDocument>(
                        &mut server,
                        json!({"textDocument":{"uri":uri(file)}}),
                    );
                }
            }
        }
        for (index, step) in spec.oracle["lifecycle"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            let text = string(step, "source").replace('\n', if crlf { "\r\n" } else { "\n" });
            let source = crate::matrix_fixture::parse_markers(&text).expect("source");
            let point = source.markers["cursor"].start;
            let file = "scripts/source_alias.vela";
            let _ = notify::<n::DidChangeTextDocument>(
                &mut server,
                json!({"textDocument":{"uri":uri(file),"version":100+index},"contentChanges":[{"text":source.text}]}),
            );
            let response = response_value(request::<r::Completion>(
                &mut server,
                id,
                json!({"textDocument":{"uri":uri(file)},"position":{"line":point.line,"character":point.character}}),
            ));
            id += 1;
            let items = response["result"]["items"].as_array().expect("items");
            let expected = step["items"].as_array().expect("items");
            assert_eq!(items.len(), expected.len(), "{step}");
            for (item, expected) in items.iter().zip(expected) {
                assert_eq!(item["label"], expected["label"]);
                assert_eq!(item["detail"], expected["detail"]);
                assert_eq!(
                    item["data"]["resolve"]["symbol"],
                    json!({"kind":"source","name":expected["symbol"]})
                );
                assert_eq!(item["textEdit"]["newText"], expected["insert"]);
            }
        }
        std::fs::remove_dir_all(temp).expect("cleanup");
    }
}
fn completion_text(item: &Value) -> &str {
    item["textEdit"]["newText"]
        .as_str()
        .or_else(|| item["insertText"].as_str())
        .unwrap_or_else(|| string(item, "label"))
}

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().expect("fixture string")
}
