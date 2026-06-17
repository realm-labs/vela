use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn references_find_imported_module_segments() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let other = DocumentId::from("/workspace/scripts/game/other.vela");
    let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
    let main_text = "\
use game::reward::grant
pub fn main() -> i64 { return grant() }";
    let other_text = "\
use game::reward::bonus
pub fn other() -> i64 { return bonus() }";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(other.clone(), other_text),
        SourceFileSnapshot::new(
            helper,
            "pub fn grant() -> i64 { return 1 }\npub fn bonus() -> i64 { return 2 }",
        ),
    ]);

    let references = databases.references(
        &main,
        Position::new(
            0,
            line(main_text, 0).find("reward").expect("module segment"),
        ),
        true,
    );

    assert_eq!(references.len(), 2, "{references:?}");
    assert_reference_in_document(
        &references,
        &main,
        0,
        line(main_text, 0)
            .find("reward")
            .expect("first module segment"),
        ReferenceKind::Import,
    );
    assert_reference_in_document(
        &references,
        &other,
        0,
        line(other_text, 0)
            .find("reward")
            .expect("second module segment"),
        ReferenceKind::Import,
    );
}

#[test]
fn document_highlight_marks_imported_module_segments() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
    let main_text = "\
use game::reward::grant
use game::reward::bonus
pub fn main() -> i64 {
    return grant() + bonus()
}";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(
            helper,
            "pub fn grant() -> i64 { return 1 }\npub fn bonus() -> i64 { return 2 }",
        ),
    ]);

    let highlights = databases.document_highlights(
        &main,
        Position::new(
            0,
            line(main_text, 0).find("reward").expect("module segment"),
        ),
    );

    assert_eq!(highlights.len(), 2, "{highlights:?}");
    assert_highlight(
        &highlights,
        0,
        line(main_text, 0)
            .find("reward")
            .expect("first module segment"),
        DocumentHighlightKind::Text,
    );
    assert_highlight(
        &highlights,
        1,
        line(main_text, 1)
            .find("reward")
            .expect("second module segment"),
        DocumentHighlightKind::Text,
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

fn assert_highlight(
    highlights: &[DocumentHighlight],
    line: usize,
    character: usize,
    kind: DocumentHighlightKind,
) {
    assert!(
        highlights.iter().any(|highlight| {
            highlight.range().start().line == line
                && highlight.range().start().character == character
                && highlight.kind() == kind
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
