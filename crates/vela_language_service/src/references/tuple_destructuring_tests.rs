use super::*;
use crate::{
    LineIndex, SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};

#[test]
fn references_find_tuple_destructuring_binding_uses() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(pairs: Array<(String, i64)>) {
    let (name, score) = (\"Ada\", 1)
    name
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

    let name_references = databases.references(&document, position_for_nth(text, "name", 3), true);
    assert_eq!(name_references.len(), 3, "{name_references:?}");
    assert_reference(
        &name_references,
        text,
        "name",
        1,
        ReferenceKind::Declaration,
    );
    assert_reference(&name_references, text, "name", 2, ReferenceKind::Read);
    assert_reference(&name_references, text, "name", 3, ReferenceKind::Read);
    assert_all_symbols(
        &name_references,
        &local_symbol_at(&document, text, "name", 1),
    );

    let for_references =
        databases.references(&document, position_for_nth(text, "item_score", 2), true);
    assert_eq!(for_references.len(), 2, "{for_references:?}");
    assert_reference(
        &for_references,
        text,
        "item_score",
        1,
        ReferenceKind::Declaration,
    );
    assert_reference(&for_references, text, "item_score", 2, ReferenceKind::Read);

    let match_references =
        databases.references(&document, position_for_nth(text, "match_name", 2), true);
    assert_eq!(match_references.len(), 2, "{match_references:?}");
    assert_reference(
        &match_references,
        text,
        "match_name",
        1,
        ReferenceKind::Declaration,
    );
    assert_reference(
        &match_references,
        text,
        "match_name",
        2,
        ReferenceKind::Read,
    );
}

fn assert_reference(
    references: &[Reference],
    text: &str,
    needle: &str,
    occurrence: usize,
    kind: ReferenceKind,
) {
    let offset = nth_offset(text, needle, occurrence);
    let position = LineIndex::new(text).position(offset);
    assert!(
        references
            .iter()
            .any(|reference| reference.range().start() == position && reference.kind() == kind),
        "missing {kind:?} reference for {needle} occurrence {occurrence}: {references:?}"
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
