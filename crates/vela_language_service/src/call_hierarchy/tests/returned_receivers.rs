use super::*;

#[test]
fn call_hierarchy_uses_source_method_calls_on_source_method_return_receivers() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Player {
    level: i64
}

pub struct Inventory {
    count: i64
}

impl Player {
    pub fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}

impl Inventory {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

pub fn main(player: Player) -> i64 {
    let first = player.inventory().grant(1)
    return player.inventory().grant(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(main.clone(), text)]);

    let prepared_from_declaration = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            13,
            line(text, 13)
                .find("grant")
                .expect("method declaration should exist"),
        ),
    );
    let prepared_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            17,
            line(text, 17)
                .find("grant")
                .expect("method-return receiver method call should exist"),
        ),
    );

    assert_eq!(prepared_from_declaration.len(), 1);
    assert_eq!(prepared_from_declaration[0].name(), "grant");
    assert_eq!(prepared_from_declaration[0].document_id(), &main);
    assert_eq!(prepared_from_call, prepared_from_declaration);

    let incoming = databases.incoming_calls(&prepared_from_declaration[0]);
    assert_eq!(incoming.len(), 1, "{incoming:?}");
    assert_eq!(incoming[0].from().name(), "main");
    assert_range(
        incoming[0].from_ranges(),
        17,
        line(text, 17).find("grant").expect("first method call"),
    );
    assert_range(
        incoming[0].from_ranges(),
        18,
        line(text, 18).find("grant").expect("second method call"),
    );

    let main_item = databases
        .prepare_call_hierarchy(
            &main,
            Position::new(16, line(text, 16).find("main").expect("main")),
        )
        .pop()
        .expect("main should prepare a call hierarchy item");
    let outgoing = databases.outgoing_calls(&main_item);
    assert_eq!(outgoing.len(), 2, "{outgoing:?}");
    assert_outgoing_call(
        &outgoing,
        "inventory",
        &main,
        17,
        line(text, 17)
            .find("inventory")
            .expect("first inventory call"),
    );
    assert_outgoing_call(
        &outgoing,
        "grant",
        &main,
        17,
        line(text, 17).find("grant").expect("first grant call"),
    );
    assert_outgoing_call(
        &outgoing,
        "inventory",
        &main,
        18,
        line(text, 18)
            .find("inventory")
            .expect("second inventory call"),
    );
    assert_outgoing_call(
        &outgoing,
        "grant",
        &main,
        18,
        line(text, 18).find("grant").expect("second grant call"),
    );
}

#[test]
fn call_hierarchy_uses_source_trait_default_method_calls_on_source_method_return_receivers() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub trait Rewardable {
    fn preview(self, amount: i64) -> i64 { return amount }
}

pub struct Player {
    level: i64
}

pub struct Inventory {
    count: i64
}

impl Player {
    pub fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}

impl Rewardable for Inventory {}

pub fn main(player: Player) -> i64 {
    let first = player.inventory().preview(1)
    return player.inventory().preview(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(main.clone(), text)]);

    let prepared_from_declaration = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            1,
            line(text, 1)
                .find("preview")
                .expect("trait method declaration should exist"),
        ),
    );
    let prepared_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            19,
            line(text, 19)
                .find("preview")
                .expect("method-return receiver trait method call should exist"),
        ),
    );

    assert_eq!(prepared_from_declaration.len(), 1);
    assert_eq!(prepared_from_declaration[0].name(), "preview");
    assert_eq!(prepared_from_declaration[0].document_id(), &main);
    assert_eq!(prepared_from_call, prepared_from_declaration);

    let incoming = databases.incoming_calls(&prepared_from_declaration[0]);
    assert_eq!(incoming.len(), 1, "{incoming:?}");
    assert_eq!(incoming[0].from().name(), "main");
    assert_range(
        incoming[0].from_ranges(),
        19,
        line(text, 19).find("preview").expect("first preview call"),
    );
    assert_range(
        incoming[0].from_ranges(),
        20,
        line(text, 20).find("preview").expect("second preview call"),
    );

    let main_item = databases
        .prepare_call_hierarchy(
            &main,
            Position::new(18, line(text, 18).find("main").expect("main")),
        )
        .pop()
        .expect("main should prepare a call hierarchy item");
    let outgoing = databases.outgoing_calls(&main_item);
    assert_eq!(outgoing.len(), 2, "{outgoing:?}");
    assert_outgoing_call(
        &outgoing,
        "inventory",
        &main,
        19,
        line(text, 19)
            .find("inventory")
            .expect("first inventory call"),
    );
    assert_outgoing_call(
        &outgoing,
        "preview",
        &main,
        19,
        line(text, 19).find("preview").expect("first preview call"),
    );
    assert_outgoing_call(
        &outgoing,
        "inventory",
        &main,
        20,
        line(text, 20)
            .find("inventory")
            .expect("second inventory call"),
    );
    assert_outgoing_call(
        &outgoing,
        "preview",
        &main,
        20,
        line(text, 20).find("preview").expect("second preview call"),
    );
}

#[test]
fn call_hierarchy_uses_schema_trait_method_calls_on_schema_function_return_receivers() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "\
pub fn main() -> i64 {
    let first = current_reward().preview(1)
    return current_reward().preview(first)
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
    let preview_start = schema_text.find("preview").expect("preview marker");
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "traits": [
                {
                    "name": "Rewardable",
                    "fact": { "kind": "trait", "name": "Rewardable" }
                }
            ],
            "functions": [
                {
                    "name": "current_reward",
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
                        "start": preview_start,
                        "end": preview_start + "preview".len()
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

    let preview_from_declaration = databases.prepare_call_hierarchy(
        &schema,
        Position::new(
            0,
            line(schema_text, 0)
                .find("preview")
                .expect("preview declaration"),
        ),
    );
    let preview_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(1, line(main_text, 1).find("preview").expect("preview call")),
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
    assert_eq!(outgoing.len(), 1, "{outgoing:?}");
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
        "preview",
        &schema,
        2,
        line(main_text, 2)
            .find("preview")
            .expect("second preview call"),
    );
}
