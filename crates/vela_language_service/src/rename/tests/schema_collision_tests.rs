use super::*;

#[test]
fn source_backed_schema_member_rename_rejects_same_kind_collisions() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/_schema_defs.vela");
    let main_text = "\
pub fn main(player: Player) -> i64 {
    let first = player.level
    return player.grant(first)
}";
    let schema_text = "\
pub fn level() { return 1 }
pub fn rank() { return 2 }
pub fn grant() { return 3 }
pub fn award() { return 4 }";
    let mut databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(schema.clone(), schema_text),
    ]);
    let schema_record = databases
        .source_db()
        .records()
        .get(&schema)
        .expect("schema source should be indexed");
    let source_id = schema_record.source_id().get();
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
                source_backed_member("Player", "level", schema_text, source_id),
                {
                    "owner": "Player",
                    "name": "rank",
                    "fact": { "kind": "primitive", "name": "i64" }
                }
            ],
            "methods": [
                source_backed_method("Player", "grant", schema_text, source_id),
                {
                    "owner": "Player",
                    "name": "award",
                    "fact": {
                        "kind": "function",
                        "params": [{ "kind": "primitive", "name": "i64" }],
                        "returns": { "kind": "primitive", "name": "i64" }
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

    assert_eq!(
        databases.rename(
            &main,
            Position::new(1, line(main_text, 1).find("level").expect("field read")),
            "rank",
        ),
        None
    );
    assert_eq!(
        databases.rename(
            &main,
            Position::new(2, line(main_text, 2).find("grant").expect("method call")),
            "award",
        ),
        None
    );
}

fn source_backed_member(
    owner: &str,
    name: &str,
    schema_text: &str,
    source_id: u32,
) -> serde_json::Value {
    let start = schema_text.find(name).expect("schema member should exist");
    serde_json::json!({
        "owner": owner,
        "name": name,
        "fact": { "kind": "primitive", "name": "i64" },
        "sourceSpan": {
            "source": source_id,
            "start": start,
            "end": start + name.len()
        }
    })
}

fn source_backed_method(
    owner: &str,
    name: &str,
    schema_text: &str,
    source_id: u32,
) -> serde_json::Value {
    let start = schema_text.find(name).expect("schema method should exist");
    serde_json::json!({
        "owner": owner,
        "name": name,
        "fact": {
            "kind": "function",
            "params": [{ "kind": "primitive", "name": "i64" }],
            "returns": { "kind": "primitive", "name": "i64" }
        },
        "sourceSpan": {
            "source": source_id,
            "start": start,
            "end": start + name.len()
        }
    })
}
