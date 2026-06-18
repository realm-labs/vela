use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};

use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn definition_follows_local_binding() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main(amount: i64) -> i64 { return amount }";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let definition = databases
        .definition(
            &document,
            Position::new(0, text.rfind("amount").expect("amount use")),
        )
        .expect("definition should resolve parameter binding");

    assert_eq!(definition.document_id(), &document);
    assert_eq!(definition.range().start().line, 0);
    assert_eq!(
        definition.range().start().character,
        text.find("amount")
            .expect("parameter declaration should exist")
    );
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::local_at(
            "amount",
            document.clone(),
            TextRange::new(12, 18)
        ))
    );
}

#[test]
fn declaration_follows_local_binding() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main(amount: i64) -> i64 { return amount }";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let declaration = databases
        .declaration(
            &document,
            Position::new(0, text.rfind("amount").expect("amount use")),
        )
        .expect("declaration should resolve parameter binding");

    assert_eq!(declaration.document_id(), &document);
    assert_eq!(declaration.range().start().line, 0);
    assert_eq!(
        declaration.range().start().character,
        text.find("amount")
            .expect("parameter declaration should exist")
    );
    assert_eq!(
        declaration.symbol(),
        Some(&SymbolRef::local_at(
            "amount",
            document.clone(),
            TextRange::new(12, 18)
        ))
    );
}

#[test]
fn definition_follows_function_call_after_qualified_stdlib_call() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"
fn add_mixed(value) {
math::abs(value);
return value + 1i8;
}

fn main() {
return add_mixed(1);
}
"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let definition = databases
        .definition(
            &document,
            Position::new(
                7,
                text.lines()
                    .nth(7)
                    .expect("call line")
                    .find("add_mixed")
                    .expect("call should exist"),
            ),
        )
        .expect("definition should resolve function call");

    assert_eq!(definition.document_id(), &document);
    assert_eq!(definition.range().start().line, 1);
    assert_eq!(definition.range().start().character, 3);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Source("game::main::add_mixed".into()))
    );
}

#[test]
fn definition_follows_source_struct_field_member_access() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"struct Cell {
value: i64,
}

fn assign_cell(cell: Cell, value) {
cell.value = value;
return cell.value;
}"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let field_line = text.lines().nth(5).expect("field write line");

    let definition = databases
        .definition(
            &document,
            Position::new(5, field_line.find("value").expect("field use")),
        )
        .expect("definition should resolve source field");

    assert_eq!(definition.document_id(), &document);
    assert_eq!(definition.range().start().line, 1);
    assert_eq!(
        definition.range().start().character,
        text.lines()
            .nth(1)
            .expect("field declaration line")
            .find("value")
            .expect("field declaration")
    );
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Source("game::main::Cell.value".into()))
    );
}

#[test]
fn definition_does_not_fallback_to_enclosing_function_for_unknown_member() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"struct Cell {
value: i64,
}

fn assign_cell(cell: Cell) {
return cell.missing;
}"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let use_line = text.lines().nth(5).expect("member use line");

    let definition = databases.definition(
        &document,
        Position::new(5, use_line.find("missing").expect("unknown field use")),
    );

    assert!(definition.is_none());
}

#[test]
fn type_definition_follows_local_source_type() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"struct Player {
level: i64,
}

fn main(player: Player) {
return player;
}"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let use_line = text.lines().nth(5).expect("player use line");

    let definition = databases
        .type_definition(
            &document,
            Position::new(5, use_line.find("player").expect("player use")),
        )
        .expect("type definition should resolve source struct");

    assert_eq!(definition.document_id(), &document);
    assert_eq!(definition.range().start().line, 0);
    assert_eq!(definition.range().start().character, 7);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Source("game::main::Player".into()))
    );
}

#[test]
fn type_definition_follows_source_field_type() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"struct Inventory {
slots: i64,
}

struct Player {
inventory: Inventory,
}

fn main(player: Player) {
return player.inventory;
}"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let use_line = text.lines().nth(9).expect("field use line");

    let definition = databases
        .type_definition(
            &document,
            Position::new(9, use_line.find("inventory").expect("field use")),
        )
        .expect("type definition should resolve source field type");

    assert_eq!(definition.document_id(), &document);
    assert_eq!(definition.range().start().line, 0);
    assert_eq!(definition.range().start().character, 7);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Source("game::main::Inventory".into()))
    );
}

#[test]
fn type_definition_returns_none_for_source_primitive_field() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"struct Cell {
value: i64,
}

fn main(cell: Cell) {
return cell.value;
}"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let use_line = text.lines().nth(5).expect("field use line");

    let definition = databases.type_definition(
        &document,
        Position::new(5, use_line.find("value").expect("field use")),
    );

    assert!(definition.is_none());
}

#[test]
fn type_definition_follows_schema_source_span() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "pub fn main(player: Player) { return 1 }";
    let schema_text = "pub fn host_player_schema() { return 1 }";
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
        .find("host_player_schema")
        .expect("schema marker should exist");
    let target_end = target_start + "host_player_schema".len();
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "Player",
                    "fact": { "kind": "host", "name": "Player" },
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

    let definition = databases
        .type_definition(
            &main,
            Position::new(0, main_text.find("Player").expect("type hint should exist")),
        )
        .expect("type definition should resolve schema source span");

    assert_eq!(definition.document_id(), &schema_source);
    assert_eq!(definition.range().start().line, 0);
    assert_eq!(definition.range().start().character, target_start);
    assert_eq!(definition.range().end().character, target_end);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Schema("Player".into()))
    );
}

#[test]
fn schema_type_without_source_span_does_not_fabricate_definition() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let main_text = "pub fn main(player: Player) { return 1 }";
    let mut databases = databases_for(vec![SourceFileSnapshot::new(main.clone(), main_text)]);
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    databases.set_schema_facts(schema);

    let definition = databases.type_definition(
        &main,
        Position::new(
            0,
            main_text
                .find("Player")
                .expect("schema type hint should exist"),
        ),
    );

    assert!(definition.is_none());
}

#[test]
fn definition_follows_imported_module_declaration() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let helper = DocumentId::from("/workspace/scripts/game/helper.vela");
    let main_text = "use game::helper::grant\npub fn main() { return grant() }";
    let helper_text = "pub fn grant() -> i64 { return 1 }";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(helper.clone(), helper_text),
    ]);
    let call_line = main_text.lines().nth(1).expect("call line should exist");

    let definition = databases
        .definition(
            &main,
            Position::new(1, call_line.find("grant").expect("grant call")),
        )
        .expect("definition should resolve imported function");

    assert_eq!(definition.document_id(), &helper);
    assert_eq!(definition.range().start().line, 0);
    assert_eq!(
        definition.range().start().character,
        helper_text.find("grant").expect("helper function name")
    );
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Source("game::helper::grant".into()))
    );
}

#[test]
fn definition_follows_schema_source_span() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "pub fn main(player: Player) { return 1 }";
    let schema_text = "pub fn host_player_schema() { return 1 }";
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
        .find("host_player_schema")
        .expect("schema marker should exist");
    let target_end = target_start + "host_player_schema".len();
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "Player",
                    "fact": { "kind": "host", "name": "Player" },
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

    let definition = databases
        .definition(
            &main,
            Position::new(0, main_text.find("Player").expect("type hint should exist")),
        )
        .expect("definition should resolve schema source span");

    assert_eq!(definition.document_id(), &schema_source);
    assert_eq!(definition.range().start().line, 0);
    assert_eq!(definition.range().start().character, target_start);
    assert_eq!(definition.range().end().character, target_end);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Schema("Player".into()))
    );
}

#[test]
fn definition_follows_schema_field_source_span() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "pub fn main(player: Player) { return player.level }";
    let schema_text = "pub fn level_marker() { return 1 }";
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
        .find("level_marker")
        .expect("schema marker should exist");
    let target_end = target_start + "level_marker".len();
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "Player",
                    "fact": { "kind": "host", "name": "Player" }
                }
            ],
            "fields": [
                {
                    "owner": "Player",
                    "name": "level",
                    "fact": { "kind": "primitive", "name": "i64" },
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

    let definition = databases
        .definition(
            &main,
            Position::new(0, main_text.find("level").expect("field use should exist")),
        )
        .expect("definition should resolve schema field source span");

    assert_eq!(definition.document_id(), &schema_source);
    assert_eq!(definition.range().start().character, target_start);
    assert_eq!(definition.range().end().character, target_end);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Schema("Player.level".into()))
    );
}

#[test]
fn definition_follows_schema_method_source_span() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "pub fn main(player: Player) { return player.grant(1) }";
    let schema_text = "pub fn grant_marker() { return true }";
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
        .find("grant_marker")
        .expect("schema marker should exist");
    let target_end = target_start + "grant_marker".len();
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "Player",
                    "fact": { "kind": "host", "name": "Player" }
                }
            ],
            "methods": [
                {
                    "owner": "Player",
                    "name": "grant",
                    "fact": {
                        "kind": "function",
                        "params": [{ "kind": "primitive", "name": "i64" }],
                        "returns": { "kind": "primitive", "name": "bool" }
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

    let definition = databases
        .definition(
            &main,
            Position::new(0, main_text.find("grant").expect("method use should exist")),
        )
        .expect("definition should resolve schema method source span");

    assert_eq!(definition.document_id(), &schema_source);
    assert_eq!(definition.range().start().character, target_start);
    assert_eq!(definition.range().end().character, target_end);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Schema("Player.grant".into()))
    );
}

#[test]
fn definition_follows_schema_trait_method_source_span() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "pub fn main(rewardable: Rewardable) { return rewardable.preview(1) }";
    let schema_text = "pub fn preview_marker() { return true }";
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
        .find("preview_marker")
        .expect("schema marker should exist");
    let target_end = target_start + "preview_marker".len();
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "traits": [
                {
                    "name": "Rewardable",
                    "fact": { "kind": "trait", "name": "Rewardable" }
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

    let definition = databases
        .definition(
            &main,
            Position::new(
                0,
                main_text
                    .find("preview")
                    .expect("trait method use should exist"),
            ),
        )
        .expect("definition should resolve schema trait method source span");

    assert_eq!(definition.document_id(), &schema_source);
    assert_eq!(definition.range().start().character, target_start);
    assert_eq!(definition.range().end().character, target_end);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Schema("Rewardable.preview".into()))
    );
}

#[test]
fn definition_follows_schema_variant_source_span() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "pub fn main() { return QuestState::Active }";
    let schema_text = "pub fn active_marker() { return 1 }";
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
        .find("active_marker")
        .expect("schema marker should exist");
    let target_end = target_start + "active_marker".len();
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "QuestState",
                    "fact": { "kind": "enum", "name": "QuestState", "variant": null }
                }
            ],
            "variants": [
                {
                    "owner": "QuestState",
                    "name": "Active",
                    "fact": {
                        "kind": "enum",
                        "name": "QuestState",
                        "variant": "Active"
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

    let definition = databases
        .definition(
            &main,
            Position::new(
                0,
                main_text.find("Active").expect("variant use should exist"),
            ),
        )
        .expect("definition should resolve schema variant source span");

    assert_eq!(definition.document_id(), &schema_source);
    assert_eq!(definition.range().start().character, target_start);
    assert_eq!(definition.range().end().character, target_end);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Schema("QuestState::Active".into()))
    );
}

#[test]
fn definition_follows_qualified_schema_variant_when_name_is_not_unique() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "pub fn main() { return QuestState::Active }";
    let schema_text = "pub fn active_marker() { return 1 }";
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
        .find("active_marker")
        .expect("schema marker should exist");
    let target_end = target_start + "active_marker".len();
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "QuestState",
                    "fact": { "kind": "enum", "name": "QuestState", "variant": null }
                },
                {
                    "name": "OtherState",
                    "fact": { "kind": "enum", "name": "OtherState", "variant": null }
                }
            ],
            "variants": [
                {
                    "owner": "QuestState",
                    "name": "Active",
                    "fact": {
                        "kind": "enum",
                        "name": "QuestState",
                        "variant": "Active"
                    },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": target_start,
                        "end": target_end
                    }
                },
                {
                    "owner": "OtherState",
                    "name": "Active",
                    "fact": {
                        "kind": "enum",
                        "name": "OtherState",
                        "variant": "Active"
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

    let definition = databases
        .definition(
            &main,
            Position::new(
                0,
                main_text.find("Active").expect("variant use should exist"),
            ),
        )
        .expect("definition should resolve qualified schema variant source span");

    assert_eq!(definition.document_id(), &schema_source);
    assert_eq!(definition.range().start().character, target_start);
    assert_eq!(definition.range().end().character, target_end);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Schema("QuestState::Active".into()))
    );
}

#[test]
fn type_definition_follows_schema_field_type_source_span() {
    assert_schema_member_type_definition(
        "pub fn main(player: Player) { return player.inventory }",
        "inventory",
        "pub fn inventory_type_marker() { return 1 }",
        "inventory_type_marker",
        |source, start, end| {
            serde_json::json!({
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" }
                    },
                    {
                        "name": "Inventory",
                        "fact": { "kind": "host", "name": "Inventory" },
                        "sourceSpan": {
                            "source": source,
                            "start": start,
                            "end": end
                        }
                    }
                ],
                "fields": [
                    {
                        "owner": "Player",
                        "name": "inventory",
                        "fact": { "kind": "host", "name": "Inventory" }
                    }
                ]
            })
        },
    );
}

#[test]
fn type_definition_returns_none_for_schema_primitive_field() {
    assert_schema_member_type_definition_none(
        "pub fn main(player: Player) { return player.level }",
        "level",
        "pub fn level_marker() { return 1 }",
        |source, start, end| {
            serde_json::json!({
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" }
                    }
                ],
                "fields": [
                    {
                        "owner": "Player",
                        "name": "level",
                        "fact": { "kind": "primitive", "name": "i64" },
                        "sourceSpan": {
                            "source": source,
                            "start": start,
                            "end": end
                        }
                    }
                ]
            })
        },
    );
}

#[test]
fn type_definition_returns_none_for_schema_method() {
    assert_schema_member_type_definition_none(
        "pub fn main(player: Player) { return player.grant(1) }",
        "grant",
        "pub fn grant_marker() { return true }",
        |source, start, end| {
            serde_json::json!({
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" }
                    }
                ],
                "methods": [
                    {
                        "owner": "Player",
                        "name": "grant",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "primitive", "name": "i64" }],
                            "returns": { "kind": "primitive", "name": "bool" }
                        },
                        "sourceSpan": {
                            "source": source,
                            "start": start,
                            "end": end
                        }
                    }
                ]
            })
        },
    );
}

#[test]
fn type_definition_returns_none_for_schema_trait_method() {
    assert_schema_member_type_definition_none(
        "pub fn main(rewardable: Rewardable) { return rewardable.preview(1) }",
        "preview",
        "pub fn preview_marker() { return true }",
        |source, start, end| {
            serde_json::json!({
                "traits": [
                    {
                        "name": "Rewardable",
                        "fact": { "kind": "trait", "name": "Rewardable" }
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
                            "start": start,
                            "end": end
                        }
                    }
                ]
            })
        },
    );
}

#[test]
fn type_definition_returns_none_for_schema_variant_without_owner_type_span() {
    let main_text = "pub fn main() { return QuestState::Active }";
    assert_schema_member_type_definition_none(
        main_text,
        "Active",
        "pub fn active_marker() { return 1 }",
        |source, start, end| {
            serde_json::json!({
                "types": [
                    {
                        "name": "QuestState",
                        "fact": { "kind": "enum", "name": "QuestState", "variant": null }
                    }
                ],
                "variants": [
                    {
                        "owner": "QuestState",
                        "name": "Active",
                        "fact": {
                            "kind": "enum",
                            "name": "QuestState",
                            "variant": "Active"
                        },
                        "sourceSpan": {
                            "source": source,
                            "start": start,
                            "end": end
                        }
                    }
                ]
            })
        },
    );
}

fn assert_schema_member_type_definition<F>(
    main_text: &str,
    usage_needle: &str,
    schema_text: &str,
    schema_marker: &str,
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

    let definition = databases
        .type_definition(
            &main,
            Position::new(0, main_text.find(usage_needle).expect("usage should exist")),
        )
        .expect("type definition should resolve schema source span");

    assert_eq!(definition.document_id(), &schema_source);
    assert_eq!(definition.range().start().character, target_start);
    assert_eq!(definition.range().end().character, target_end);
}

fn assert_schema_member_type_definition_none<F>(
    main_text: &str,
    usage_needle: &str,
    schema_text: &str,
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
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": facts(
            schema_record.source_id().get(),
            0,
            schema_text.len(),
        )
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

    let definition = databases.type_definition(
        &main,
        Position::new(0, main_text.find(usage_needle).expect("usage should exist")),
    );

    assert!(definition.is_none());
}

fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases
}
