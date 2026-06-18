use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn semantic_tokens_range_filters_tokens() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main() {
    let first = 1
    let second = first + 2
    return second
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let tokens = databases.semantic_tokens_in_range(
        &document,
        DiagnosticRange::new(Position::new(2, 0), Position::new(3, 0)),
    );

    assert!(
        !tokens.tokens().is_empty(),
        "range should include line 2 tokens"
    );
    assert!(
        tokens.tokens().iter().all(|token| token.start().line == 2),
        "range tokens should stay inside requested line: {:?}",
        tokens.tokens()
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2).find("second").expect("local declaration"),
        "second".len(),
        SemanticTokenType::Variable,
        SemanticTokenModifiers::DECLARATION.union(SemanticTokenModifiers::SOURCE),
    );
}

#[test]
fn semantic_tokens_range_returns_empty_for_empty_prefix_range() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main() {
    let value = 1
    return value
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let tokens = databases.semantic_tokens_in_range(
        &document,
        DiagnosticRange::new(Position::new(1, 0), Position::new(1, 0)),
    );

    assert!(tokens.tokens().is_empty(), "{:?}", tokens.tokens());
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
            token.start().line == line
                && token.start().character == character
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
