use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn references_find_enum_record_variant_field_labels_and_patterns() {
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
        QuestState::Active { count: current } => { return current }
        QuestState::Done => { return 0 }
    }
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let references = databases.references(
        &document,
        Position::new(1, line(text, 1).find("count").expect("field declaration")),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        &references,
        1,
        line(text, 1).find("count").expect("field declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &references,
        6,
        line(text, 6)
            .find("count")
            .expect("constructor field label"),
        ReferenceKind::Read,
    );
    assert_reference(
        &references,
        11,
        line(text, 11).find("count").expect("pattern field label"),
        ReferenceKind::Pattern,
    );
}

#[test]
fn references_find_cross_file_imported_source_enum_variant_uses() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let types = DocumentId::from("/workspace/scripts/game/types.vela");
    let main_text = "\
use game::types::QuestState

pub fn active(count: i64) -> QuestState {
    return QuestState::Active { count: count }
}

pub fn main(state: QuestState) -> i64 {
    match state {
        QuestState::Active { count } => { return count }
        QuestState::Done => { return 0 }
    }
}";
    let types_text = "\
pub enum QuestState {
    Active { count: i64 },
    Done
}";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(types.clone(), types_text),
        SourceFileSnapshot::new(main.clone(), main_text),
    ]);

    let references = databases.references(
        &main,
        Position::new(
            3,
            line(main_text, 3)
                .find("Active")
                .expect("constructor variant should exist"),
        ),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference_in_document(
        &references,
        &types,
        1,
        line(types_text, 1)
            .find("Active")
            .expect("variant declaration should exist"),
        ReferenceKind::Declaration,
    );
    assert_reference_in_document(
        &references,
        &main,
        3,
        line(main_text, 3)
            .find("Active")
            .expect("constructor variant should exist"),
        ReferenceKind::Read,
    );
    assert_reference_in_document(
        &references,
        &main,
        8,
        line(main_text, 8)
            .find("Active")
            .expect("pattern variant should exist"),
        ReferenceKind::Pattern,
    );
}

#[test]
fn references_find_cross_file_imported_source_enum_record_variant_field_uses() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let types = DocumentId::from("/workspace/scripts/game/types.vela");
    let main_text = "\
use game::types::QuestState

pub fn active(count: i64) -> QuestState {
    return QuestState::Active { count: count }
}

pub fn main(state: QuestState) -> i64 {
    match state {
        QuestState::Active { count: current } => { return current }
        QuestState::Done => { return 0 }
    }
}";
    let types_text = "\
pub enum QuestState {
    Active { count: i64 },
    Done
}";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(types.clone(), types_text),
        SourceFileSnapshot::new(main.clone(), main_text),
    ]);

    let references = databases.references(
        &main,
        Position::new(
            3,
            line(main_text, 3)
                .find("count")
                .expect("constructor field should exist"),
        ),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference_in_document(
        &references,
        &types,
        1,
        line(types_text, 1)
            .find("count")
            .expect("variant field declaration should exist"),
        ReferenceKind::Declaration,
    );
    assert_reference_in_document(
        &references,
        &main,
        3,
        line(main_text, 3)
            .find("count")
            .expect("constructor field should exist"),
        ReferenceKind::Read,
    );
    assert_reference_in_document(
        &references,
        &main,
        8,
        line(main_text, 8)
            .find("count")
            .expect("pattern field should exist"),
        ReferenceKind::Pattern,
    );
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
