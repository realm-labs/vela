use super::*;
use crate::{
    LineIndex, SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};

#[test]
fn expression_completion_suggests_tuple_destructuring_bindings() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(pairs: Array<(String, i64)>) {
    let (name, score) = (\"Ada\", 1)
    na
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let completions = databases.completion_items(&document, position_after_nth(text, "na", 2));

    assert_eq!(
        completions.context().kind(),
        CompletionContextKind::Expression
    );
    let name = completion(&completions, "name").expect("name completion");
    assert_eq!(name.kind(), CompletionKind::Binding);
    assert_eq!(name.detail(), "String");
}

fn completion<'a>(list: &'a CompletionList, label: &str) -> Option<&'a CompletionItem> {
    list.items().iter().find(|item| item.label() == label)
}

fn position_after_nth(text: &str, needle: &str, occurrence: usize) -> Position {
    let offset = text
        .match_indices(needle)
        .nth(occurrence - 1)
        .map(|(offset, _)| offset)
        .unwrap_or_else(|| panic!("{needle} occurrence {occurrence} should exist"))
        + needle.len();
    LineIndex::new(text).position(offset)
}

fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases
}
