use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn definition_follows_tuple_destructuring_binding() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(pairs: Array<(String, i64)>) {
    let (name, score) = (\"Ada\", 1)
    name
    for (item_name, item_score) in pairs {
        item_score
    }
    match (\"Grace\", 2) {
        (match_name, match_score) => {
            match_name
        },
    }
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let let_definition = databases
        .definition(&document, position_for_nth(text, "name", 2))
        .expect("tuple let binding should resolve");
    assert_eq!(
        let_definition.symbol(),
        Some(&local_symbol_at(&document, text, "name", 1))
    );

    let for_definition = databases
        .definition(&document, position_for_nth(text, "item_score", 2))
        .expect("tuple for binding should resolve");
    assert_eq!(
        for_definition.symbol(),
        Some(&local_symbol_at(&document, text, "item_score", 1))
    );

    let match_definition = databases
        .definition(&document, position_for_nth(text, "match_name", 2))
        .expect("tuple match binding should resolve");
    assert_eq!(
        match_definition.symbol(),
        Some(&local_symbol_at(&document, text, "match_name", 1))
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
