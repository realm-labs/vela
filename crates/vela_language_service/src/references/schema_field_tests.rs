use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn references_find_schema_record_constructor_field_labels() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "\
pub fn make(level: i64) -> Player {
    let player = Player { level: level }
    return player
}

pub fn main(player: Player) -> i64 {
    return player.level
}";
    let schema_text = "pub fn level() { return 1 }";
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
        .find("level")
        .expect("schema marker should exist");
    let target_end = target_start + "level".len();
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

    let references = databases.references(
        &main,
        Position::new(
            1,
            line(main_text, 1)
                .find("level")
                .expect("constructor field label"),
        ),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference_in_document(
        &references,
        &schema,
        0,
        schema_text.find("level").expect("schema field declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference_in_document(
        &references,
        &main,
        1,
        line(main_text, 1)
            .find("level")
            .expect("constructor field label"),
        ReferenceKind::Read,
    );
    assert_reference_in_document(
        &references,
        &main,
        6,
        line(main_text, 6).find("level").expect("member field read"),
        ReferenceKind::Read,
    );
}

fn assert_reference_in_document(
    references: &[Reference],
    document_id: &DocumentId,
    line: usize,
    character: usize,
    kind: ReferenceKind,
) {
    assert!(
        references.iter().any(|reference| {
            reference.document_id() == document_id
                && reference.range().start().line == line
                && reference.range().start().character == character
                && reference.kind() == kind
        }),
        "{references:?}"
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
