use super::*;
use crate::{
    SourceFileSnapshot, SymbolRef, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};

#[test]
fn references_find_imported_const_and_global_uses() {
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
    assert_reference_in_document(
        &const_references,
        &rewards,
        0,
        line(rewards_text, 0)
            .find("BASE_REWARD")
            .expect("const declaration should exist"),
        ReferenceKind::Declaration,
    );
    assert_reference_in_document(
        &const_references,
        &main,
        0,
        line(main_text, 0)
            .find("BASE_REWARD")
            .expect("const import should exist"),
        ReferenceKind::Import,
    );
    assert_reference_in_document(
        &const_references,
        &main,
        3,
        line(main_text, 3)
            .find("BASE_REWARD")
            .expect("const use should exist"),
        ReferenceKind::Read,
    );
    assert_all_symbols(
        &const_references,
        &SymbolRef::Source("game::rewards::BASE_REWARD".into()),
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
    assert_reference_in_document(
        &global_references,
        &rewards,
        1,
        line(rewards_text, 1)
            .find("reward_scale")
            .expect("global declaration should exist"),
        ReferenceKind::Declaration,
    );
    assert_reference_in_document(
        &global_references,
        &main,
        1,
        line(main_text, 1)
            .find("reward_scale")
            .expect("global import should exist"),
        ReferenceKind::Import,
    );
    assert_reference_in_document(
        &global_references,
        &main,
        4,
        line(main_text, 4)
            .find("reward_scale")
            .expect("global use should exist"),
        ReferenceKind::Read,
    );
    assert_all_symbols(
        &global_references,
        &SymbolRef::Source("game::rewards::reward_scale".into()),
    );
}

#[test]
fn references_find_imported_function_alias_uses() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
    let main_text = "\
use game::reward::grant as award
pub fn main(amount: i64) -> i64 {
    let first = award(amount)
    return award(first)
}";
    let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(helper.clone(), helper_text),
    ]);

    let references = databases.references(
        &main,
        Position::new(2, line(main_text, 2).find("award").expect("alias call")),
        true,
    );

    assert_eq!(references.len(), 4, "{references:?}");
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
        line(main_text, 0).find("award").expect("import alias"),
        ReferenceKind::Import,
    );
    assert_reference_in_document(
        &references,
        &main,
        2,
        line(main_text, 2).find("award").expect("first alias call"),
        ReferenceKind::Call,
    );
    assert_reference_in_document(
        &references,
        &main,
        3,
        line(main_text, 3).find("award").expect("second alias call"),
        ReferenceKind::Call,
    );
    assert_all_symbols(
        &references,
        &SymbolRef::Source("game::reward::grant".into()),
    );
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
