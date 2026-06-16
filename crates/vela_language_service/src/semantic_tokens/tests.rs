use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn semantic_tokens_cover_lexical_classes() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main() { let bytes = b\"ok\" return bytes + 1 }";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let tokens = databases.semantic_tokens(&document);

    assert_token_at(
        &tokens,
        0,
        text.find("pub").expect("keyword should exist"),
        "pub".len(),
        SemanticTokenType::Keyword,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        0,
        text.find("b\"ok\"").expect("bytes literal should exist"),
        "b\"ok\"".len(),
        SemanticTokenType::Bytes,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        0,
        text.find('+').expect("operator should exist"),
        1,
        SemanticTokenType::Operator,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        0,
        text.find('1').expect("number should exist"),
        1,
        SemanticTokenType::Number,
        SemanticTokenModifiers::NONE,
    );
}

#[test]
fn semantic_tokens_mark_resolved_symbols() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
    let main_text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    let next = grant(amount)
    return next
}";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(helper, "pub fn grant(amount: i64) -> i64 { return amount }"),
    ]);

    let tokens = databases.semantic_tokens(&main);

    assert_token_at(
        &tokens,
        1,
        line(main_text, 1).find("main").expect("main should exist"),
        "main".len(),
        SemanticTokenType::Function,
        SemanticTokenModifiers::DECLARATION.union(SemanticTokenModifiers::DEFINITION),
    );
    assert_token_at(
        &tokens,
        1,
        line(main_text, 1)
            .find("amount")
            .expect("parameter should exist"),
        "amount".len(),
        SemanticTokenType::Parameter,
        SemanticTokenModifiers::DECLARATION,
    );
    assert_token_at(
        &tokens,
        2,
        line(main_text, 2).find("next").expect("local should exist"),
        "next".len(),
        SemanticTokenType::Variable,
        SemanticTokenModifiers::DECLARATION,
    );
    assert_token_at(
        &tokens,
        2,
        line(main_text, 2).find("grant").expect("call should exist"),
        "grant".len(),
        SemanticTokenType::Function,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        2,
        line(main_text, 2)
            .find("amount")
            .expect("argument should exist"),
        "amount".len(),
        SemanticTokenType::Parameter,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        3,
        line(main_text, 3)
            .find("next")
            .expect("return value should exist"),
        "next".len(),
        SemanticTokenType::Variable,
        SemanticTokenModifiers::NONE,
    );
}

#[test]
fn semantic_tokens_include_comments() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
#! /usr/bin/env vela
// setup
pub fn main() {
    let text = \"not // a comment\"
    /* outer
       /* nested */
       done */
    return text
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let tokens = databases.semantic_tokens(&document);

    assert_token_at(
        &tokens,
        0,
        0,
        line(text, 0).len(),
        SemanticTokenType::Comment,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        1,
        0,
        line(text, 1).len(),
        SemanticTokenType::Comment,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        4,
        line(text, 4)
            .find("/* outer")
            .expect("block comment should exist"),
        "/* outer".len(),
        SemanticTokenType::Comment,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        5,
        0,
        line(text, 5).len(),
        SemanticTokenType::Comment,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        6,
        0,
        line(text, 6).len(),
        SemanticTokenType::Comment,
        SemanticTokenModifiers::NONE,
    );
    assert!(
        tokens.tokens().iter().all(|token| {
            token.start().line != 3
                || token.token_type() != SemanticTokenType::Comment
                || token.start().character
                    != line(text, 3)
                        .find("//")
                        .expect("string should contain comment marker")
        }),
        "string contents must not produce comment tokens: {:?}",
        tokens.tokens()
    );
}

#[test]
fn semantic_tokens_degrade_under_parse_errors() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main( {
    let value = 1 +
    // keep tokenization alive
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let tokens = databases.semantic_tokens(&document);

    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("pub").expect("keyword should exist"),
        "pub".len(),
        SemanticTokenType::Keyword,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        1,
        line(text, 1).find("let").expect("keyword should exist"),
        "let".len(),
        SemanticTokenType::Keyword,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        1,
        line(text, 1).find('1').expect("number should exist"),
        1,
        SemanticTokenType::Number,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2).find("// keep").expect("comment should exist"),
        line(text, 2).trim_start().len(),
        SemanticTokenType::Comment,
        SemanticTokenModifiers::NONE,
    );
}

#[test]
fn semantic_tokens_classify_script_members() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Reward {
    amount: i64
}

pub enum Progress {
    Started
    Active { quest_id: String }
    Finished(result: String)
}

pub trait Scored {
    fn score(value: Reward) -> i64
}

impl Reward {
    fn bonus(value: Reward) -> i64 { return 1 }
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let tokens = databases.semantic_tokens(&document);
    let member_modifiers =
        SemanticTokenModifiers::DECLARATION.union(SemanticTokenModifiers::DEFINITION);

    assert_token_at(
        &tokens,
        1,
        line(text, 1).find("amount").expect("field should exist"),
        "amount".len(),
        SemanticTokenType::Field,
        member_modifiers,
    );
    assert_token_at(
        &tokens,
        5,
        line(text, 5).find("Started").expect("variant should exist"),
        "Started".len(),
        SemanticTokenType::EnumMember,
        member_modifiers,
    );
    assert_token_at(
        &tokens,
        6,
        line(text, 6)
            .find("quest_id")
            .expect("record variant field should exist"),
        "quest_id".len(),
        SemanticTokenType::Field,
        member_modifiers,
    );
    assert_token_at(
        &tokens,
        7,
        line(text, 7)
            .find("result")
            .expect("tuple variant field should exist"),
        "result".len(),
        SemanticTokenType::Field,
        member_modifiers,
    );
    assert_token_at(
        &tokens,
        11,
        line(text, 11)
            .find("score")
            .expect("trait method should exist"),
        "score".len(),
        SemanticTokenType::Method,
        member_modifiers,
    );
    assert_token_at(
        &tokens,
        15,
        line(text, 15)
            .find("bonus")
            .expect("impl method should exist"),
        "bonus".len(),
        SemanticTokenType::Method,
        member_modifiers,
    );
}

#[test]
fn semantic_tokens_classify_script_member_uses() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Reward {
    amount: i64
}

impl Reward {
    fn bonus(value: Reward) -> i64 { return value.amount }
}

pub fn main(reward: Reward) -> i64 {
    return reward.amount + reward.bonus()
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let tokens = databases.semantic_tokens(&document);

    assert_token_at(
        &tokens,
        9,
        line(text, 9)
            .find("amount")
            .expect("field use should exist"),
        "amount".len(),
        SemanticTokenType::Property,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        9,
        line(text, 9)
            .find("bonus")
            .expect("method use should exist"),
        "bonus".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::NONE,
    );
}

#[test]
fn semantic_tokens_classify_script_trait_method_uses() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub trait Rewardable {
    fn preview(self, amount: i64) -> i64
}

pub fn main(rewardable: Rewardable) -> i64 {
    return rewardable.preview(1)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let tokens = databases.semantic_tokens(&document);

    assert_token_at(
        &tokens,
        5,
        line(text, 5)
            .find("preview")
            .expect("trait method call should exist"),
        "preview".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::NONE,
    );
}

#[test]
fn semantic_tokens_classify_schema_and_stdlib_member_uses() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(player: Player, names: Array<String>) -> i64 {
    let level = player.level
    player.grant(level)
    return names.len()
}";
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    schema.insert_field("Player", "level", TypeFact::I64);
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

    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .rfind("level")
            .expect("host field use should exist"),
        "level".len(),
        SemanticTokenType::Property,
        SemanticTokenModifiers::HOST,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2)
            .find("grant")
            .expect("host method use should exist"),
        "grant".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::HOST,
    );
    assert_token_at(
        &tokens,
        3,
        line(text, 3)
            .find("len")
            .expect("stdlib method use should exist"),
        "len".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::BUILTIN,
    );
}

#[test]
fn semantic_tokens_classify_schema_trait_method_uses_as_host() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(rewardable: Rewardable) -> i64 {
    return rewardable.preview(1)
}";
    let mut schema = RegistryFacts::default();
    schema.insert_trait("Rewardable", TypeFact::trait_type("Rewardable"));
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

    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("preview")
            .expect("schema trait method call should exist"),
        "preview".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::HOST,
    );
}

#[test]
fn semantic_tokens_classify_schema_and_stdlib_function_calls() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(player: Player) -> i64 {
    let reward = grant_reward(player)
    return math::max(reward, 10)
}";
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    schema.insert_function(
        "grant_reward",
        TypeFact::function(vec![TypeFact::host("Player")], TypeFact::I64),
    );
    let databases = databases_for_with_schema(
        vec![SourceFileSnapshot::new(document.clone(), text)],
        schema,
    );

    let tokens = databases.semantic_tokens(&document);

    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("grant_reward")
            .expect("schema function call should exist"),
        "grant_reward".len(),
        SemanticTokenType::Function,
        SemanticTokenModifiers::HOST,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2)
            .find("max")
            .expect("stdlib function call should exist"),
        "max".len(),
        SemanticTokenType::Function,
        SemanticTokenModifiers::BUILTIN,
    );
}

#[test]
fn semantic_tokens_classify_host_and_builtin_type_hints() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(player: Player, names: Array<String>) -> i64 {
    let next: Player = player
    return 1
}";
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    let databases = databases_for_with_schema(
        vec![SourceFileSnapshot::new(document.clone(), text)],
        schema,
    );

    let tokens = databases.semantic_tokens(&document);

    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("Player").expect("schema type hint"),
        "Player".len(),
        SemanticTokenType::Type,
        SemanticTokenModifiers::HOST,
    );
    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("Array").expect("builtin array hint"),
        "Array".len(),
        SemanticTokenType::Type,
        SemanticTokenModifiers::BUILTIN,
    );
    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("String").expect("builtin string hint"),
        "String".len(),
        SemanticTokenType::Type,
        SemanticTokenModifiers::BUILTIN,
    );
    assert_token_at(
        &tokens,
        0,
        line(text, 0).rfind("i64").expect("builtin return hint"),
        "i64".len(),
        SemanticTokenType::Type,
        SemanticTokenModifiers::BUILTIN,
    );
    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("Player")
            .expect("local schema type hint"),
        "Player".len(),
        SemanticTokenType::Type,
        SemanticTokenModifiers::HOST,
    );
}

#[test]
fn semantic_token_delta_matches_full_tokens() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main() { let value = 1 return value }";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let full = databases.semantic_tokens(&document);
    let unchanged = databases.semantic_token_delta(&document, full.result_id());

    assert_eq!(unchanged.result_id(), full.result_id());
    assert!(unchanged.edits().is_empty());

    let changed_text = "pub fn main() { let value = 20 return value }";
    let changed = databases_for(vec![SourceFileSnapshot::new(
        document.clone(),
        changed_text,
    )]);
    let changed_full = changed.semantic_tokens(&document);
    let delta = changed.semantic_token_delta(&document, full.result_id());

    assert_eq!(delta.result_id(), changed_full.result_id());
    let edit = delta
        .edits()
        .first()
        .expect("changed tokens should produce a replacement edit");
    assert_eq!(edit.start(), 0);
    assert_eq!(edit.delete_count(), full.tokens().len());
    assert_eq!(edit.tokens(), changed_full.tokens());
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
        "{:?}",
        tokens.tokens()
    );
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}

fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    databases_for_with_schema(files, RegistryFacts::default())
}

fn databases_for_with_schema(
    files: Vec<SourceFileSnapshot>,
    schema: RegistryFacts,
) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.set_schema_facts(schema);
    databases.update(&project);
    databases
}
