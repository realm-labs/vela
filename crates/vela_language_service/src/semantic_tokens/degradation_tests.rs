use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn semantic_tokens_degrade_schema_type_hints_when_schema_is_missing() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(player: Player, names: Array<String>) -> i64 {
    let level = 1
    return level
}";
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(
        &config,
        &[SourceFileSnapshot::new(document.clone(), text)],
        &Workspace::new().snapshot(),
    );
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases.mark_schema_missing("/workspace/target/vela/schema.json");

    let tokens = databases.semantic_tokens(&document);

    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("Player").expect("schema type hint"),
        "Player".len(),
        SemanticTokenType::Type,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("Array").expect("builtin array hint"),
        "Array".len(),
        SemanticTokenType::BuiltinType,
        SemanticTokenModifiers::BUILTIN,
    );
    assert!(
        tokens.tokens().iter().all(|token| {
            token.start().line != 0
                || token.start().character != line(text, 0).find("Player").expect("Player")
                || token.modifiers()
                    != SemanticTokenModifiers::HOST.union(SemanticTokenModifiers::SCHEMA)
        }),
        "missing schema should not produce schema-backed token modifiers: {:?}",
        tokens.tokens()
    );
    assert!(
        databases
            .schema_db()
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message().contains("host schema")),
        "missing schema diagnostic should be preserved"
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
