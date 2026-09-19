use crate::matrix_fixture::{FixtureWorkspace, load};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, SymbolRef, TextRange,
    Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use serde_json::Value;

#[test]
fn enum_alias_matrix_preserves_candidates_and_applied_reference_owners() {
    assert_enum_alias("completion-enum-aliases");
}

#[test]
fn pattern_field_matrix_preserves_owned_labels_and_binding_boundaries() {
    assert_enum_alias("completion-pattern-fields");
}

#[test]
fn enum_alias_overlay_transitions_match_fresh_owner_facts() {
    let spec = load("completion-enum-aliases");
    let lifecycle = &spec.oracle["lifecycle"];
    let file = string(lifecycle, "file");
    let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
    let original = fixture.disk[file].clone();
    let suffix = original
        .text
        .split_once('\n')
        .expect("enum fixture invariant")
        .1;
    let column = original.markers["cursor"].start.byte
        - original.text.find('\n').expect("enum fixture invariant")
        - 1;
    let mut db = databases(&fixture);
    for state in lifecycle["states"]
        .as_array()
        .expect("enum fixture invariant")
    {
        let text = format!("{}\n{suffix}", string(state, "imports"));
        fixture
            .disk
            .get_mut(file)
            .expect("enum fixture invariant")
            .text = text.clone();
        update(&mut db, &fixture);
        let position = position(
            &text,
            text.find('\n').expect("enum fixture invariant") + 1 + column,
        );
        let items = db.completion_items(&uri(file), position);
        assert_eq!(
            items,
            databases(&fixture).completion_items(&uri(file), position),
            "{state}"
        );
        if state["detail"].is_null() {
            assert!(items.items().is_empty(), "{state}");
        } else {
            assert_eq!(items.items().len(), 1, "{state}");
            let item = &items.items()[0];
            assert_eq!(item.label(), "tag");
            assert_eq!(item.detail(), string(state, "detail"));
            assert_eq!(item.symbol(), Some(&symbol(state)));
        }
    }
}

fn assert_enum_alias(fixture_id: &str) {
    for crlf in [false, true] {
        let mut spec = load(fixture_id);
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let db = databases(&fixture);
        for case in spec.oracle["queries"].as_array().expect("queries") {
            let file = string(case, "file");
            let source = fixture.document(file).expect("source");
            let range = source.markers["replace"];
            let pos = position(&source.text, source.markers["cursor"].start.byte);
            let result = db.completion_items(&uri(file), pos);
            assert_eq!(
                format!("{:?}", result.context().kind()),
                case["context"],
                "{case}"
            );
            assert_eq!(result, db.completion_items(&uri(file), pos));
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
                            && i.insert_text().unwrap_or(i.label()) == string(expected, "insert")
                    })
                    .expect("item");
                assert_eq!(format!("{:?}", item.kind()), expected["kind"], "{case}");
                assert_eq!(item.detail(), string(expected, "detail"), "{case}");
                let expected_symbol = expected["serviceSymbol"]
                    .as_str()
                    .map(|name| SymbolRef::Source(name.to_owned()))
                    .unwrap_or_else(|| symbol(expected));
                if expected["origin"] != "local" {
                    assert_eq!(item.symbol(), Some(&expected_symbol), "{case} {expected}");
                }
                let edit = item.text_edit().expect("explicit edit");
                assert!(item.documentation().is_none());
                assert_eq!(
                    serde_json::json!(
                        db.completion_documentation(item.resolve_payload().expect("resolve"))
                    ),
                    expected["docs"],
                    "{case}"
                );
                assert_eq!(
                    edit.range(),
                    TextRange::new(range.start.byte, range.end.byte)
                );
                assert_eq!(edit.new_text(), string(expected, "insert"));
                let insertion = format!(
                    "{}{}",
                    edit.new_text().replace("$0", "1"),
                    string(case, "applySuffix")
                );
                let mut edited = source.text.clone();
                edited.replace_range(range.start.byte..range.end.byte, &insertion);
                assert!(
                    vela_syntax::parse::parse_source(&edited)
                        .diagnostics()
                        .is_empty(),
                    "{case}: {edited}"
                );
                let mut fresh = FixtureWorkspace::new(&spec).expect("fresh");
                fresh.disk.get_mut(file).expect("file").text = edited.clone();
                let fresh = databases(&fresh);
                if case["member"] == true {
                    let byte = source.markers["member"].start.byte + insertion.len()
                        - (range.end.byte - range.start.byte);
                    let members = fresh.completion_items(&uri(file), position(&edited, byte));
                    let crate::CompletionAnalysisKind::DotAccess(dot) = members.analysis().kind()
                    else {
                        panic!("member context")
                    };
                    assert_eq!(
                        dot.receiver_fact().map(|fact| fact.display_name()),
                        Some(string(expected, "receiver").to_owned()),
                        "{case}"
                    );
                    assert_eq!(
                        members
                            .items()
                            .iter()
                            .map(|item| item.label())
                            .collect::<Vec<_>>(),
                        expected["members"]
                            .as_array()
                            .expect("members")
                            .iter()
                            .map(|item| string(item, "label"))
                            .collect::<Vec<_>>(),
                        "{case}"
                    );
                    for (actual, wanted) in members
                        .items()
                        .iter()
                        .zip(expected["members"].as_array().expect("members"))
                    {
                        assert_eq!(actual.detail(), string(wanted, "detail"), "{case}");
                    }
                }
                let start = range.start.byte + insertion.rfind("::").map_or(0, |i| i + 2);
                if case["introducedBinding"] == true {
                    let end = range.start.byte + string(expected, "label").len();
                    let definition = fresh
                        .definition(&uri(file), position(&edited, range.start.byte + 1))
                        .expect("introduced shorthand binding");
                    assert_eq!(definition.document_id(), &uri(file));
                    assert_eq!(
                        definition.range().start(),
                        position(&edited, range.start.byte)
                    );
                    assert_eq!(definition.range().end(), position(&edited, end));
                    assert_eq!(
                        definition.symbol(),
                        Some(&SymbolRef::local_at(
                            string(expected, "label"),
                            uri(file),
                            TextRange::new(range.start.byte, end)
                        ))
                    );
                } else if let Some(target) = expected["target"].as_str() {
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
                    if expected["kind"] != "Module" {
                        assert_eq!(definition.symbol(), Some(&expected_symbol));
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
fn databases(f: &FixtureWorkspace) -> LanguageServiceDatabases {
    let mut db = LanguageServiceDatabases::new();
    update(&mut db, f);
    db.load_schema_artifact_json("/workspace/schema.json", &f.disk["schema.json"].text);
    assert!(db.schema_db().diagnostics().is_empty());
    db
}

fn update(db: &mut LanguageServiceDatabases, f: &FixtureWorkspace) {
    let files = f
        .disk
        .iter()
        .filter(|(p, _)| p.ends_with(".vela"))
        .map(|(p, s)| SourceFileSnapshot::new(uri(p), s.text.as_str()))
        .collect::<Vec<_>>();
    db.update(&assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]),
        &files,
        &Workspace::new().snapshot(),
    ));
}
