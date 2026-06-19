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

#[test]
fn semantic_tokens_classify_imported_source_method_on_source_function_return() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let player = DocumentId::from("/workspace/scripts/game/player.vela");
    let main_text = "\
use game::player::current_player
pub fn main() -> i64 {
    return current_player().grant(1)
}";
    let player_text = "\
pub struct Player { level: i64 }
impl Player {
    fn grant(self, amount: i64) -> i64 { return amount }
}
pub fn current_player() -> Player { return Player { level: 1 } }";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(player, player_text),
    ]);

    let tokens = databases.semantic_tokens(&main);

    assert_token_at(
        &tokens,
        2,
        line(main_text, 2)
            .find("current_player")
            .expect("imported source function call should exist"),
        "current_player".len(),
        SemanticTokenType::Function,
        SemanticTokenModifiers::SOURCE,
    );
    assert_token_at(
        &tokens,
        2,
        line(main_text, 2)
            .find("grant")
            .expect("imported source method call should exist"),
        "grant".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::SOURCE,
    );
}

#[test]
fn semantic_tokens_classify_source_method_on_source_method_return() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
struct Player { level: i64 }
struct Inventory { count: i64 }
impl Player {
    fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}
impl Inventory {
    fn grant(self, amount: i64) -> i64 { return amount }
}
pub fn main(player: Player) -> i64 {
    return player.inventory().grant(1)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let tokens = databases.semantic_tokens(&document);

    assert_token_at(
        &tokens,
        9,
        line(text, 9)
            .find("inventory")
            .expect("source method call should exist"),
        "inventory".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::SOURCE,
    );
    assert_token_at(
        &tokens,
        9,
        line(text, 9)
            .find("grant")
            .expect("chained source method call should exist"),
        "grant".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::SOURCE,
    );
}

#[test]
fn semantic_tokens_classify_source_trait_method_on_source_function_return() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
trait Rewardable {
    fn preview(self, amount: i64) -> i64 { return amount }
}
struct Player { level: i64 }
impl Rewardable for Player {}
fn current_player() -> Player { return Player { level: 1 } }
pub fn main() -> i64 {
    return current_player().preview(1)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let tokens = databases.semantic_tokens(&document);

    assert_token_at(
        &tokens,
        7,
        line(text, 7)
            .find("current_player")
            .expect("source function call should exist"),
        "current_player".len(),
        SemanticTokenType::Function,
        SemanticTokenModifiers::SOURCE,
    );
    assert_token_at(
        &tokens,
        7,
        line(text, 7)
            .find("preview")
            .expect("source trait method call should exist"),
        "preview".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::SOURCE,
    );
}

#[test]
fn semantic_tokens_classify_source_trait_method_on_source_method_return() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
trait Rewardable {
    fn preview(self, amount: i64) -> i64 { return amount }
}
struct Player { level: i64 }
struct Inventory { count: i64 }
impl Player {
    fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}
impl Rewardable for Inventory {}
pub fn main(player: Player) -> i64 {
    return player.inventory().preview(1)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let tokens = databases.semantic_tokens(&document);

    assert_token_at(
        &tokens,
        10,
        line(text, 10)
            .find("inventory")
            .expect("source method call should exist"),
        "inventory".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::SOURCE,
    );
    assert_token_at(
        &tokens,
        10,
        line(text, 10)
            .find("preview")
            .expect("source trait method call should exist"),
        "preview".len(),
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
