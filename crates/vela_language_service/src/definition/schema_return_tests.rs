use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn definition_follows_schema_method_on_schema_method_return_receiver() {
    assert_schema_method_return_navigation(NavigationKind::Definition);
}

#[test]
fn declaration_follows_schema_method_on_schema_method_return_receiver() {
    assert_schema_method_return_navigation(NavigationKind::Declaration);
}

#[test]
fn definition_follows_schema_trait_method_on_schema_method_return_receiver() {
    assert_schema_trait_method_return_navigation(NavigationKind::Definition);
}

#[test]
fn declaration_follows_schema_trait_method_on_schema_method_return_receiver() {
    assert_schema_trait_method_return_navigation(NavigationKind::Declaration);
}

#[derive(Clone, Copy)]
enum NavigationKind {
    Definition,
    Declaration,
}

fn assert_schema_method_return_navigation(kind: NavigationKind) {
    assert_schema_return_navigation(
        kind,
        "pub fn grant_marker() { return true }",
        "grant_marker",
        "pub fn main(player: Player) { return player.inventory().grant(1) }",
        "grant",
        "Inventory.grant",
        |source, target_start, target_end| {
            serde_json::json!({
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
                            "returns": { "kind": "primitive", "name": "bool" }
                        },
                        "sourceSpan": {
                            "source": source,
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            })
        },
    );
}

fn assert_schema_trait_method_return_navigation(kind: NavigationKind) {
    assert_schema_return_navigation(
        kind,
        "pub fn preview_marker() { return true }",
        "preview_marker",
        "pub fn main(player: Player) { return player.rewardable().preview(1) }",
        "preview",
        "Rewardable.preview",
        |source, target_start, target_end| {
            serde_json::json!({
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
                            "returns": { "kind": "primitive", "name": "bool" }
                        },
                        "sourceSpan": {
                            "source": source,
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            })
        },
    );
}

fn assert_schema_return_navigation<F>(
    kind: NavigationKind,
    schema_text: &str,
    schema_marker: &str,
    main_text: &str,
    usage_needle: &str,
    expected_symbol: &str,
    facts: F,
) where
    F: FnOnce(u32, usize, usize) -> serde_json::Value,
{
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let mut databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(schema_source.clone(), schema_text),
    ]);
    let schema_record = databases
        .source_db()
        .records()
        .get(&schema_source)
        .expect("schema source should be indexed");
    let target_start = schema_text
        .find(schema_marker)
        .expect("schema marker should exist");
    let target_end = target_start + schema_marker.len();
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": facts(schema_record.source_id().get(), target_start, target_end)
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);
    let position = Position::new(
        0,
        main_text
            .find(usage_needle)
            .expect("schema member use should exist"),
    );

    let definition = match kind {
        NavigationKind::Definition => databases.definition(&main, position),
        NavigationKind::Declaration => databases.declaration(&main, position),
    }
    .expect("navigation should resolve schema method source span");

    assert_eq!(definition.document_id(), &schema_source);
    assert_eq!(definition.range().start().character, target_start);
    assert_eq!(definition.range().end().character, target_end);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Schema(expected_symbol.to_owned()))
    );
}

fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases
}
