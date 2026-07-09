use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn local_rename_updates_tuple_destructuring_binding_uses() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main() {
    let (name, score) = (\"Ada\", 1)
    name
    name
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let symbol = local_symbol_at(&document, text, "name", 1);

    let prepare = databases
        .prepare_rename(&document, position_for_nth(text, "name", 2))
        .expect("tuple destructuring binding should be renameable");
    assert_eq!(prepare.placeholder(), "name");
    assert_eq!(prepare.symbol(), &symbol);

    let edit = databases
        .rename(&document, position_for_nth(text, "name", 2), "first")
        .expect("tuple destructuring binding rename should produce edits");
    assert_eq!(edit.symbol(), Some(&symbol));

    let document_edit = edit
        .document_edits()
        .first()
        .expect("rename should edit one document");
    assert_eq!(document_edit.document_id(), &document);
    assert_eq!(document_edit.edits().len(), 3);
    assert_edit_at(document_edit.edits(), text, "name", 1, "first");
    assert_edit_at(document_edit.edits(), text, "name", 2, "first");
    assert_edit_at(document_edit.edits(), text, "name", 3, "first");
}

fn assert_edit_at(edits: &[TextEdit], text: &str, needle: &str, occurrence: usize, new_text: &str) {
    let position = LineIndex::new(text).position(nth_offset(text, needle, occurrence));
    assert!(
        edits
            .iter()
            .any(|edit| edit.range().start() == position && edit.new_text() == new_text),
        "missing edit for {needle} occurrence {occurrence}: {edits:?}"
    );
}

fn position_for_nth(text: &str, needle: &str, occurrence: usize) -> Position {
    LineIndex::new(text).position(nth_offset(text, needle, occurrence))
}

fn local_symbol_at(
    document: &DocumentId,
    text: &str,
    needle: &str,
    occurrence: usize,
) -> SymbolRef {
    let start = nth_offset(text, needle, occurrence);
    SymbolRef::local_at(
        needle.to_owned(),
        document.clone(),
        TextRange::new(start, start + needle.len()),
    )
}

fn nth_offset(text: &str, needle: &str, occurrence: usize) -> usize {
    text.match_indices(needle)
        .nth(occurrence - 1)
        .map(|(offset, _)| offset)
        .unwrap_or_else(|| panic!("{needle} occurrence {occurrence} should exist"))
}

fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases
}
