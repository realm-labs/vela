use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn rename_rejects_source_any_return_receiver_member() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"fn source_any() -> Any { return 1 }
pub fn main() -> i64 {
    return source_any().level
}"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let member = line(text, 2);
    let position = Position::new(
        2,
        member
            .find("level")
            .expect("source Any receiver member should exist"),
    );

    assert_eq!(databases.prepare_rename(&document, position), None);
    assert_eq!(databases.rename(&document, position, "rank"), None);
}

#[test]
fn source_trait_default_method_rename_updates_source_function_return_receiver_calls() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"trait Rewardable {
    fn preview(self, amount: i64) -> i64 { return amount }
}
struct Player {
    level: i64,
}
impl Rewardable for Player {}
fn current_player() -> Player { return Player { level: 1 } }
pub fn main() -> i64 {
    let first = current_player().preview(1)
    return current_player().preview(first)
}"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let first_call = line(text, 9);
    let position = Position::new(
        9,
        first_call
            .find("preview")
            .expect("trait default method call should exist"),
    );

    let prepare = databases
        .prepare_rename(&document, position)
        .expect("prepareRename should resolve source trait default method");
    assert_eq!(prepare.placeholder(), "preview");
    assert_eq!(
        prepare.symbol(),
        &SymbolRef::Source("game::main::Rewardable.preview".into())
    );

    let edit = databases
        .rename(&document, position, "inspect")
        .expect("rename should update source trait default method call sites");
    let document_edit = document_edit(&edit, &document);
    assert_eq!(document_edit.edits().len(), 3, "{document_edit:?}");
    assert_edit_at(
        document_edit.edits(),
        1,
        line(text, 1)
            .find("preview")
            .expect("trait method declaration should exist"),
        "inspect",
    );
    assert_edit_at(
        document_edit.edits(),
        9,
        line(text, 9)
            .find("preview")
            .expect("first trait default method call should exist"),
        "inspect",
    );
    assert_edit_at(
        document_edit.edits(),
        10,
        line(text, 10)
            .find("preview")
            .expect("second trait default method call should exist"),
        "inspect",
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
