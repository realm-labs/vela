mod schema_lifecycle;
mod source_overlays;

use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, SymbolRef, TextRange,
    Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use serde_json::Value;

#[test]
fn type_ownership_matrix_preserves_candidates_and_applied_reference_owners() {
    assert_type_ownership("completion-type-ownership");
}

#[test]
fn package_type_ownership_preserves_dependency_boundaries_and_applied_targets() {
    assert_type_ownership("completion-package-type-ownership");
}

#[test]
fn package_callable_ownership_preserves_sets_signatures_and_applied_targets() {
    assert_type_ownership("completion-package-callables");
}

#[test]
fn package_member_ownership_preserves_sets_signatures_returns_and_targets() {
    assert_type_ownership("completion-package-members");
}

#[test]
fn returned_receiver_flow_preserves_owned_sets_signatures_and_targets() {
    assert_type_ownership("completion-return-flow");
}

#[test]
fn callback_contract_results_preserve_direct_lambda_facts_and_erased_boundaries() {
    assert_type_ownership("completion-callback-results");
}

#[test]
fn callback_slot_matrix_preserves_registered_slots_and_reordered_result_facts() {
    assert_type_ownership("completion-callback-slots");
}

#[test]
fn async_callable_matrix_preserves_owner_metadata_and_applied_await_contracts() {
    assert_type_ownership("completion-async-callables");
}

#[test]
fn await_context_matrix_preserves_owned_choices_and_exact_syntax_diagnostics() {
    assert_type_ownership("completion-await-contexts");
}

#[test]
fn receiver_assignment_flow_preserves_possible_owners_and_rejects_stale_members() {
    assert_type_ownership("completion-receiver-assignments");
}

#[test]
fn shared_method_matrix_preserves_merged_details_signatures_and_ambiguous_targets() {
    assert_type_ownership("completion-shared-methods");
}

#[test]
fn abrupt_flow_matrix_preserves_reachable_receivers_and_explicit_lambda_returns() {
    assert_type_ownership("completion-abrupt-flow");
}

#[test]
fn local_exit_matrix_preserves_reachable_assignment_owners() {
    assert_type_ownership("completion-local-exits");
}

#[test]
fn loop_match_matrix_preserves_backedge_owners_and_reachable_results() {
    assert_type_ownership("completion-loop-match-flow");
}

#[test]
fn schema_callable_lifecycle_preserves_current_contracts_and_stale_resolve() {
    assert_type_ownership("completion-schema-callable-lifecycle");
}

#[test]
fn unavailable_schema_lifecycle_preserves_source_authoring_and_clears_stale_host_facts() {
    assert_type_ownership("completion-schema-unavailable");
}

#[test]
fn source_callable_lifecycle_preserves_package_owners_after_edits_deletion_and_recreation() {
    assert_type_ownership("completion-source-callable-lifecycle");
}

#[test]
fn source_overlay_lifecycle_preserves_dirty_owners_save_and_close_restoration() {
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

#[test]
fn visible_binding_matrix_preserves_loop_pattern_and_initializer_scopes() {
    assert_type_ownership("completion-visible-bindings");
}

#[test]
fn declaration_context_matrix_preserves_initializer_and_parameter_owners() {
    assert_type_ownership("completion-declaration-contexts");
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
        let layout = Layout::new(&fixture);
        let uri = |file: &str| layout.uri(file);
        let mut db = databases(&fixture, &layout);
        let mut overlays = (spec.oracle["sourceOverlays"] == true)
            .then(|| source_overlays::State::new(&spec, &layout));
        let source_lifecycle = spec.oracle["sourceLifecycle"].is_array();
        let phases = spec
            .oracle
            .get("sourceLifecycle")
            .unwrap_or(&spec.oracle["schemaLifecycle"])
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![Value::Null]);
        let mut saved =
            std::collections::BTreeMap::<String, crate::CompletionResolvePayload>::new();
        for phase in phases {
            let mut current = spec.clone();
            if source_lifecycle {
                current = crate::matrix_fixture::source_lifecycle_spec(&spec, &phase, crlf);
                let fixture = FixtureWorkspace::new(&current).expect("source phase");
                if let Some(state) = &mut overlays {
                    state.apply(&mut db, &layout, &phase, crlf);
                    crate::matrix_fixture::assert_source_overlay_state(
                        &state.fixture,
                        &fixture,
                        &phase,
                        crlf,
                    );
                } else {
                    update(&mut db, &fixture, &layout);
                }
                for payload in saved.values() {
                    assert_eq!(
                        db.completion_documentation(payload),
                        None,
                        "source resolve: {phase}"
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
                schema_lifecycle::apply_phase(&mut db, &phase);
                for (name, payload) in &saved {
                    assert_eq!(
                        serde_json::json!(db.completion_documentation(payload)),
                        phase["resolveDocs"][name],
                        "stale resolve {name}: {phase}"
                    );
                }
            }
            let spec = current;
            let fixture = overlays.as_ref().map_or_else(
                || FixtureWorkspace::new(&spec).expect("phase fixture"),
                |state| state.fixture.clone(),
            );
            if let Some(file) = spec.oracle["schemaDiagnosticFile"].as_str() {
                schema_lifecycle::assert_diagnostics(&db, &layout, file, &phase);
            }
            for query in phase["signatureQueries"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
            {
                let file = string(query, "file");
                let source = fixture.document(file).expect("signature document");
                let point = position(&source.text, source.markers["cursor"].start.byte);
                let result = db.signature_help(&uri(file), point);
                assert_eq!(
                    result,
                    phase_databases(&fixture, &layout, &phase).signature_help(&uri(file), point),
                    "fresh signature: {phase}"
                );
                let labels = result.as_ref().map(|help| {
                    help.signatures()
                        .iter()
                        .map(|signature| signature.label())
                        .collect::<Vec<_>>()
                });
                assert_eq!(
                    serde_json::json!(labels),
                    query["labels"],
                    "cached signature: {phase}"
                );
                if source_lifecycle {
                    let point = position(&source.text, source.markers["callee"].start.byte + 1);
                    let actual = db.definition(&uri(file), point);
                    assert_eq!(
                        actual,
                        databases(&fixture, &layout).definition(&uri(file), point)
                    );
                    if let Some(target) = query["target"].as_str() {
                        let target_source = fixture.document(target).expect("target");
                        let marker = target_source.markers[string(query, "marker")];
                        let actual = actual.expect("existing-call definition");
                        assert_eq!(actual.document_id(), &uri(target));
                        assert_eq!(
                            actual.range().start(),
                            position(&target_source.text, marker.start.byte)
                        );
                        assert_eq!(
                            actual.range().end(),
                            position(&target_source.text, marker.end.byte)
                        );
                    } else {
                        assert!(actual.is_none(), "removed dependency: {query}");
                    }
                }
            }
            for case in spec.oracle["queries"].as_array().expect("queries") {
                let file = string(case, "file");
                let source = fixture.document(file).expect("source");
                let range = source.markers["replace"];
                if case["parseErrors"] == true {
                    assert!(
                        !vela_syntax::parse::parse_source(&source.text)
                            .diagnostics()
                            .is_empty()
                    );
                }
                if let Some(signature) = case["callSignature"].as_str() {
                    let point = position(&source.text, source.markers["call-cursor"].start.byte);
                    let help = db
                        .signature_help(&uri(file), point)
                        .expect("known dynamic-return callable");
                    assert_eq!(help.signatures().len(), 1, "{case}");
                    assert_eq!(help.signatures()[0].label(), signature, "{case}");
                }
                let pos = position(&source.text, source.markers["cursor"].start.byte);
                let result = db.completion_items(&uri(file), pos);
                if !phase.is_null() {
                    assert_eq!(
                        result,
                        phase_databases(&fixture, &layout, &phase)
                            .completion_items(&uri(file), pos),
                        "fresh lifecycle state: {case}"
                    );
                    for item in result.items() {
                        match item.symbol() {
                            Some(SymbolRef::Schema(name)) if !source_lifecycle => {
                                saved.insert(
                                    name.clone(),
                                    item.resolve_payload().expect("resolve").clone(),
                                );
                            }
                            Some(SymbolRef::Source(_)) if source_lifecycle => {
                                saved.insert(
                                    format!("{}:{}", string(case, "id"), item.label()),
                                    item.resolve_payload().expect("resolve").clone(),
                                );
                            }
                            _ => {}
                        }
                    }
                }
                assert_eq!(
                    result.context().kind(),
                    match case["context"].as_str() {
                        Some("StructFieldDeclaration") =>
                            crate::CompletionContextKind::StructFieldDeclaration,
                        Some("Expression") => crate::CompletionContextKind::Expression,
                        Some("ModulePath") => crate::CompletionContextKind::ModulePath,
                        Some("Member") => crate::CompletionContextKind::Member,
                        Some("RecordField") => crate::CompletionContextKind::RecordField,
                        Some("Pattern") => crate::CompletionContextKind::Pattern,
                        _ => crate::CompletionContextKind::TypeHint,
                    },
                    "{case}"
                );
                assert_eq!(result, db.completion_items(&uri(file), pos));
                if let Some(scope) = case.get("visibleScope") {
                    assert_eq!(
                        &serde_json::json!(result.analysis().visible_scope()),
                        scope,
                        "{case}"
                    );
                }
                if let Some(receiver) = case["receiver"].as_str() {
                    let crate::CompletionAnalysisKind::DotAccess(dot) = result.analysis().kind()
                    else {
                        panic!("member analysis: {case}");
                    };
                    assert_eq!(
                        dot.receiver_fact().map(|fact| fact.display_name()),
                        Some(receiver.to_owned()),
                        "{case}"
                    );
                }
                let expected = case["items"].as_array().expect("items");
                let mut keys = expected
                    .iter()
                    .map(|i| (string(i, "label"), string(i, "insert")))
                    .collect::<Vec<_>>();
                let mut actual = result
                    .items()
                    .iter()
                    .map(|i| (i.label(), i.insert_text().unwrap_or(i.label())))
                    .collect::<Vec<_>>();
                keys.sort();
                actual.sort();
                assert_eq!(actual, keys, "{case}");
                for expected in expected {
                    let item = result
                        .items()
                        .iter()
                        .find(|i| {
                            i.label() == expected["label"]
                                && i.insert_text().unwrap_or(i.label())
                                    == string(expected, "insert")
                        })
                        .expect("item");
                    assert_eq!(format!("{:?}", item.kind()), expected["kind"], "{case}");
                    assert_eq!(item.detail(), string(expected, "detail"), "{case}");
                    if let Some(format) = expected["insertFormat"].as_str() {
                        assert_eq!(format!("{:?}", item.insert_format()), format, "{case}");
                    }
                    let expected_symbol = (expected["origin"] != "none").then(|| {
                        expected["serviceSymbol"]
                            .as_str()
                            .map(|name| SymbolRef::Source(name.to_owned()))
                            .unwrap_or_else(|| symbol(expected))
                    });
                    if expected["origin"] != "local" {
                        assert_eq!(item.symbol(), expected_symbol.as_ref(), "{case} {expected}");
                    }
                    let insert = if expected["implicitEdit"] == true {
                        assert!(item.text_edit().is_none(), "{case}");
                        assert!(item.insert_text().is_none(), "{case}");
                        item.label()
                    } else {
                        let edit = item
                            .text_edit()
                            .unwrap_or_else(|| panic!("explicit edit: {case} {expected}"));
                        assert_eq!(
                            edit.range(),
                            TextRange::new(range.start.byte, range.end.byte)
                        );
                        edit.new_text()
                    };
                    assert!(item.documentation().is_none());
                    if expected["origin"] == "none" {
                        assert!(
                            item.resolve_payload().is_none(),
                            "no invented identity: {case}"
                        );
                    } else {
                        assert_eq!(
                            serde_json::json!(db.completion_documentation(
                                item.resolve_payload().expect("resolve")
                            )),
                            expected["docs"],
                            "{case}"
                        );
                    }
                    assert_eq!(insert, string(expected, "insert"));
                    let insertion = format!(
                        "{}{}",
                        insert.replace("$0", expected["value"].as_str().unwrap_or("1")),
                        string(case, "applySuffix")
                    );
                    let mut edited = source.text.clone();
                    edited.replace_range(range.start.byte..range.end.byte, &insertion);
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
                    let mut fresh = FixtureWorkspace::new(&spec).expect("fresh");
                    fresh.disk.get_mut(file).expect("file").text = edited.clone();
                    let fresh = phase_databases(&fresh, &layout, &phase);
                    if let Some(applied_file) = expected["appliedFile"].as_str() {
                        let applied = fixture.document(applied_file).expect("applied oracle");
                        assert_eq!(edited, applied.text, "{case}");
                        let diagnostics = fresh.diagnostics_for_document(&uri(file));
                        let actual = diagnostics
                            .diagnostics()
                            .iter()
                            .filter(|d| d.code().is_some_and(|code| code.contains("await")))
                            .collect::<Vec<_>>();
                        let oracle = expected["diagnostics"]
                            .as_array()
                            .expect("await diagnostics");
                        assert_eq!(actual.len(), oracle.len(), "{case}: {actual:?}");
                        for (actual, expected) in actual.iter().zip(oracle) {
                            assert_eq!(actual.code(), expected["code"].as_str());
                            assert_eq!(actual.message(), string(expected, "message"));
                            assert_eq!(actual.severity(), crate::ServiceDiagnosticSeverity::Error);
                            let range = applied.markers[string(expected, "marker")];
                            let actual_range = actual.range().expect("await diagnostic range");
                            assert_eq!(actual_range.start(), position(&edited, range.start.byte));
                            assert_eq!(actual_range.end(), position(&edited, range.end.byte));
                            assert_eq!(actual.labels().len(), 1);
                            assert_eq!(actual.labels()[0].message(), string(expected, "label"));
                            assert_eq!(actual.labels()[0].document_id(), &uri(file));
                            assert_eq!(actual.labels()[0].range(), actual_range);
                        }
                    }
                    if case["checkAwait"] == true {
                        let diagnostics = fresh.diagnostics_for_document(&uri(file));
                        let missing = diagnostics
                            .diagnostics()
                            .iter()
                            .filter(|diagnostic| {
                                diagnostic.code() == Some("analysis::async_call_requires_await")
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
                            let end = call.end.byte + insertion.len()
                                - (range.end.byte - range.start.byte);
                            assert_eq!(
                                diagnostic.severity(),
                                crate::ServiceDiagnosticSeverity::Error
                            );
                            let diagnostic_range =
                                diagnostic.range().expect("call diagnostic range");
                            assert_eq!(
                                diagnostic_range.start(),
                                position(&edited, call.start.byte)
                            );
                            assert_eq!(diagnostic_range.end(), position(&edited, end));
                        }
                    }
                    if let Some(signatures) = expected["signatures"].as_array() {
                        let open = range.start.byte + insertion.find('(').expect("call");
                        let help = fresh
                            .signature_help(&uri(file), position(&edited, open + 1))
                            .expect("applied alternative signatures");
                        let mut actual = help
                            .signatures()
                            .iter()
                            .map(|signature| signature.label())
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
                        let open = range.start.byte + insertion.find('(').expect("call");
                        let help = fresh
                            .signature_help(&uri(file), position(&edited, open + 1))
                            .expect("applied signature");
                        assert_eq!(help.signatures().len(), 1, "{case}");
                        assert_eq!(help.signatures()[0].label(), signature, "{case} {expected}");
                    }
                    if let Some(argument) = expected.get("argument") {
                        if let Some(label) = expected["inlay"].as_str() {
                            let start = range.start.byte + insertion.find('(').expect("call") + 1;
                            let hints = fresh.inlay_hints(
                                &uri(file),
                                crate::DiagnosticRange::new(
                                    position(&edited, start),
                                    position(&edited, start),
                                ),
                            );
                            assert_eq!(hints.len(), 1, "{case}");
                            assert_eq!(hints[0].kind(), crate::InlayHintKind::Parameter);
                            assert_eq!(hints[0].label(), label, "{case}");
                            assert_eq!(hints[0].position(), position(&edited, start));
                        }
                        let text = source.text[..range.start.byte].to_owned()
                            + &string(expected, "insert").replace("$0", "zz_")
                            + &source.text[range.end.byte..];
                        let start = range.start.byte
                            + string(expected, "insert").find('(').expect("call")
                            + 1;
                        let mut argument_fixture =
                            FixtureWorkspace::new(&spec).expect("argument fixture");
                        argument_fixture.disk.get_mut(file).expect("file").text = text.clone();
                        let arguments = phase_databases(&argument_fixture, &layout, &phase)
                            .completion_items(&uri(file), position(&text, start + 3));
                        assert_eq!(
                            arguments
                                .items()
                                .iter()
                                .map(|i| i.label())
                                .collect::<Vec<_>>(),
                            [string(argument, "label")],
                            "{case}"
                        );
                        let parameter = &arguments.items()[0];
                        assert_eq!(parameter.kind(), crate::CompletionKind::Parameter);
                        assert_eq!(parameter.detail(), string(argument, "detail"));
                        let edit = parameter.text_edit().expect("parameter edit");
                        assert_eq!(edit.range(), TextRange::new(start, start + 3));
                        assert_eq!(edit.new_text(), format!("{} = ", string(argument, "label")));
                    }
                    if case["member"] == true {
                        let byte = source.markers["member"].start.byte + insertion.len()
                            - (range.end.byte - range.start.byte);
                        let members = fresh.completion_items(&uri(file), position(&edited, byte));
                        let crate::CompletionAnalysisKind::DotAccess(dot) =
                            members.analysis().kind()
                        else {
                            panic!("member context")
                        };
                        assert_eq!(
                            dot.receiver_fact().map(|fact| fact.display_name()),
                            Some(string(expected, "symbol").to_owned()),
                            "{case}"
                        );
                        assert_eq!(
                            members
                                .items()
                                .iter()
                                .map(|item| item.label())
                                .collect::<Vec<_>>(),
                            ["tag"]
                        );
                        if let Some(detail) = expected["fieldDetail"].as_str() {
                            assert_eq!(members.items()[0].detail(), detail, "{case} {expected}");
                        }
                    }
                    let start = range.start.byte
                        + insertion
                            .split('(')
                            .next()
                            .expect("path")
                            .rfind("::")
                            .map_or(0, |i| i + 2);
                    if let Some(target) = expected["target"].as_str() {
                        let target = if target == "self" { file } else { target };
                        let definition = fresh
                            .definition(&uri(file), position(&edited, start + 1))
                            .unwrap_or_else(|| panic!("missing definition {case} {expected}"));
                        assert_eq!(definition.document_id(), &uri(target));
                        let target_doc = fixture.document(target).expect("target");
                        let target_range = target_doc.markers[string(expected, "marker")];
                        assert_eq!(
                            definition.range().start(),
                            position(&target_doc.text, target_range.start.byte)
                        );
                        assert_eq!(
                            definition.range().end(),
                            position(&target_doc.text, target_range.end.byte)
                        );
                        if expected["kind"] != "Module" && expected["origin"] != "none" {
                            assert_eq!(definition.symbol(), expected_symbol.as_ref());
                        }
                    } else {
                        assert!(
                            fresh
                                .definition(&uri(file), position(&edited, start + 1))
                                .is_none(),
                            "metadata-only {case}"
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn type_ownership_lifecycle_rebinds_rejects_and_restores_cached_candidates() {
    let spec = load("completion-type-ownership");
    let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
    let layout = Layout::new(&fixture);
    let uri = |file: &str| layout.uri(file);
    let mut db = databases(&fixture, &layout);
    let file = "scripts/source_alias.vela";
    for crlf in [false, true] {
        for step in spec.oracle["lifecycle"].as_array().expect("steps") {
            let text = string(step, "source").replace('\n', if crlf { "\r\n" } else { "\n" });
            let source = crate::matrix_fixture::parse_markers(&text).expect("source");
            fixture.disk.get_mut(file).expect("file").text = source.text.clone();
            update(&mut db, &fixture, &layout);
            let result = db.completion_items(
                &uri(file),
                position(&source.text, source.markers["cursor"].start.byte),
            );
            let expected = step["items"].as_array().expect("items");
            assert_eq!(result.items().len(), expected.len(), "{step}");
            for (item, expected) in result.items().iter().zip(expected) {
                assert_eq!(item.label(), string(expected, "label"));
                assert_eq!(item.detail(), string(expected, "detail"));
                assert_eq!(item.symbol(), Some(&symbol(expected)));
                assert_eq!(
                    item.text_edit().expect("edit").new_text(),
                    string(expected, "insert")
                );
            }
        }
    }
}

fn symbol(expected: &Value) -> SymbolRef {
    let name = string(expected, "symbol").to_owned();
    match string(expected, "origin") {
        "source" => SymbolRef::Source(name),
        "schema" => SymbolRef::Schema(name),
        "builtin" => SymbolRef::Builtin(name),
        "local" => SymbolRef::local(name),
        other => panic!("{other}"),
    }
}
fn string<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().expect("fixture string")
}
fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}
fn position(text: &str, byte: usize) -> Position {
    Position::new(
        text[..byte].bytes().filter(|c| *c == b'\n').count(),
        byte - text[..byte].rfind('\n').map_or(0, |i| i + 1),
    )
}
fn phase_databases(
    fixture: &FixtureWorkspace,
    layout: &Layout,
    phase: &Value,
) -> LanguageServiceDatabases {
    if phase.is_null() || phase["files"].is_object() {
        return databases(fixture, layout);
    }
    let mut db = LanguageServiceDatabases::new();
    update(&mut db, fixture, layout);
    schema_lifecycle::apply_phase(&mut db, phase);
    db
}

fn databases(f: &FixtureWorkspace, layout: &Layout) -> LanguageServiceDatabases {
    let mut db = LanguageServiceDatabases::new();
    update(&mut db, f, layout);
    let artifact = if let Some(markers) = f.disk.get("schema-markers.json") {
        crate::matrix_fixture::schema_artifact(
            &serde_json::from_str::<Value>(&markers.text).expect("schema markers"),
            f,
            |file| {
                db.source_db().records()[&layout.uri(file)]
                    .source_id()
                    .get()
            },
        )
        .to_string()
    } else {
        f.disk["schema.json"].text.clone()
    };
    db.load_schema_artifact_json("/workspace/schema.json", &artifact);
    assert!(db.schema_db().diagnostics().is_empty());
    db
}

fn update(db: &mut LanguageServiceDatabases, f: &FixtureWorkspace, layout: &Layout) {
    let mut workspace = Workspace::new();
    for (file, document) in &f.open {
        workspace.open_document(
            layout.uri(file),
            document.text.as_str(),
            crate::SourceVersion::new(1),
        );
    }
    update_with_workspace(db, f, layout, &workspace);
}

fn update_with_workspace(
    db: &mut LanguageServiceDatabases,
    f: &FixtureWorkspace,
    layout: &Layout,
    workspace: &Workspace,
) {
    let files = f
        .disk
        .iter()
        .filter(|(p, _)| p.ends_with(".vela"))
        .map(|(p, s)| SourceFileSnapshot::new(layout.uri(p), s.text.as_str()))
        .collect::<Vec<_>>();
    if let Some(graph) = &layout.package {
        db.update(&crate::assemble_package_project_sources(
            graph,
            &files,
            &workspace.snapshot(),
        ));
        return;
    }
    db.update(&assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]),
        &files,
        &workspace.snapshot(),
    ));
}

struct Layout {
    root: Option<std::path::PathBuf>,
    package: Option<vela_package::PackageGraph>,
}
impl Layout {
    fn new(f: &FixtureWorkspace) -> Self {
        if !f.disk.contains_key("deps/external/vela.toml") {
            return Self {
                root: None,
                package: None,
            };
        }
        let root = std::env::temp_dir().join(format!(
            "vela-type-ownership-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        f.materialize(&root).expect("package fixture");
        let package =
            vela_package::load_package_graph(root.join("vela.toml"), std::slice::from_ref(&root))
                .expect("packages");
        Self {
            root: Some(root),
            package: Some(package),
        }
    }
    fn uri(&self, file: &str) -> DocumentId {
        self.root.as_ref().map_or_else(
            || uri(file),
            |root| DocumentId::from(root.join(file).to_string_lossy().replace('\\', "/")),
        )
    }
}
impl Drop for Layout {
    fn drop(&mut self) {
        if let Some(root) = &self.root {
            std::fs::remove_dir_all(root).expect("cleanup");
        }
    }
}
