use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn semantic_tokens_classify_source_method_on_source_function_return() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
struct Player { level: i64 }
impl Player {
    fn grant(self, amount: i64) -> i64 { return amount }
}
fn current_player() -> Player { return Player { level: 1 } }
pub fn main() -> i64 {
    return current_player().grant(1)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let tokens = databases.semantic_tokens(&document);

    assert_token_at(
        &tokens,
        6,
        line(text, 6)
            .find("current_player")
            .expect("source function call should exist"),
        "current_player".len(),
        SemanticTokenType::Function,
        SemanticTokenModifiers::SOURCE,
    );
    assert_token_at(
        &tokens,
        6,
        line(text, 6)
            .find("grant")
            .expect("source method call should exist"),
        "grant".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::SOURCE,
    );
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
