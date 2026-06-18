use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn references_find_source_method_calls_on_source_function_return_receivers() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Player {
    level: i64
}

impl Player {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

fn current_player() -> Player { return Player { level: 1 } }

pub fn main() -> i64 {
    let first = current_player().grant(1)
    return current_player().grant(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let references = databases.references(
        &document,
        Position::new(
            11,
            line(text, 11)
                .find("grant")
                .expect("first returned receiver method call"),
        ),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        &references,
        5,
        line(text, 5).find("grant").expect("method declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &references,
        11,
        line(text, 11).find("grant").expect("first method call"),
        ReferenceKind::Call,
    );
    assert_reference(
        &references,
        12,
        line(text, 12).find("grant").expect("second method call"),
        ReferenceKind::Call,
    );
    assert_all_symbols(
        &references,
        &SymbolRef::Source("game::main::Player.grant".into()),
    );
}

#[test]
fn document_highlight_marks_source_method_calls_on_source_function_return_receivers() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Player {
    level: i64
}

impl Player {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

fn current_player() -> Player { return Player { level: 1 } }

pub fn main() -> i64 {
    let first = current_player().grant(1)
    return current_player().grant(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let highlights = databases.document_highlights(
        &document,
        Position::new(
            11,
            line(text, 11)
                .find("grant")
                .expect("first returned receiver method call"),
        ),
    );

    assert_eq!(highlights.len(), 3, "{highlights:?}");
    assert_highlight(
        &highlights,
        5,
        line(text, 5).find("grant").expect("method declaration"),
        DocumentHighlightKind::Text,
    );
    assert_highlight(
        &highlights,
        11,
        line(text, 11).find("grant").expect("first method call"),
        DocumentHighlightKind::Call,
    );
    assert_highlight(
        &highlights,
        12,
        line(text, 12).find("grant").expect("second method call"),
        DocumentHighlightKind::Call,
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

fn assert_reference(references: &[Reference], line: usize, character: usize, kind: ReferenceKind) {
    assert!(
        references.iter().any(|reference| {
            reference.range().start() == Position::new(line, character) && reference.kind() == kind
        }),
        "{references:?}"
    );
}

fn assert_highlight(
    highlights: &[DocumentHighlight],
    line: usize,
    character: usize,
    kind: DocumentHighlightKind,
) {
    assert!(
        highlights.iter().any(|highlight| {
            highlight.range().start() == Position::new(line, character) && highlight.kind() == kind
        }),
        "{highlights:?}"
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
