use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn semantic_tokens_classify_schema_method_on_schema_function_return() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main() -> i64 {
    return current_player().grant(1)
}";
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    schema.insert_function(
        "current_player",
        TypeFact::function(Vec::new(), TypeFact::host("Player")),
    );
    schema.insert_method(
        "Player",
        "grant",
        TypeFact::function(vec![TypeFact::I64], TypeFact::I64),
    );
    let databases = databases_for_with_schema(
        vec![SourceFileSnapshot::new(document.clone(), text)],
        schema,
    );

    let tokens = databases.semantic_tokens(&document);
    let schema_host = SemanticTokenModifiers::HOST.union(SemanticTokenModifiers::SCHEMA);

    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("current_player")
            .expect("schema function call should exist"),
        "current_player".len(),
        SemanticTokenType::Function,
        schema_host,
    );
    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("grant")
            .expect("schema method call should exist"),
        "grant".len(),
        SemanticTokenType::Method,
        schema_host,
    );
}

#[test]
fn semantic_tokens_classify_schema_method_on_schema_method_return() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(player: Player) -> i64 {
    return player.inventory().grant(1)
}";
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    schema.insert_type("Inventory", TypeFact::host("Inventory"));
    schema.insert_method(
        "Player",
        "inventory",
        TypeFact::function(Vec::new(), TypeFact::host("Inventory")),
    );
    schema.insert_method(
        "Inventory",
        "grant",
        TypeFact::function(vec![TypeFact::I64], TypeFact::I64),
    );
    let databases = databases_for_with_schema(
        vec![SourceFileSnapshot::new(document.clone(), text)],
        schema,
    );

    let tokens = databases.semantic_tokens(&document);
    let schema_host = SemanticTokenModifiers::HOST.union(SemanticTokenModifiers::SCHEMA);

    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("inventory")
            .expect("schema method call should exist"),
        "inventory".len(),
        SemanticTokenType::Method,
        schema_host,
    );
    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("grant")
            .expect("chained schema method call should exist"),
        "grant".len(),
        SemanticTokenType::Method,
        schema_host,
    );
}

#[test]
fn semantic_tokens_classify_schema_trait_method_on_schema_function_return() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main() -> i64 {
    return current_reward().preview(1)
}";
    let mut schema = RegistryFacts::default();
    schema.insert_trait("Rewardable", TypeFact::trait_type("Rewardable"));
    schema.insert_function(
        "current_reward",
        TypeFact::function(Vec::new(), TypeFact::trait_type("Rewardable")),
    );
    schema.insert_trait_method(
        "Rewardable",
        "preview",
        TypeFact::function(vec![TypeFact::I64], TypeFact::I64),
    );
    let databases = databases_for_with_schema(
        vec![SourceFileSnapshot::new(document.clone(), text)],
        schema,
    );

    let tokens = databases.semantic_tokens(&document);
    let schema_host = SemanticTokenModifiers::HOST.union(SemanticTokenModifiers::SCHEMA);

    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("current_reward")
            .expect("schema function call should exist"),
        "current_reward".len(),
        SemanticTokenType::Function,
        schema_host,
    );
    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("preview")
            .expect("schema trait method call should exist"),
        "preview".len(),
        SemanticTokenType::Method,
        schema_host,
    );
}

#[test]
fn semantic_tokens_classify_schema_trait_method_on_schema_method_return() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(player: Player) -> i64 {
    return player.rewardable().preview(1)
}";
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    schema.insert_trait("Rewardable", TypeFact::trait_type("Rewardable"));
    schema.insert_method(
        "Player",
        "rewardable",
        TypeFact::function(Vec::new(), TypeFact::trait_type("Rewardable")),
    );
    schema.insert_trait_method(
        "Rewardable",
        "preview",
        TypeFact::function(vec![TypeFact::I64], TypeFact::I64),
    );
    let databases = databases_for_with_schema(
        vec![SourceFileSnapshot::new(document.clone(), text)],
        schema,
    );

    let tokens = databases.semantic_tokens(&document);
    let schema_host = SemanticTokenModifiers::HOST.union(SemanticTokenModifiers::SCHEMA);

    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("rewardable")
            .expect("schema method call should exist"),
        "rewardable".len(),
        SemanticTokenType::Method,
        schema_host,
    );
    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("preview")
            .expect("schema trait method call should exist"),
        "preview".len(),
        SemanticTokenType::Method,
        schema_host,
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

fn databases_for_with_schema(
    files: Vec<SourceFileSnapshot>,
    schema: RegistryFacts,
) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases.set_schema_facts(schema);
    databases
}
