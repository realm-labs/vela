use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn semantic_tokens_classify_imported_source_enum_variant_uses() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let quest = DocumentId::from("/workspace/scripts/game/quest.vela");
    let main_text = "\
use game::quest::Progress
pub fn main(progress: Progress) -> Progress {
    let started = Progress::Started
    let done = Progress::Done(\"ok\")
    match progress {
        Progress::Started => started
        Progress::Done(value) => done
    }
}";
    let quest_text = "\
pub enum Progress {
    Started
    Done(result: String)
}";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(quest, quest_text),
    ]);

    let tokens = databases.semantic_tokens(&main);

    for (line_index, variant) in [(2, "Started"), (3, "Done"), (5, "Started"), (6, "Done")] {
        assert_token_at(
            &tokens,
            line_index,
            line(main_text, line_index)
                .find(variant)
                .unwrap_or_else(|| panic!("{variant} should exist")),
            variant.len(),
            SemanticTokenType::EnumMember,
            SemanticTokenModifiers::SOURCE,
        );
    }
}

fn assert_token_at(
    tokens: &SemanticTokens,
    line: usize,
    character: usize,
    length: usize,
    token_type: SemanticTokenType,
    modifiers: SemanticTokenModifiers,
) {
    assert!(
        tokens.tokens().iter().any(|token| {
            token.start() == Position::new(line, character)
                && token.length() == length
                && token.token_type() == token_type
                && token.modifiers() == modifiers
        }),
        "missing token at {line}:{character} len {length} as {token_type:?} with {modifiers:?}; tokens: {:?}",
        tokens.tokens()
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
