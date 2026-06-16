use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn prepare_rename_rejects_keywords_and_literals() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(amount: i64) -> i64 {
    return amount + 1
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    assert_eq!(
        databases.prepare_rename(
            &document,
            Position::new(1, line(text, 1).find("return").expect("return keyword"))
        ),
        None
    );
    assert_eq!(
        databases.prepare_rename(
            &document,
            Position::new(1, line(text, 1).find('1').expect("literal"))
        ),
        None
    );
}

#[test]
fn local_rename_updates_all_function_uses() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    next += amount
    return next
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let prepare = databases
        .prepare_rename(
            &document,
            Position::new(2, line(text, 2).find("next").expect("next write")),
        )
        .expect("local binding should be renameable");

    assert_eq!(prepare.document_id(), &document);
    assert_eq!(prepare.placeholder(), "next");
    assert_eq!(prepare.range().start(), Position::new(2, 4));

    let edit = databases
        .rename(
            &document,
            Position::new(2, line(text, 2).find("next").expect("next write")),
            "score",
        )
        .expect("local rename should produce edits");

    let document_edit = edit
        .document_edits()
        .first()
        .expect("rename should edit one document");
    assert_eq!(document_edit.document_id(), &document);
    assert_eq!(document_edit.edits().len(), 3);
    assert_edit_at(document_edit.edits(), 1, 8, "score");
    assert_edit_at(document_edit.edits(), 2, 4, "score");
    assert_edit_at(document_edit.edits(), 3, 11, "score");
}

#[test]
fn rename_rejects_scope_collision() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    assert_eq!(
        databases.rename(
            &document,
            Position::new(1, line(text, 1).find("next").expect("next local")),
            "amount",
        ),
        None
    );
}

#[test]
fn private_value_declaration_rename_updates_uses() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
const BONUS: i64 = 5
pub fn main() -> i64 {
    return BONUS + BONUS
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let prepare = databases
        .prepare_rename(
            &document,
            Position::new(2, line(text, 2).find("BONUS").expect("BONUS read")),
        )
        .expect("private const should be renameable from a use site");

    assert_eq!(prepare.placeholder(), "BONUS");
    assert_eq!(prepare.range().start(), Position::new(2, 11));

    let edit = databases
        .rename(
            &document,
            Position::new(2, line(text, 2).find("BONUS").expect("BONUS read")),
            "BASE",
        )
        .expect("private const rename should produce edits");

    let document_edit = document_edit(&edit, &document);
    assert_eq!(document_edit.edits().len(), 3);
    assert_edit_at(document_edit.edits(), 0, 6, "BASE");
    assert_edit_at(document_edit.edits(), 2, 11, "BASE");
    assert_edit_at(document_edit.edits(), 2, 19, "BASE");
    assert!(edit.risks().is_empty());
}

#[test]
fn private_type_declaration_rename_updates_type_hints() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
struct Reward {
    amount: i64
}

fn grant(reward: Reward) -> Reward {
    let next: Reward = reward
    return next
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let prepare = databases
        .prepare_rename(
            &document,
            Position::new(4, line(text, 4).rfind("Reward").expect("return type")),
        )
        .expect("private type should be renameable from a type hint");

    assert_eq!(prepare.placeholder(), "Reward");
    assert_eq!(prepare.range().start(), Position::new(4, 28));

    let edit = databases
        .rename(
            &document,
            Position::new(4, line(text, 4).rfind("Reward").expect("return type")),
            "Prize",
        )
        .expect("private type rename should produce edits");

    let document_edit = document_edit(&edit, &document);
    assert_eq!(document_edit.edits().len(), 4);
    assert_edit_at(document_edit.edits(), 0, 7, "Prize");
    assert_edit_at(document_edit.edits(), 4, 17, "Prize");
    assert_edit_at(document_edit.edits(), 4, 28, "Prize");
    assert_edit_at(document_edit.edits(), 5, 14, "Prize");
    assert!(edit.risks().is_empty());
}

#[test]
fn private_function_rename_updates_imports() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
    let main_text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    return grant(amount)
}";
    let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(helper.clone(), helper_text),
    ]);

    let prepare = databases
        .prepare_rename(
            &main,
            Position::new(2, line(main_text, 2).find("grant").expect("grant call")),
        )
        .expect("script function should be renameable from call site");

    assert_eq!(prepare.placeholder(), "grant");
    assert_eq!(prepare.range().start(), Position::new(2, 11));

    let edit = databases
        .rename(
            &main,
            Position::new(2, line(main_text, 2).find("grant").expect("grant call")),
            "award",
        )
        .expect("script function rename should produce workspace edits");

    let main_edit = document_edit(&edit, &main);
    assert_eq!(main_edit.edits().len(), 2);
    assert_edit_at(main_edit.edits(), 0, 18, "award");
    assert_edit_at(main_edit.edits(), 2, 11, "award");

    let helper_edit = document_edit(&edit, &helper);
    assert_eq!(helper_edit.edits().len(), 1);
    assert_edit_at(helper_edit.edits(), 0, 7, "award");
}

#[test]
fn public_export_rename_reports_hot_reload_risk() {
    let document = DocumentId::from("/workspace/scripts/game/reward.vela");
    let text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let edit = databases
        .rename(
            &document,
            Position::new(0, line(text, 0).find("grant").expect("grant declaration")),
            "award",
        )
        .expect("public function rename should still produce edits");

    assert_eq!(edit.risks().len(), 1);
    assert_eq!(edit.risks()[0].kind(), RenameRiskKind::HotReloadAbi);
    assert!(
        edit.risks()[0]
            .message()
            .contains("public function `grant`")
    );
    let document_edit = document_edit(&edit, &document);
    assert_eq!(document_edit.edits().len(), 1);
    assert_edit_at(document_edit.edits(), 0, 7, "award");
}

#[test]
fn rename_rejects_module_declaration_collision() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn grant(amount: i64) -> i64 { return amount }
pub fn award(amount: i64) -> i64 { return amount + 1 }";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    assert_eq!(
        databases.rename(
            &document,
            Position::new(0, line(text, 0).find("grant").expect("grant declaration")),
            "award",
        ),
        None
    );
}

fn assert_edit_at(edits: &[TextEdit], line: usize, character: usize, new_text: &str) {
    assert!(
        edits.iter().any(|edit| {
            edit.range().start() == Position::new(line, character) && edit.new_text() == new_text
        }),
        "{edits:?}"
    );
}

fn document_edit<'a>(edit: &'a WorkspaceEdit, document_id: &DocumentId) -> &'a DocumentTextEdit {
    edit.document_edits()
        .iter()
        .find(|document_edit| document_edit.document_id() == document_id)
        .unwrap_or_else(|| panic!("workspace edit should contain {document_id:?}"))
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
