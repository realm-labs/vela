use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn references_find_local_binding_uses() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next + amount
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let references = databases.references(
        &document,
        Position::new(2, line(text, 2).find("amount").expect("amount use")),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        &references,
        0,
        line(text, 0).find("amount").expect("parameter declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &references,
        1,
        line(text, 1).find("amount").expect("first read"),
        ReferenceKind::Read,
    );
    assert_reference(
        &references,
        2,
        line(text, 2).find("amount").expect("second read"),
        ReferenceKind::Read,
    );
}

#[test]
fn references_can_exclude_local_declaration() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main(amount: i64) -> i64 { return amount }";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let references = databases.references(
        &document,
        Position::new(0, text.find("amount").expect("parameter declaration")),
        false,
    );

    assert_eq!(references.len(), 1);
    assert_reference(
        &references,
        0,
        text.rfind("amount").expect("parameter read"),
        ReferenceKind::Read,
    );
}

#[test]
fn references_find_imported_function_uses() {
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

    assert_eq!(references.len(), 4);
    assert_reference_in_document(
        &references,
        &helper,
        0,
        helper_text.find("grant").expect("function declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference_in_document(
        &references,
        &main,
        0,
        line(main_text, 0).find("grant").expect("import"),
        ReferenceKind::Import,
    );
    assert_reference_in_document(
        &references,
        &main,
        2,
        line(main_text, 2).find("grant").expect("first call"),
        ReferenceKind::Call,
    );
    assert_reference_in_document(
        &references,
        &main,
        3,
        line(main_text, 3).find("grant").expect("second call"),
        ReferenceKind::Call,
    );
}

#[test]
fn references_find_field_reads_and_writes() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Reward {
    amount: i64
}

pub fn main(reward: Reward) -> i64 {
    let first = reward.amount
    reward.amount += 1
    return reward.amount + first
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let references = databases.references(
        &document,
        Position::new(5, line(text, 5).find("amount").expect("first field read")),
        true,
    );

    assert_eq!(references.len(), 4);
    assert_reference(
        &references,
        1,
        line(text, 1).find("amount").expect("field declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &references,
        5,
        line(text, 5).find("amount").expect("first field read"),
        ReferenceKind::Read,
    );
    assert_reference(
        &references,
        6,
        line(text, 6).find("amount").expect("field write"),
        ReferenceKind::Write,
    );
    assert_reference(
        &references,
        7,
        line(text, 7).find("amount").expect("second field read"),
        ReferenceKind::Read,
    );
}

#[test]
fn references_find_enum_variant_constructors_and_patterns() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub enum QuestState {
    Active { count: i64 },
    Done
}

pub fn active(count: i64) -> QuestState {
    return QuestState::Active { count: count }
}

pub fn main(state: QuestState) -> i64 {
    match state {
        QuestState::Active { count } => { return count }
        QuestState::Done => { return 0 }
    }
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let references = databases.references(
        &document,
        Position::new(
            6,
            line(text, 6)
                .find("Active")
                .expect("Active constructor use"),
        ),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        &references,
        1,
        line(text, 1).find("Active").expect("Active declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &references,
        6,
        line(text, 6)
            .find("Active")
            .expect("Active constructor use"),
        ReferenceKind::Read,
    );
    assert_reference(
        &references,
        11,
        line(text, 11).find("Active").expect("Active pattern use"),
        ReferenceKind::Pattern,
    );
}

#[test]
fn references_find_script_method_calls() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Reward {
    amount: i64
}

impl Reward {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

pub fn main(reward: Reward) -> i64 {
    let first = reward.grant(1)
    return reward.grant(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let references = databases.references(
        &document,
        Position::new(9, line(text, 9).find("grant").expect("first method call")),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        &references,
        5,
        line(text, 5).find("grant").expect("method declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &references,
        9,
        line(text, 9).find("grant").expect("first method call"),
        ReferenceKind::Call,
    );
    assert_reference(
        &references,
        10,
        line(text, 10).find("grant").expect("second method call"),
        ReferenceKind::Call,
    );
}

#[test]
fn references_find_trait_impl_uses() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub trait Rewardable {
    fn grant(self, amount: i64) -> i64;
}

pub struct Player {
    level: i64
}

pub struct Chest {
    amount: i64
}

impl Rewardable for Player {
    fn grant(self, amount: i64) -> i64 { return amount }
}

impl Rewardable for Chest {
    fn grant(self, amount: i64) -> i64 { return amount }
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let references = databases.references(
        &document,
        Position::new(
            12,
            line(text, 12).find("Rewardable").expect("first impl use"),
        ),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        &references,
        0,
        line(text, 0).find("Rewardable").expect("trait declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &references,
        12,
        line(text, 12).find("Rewardable").expect("first impl use"),
        ReferenceKind::Read,
    );
    assert_reference(
        &references,
        16,
        line(text, 16).find("Rewardable").expect("second impl use"),
        ReferenceKind::Read,
    );
}

#[test]
fn document_highlight_marks_local_declaration_and_reads() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next + amount
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let highlights = databases.document_highlights(
        &document,
        Position::new(2, line(text, 2).find("amount").expect("amount use")),
    );

    assert_eq!(highlights.len(), 3);
    assert_highlight(
        &highlights,
        0,
        line(text, 0).find("amount").expect("parameter declaration"),
        DocumentHighlightKind::Text,
    );
    assert_highlight(
        &highlights,
        1,
        line(text, 1).find("amount").expect("first read"),
        DocumentHighlightKind::Read,
    );
    assert_highlight(
        &highlights,
        2,
        line(text, 2).find("amount").expect("second read"),
        DocumentHighlightKind::Read,
    );
}

#[test]
fn document_highlight_marks_import_and_calls_in_active_document() {
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
        SourceFileSnapshot::new(helper, helper_text),
    ]);

    let highlights = databases.document_highlights(
        &main,
        Position::new(2, line(main_text, 2).find("grant").expect("grant call")),
    );

    assert_eq!(highlights.len(), 3);
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
}

#[test]
fn document_highlight_marks_read_write_call() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn grant(amount: i64) -> i64 { return amount }
pub fn main(amount: i64) -> i64 {
    let score = amount
    score += grant(amount)
    return score + grant(score)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let score_highlights = databases.document_highlights(
        &document,
        Position::new(3, line(text, 3).find("score").expect("score write")),
    );

    assert_eq!(score_highlights.len(), 4);
    assert_highlight(
        &score_highlights,
        2,
        line(text, 2).find("score").expect("score declaration"),
        DocumentHighlightKind::Text,
    );
    assert_highlight(
        &score_highlights,
        3,
        line(text, 3).find("score").expect("score write"),
        DocumentHighlightKind::Write,
    );
    assert_highlight(
        &score_highlights,
        4,
        line(text, 4).find("score").expect("score read"),
        DocumentHighlightKind::Read,
    );
    assert_highlight(
        &score_highlights,
        4,
        line(text, 4).rfind("score").expect("score argument read"),
        DocumentHighlightKind::Read,
    );

    let grant_highlights = databases.document_highlights(
        &document,
        Position::new(3, line(text, 3).find("grant").expect("grant call")),
    );

    assert_eq!(grant_highlights.len(), 3);
    assert_highlight(
        &grant_highlights,
        0,
        line(text, 0).find("grant").expect("grant declaration"),
        DocumentHighlightKind::Text,
    );
    assert_highlight(
        &grant_highlights,
        3,
        line(text, 3).find("grant").expect("first grant call"),
        DocumentHighlightKind::Call,
    );
    assert_highlight(
        &grant_highlights,
        4,
        line(text, 4).find("grant").expect("second grant call"),
        DocumentHighlightKind::Call,
    );
}

#[test]
fn document_highlight_marks_script_method_calls() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Reward {
    amount: i64
}

impl Reward {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

pub fn main(reward: Reward) -> i64 {
    let first = reward.grant(1)
    return reward.grant(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let highlights = databases.document_highlights(
        &document,
        Position::new(9, line(text, 9).find("grant").expect("first method call")),
    );

    assert_eq!(highlights.len(), 3, "{highlights:?}");
    assert_highlight(
        &highlights,
        5,
        line(text, 5).find("grant").expect("method declaration"),
        DocumentHighlightKind::Text,
    );
    assert_highlight(
        &highlights,
        9,
        line(text, 9).find("grant").expect("first method call"),
        DocumentHighlightKind::Call,
    );
    assert_highlight(
        &highlights,
        10,
        line(text, 10).find("grant").expect("second method call"),
        DocumentHighlightKind::Call,
    );
}

#[test]
fn document_highlight_marks_trait_impl_uses() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub trait Rewardable {
    fn grant(self, amount: i64) -> i64;
}

pub struct Player { level: i64 }

impl Rewardable for Player {
    fn grant(self, amount: i64) -> i64 { return amount }
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let highlights = databases.document_highlights(
        &document,
        Position::new(6, line(text, 6).find("Rewardable").expect("impl use")),
    );

    assert_eq!(highlights.len(), 2, "{highlights:?}");
    assert_highlight(
        &highlights,
        0,
        line(text, 0).find("Rewardable").expect("trait declaration"),
        DocumentHighlightKind::Text,
    );
    assert_highlight(
        &highlights,
        6,
        line(text, 6).find("Rewardable").expect("impl use"),
        DocumentHighlightKind::Read,
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

fn assert_reference(references: &[Reference], line: usize, character: usize, kind: ReferenceKind) {
    assert!(
        references.iter().any(|reference| {
            reference.range().start().line == line
                && reference.range().start().character == character
                && reference.kind() == kind
        }),
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
