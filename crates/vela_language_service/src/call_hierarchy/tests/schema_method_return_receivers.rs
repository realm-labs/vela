use super::*;

#[test]
fn call_hierarchy_uses_schema_method_calls_on_schema_method_return_receivers() {
    let (databases, main, schema, main_text, schema_text) = schema_method_return_fixture();

    let grant_from_declaration = databases.prepare_call_hierarchy(
        &schema,
        Position::new(1, line(schema_text, 1).find("grant").expect("grant")),
    );
    let grant_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(1, line(main_text, 1).find("grant").expect("grant")),
    );

    assert_eq!(grant_from_declaration.len(), 1);
    assert_eq!(grant_from_declaration[0].name(), "grant");
    assert_eq!(grant_from_declaration[0].document_id(), &schema);
    assert_eq!(grant_from_call, grant_from_declaration);

    let incoming = databases.incoming_calls(&grant_from_declaration[0]);
    assert_eq!(incoming.len(), 1, "{incoming:?}");
    assert_eq!(incoming[0].from().name(), "main");
    assert_range(
        incoming[0].from_ranges(),
        1,
        line(main_text, 1).find("grant").expect("first grant call"),
    );
    assert_range(
        incoming[0].from_ranges(),
        2,
        line(main_text, 2).find("grant").expect("second grant call"),
    );

    let main_item = databases
        .prepare_call_hierarchy(
            &main,
            Position::new(0, line(main_text, 0).find("main").expect("main")),
        )
        .pop()
        .expect("main should prepare a call hierarchy item");
    let outgoing = databases.outgoing_calls(&main_item);
    assert_eq!(outgoing.len(), 2, "{outgoing:?}");
    assert_outgoing_call(
        &outgoing,
        "inventory",
        &schema,
        1,
        line(main_text, 1)
            .find("inventory")
            .expect("first inventory call"),
    );
    assert_outgoing_call(
        &outgoing,
        "grant",
        &schema,
        1,
        line(main_text, 1).find("grant").expect("first grant call"),
    );
    assert_outgoing_call(
        &outgoing,
        "inventory",
        &schema,
        2,
        line(main_text, 2)
            .find("inventory")
            .expect("second inventory call"),
    );
    assert_outgoing_call(
        &outgoing,
        "grant",
        &schema,
        2,
        line(main_text, 2).find("grant").expect("second grant call"),
    );
}

#[test]
fn call_hierarchy_uses_schema_trait_method_calls_on_schema_method_return_receivers() {
    let (databases, main, schema, main_text, schema_text) = schema_trait_method_return_fixture();

    let preview_from_declaration = databases.prepare_call_hierarchy(
        &schema,
        Position::new(1, line(schema_text, 1).find("preview").expect("preview")),
    );
    let preview_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(1, line(main_text, 1).find("preview").expect("preview")),
    );

    assert_eq!(preview_from_declaration.len(), 1);
    assert_eq!(preview_from_declaration[0].name(), "preview");
    assert_eq!(preview_from_declaration[0].document_id(), &schema);
    assert_eq!(preview_from_call, preview_from_declaration);

    let incoming = databases.incoming_calls(&preview_from_declaration[0]);
    assert_eq!(incoming.len(), 1, "{incoming:?}");
    assert_eq!(incoming[0].from().name(), "main");
    assert_range(
        incoming[0].from_ranges(),
        1,
        line(main_text, 1)
            .find("preview")
            .expect("first preview call"),
    );
    assert_range(
        incoming[0].from_ranges(),
        2,
        line(main_text, 2)
            .find("preview")
            .expect("second preview call"),
    );

    let main_item = databases
        .prepare_call_hierarchy(
            &main,
            Position::new(0, line(main_text, 0).find("main").expect("main")),
        )
        .pop()
        .expect("main should prepare a call hierarchy item");
    let outgoing = databases.outgoing_calls(&main_item);
    assert_eq!(outgoing.len(), 2, "{outgoing:?}");
    assert_outgoing_call(
        &outgoing,
        "rewardable",
        &schema,
        1,
        line(main_text, 1)
            .find("rewardable")
            .expect("first rewardable call"),
    );
    assert_outgoing_call(
        &outgoing,
        "preview",
        &schema,
        1,
        line(main_text, 1)
            .find("preview")
            .expect("first preview call"),
    );
    assert_outgoing_call(
        &outgoing,
        "rewardable",
        &schema,
        2,
        line(main_text, 2)
            .find("rewardable")
            .expect("second rewardable call"),
    );
    assert_outgoing_call(
        &outgoing,
        "preview",
        &schema,
        2,
        line(main_text, 2)
            .find("preview")
            .expect("second preview call"),
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
    let schema_text = "\
pub fn inventory() { return 1 }
pub fn grant() { return 1 }";
    let mut databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(schema.clone(), schema_text),
    ]);
    let schema_record = databases
        .source_db()
        .records()
        .get(&schema)
        .expect("schema source should be indexed");
    let inventory_start = schema_text.find("inventory").expect("inventory marker");
    let grant_start = schema_text.find("grant").expect("grant marker");
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
                    },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": inventory_start,
                        "end": inventory_start + "inventory".len()
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
                        "start": grant_start,
                        "end": grant_start + "grant".len()
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
    let schema_text = "\
pub fn rewardable() { return 1 }
pub fn preview() { return 1 }";
    let mut databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(schema.clone(), schema_text),
    ]);
    let schema_record = databases
        .source_db()
        .records()
        .get(&schema)
        .expect("schema source should be indexed");
    let rewardable_start = schema_text.find("rewardable").expect("rewardable marker");
    let preview_start = schema_text.find("preview").expect("preview marker");
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
                    },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": rewardable_start,
                        "end": rewardable_start + "rewardable".len()
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
                        "start": preview_start,
                        "end": preview_start + "preview".len()
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);
    (databases, main, schema, main_text, schema_text)
}
