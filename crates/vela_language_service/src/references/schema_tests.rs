use super::*;
use crate::{
    SourceFileSnapshot, SymbolRef, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};

#[test]
fn document_highlight_marks_schema_method_calls() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "\
pub fn main(player: Player, rewardable: Rewardable) -> i64 {
    let first = player.grant(1)
    let second = player.grant(first)
    return rewardable.preview(second)
}";
    let schema_text = "\
pub fn grant() { return 1 }
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
    let grant_start = schema_text.find("grant").expect("grant marker");
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

    let grant_highlights = databases.document_highlights(
        &main,
        Position::new(
            1,
            line(main_text, 1).find("grant").expect("first grant call"),
        ),
    );

    assert_eq!(grant_highlights.len(), 2, "{grant_highlights:?}");
    assert_highlight(
        &grant_highlights,
        1,
        line(main_text, 1).find("grant").expect("first grant call"),
        DocumentHighlightKind::Call,
    );
    assert_highlight(
        &grant_highlights,
        2,
        line(main_text, 2).find("grant").expect("second grant call"),
        DocumentHighlightKind::Call,
    );

    let preview_highlights = databases.document_highlights(
        &main,
        Position::new(
            3,
            line(main_text, 3)
                .find("preview")
                .expect("trait method call"),
        ),
    );

    assert_eq!(preview_highlights.len(), 1, "{preview_highlights:?}");
    assert_highlight(
        &preview_highlights,
        3,
        line(main_text, 3)
            .find("preview")
            .expect("trait method call"),
        DocumentHighlightKind::Call,
    );

    let declaration_highlights = databases.document_highlights(
        &schema,
        Position::new(
            0,
            line(schema_text, 0)
                .find("grant")
                .expect("schema method declaration"),
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
        line(schema_text, 0)
            .find("grant")
            .expect("schema method declaration"),
        DocumentHighlightKind::Text,
    );
}

#[test]
fn references_find_schema_field_reads_and_writes() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "\
pub fn main(player: Player) -> i64 {
    let first = player.level
    player.level += 1
    return player.level + first
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
        Position::new(1, line(main_text, 1).find("level").expect("field read")),
        true,
    );

    assert_eq!(references.len(), 4, "{references:?}");
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
        line(main_text, 1).find("level").expect("field read"),
        ReferenceKind::Read,
    );
    assert_reference_in_document(
        &references,
        &main,
        2,
        line(main_text, 2).find("level").expect("field write"),
        ReferenceKind::Write,
    );
    assert_reference_in_document(
        &references,
        &main,
        3,
        line(main_text, 3).find("level").expect("second field read"),
        ReferenceKind::Read,
    );
    assert_all_symbols(&references, &SymbolRef::Schema("Player.level".into()));

    let declaration_references = databases.references(
        &schema,
        Position::new(
            0,
            schema_text.find("level").expect("schema field declaration"),
        ),
        true,
    );

    assert_eq!(declaration_references, references);
}

#[test]
fn references_find_schema_method_calls() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "\
pub fn main(player: Player) -> i64 {
    let first = player.grant(1)
    return player.grant(first)
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
                }
            ],
            "methods": [
                {
                    "owner": "Player",
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

    let declaration_references = databases.references(
        &schema,
        Position::new(
            0,
            schema_text
                .find("grant")
                .expect("schema method declaration"),
        ),
        true,
    );

    assert_eq!(declaration_references, references);
}

#[test]
fn references_find_schema_method_calls_on_schema_function_return_receivers() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "\
pub fn main() -> i64 {
    let first = current_player().grant(1)
    return current_player().grant(first)
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
                }
            ],
            "functions": [
                {
                    "name": "current_player",
                    "fact": {
                        "kind": "function",
                        "params": [],
                        "returns": { "kind": "host", "name": "Player" }
                    }
                }
            ],
            "methods": [
                {
                    "owner": "Player",
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
    assert_all_symbols(&references, &SymbolRef::Schema("Player.grant".into()));
}

#[test]
fn references_find_schema_trait_method_calls_on_schema_function_return_receivers() {
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
    let target_start = schema_text
        .find("preview")
        .expect("schema marker should exist");
    let target_end = target_start + "preview".len();
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
fn document_highlight_marks_schema_method_calls_on_schema_function_return_receivers() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "\
pub fn main() -> i64 {
    let first = current_player().grant(1)
    return current_player().grant(first)
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
                }
            ],
            "functions": [
                {
                    "name": "current_player",
                    "fact": {
                        "kind": "function",
                        "params": [],
                        "returns": { "kind": "host", "name": "Player" }
                    }
                }
            ],
            "methods": [
                {
                    "owner": "Player",
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
fn document_highlight_marks_schema_trait_method_calls_on_schema_function_return_receivers() {
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
    let target_start = schema_text
        .find("preview")
        .expect("schema marker should exist");
    let target_end = target_start + "preview".len();
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
                        "start": target_start,
                        "end": target_end
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

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

#[test]
fn references_find_schema_variant_constructors_and_patterns() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "\
pub fn main(state: QuestState) -> i64 {
    let next = QuestState::Active
    match state {
        QuestState::Active => { return 1 }
        QuestState::Done => { return 2 }
    }
    return 0
}";
    let schema_text = "pub fn Active() { return 1 }\npub fn Done() { return 2 }";
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
    let done_start = schema_text
        .find("Done")
        .expect("schema Done marker should exist");
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
                        "start": active_start,
                        "end": active_start + "Active".len()
                    }
                },
                {
                    "owner": "QuestState",
                    "name": "Done",
                    "fact": {
                        "kind": "enum",
                        "name": "QuestState",
                        "variant": "Done"
                    },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": done_start,
                        "end": done_start + "Done".len()
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

    let references = databases.references(
        &main,
        Position::new(1, line(main_text, 1).find("Active").expect("constructor")),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference_in_document(
        &references,
        &schema,
        0,
        schema_text
            .find("Active")
            .expect("schema variant declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference_in_document(
        &references,
        &main,
        1,
        line(main_text, 1).find("Active").expect("constructor"),
        ReferenceKind::Read,
    );
    assert_reference_in_document(
        &references,
        &main,
        3,
        line(main_text, 3).find("Active").expect("pattern"),
        ReferenceKind::Pattern,
    );

    let declaration_references = databases.references(
        &schema,
        Position::new(
            0,
            schema_text
                .find("Active")
                .expect("schema variant declaration"),
        ),
        true,
    );

    assert_eq!(declaration_references, references);
}

#[test]
fn references_find_schema_trait_method_calls() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "\
pub fn main(player: Rewardable) -> i64 {
    let first = player.grant(1)
    return player.grant(first)
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
            "traits": [
                {
                    "name": "Rewardable",
                    "fact": { "kind": "trait", "name": "Rewardable" }
                }
            ],
            "traitMethods": [
                {
                    "owner": "Rewardable",
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
            .expect("schema trait method declaration"),
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

    let declaration_references = databases.references(
        &schema,
        Position::new(
            0,
            schema_text
                .find("grant")
                .expect("schema trait method declaration"),
        ),
        true,
    );

    assert_eq!(declaration_references, references);
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
