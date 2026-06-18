use super::*;

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
