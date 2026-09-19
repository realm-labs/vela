use crate::matrix_fixture::{load, parse_markers};
use crate::{
    CompletionKind, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, TextRange,
    Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use serde_json::{Value, json};

#[test]
fn authoring_surface_preserves_complete_templates_compact_details_and_applied_source() {
    let spec = load("completion-authoring-surface");
    for crlf in [false, true] {
        for query in spec.oracle["queries"].as_array().expect("queries") {
            let file = query["file"].as_str().expect("file");
            let source = parse_markers(&eol(&spec.files[file], crlf)).expect("source");
            let document = DocumentId::from("/workspace/scripts/main.vela");
            let db = databases(&document, &source.text);
            let point = source.markers["cursor"].start;
            let position = position(&source.text, point.byte);
            let result = db.completion_items(&document, position);
            assert_eq!(result, db.completion_items(&document, position));
            assert_eq!(
                format!("{:?}", result.context().kind()),
                query["context"],
                "{query}"
            );
            let items = query["items"].as_array().expect("items");
            let allowed = query["allowedBuiltins"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let mut expected_labels = items
                .iter()
                .map(|i| i["label"].as_str().expect("label"))
                .chain(allowed.iter().map(|i| i.as_str().expect("builtin")))
                .collect::<Vec<_>>();
            // The fixture lists builtin alternatives separately from authoring templates.
            // Their complete set and ownership still form part of the negative oracle.
            if !allowed.is_empty() {
                expected_labels.sort_unstable();
            }
            if let Some(inventory) = query["inventory"].as_array() {
                expected_labels = inventory
                    .iter()
                    .map(|i| i.as_str().expect("method"))
                    .collect();
                let owner = query["owner"].as_str().expect("owner");
                let range = source.markers["replace"];
                for item in result.items() {
                    assert_eq!(item.kind(), CompletionKind::Method);
                    assert_eq!(
                        item.symbol(),
                        Some(&crate::CompletionSymbol::Builtin(format!(
                            "{owner}.{}",
                            item.label()
                        )))
                    );
                    let edit = item.text_edit().expect("method edit");
                    assert_eq!(
                        edit.range(),
                        TextRange::new(range.start.byte, range.end.byte)
                    );
                    assert_eq!(edit.new_text(), format!("{}($0)", item.label()));
                    assert!(item.documentation().is_none());
                    assert!(
                        db.completion_documentation(item.resolve_payload().expect("payload"))
                            .is_none()
                    );
                }
            }
            assert_eq!(
                result.items().iter().map(|i| i.label()).collect::<Vec<_>>(),
                expected_labels,
                "{query}"
            );
            for label in &allowed {
                let label = label.as_str().expect("builtin");
                let item = result
                    .items()
                    .iter()
                    .find(|i| i.label() == label)
                    .expect("builtin item");
                assert_eq!(item.kind(), CompletionKind::Function);
                assert_eq!(
                    item.symbol(),
                    Some(&crate::CompletionSymbol::Builtin(label.to_owned()))
                );
                assert!(item.documentation().is_none());
            }
            for expected in items {
                let item = result
                    .items()
                    .iter()
                    .find(|i| i.label() == expected["label"])
                    .expect("expected candidate");
                assert_eq!(format!("{:?}", item.kind()), expected["kind"]);
                assert_eq!(item.detail(), expected["detail"]);
                assert_eq!(item.detail_parts().render(), expected["detail"]);
                assert_eq!(json!(item.label_details().detail()), expected["detail"]);
                assert!(item.label_details().description().is_none());
                assert_eq!(item.lookup(), item.label());
                assert_eq!(item.filter_text(), item.label());
                if let Some(symbol) = expected["symbol"].as_str() {
                    assert_eq!(
                        item.symbol(),
                        Some(&crate::CompletionSymbol::Builtin(symbol.to_owned()))
                    );
                    assert!(
                        db.completion_documentation(item.resolve_payload().expect("resolve"))
                            .is_none()
                    );
                } else {
                    assert!(item.symbol().is_none());
                    assert!(item.resolve_payload().is_none());
                }
                assert!(item.documentation().is_none());
                assert_eq!(json!(item.insert_text()), expected["insert"]);
                assert_eq!(
                    format!("{:?}", item.insert_format()),
                    if expected["format"] == 2 {
                        "Snippet"
                    } else {
                        "PlainText"
                    }
                );
                let range = source.markers["replace"];
                let edit = item.text_edit().expect("edit");
                assert_eq!(
                    edit.range(),
                    TextRange::new(range.start.byte, range.end.byte)
                );
                assert_eq!(edit.new_text(), expected["insert"]);
                let expanded = expand(edit.new_text(), expected);
                let mut applied = source.text.clone();
                applied.replace_range(range.start.byte..range.end.byte, &expanded);
                // Snippet bodies use LF; surrounding CRLF must remain untouched.
                let expected_text = expected["applied"].as_str().expect("applied");
                let expected_text =
                    eol(expected_text, crlf).replacen(&eol(&expanded, crlf), &expanded, 1);
                assert_eq!(applied, expected_text, "{query}");
                let parsed = vela_syntax::parse::parse_source(&applied);
                assert!(
                    parsed.diagnostics().is_empty(),
                    "{query}: {:?}",
                    parsed.diagnostics()
                );
                let again = databases(&document, &applied).completion_items(
                    &document,
                    self::position(
                        &applied,
                        range.start.byte
                            + if item.kind() == CompletionKind::Parameter {
                                expanded.len()
                            } else {
                                0
                            },
                    ),
                );
                let labels = again
                    .items()
                    .iter()
                    .filter(|i| {
                        if query["inventory"].is_array() {
                            i.kind() == CompletionKind::Method
                        } else if item.kind() == CompletionKind::Parameter {
                            i.kind() == CompletionKind::Parameter
                        } else {
                            matches!(i.kind(), CompletionKind::Keyword | CompletionKind::Snippet)
                        }
                    })
                    .map(|i| i.label())
                    .collect::<Vec<_>>();
                assert_eq!(
                    json!(labels),
                    expected["requery"],
                    "applied query: {query}, {applied}"
                );
            }
        }
    }
}

fn expand(insert: &str, expected: &Value) -> String {
    let mut expanded = insert.to_owned();
    for fill in expected["fills"].as_array().expect("fills") {
        let from = fill[0].as_str().expect("placeholder");
        assert!(expanded.contains(from));
        expanded = expanded.replace(from, fill[1].as_str().expect("fill"));
    }
    assert_eq!(expanded, expected["expanded"]);
    assert!(!expanded.contains('$'));
    expanded
}
fn eol(text: &str, crlf: bool) -> String {
    text.replace('\n', if crlf { "\r\n" } else { "\n" })
}
fn position(text: &str, byte: usize) -> Position {
    Position::new(
        text[..byte].bytes().filter(|c| *c == b'\n').count(),
        byte - text[..byte].rfind('\n').map_or(0, |i| i + 1),
    )
}
fn databases(document: &DocumentId, text: &str) -> LanguageServiceDatabases {
    let project = assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]),
        &[SourceFileSnapshot::new(document.clone(), text)],
        &Workspace::new().snapshot(),
    );
    let mut db = LanguageServiceDatabases::new();
    db.update(&project);
    db
}
