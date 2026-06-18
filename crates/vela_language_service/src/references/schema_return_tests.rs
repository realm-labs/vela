use super::*;
use crate::{
    SourceFileSnapshot, SymbolRef, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};

#[test]
fn references_find_schema_method_calls_on_schema_method_return_receivers() {
    let (databases, main, schema, main_text, schema_text) = schema_method_return_fixture();

    let references = databases.references(
        &main,
        Position::new(1, line(main_text, 1).find("grant").expect("method call")),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference_in_document(
        &references,
        &schema,
        0,
        schema_text
            .find("grant")
            .expect("schema method declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference_in_document(
        &references,
        &main,
        1,
        line(main_text, 1).find("grant").expect("first method call"),
        ReferenceKind::Call,
    );
    assert_reference_in_document(
        &references,
        &main,
        2,
        line(main_text, 2)
            .find("grant")
            .expect("second method call"),
        ReferenceKind::Call,
    );
    assert_all_symbols(&references, &SymbolRef::Schema("Inventory.grant".into()));
}

#[test]
fn references_find_schema_trait_method_calls_on_schema_method_return_receivers() {
    let (databases, main, schema, main_text, schema_text) = schema_trait_method_return_fixture();

    let references = databases.references(
        &main,
        Position::new(1, line(main_text, 1).find("preview").expect("method call")),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference_in_document(
        &references,
        &schema,
        0,
        schema_text
            .find("preview")
            .expect("schema trait method declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference_in_document(
        &references,
        &main,
        1,
        line(main_text, 1)
            .find("preview")
            .expect("first method call"),
        ReferenceKind::Call,
    );
    assert_reference_in_document(
        &references,
        &main,
        2,
        line(main_text, 2)
            .find("preview")
            .expect("second method call"),
        ReferenceKind::Call,
    );
    assert_all_symbols(&references, &SymbolRef::Schema("Rewardable.preview".into()));
}

#[test]
fn document_highlight_marks_schema_method_calls_on_schema_method_return_receivers() {
    let (databases, main, _, main_text, _) = schema_method_return_fixture();

    let highlights = databases.document_highlights(
        &main,
        Position::new(1, line(main_text, 1).find("grant").expect("method call")),
    );

    assert_eq!(highlights.len(), 2, "{highlights:?}");
    assert_highlight(
        &highlights,
        1,
        line(main_text, 1).find("grant").expect("first method call"),
        DocumentHighlightKind::Call,
    );
    assert_highlight(
        &highlights,
        2,
        line(main_text, 2)
            .find("grant")
            .expect("second method call"),
        DocumentHighlightKind::Call,
    );
}

#[test]
fn document_highlight_marks_schema_trait_method_calls_on_schema_method_return_receivers() {
    let (databases, main, _, main_text, _) = schema_trait_method_return_fixture();

    let highlights = databases.document_highlights(
        &main,
        Position::new(1, line(main_text, 1).find("preview").expect("method call")),
    );

    assert_eq!(highlights.len(), 2, "{highlights:?}");
    assert_highlight(
        &highlights,
        1,
        line(main_text, 1)
            .find("preview")
            .expect("first method call"),
        DocumentHighlightKind::Call,
    );
    assert_highlight(
        &highlights,
        2,
        line(main_text, 2)
            .find("preview")
            .expect("second method call"),
        DocumentHighlightKind::Call,
    );
}

fn schema_method_return_fixture() -> (
    LanguageServiceDatabases,
    DocumentId,
    DocumentId,
    &'static str,
    &'static str,
) {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "\
pub fn main(player: Player) -> i64 {
    let first = player.inventory().grant(1)
    return player.inventory().grant(first)
}";
    let schema_text = "pub fn grant() { return 1 }";
    let mut databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(schema.clone(), schema_text),
    ]);
    let schema_record = databases
        .source_db()
        .records()
        .get(&schema)
        .expect("schema source should be indexed");
    let target_start = schema_text
        .find("grant")
        .expect("schema marker should exist");
    let target_end = target_start + "grant".len();
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "Player",
                    "fact": { "kind": "host", "name": "Player" }
                },
                {
                    "name": "Inventory",
                    "fact": { "kind": "host", "name": "Inventory" }
                }
            ],
            "methods": [
                {
                    "owner": "Player",
                    "name": "inventory",
                    "fact": {
                        "kind": "function",
                        "params": [],
                        "returns": { "kind": "host", "name": "Inventory" }
                    }
                },
                {
                    "owner": "Inventory",
                    "name": "grant",
                    "fact": {
                        "kind": "function",
                        "params": [{ "kind": "primitive", "name": "i64" }],
                        "returns": { "kind": "primitive", "name": "i64" }
                    },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": target_start,
                        "end": target_end
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);
    (databases, main, schema, main_text, schema_text)
}

fn schema_trait_method_return_fixture() -> (
    LanguageServiceDatabases,
    DocumentId,
    DocumentId,
    &'static str,
    &'static str,
) {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "\
pub fn main(player: Player) -> i64 {
    let first = player.rewardable().preview(1)
    return player.rewardable().preview(first)
}";
    let schema_text = "pub fn preview() { return 1 }";
    let mut databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(schema.clone(), schema_text),
    ]);
    let schema_record = databases
        .source_db()
        .records()
        .get(&schema)
        .expect("schema source should be indexed");
    let target_start = schema_text
        .find("preview")
        .expect("schema marker should exist");
    let target_end = target_start + "preview".len();
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "Player",
                    "fact": { "kind": "host", "name": "Player" }
                }
            ],
            "traits": [
                {
                    "name": "Rewardable",
                    "fact": { "kind": "trait", "name": "Rewardable" }
                }
            ],
            "methods": [
                {
                    "owner": "Player",
                    "name": "rewardable",
                    "fact": {
                        "kind": "function",
                        "params": [],
                        "returns": { "kind": "trait", "name": "Rewardable" }
                    }
                }
            ],
            "traitMethods": [
                {
                    "owner": "Rewardable",
                    "name": "preview",
                    "fact": {
                        "kind": "function",
                        "params": [{ "kind": "primitive", "name": "i64" }],
                        "returns": { "kind": "primitive", "name": "i64" }
                    },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": target_start,
                        "end": target_end
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);
    (databases, main, schema, main_text, schema_text)
}

fn assert_all_symbols(references: &[Reference], symbol: &SymbolRef) {
    assert!(
        references
            .iter()
            .all(|reference| reference.symbol() == symbol),
        "{references:?}"
    );
}

fn assert_reference_in_document(
    references: &[Reference],
    document: &DocumentId,
    line: usize,
    character: usize,
    kind: ReferenceKind,
) {
    assert!(
        references.iter().any(|reference| {
            reference.document_id() == document
                && reference.range().start() == Position::new(line, character)
                && reference.kind() == kind
        }),
        "{references:?}"
    );
}

fn assert_highlight(
    highlights: &[DocumentHighlight],
    line: usize,
    character: usize,
    kind: DocumentHighlightKind,
) {
    assert!(
        highlights.iter().any(|highlight| {
            highlight.range().start() == Position::new(line, character) && highlight.kind() == kind
        }),
        "{highlights:?}"
    );
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}

fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases
}
