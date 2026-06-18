use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn references_find_record_constructor_field_labels() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Reward {
    amount: i64
}

pub fn make(amount: i64) -> Reward {
    return Reward { amount: amount }
}

pub fn main(reward: Reward) -> i64 {
    return reward.amount
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let references = databases.references(
        &document,
        Position::new(
            5,
            line(text, 5)
                .find("amount")
                .expect("constructor field label"),
        ),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        &references,
        1,
        line(text, 1).find("amount").expect("field declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &references,
        5,
        line(text, 5)
            .find("amount")
            .expect("constructor field label"),
        ReferenceKind::Read,
    );
    assert_reference(
        &references,
        9,
        line(text, 9).find("amount").expect("member field read"),
        ReferenceKind::Read,
    );
}

#[test]
fn references_find_record_constructor_shorthand_field_labels() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Reward {
    amount: i64
}

pub fn make(amount: i64) -> Reward {
    return Reward { amount }
}

pub fn main(reward: Reward) -> i64 {
    return reward.amount
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let references = databases.references(
        &document,
        Position::new(1, line(text, 1).find("amount").expect("field declaration")),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        &references,
        1,
        line(text, 1).find("amount").expect("field declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &references,
        5,
        line(text, 5)
            .find("amount")
            .expect("constructor shorthand field label"),
        ReferenceKind::Read,
    );
    assert_reference(
        &references,
        9,
        line(text, 9).find("amount").expect("member field read"),
        ReferenceKind::Read,
    );

    let local_references = databases.references(
        &document,
        Position::new(
            5,
            line(text, 5)
                .find("amount")
                .expect("constructor shorthand local read"),
        ),
        true,
    );

    assert_eq!(local_references.len(), 2, "{local_references:?}");
    assert_reference(
        &local_references,
        4,
        line(text, 4).find("amount").expect("parameter declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &local_references,
        5,
        line(text, 5)
            .find("amount")
            .expect("constructor shorthand local read"),
        ReferenceKind::Read,
    );
}

#[test]
fn references_find_cross_file_imported_source_field_and_method_uses() {
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
    assert_reference_in_document(
        &field_references,
        &types,
        1,
        line(types_text, 1)
            .find("amount")
            .expect("field declaration should exist"),
        ReferenceKind::Declaration,
    );
    assert_reference_in_document(
        &field_references,
        &main,
        3,
        line(main_text, 3)
            .find("amount")
            .expect("first field read should exist"),
        ReferenceKind::Read,
    );
    assert_reference_in_document(
        &field_references,
        &main,
        5,
        line(main_text, 5)
            .find("amount")
            .expect("second field read should exist"),
        ReferenceKind::Read,
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
    assert_reference_in_document(
        &method_references,
        &types,
        5,
        line(types_text, 5)
            .find("total")
            .expect("method declaration should exist"),
        ReferenceKind::Declaration,
    );
    assert_reference_in_document(
        &method_references,
        &main,
        4,
        line(main_text, 4)
            .find("total")
            .expect("first method call should exist"),
        ReferenceKind::Call,
    );
    assert_reference_in_document(
        &method_references,
        &main,
        5,
        line(main_text, 5)
            .find("total")
            .expect("second method call should exist"),
        ReferenceKind::Call,
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
