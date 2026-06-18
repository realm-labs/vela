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

#[test]
fn document_highlight_imported_symbol_stays_in_active_document() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
    let main_text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    let first = grant(amount)
    return grant(first)
}";
    let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(helper.clone(), helper_text),
    ]);

    let references = databases.references(
        &main,
        Position::new(2, line(main_text, 2).find("grant").expect("grant call")),
        true,
    );
    assert_eq!(references.len(), 4, "{references:?}");
    assert_reference(
        &references,
        &helper,
        0,
        helper_text.find("grant").expect("helper declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &references,
        &main,
        0,
        line(main_text, 0).find("grant").expect("import"),
        ReferenceKind::Import,
    );
    assert_reference(
        &references,
        &main,
        2,
        line(main_text, 2).find("grant").expect("first call"),
        ReferenceKind::Call,
    );
    assert_reference(
        &references,
        &main,
        3,
        line(main_text, 3).find("grant").expect("second call"),
        ReferenceKind::Call,
    );

    let highlights = databases.document_highlights(
        &main,
        Position::new(2, line(main_text, 2).find("grant").expect("grant call")),
    );
    assert_eq!(highlights.len(), 3, "{highlights:?}");
    assert_highlight(
        &highlights,
        0,
        line(main_text, 0).find("grant").expect("import"),
        DocumentHighlightKind::Text,
    );
    assert_highlight(
        &highlights,
        2,
        line(main_text, 2).find("grant").expect("first call"),
        DocumentHighlightKind::Call,
    );
    assert_highlight(
        &highlights,
        3,
        line(main_text, 3).find("grant").expect("second call"),
        DocumentHighlightKind::Call,
    );
    assert!(
        highlights.iter().all(|highlight| {
            highlight.range().start().line != 0
                || highlight.range().start().character
                    != helper_text.find("grant").expect("helper declaration")
        }),
        "{highlights:?}"
    );
}

#[test]
fn document_highlight_imported_const_and_global_stays_in_active_document() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let rewards = DocumentId::from("/workspace/scripts/game/rewards.vela");
    let main_text = "\
use game::rewards::BASE_REWARD
use game::rewards::reward_scale
pub fn main() -> i64 {
    let first = BASE_REWARD
    return first + reward_scale
}";
    let rewards_text = "\
pub const BASE_REWARD = 4
pub global reward_scale: i64";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(rewards.clone(), rewards_text),
    ]);

    let const_references = databases.references(
        &main,
        Position::new(
            3,
            line(main_text, 3)
                .find("BASE_REWARD")
                .expect("const use should exist"),
        ),
        true,
    );
    assert_eq!(const_references.len(), 3, "{const_references:?}");
    assert_reference(
        &const_references,
        &rewards,
        0,
        line(rewards_text, 0)
            .find("BASE_REWARD")
            .expect("const declaration should exist"),
        ReferenceKind::Declaration,
    );

    let const_highlights = databases.document_highlights(
        &main,
        Position::new(
            3,
            line(main_text, 3)
                .find("BASE_REWARD")
                .expect("const use should exist"),
        ),
    );
    assert_eq!(const_highlights.len(), 2, "{const_highlights:?}");
    assert_highlight(
        &const_highlights,
        0,
        line(main_text, 0)
            .find("BASE_REWARD")
            .expect("const import should exist"),
        DocumentHighlightKind::Text,
    );
    assert_highlight(
        &const_highlights,
        3,
        line(main_text, 3)
            .find("BASE_REWARD")
            .expect("const use should exist"),
        DocumentHighlightKind::Read,
    );
    assert_no_highlight(
        &const_highlights,
        0,
        line(rewards_text, 0)
            .find("BASE_REWARD")
            .expect("const declaration should exist"),
    );

    let global_references = databases.references(
        &main,
        Position::new(
            4,
            line(main_text, 4)
                .find("reward_scale")
                .expect("global use should exist"),
        ),
        true,
    );
    assert_eq!(global_references.len(), 3, "{global_references:?}");
    assert_reference(
        &global_references,
        &rewards,
        1,
        line(rewards_text, 1)
            .find("reward_scale")
            .expect("global declaration should exist"),
        ReferenceKind::Declaration,
    );

    let global_highlights = databases.document_highlights(
        &main,
        Position::new(
            4,
            line(main_text, 4)
                .find("reward_scale")
                .expect("global use should exist"),
        ),
    );
    assert_eq!(global_highlights.len(), 2, "{global_highlights:?}");
    assert_highlight(
        &global_highlights,
        1,
        line(main_text, 1)
            .find("reward_scale")
            .expect("global import should exist"),
        DocumentHighlightKind::Text,
    );
    assert_highlight(
        &global_highlights,
        4,
        line(main_text, 4)
            .find("reward_scale")
            .expect("global use should exist"),
        DocumentHighlightKind::Read,
    );
    assert_no_highlight(
        &global_highlights,
        1,
        line(rewards_text, 1)
            .find("reward_scale")
            .expect("global declaration should exist"),
    );
}

#[test]
fn document_highlight_imported_source_field_and_method_stays_in_active_document() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let types = DocumentId::from("/workspace/scripts/game/types.vela");
    let main_text = "\
use game::types::Reward

pub fn main(reward: Reward) -> i64 {
    let first = reward.amount
    let second = reward.total()
    return first + second + reward.amount + reward.total()
}";
    let types_text = "\
pub struct Reward {
    amount: i64
}

impl Reward {
    pub fn total(self) -> i64 { return 1 }
}";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(types.clone(), types_text),
        SourceFileSnapshot::new(main.clone(), main_text),
    ]);

    let field_references = databases.references(
        &main,
        Position::new(
            3,
            line(main_text, 3)
                .find("amount")
                .expect("first field read should exist"),
        ),
        true,
    );
    assert_eq!(field_references.len(), 3, "{field_references:?}");
    assert_reference(
        &field_references,
        &types,
        1,
        line(types_text, 1)
            .find("amount")
            .expect("field declaration should exist"),
        ReferenceKind::Declaration,
    );

    let field_highlights = databases.document_highlights(
        &main,
        Position::new(
            3,
            line(main_text, 3)
                .find("amount")
                .expect("first field read should exist"),
        ),
    );
    assert_eq!(field_highlights.len(), 2, "{field_highlights:?}");
    assert_highlight(
        &field_highlights,
        3,
        line(main_text, 3)
            .find("amount")
            .expect("first field read should exist"),
        DocumentHighlightKind::Read,
    );
    assert_highlight(
        &field_highlights,
        5,
        line(main_text, 5)
            .find("amount")
            .expect("second field read should exist"),
        DocumentHighlightKind::Read,
    );
    assert_no_highlight(
        &field_highlights,
        1,
        line(types_text, 1)
            .find("amount")
            .expect("field declaration should exist"),
    );

    let method_references = databases.references(
        &main,
        Position::new(
            4,
            line(main_text, 4)
                .find("total")
                .expect("first method call should exist"),
        ),
        true,
    );
    assert_eq!(method_references.len(), 3, "{method_references:?}");
    assert_reference(
        &method_references,
        &types,
        5,
        line(types_text, 5)
            .find("total")
            .expect("method declaration should exist"),
        ReferenceKind::Declaration,
    );

    let method_highlights = databases.document_highlights(
        &main,
        Position::new(
            4,
            line(main_text, 4)
                .find("total")
                .expect("first method call should exist"),
        ),
    );
    assert_eq!(method_highlights.len(), 2, "{method_highlights:?}");
    assert_highlight(
        &method_highlights,
        4,
        line(main_text, 4)
            .find("total")
            .expect("first method call should exist"),
        DocumentHighlightKind::Call,
    );
    assert_highlight(
        &method_highlights,
        5,
        line(main_text, 5)
            .find("total")
            .expect("second method call should exist"),
        DocumentHighlightKind::Call,
    );
    assert_no_highlight(
        &method_highlights,
        5,
        line(types_text, 5)
            .find("total")
            .expect("method declaration should exist"),
    );
}

#[test]
fn document_highlight_returns_empty_for_dynamic_and_unresolved_targets() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn unresolved() { return missing }
pub fn dynamic(value: Any) { return value.level }";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let unresolved = databases.document_highlights(
        &document,
        Position::new(
            0,
            line(text, 0)
                .find("missing")
                .expect("unresolved name should exist"),
        ),
    );
    assert!(
        unresolved.is_empty(),
        "unresolved names must not produce speculative highlights"
    );

    let dynamic = databases.document_highlights(
        &document,
        Position::new(
            1,
            line(text, 1)
                .find("level")
                .expect("dynamic member should exist"),
        ),
    );
    assert!(
        dynamic.is_empty(),
        "dynamic receiver members must not invent highlight targets"
    );
}

fn assert_reference(
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

fn assert_no_highlight(highlights: &[DocumentHighlight], line: usize, character: usize) {
    assert!(
        highlights.iter().all(|highlight| {
            highlight.range().start().line != line
                || highlight.range().start().character != character
        }),
        "{highlights:?}"
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
