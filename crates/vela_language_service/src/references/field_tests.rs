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
