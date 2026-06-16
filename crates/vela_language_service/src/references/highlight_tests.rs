use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn document_highlight_marks_schema_variant_uses() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "\
pub fn main(state: QuestState) -> i64 {
    let next = QuestState::Active(1)
    match state {
        QuestState::Active(level) => return level
        QuestState::Done => return 0
    }
}";
    let schema_text = "pub enum QuestState { Active(i64), Done }";
    let mut databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(schema.clone(), schema_text),
    ]);
    let schema_record = databases
        .source_db()
        .records()
        .get(&schema)
        .expect("schema source should be indexed");
    let active_start = schema_text
        .find("Active")
        .expect("schema Active marker should exist");
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "QuestState",
                    "fact": { "kind": "enum", "name": "QuestState" }
                }
            ],
            "variants": [
                {
                    "owner": "QuestState",
                    "name": "Active",
                    "fact": {
                        "kind": "enum",
                        "name": "QuestState",
                        "variant": "Active",
                        "payload": [{ "kind": "primitive", "name": "i64" }]
                    },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": active_start,
                        "end": active_start + "Active".len()
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

    let highlights = databases.document_highlights(
        &main,
        Position::new(1, line(main_text, 1).find("Active").expect("constructor")),
    );

    assert_eq!(highlights.len(), 2, "{highlights:?}");
    assert_highlight(
        &highlights,
        1,
        line(main_text, 1).find("Active").expect("constructor"),
        DocumentHighlightKind::Read,
    );
    assert_highlight(
        &highlights,
        3,
        line(main_text, 3).find("Active").expect("pattern"),
        DocumentHighlightKind::Text,
    );

    let declaration_highlights = databases.document_highlights(
        &schema,
        Position::new(
            0,
            schema_text
                .find("Active")
                .expect("schema variant declaration"),
        ),
    );

    assert_eq!(
        declaration_highlights.len(),
        1,
        "{declaration_highlights:?}"
    );
    assert_highlight(
        &declaration_highlights,
        0,
        schema_text
            .find("Active")
            .expect("schema variant declaration"),
        DocumentHighlightKind::Text,
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
            highlight.range().start().line == line
                && highlight.range().start().character == character
                && highlight.kind() == kind
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
