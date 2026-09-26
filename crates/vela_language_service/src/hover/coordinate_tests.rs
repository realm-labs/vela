use crate::matrix_fixture::{Document, FixtureWorkspace, hover_signature as oracle};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, SymbolRef, TextRange,
    Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn position(value: &Value) -> Position {
    Position::new(
        usize::try_from(value["line"].as_u64().expect("line")).expect("usize"),
        usize::try_from(value["character"].as_u64().expect("column")).expect("usize"),
    )
}

fn expected_symbol(
    result: &Value,
    files: &BTreeMap<String, Document>,
    ids: &BTreeMap<String, DocumentId>,
) -> SymbolRef {
    let symbol = &result["symbol"];
    let name = symbol["name"].as_str().expect("symbol name");
    match symbol["kind"].as_str().expect("symbol kind") {
        "source" => SymbolRef::Source(name.to_owned()),
        "local" => {
            let file = symbol["file"].as_str().expect("local file");
            let marker = files[file].markers[symbol["marker"].as_str().expect("local declaration")];
            SymbolRef::local_at(
                name,
                ids[file].clone(),
                TextRange::new(marker.start.byte, marker.end.byte),
            )
        }
        kind => panic!("unreviewed symbol kind {kind}"),
    }
}

#[test]
fn hover_signature_coordinates_preserve_complete_unicode_results_and_repeats() {
    for crlf in [false, true] {
        let mut databases = LanguageServiceDatabases::new();
        let mut original = None;
        for shifted in [false, true, false] {
            let spec = oracle::spec(crlf, shifted);
            let fixture = FixtureWorkspace::new(&spec).expect("fixture");
            let ids = fixture
                .disk
                .keys()
                .filter(|file| file.ends_with(".vela"))
                .map(|file| {
                    (
                        file.clone(),
                        DocumentId::from(format!("/workspace/中文 % hover/{file}")),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let sources = ids
                .iter()
                .map(|(file, id)| {
                    SourceFileSnapshot::new(id.clone(), fixture.disk[file].text.as_str())
                })
                .collect::<Vec<_>>();
            databases.update(&assemble_project_sources(
                &WorkspaceConfig::workspace([WorkspaceRoot::from(
                    "/workspace/中文 % hover/scripts",
                )]),
                &sources,
                &Workspace::new().snapshot(),
            ));
            let mut observed = Vec::new();
            for query in spec.oracle["hover"].as_array().expect("hover queries") {
                let file = query["file"].as_str().expect("file");
                let document = &fixture.disk[file];
                let expected = oracle::hover_result(document, query, false);
                let marker = query["marker"].as_str().expect("marker");
                for offset in 0..if query["result"].is_null() { 1 } else { 2 } {
                    let point = position(&oracle::position(document, marker, false, offset));
                    let actual = databases.hover(&ids[file], point);
                    let normalized = actual.as_ref().map_or(Value::Null,|hover|json!({
                        "label":hover.label(),"kind":format!("{:?}",hover.kind()),"detail":hover.detail(),"docs":hover.docs(),
                        "range":{"start":{"line":hover.range().start().line,"character":hover.range().start().character},
                        "end":{"line":hover.range().end().line,"character":hover.range().end().character}}
                    }));
                    assert_eq!(
                        normalized, expected,
                        "{file}/{marker}, CRLF={crlf}, shifted={shifted}"
                    );
                    if let Some(hover) = &actual {
                        assert_eq!(
                            hover.symbol(),
                            Some(&expected_symbol(&query["result"], &fixture.disk, &ids))
                        );
                    }
                    assert_eq!(databases.hover(&ids[file], point), actual, "repeat hover");
                    observed.push(normalized);
                }
            }
            for query in spec.oracle["signatures"]
                .as_array()
                .expect("signature queries")
            {
                let file = query["file"].as_str().expect("file");
                let point = position(&oracle::position(
                    &fixture.disk[file],
                    query["marker"].as_str().expect("marker"),
                    false,
                    0,
                ));
                let actual = databases.signature_help(&ids[file], point);
                let normalized = actual.as_ref().map_or(Value::Null,|help|json!({
                    "activeSignature":help.active_signature(),"activeParameter":help.active_parameter(),
                    "signatures":help.signatures().iter().map(|signature|json!({"label":signature.label(),
                        "parameters":signature.parameters().iter().map(|parameter|json!({"name":parameter.name(),"label":parameter.label(),"type":parameter.type_fact().display_name()})).collect::<Vec<_>>()
                    })).collect::<Vec<_>>()
                }));
                assert_eq!(
                    normalized,
                    oracle::signature_result(query, false),
                    "{}, CRLF={crlf}, shifted={shifted}",
                    query["marker"]
                );
                assert_eq!(
                    databases.signature_help(&ids[file], point),
                    actual,
                    "repeat signature"
                );
                observed.push(normalized);
            }
            if !shifted {
                if let Some(original) = &original {
                    assert_eq!(&observed, original, "exact restoration");
                } else {
                    original = Some(observed);
                }
            }
        }
    }
}
