use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

const HIGHLIGHTING_SHOWCASE: &str =
    include_str!("../../../../tests/fixtures/lsp_highlighting/showcase.vela");

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
        SemanticTokenType::ArithmeticOperator,
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

    let source_declaration_definition = SemanticTokenModifiers::DECLARATION
        .union(SemanticTokenModifiers::DEFINITION)
        .union(SemanticTokenModifiers::SOURCE);
    let source_declaration =
        SemanticTokenModifiers::DECLARATION.union(SemanticTokenModifiers::SOURCE);

    assert_token_at(
        &tokens,
        1,
        line(main_text, 1).find("main").expect("main should exist"),
        "main".len(),
        SemanticTokenType::Function,
        source_declaration_definition,
    );
    assert_token_at(
        &tokens,
        1,
        line(main_text, 1)
            .find("amount")
            .expect("parameter should exist"),
        "amount".len(),
        SemanticTokenType::Parameter,
        source_declaration,
    );
    assert_token_at(
        &tokens,
        2,
        line(main_text, 2).find("next").expect("local should exist"),
        "next".len(),
        SemanticTokenType::Variable,
        source_declaration,
    );
    assert_token_at(
        &tokens,
        2,
        line(main_text, 2).find("grant").expect("call should exist"),
        "grant".len(),
        SemanticTokenType::Function,
        SemanticTokenModifiers::SOURCE,
    );
    assert_token_at(
        &tokens,
        2,
        line(main_text, 2)
            .find("amount")
            .expect("argument should exist"),
        "amount".len(),
        SemanticTokenType::Parameter,
        SemanticTokenModifiers::SOURCE,
    );
    assert_token_at(
        &tokens,
        3,
        line(main_text, 3)
            .find("next")
            .expect("return value should exist"),
        "next".len(),
        SemanticTokenType::Variable,
        SemanticTokenModifiers::SOURCE,
    );
}

#[test]
fn semantic_tokens_classify_import_module_path_segments() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
    let main_text = "\
use game::reward::grant
pub fn main() -> i64 {
    return grant()
}";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(helper, "pub fn grant() -> i64 { return 1 }"),
    ]);

    let tokens = databases.semantic_tokens(&main);

    assert_token_at(
        &tokens,
        0,
        line(main_text, 0).find("game").expect("module root"),
        "game".len(),
        SemanticTokenType::Module,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        0,
        line(main_text, 0).find("reward").expect("module leaf"),
        "reward".len(),
        SemanticTokenType::Module,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        0,
        line(main_text, 0)
            .find("grant")
            .expect("imported declaration"),
        "grant".len(),
        SemanticTokenType::Function,
        SemanticTokenModifiers::SOURCE,
    );
}

#[test]
fn semantic_tokens_classify_unresolved_import_leaf() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let main_text = "use missing::module::grant";
    let databases = databases_for(vec![SourceFileSnapshot::new(main.clone(), main_text)]);

    let tokens = databases.semantic_tokens(&main);

    assert_token_at(
        &tokens,
        0,
        line(main_text, 0).find("missing").expect("module root"),
        "missing".len(),
        SemanticTokenType::Module,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        0,
        line(main_text, 0).find("module").expect("module leaf"),
        "module".len(),
        SemanticTokenType::Module,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        0,
        line(main_text, 0)
            .find("grant")
            .expect("imported declaration"),
        "grant".len(),
        SemanticTokenType::UnresolvedReference,
        SemanticTokenModifiers::UNRESOLVED,
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
fn semantic_tokens_keep_hir_classifications_under_recovered_body_error() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(amount: i64) -> i64 {
    let value = amount +
    return amount
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let tokens = databases.semantic_tokens(&document);
    let source_declaration_definition = SemanticTokenModifiers::DECLARATION
        .union(SemanticTokenModifiers::DEFINITION)
        .union(SemanticTokenModifiers::SOURCE);
    let source_declaration =
        SemanticTokenModifiers::DECLARATION.union(SemanticTokenModifiers::SOURCE);

    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("main").expect("function should exist"),
        "main".len(),
        SemanticTokenType::Function,
        source_declaration_definition,
    );
    assert_token_at(
        &tokens,
        0,
        line(text, 0)
            .find("amount")
            .expect("parameter declaration should exist"),
        "amount".len(),
        SemanticTokenType::Parameter,
        source_declaration,
    );
    assert_token_at(
        &tokens,
        1,
        line(text, 1)
            .find("amount")
            .expect("parameter read should exist"),
        "amount".len(),
        SemanticTokenType::Parameter,
        SemanticTokenModifiers::SOURCE,
    );
    assert_token_at(
        &tokens,
        2,
        line(text, 2)
            .find("amount")
            .expect("recovered parameter read should exist"),
        "amount".len(),
        SemanticTokenType::Parameter,
        SemanticTokenModifiers::SOURCE,
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
    let member_modifiers = SemanticTokenModifiers::DECLARATION
        .union(SemanticTokenModifiers::DEFINITION)
        .union(SemanticTokenModifiers::SOURCE);

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
        SemanticTokenModifiers::SOURCE,
    );
    assert_token_at(
        &tokens,
        9,
        line(text, 9)
            .find("bonus")
            .expect("method use should exist"),
        "bonus".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::SOURCE,
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
        SemanticTokenModifiers::SOURCE,
    );
}

#[test]
fn semantic_tokens_classify_source_type_hints() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Reward {
    amount: i64
}

pub enum Progress {
    Started
}

pub trait Rewardable {
    fn preview(self, reward: Reward) -> Progress
}

pub fn main(reward: Reward, rewardable: Rewardable) -> Progress {
    return rewardable.preview(reward)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let tokens = databases.semantic_tokens(&document);

    assert_token_at(
        &tokens,
        9,
        line(text, 9)
            .find("Reward")
            .expect("method source type hint"),
        "Reward".len(),
        SemanticTokenType::Type,
        SemanticTokenModifiers::SOURCE,
    );
    assert_token_at(
        &tokens,
        9,
        line(text, 9)
            .find("Progress")
            .expect("method return type hint"),
        "Progress".len(),
        SemanticTokenType::Type,
        SemanticTokenModifiers::SOURCE,
    );
    assert_token_at(
        &tokens,
        12,
        line(text, 12)
            .find("Reward")
            .expect("function source type hint"),
        "Reward".len(),
        SemanticTokenType::Type,
        SemanticTokenModifiers::SOURCE,
    );
    assert_token_at(
        &tokens,
        12,
        line(text, 12)
            .find("Rewardable")
            .expect("trait source type hint"),
        "Rewardable".len(),
        SemanticTokenType::Type,
        SemanticTokenModifiers::SOURCE,
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
        SemanticTokenType::BuiltinType,
        SemanticTokenModifiers::BUILTIN,
    );
    assert_token_at(
        &tokens,
        0,
        line(text, 0).find("String").expect("builtin string hint"),
        "String".len(),
        SemanticTokenType::BuiltinType,
        SemanticTokenModifiers::BUILTIN,
    );
    assert_token_at(
        &tokens,
        0,
        line(text, 0).rfind("i64").expect("builtin return hint"),
        "i64".len(),
        SemanticTokenType::BuiltinType,
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
fn semantic_tokens_highlighting_showcase_pins_current_collapses() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let helper = DocumentId::from("/workspace/scripts/game/support.vela");
    let mut schema = RegistryFacts::default();
    schema.insert_type("SchemaPlayer", TypeFact::host("SchemaPlayer"));
    schema.insert_trait("Rewardable", TypeFact::trait_type("Rewardable"));
    schema.insert_field("SchemaPlayer", "level", TypeFact::I64);
    schema.insert_method(
        "SchemaPlayer",
        "grant",
        TypeFact::function(vec![TypeFact::I64], TypeFact::I64),
    );
    schema.insert_trait_method(
        "Rewardable",
        "preview",
        TypeFact::function(vec![TypeFact::I64], TypeFact::I64),
    );
    let databases = databases_for_with_schema(
        vec![
            SourceFileSnapshot::new(main.clone(), HIGHLIGHTING_SHOWCASE),
            SourceFileSnapshot::new(
                helper,
                "pub fn source_helper(amount: i64) -> i64 { return amount }",
            ),
        ],
        schema,
    );

    let tokens = databases.semantic_tokens(&main);
    let declaration_definition = SemanticTokenModifiers::DECLARATION
        .union(SemanticTokenModifiers::DEFINITION)
        .union(SemanticTokenModifiers::SOURCE);
    let source_declaration =
        SemanticTokenModifiers::DECLARATION.union(SemanticTokenModifiers::SOURCE);

    assert_token_at(
        &tokens,
        0,
        0,
        "#!/usr/bin/env vela".len(),
        SemanticTokenType::Comment,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        4,
        line(HIGHLIGHTING_SHOWCASE, 4)
            .find('#')
            .expect("attribute marker"),
        1,
        SemanticTokenType::Attribute,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        5,
        line(HIGHLIGHTING_SHOWCASE, 5)
            .find("Reward")
            .expect("struct name"),
        "Reward".len(),
        SemanticTokenType::Struct,
        declaration_definition,
    );
    assert_token_at(
        &tokens,
        10,
        line(HIGHLIGHTING_SHOWCASE, 10)
            .find("Progress")
            .expect("enum name"),
        "Progress".len(),
        SemanticTokenType::Enum,
        declaration_definition,
    );
    assert_token_at(
        &tokens,
        16,
        line(HIGHLIGHTING_SHOWCASE, 16)
            .find("Scored")
            .expect("trait name"),
        "Scored".len(),
        SemanticTokenType::Interface,
        declaration_definition,
    );
    assert_token_at(
        &tokens,
        13,
        line(HIGHLIGHTING_SHOWCASE, 13)
            .find("result")
            .expect("tuple variant payload field"),
        "result".len(),
        SemanticTokenType::Field,
        declaration_definition,
    );
    assert_token_at(
        &tokens,
        26,
        line(HIGHLIGHTING_SHOWCASE, 26)
            .find("game")
            .expect("module root"),
        "game".len(),
        SemanticTokenType::Module,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        26,
        line(HIGHLIGHTING_SHOWCASE, 26)
            .find("source_helper")
            .expect("imported function"),
        "source_helper".len(),
        SemanticTokenType::Function,
        SemanticTokenModifiers::SOURCE,
    );
    assert_token_at(
        &tokens,
        28,
        line(HIGHLIGHTING_SHOWCASE, 28)
            .find("START_LEVEL")
            .expect("const declaration"),
        "START_LEVEL".len(),
        SemanticTokenType::Const,
        declaration_definition,
    );
    assert_token_at(
        &tokens,
        29,
        line(HIGHLIGHTING_SHOWCASE, 29)
            .find("active_player")
            .expect("global declaration"),
        "active_player".len(),
        SemanticTokenType::Global,
        declaration_definition,
    );
    assert_token_at(
        &tokens,
        32,
        line(HIGHLIGHTING_SHOWCASE, 32)
            .find("SchemaPlayer")
            .expect("schema type hint"),
        "SchemaPlayer".len(),
        SemanticTokenType::Type,
        SemanticTokenModifiers::HOST,
    );
    assert_token_at(
        &tokens,
        34,
        line(HIGHLIGHTING_SHOWCASE, 34)
            .find("Array")
            .expect("builtin type hint"),
        "Array".len(),
        SemanticTokenType::BuiltinType,
        SemanticTokenModifiers::BUILTIN,
    );
    assert_token_at(
        &tokens,
        36,
        line(HIGHLIGHTING_SHOWCASE, 36)
            .find("source")
            .expect("local"),
        "source".len(),
        SemanticTokenType::Variable,
        source_declaration,
    );
    assert_token_at(
        &tokens,
        37,
        line(HIGHLIGHTING_SHOWCASE, 37)
            .find("true")
            .expect("boolean"),
        "true".len(),
        SemanticTokenType::Boolean,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        38,
        line(HIGHLIGHTING_SHOWCASE, 38).find("null").expect("null"),
        "null".len(),
        SemanticTokenType::Null,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        39,
        line(HIGHLIGHTING_SHOWCASE, 39)
            .find("b\"xp\"")
            .expect("bytes"),
        "b\"xp\"".len(),
        SemanticTokenType::Bytes,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        40,
        line(HIGHLIGHTING_SHOWCASE, 40).find("'x'").expect("char"),
        "'x'".len(),
        SemanticTokenType::String,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        41,
        line(HIGHLIGHTING_SHOWCASE, 41)
            .find("source_helper")
            .expect("source function call"),
        "source_helper".len(),
        SemanticTokenType::Function,
        SemanticTokenModifiers::SOURCE,
    );
    assert_token_at(
        &tokens,
        42,
        line(HIGHLIGHTING_SHOWCASE, 42)
            .rfind("level")
            .expect("host field"),
        "level".len(),
        SemanticTokenType::Property,
        SemanticTokenModifiers::HOST,
    );
    assert_token_at(
        &tokens,
        43,
        line(HIGHLIGHTING_SHOWCASE, 43)
            .find("grant")
            .expect("host method"),
        "grant".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::HOST,
    );
    assert_token_at(
        &tokens,
        44,
        line(HIGHLIGHTING_SHOWCASE, 44)
            .rfind("preview")
            .expect("schema trait method"),
        "preview".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::HOST,
    );
    assert_token_at(
        &tokens,
        45,
        line(HIGHLIGHTING_SHOWCASE, 45)
            .find("max")
            .expect("stdlib function"),
        "max".len(),
        SemanticTokenType::Function,
        SemanticTokenModifiers::BUILTIN,
    );
    assert_token_at(
        &tokens,
        45,
        line(HIGHLIGHTING_SHOWCASE, 45)
            .find("len")
            .expect("stdlib method"),
        "len".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::BUILTIN,
    );
    assert_token_at(
        &tokens,
        46,
        line(HIGHLIGHTING_SHOWCASE, 46)
            .find("bonus")
            .expect("source method collapse point"),
        "bonus".len(),
        SemanticTokenType::Variable,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        49,
        line(HIGHLIGHTING_SHOWCASE, 49)
            .find("if")
            .expect("control-flow keyword"),
        "if".len(),
        SemanticTokenType::Keyword,
        SemanticTokenModifiers::CONTROL_FLOW,
    );
    assert_token_at(
        &tokens,
        49,
        line(HIGHLIGHTING_SHOWCASE, 49)
            .find("&&")
            .expect("logical operator"),
        "&&".len(),
        SemanticTokenType::LogicalOperator,
        SemanticTokenModifiers::NONE,
    );
    assert_token_at(
        &tokens,
        60,
        line(HIGHLIGHTING_SHOWCASE, 60)
            .find("missing_symbol")
            .expect("unresolved match arm"),
        "missing_symbol".len(),
        SemanticTokenType::Variable,
        SemanticTokenModifiers::NONE,
    );
}

#[test]
fn semantic_token_taxonomy_declares_custom_fallbacks() {
    assert_eq!(SemanticTokenType::Struct.standard_fallback(), "struct");
    assert_eq!(SemanticTokenType::Enum.standard_fallback(), "enum");
    assert_eq!(
        SemanticTokenType::Interface.standard_fallback(),
        "interface"
    );
    assert_eq!(SemanticTokenType::TypeAlias.standard_fallback(), "type");
    assert_eq!(SemanticTokenType::BuiltinType.standard_fallback(), "type");
    assert_eq!(SemanticTokenType::Const.standard_fallback(), "variable");
    assert_eq!(SemanticTokenType::Global.standard_fallback(), "variable");
    assert_eq!(SemanticTokenType::Label.standard_fallback(), "variable");
    assert_eq!(SemanticTokenType::Boolean.standard_fallback(), "keyword");
    assert_eq!(SemanticTokenType::Null.standard_fallback(), "keyword");
    assert_eq!(
        SemanticTokenType::UnresolvedReference.standard_fallback(),
        "variable"
    );
    assert_eq!(
        SemanticTokenType::ArithmeticOperator.standard_fallback(),
        "operator"
    );
    assert_eq!(
        SemanticTokenType::AssignmentOperator.standard_fallback(),
        "operator"
    );
    assert_eq!(
        SemanticTokenType::BitwiseOperator.standard_fallback(),
        "operator"
    );
    assert_eq!(
        SemanticTokenType::ComparisonOperator.standard_fallback(),
        "operator"
    );
    assert_eq!(
        SemanticTokenType::LogicalOperator.standard_fallback(),
        "operator"
    );
    assert_eq!(
        SemanticTokenType::NegationOperator.standard_fallback(),
        "operator"
    );
    assert_eq!(
        SemanticTokenType::Punctuation.standard_fallback(),
        "operator"
    );
    assert_eq!(SemanticTokenType::Brace.standard_fallback(), "operator");
    assert_eq!(SemanticTokenType::Bracket.standard_fallback(), "operator");
    assert_eq!(
        SemanticTokenType::Parenthesis.standard_fallback(),
        "operator"
    );
    assert_eq!(SemanticTokenType::Comma.standard_fallback(), "operator");
    assert_eq!(SemanticTokenType::Dot.standard_fallback(), "operator");
    assert_eq!(SemanticTokenType::Colon.standard_fallback(), "operator");
    assert_eq!(SemanticTokenType::Semicolon.standard_fallback(), "operator");
    assert_eq!(
        SemanticTokenType::PathSeparator.standard_fallback(),
        "operator"
    );
    assert_eq!(SemanticTokenType::Bytes.standard_fallback(), "string");
    let custom_token_names = [
        "struct",
        "enum",
        "interface",
        "typeAlias",
        "const",
        "global",
        "boolean",
        "null",
        "builtinType",
        "label",
        "unresolvedReference",
        "arithmeticOperator",
        "assignmentOperator",
        "bitwiseOperator",
        "comparisonOperator",
        "logicalOperator",
        "negationOperator",
        "punctuation",
        "brace",
        "bracket",
        "parenthesis",
        "comma",
        "dot",
        "colon",
        "semicolon",
        "pathSeparator",
    ];
    for name in custom_token_names {
        assert!(
            SemanticTokenType::LEGEND
                .iter()
                .any(|token| token.as_str() == name),
            "semantic token legend should include {name}"
        );
    }
    assert_eq!(
        SemanticTokenModifiers::LEGEND.len(),
        SemanticTokenModifiers::FALLBACKS.len()
    );
    let expected_modifier_fallbacks = [
        ("declaration", Some("declaration")),
        ("definition", Some("definition")),
        ("readonly", Some("readonly")),
        ("deprecated", Some("deprecated")),
        ("defaultLibrary", Some("defaultLibrary")),
        ("host", None),
        ("unresolved", None),
        ("source", None),
        ("public", None),
        ("mutable", Some("modification")),
        ("callable", None),
        ("controlFlow", None),
        ("associated", Some("static")),
        ("trait", None),
        ("schema", None),
        ("documentation", Some("documentation")),
    ];
    for (name, expected_fallback) in expected_modifier_fallbacks {
        let index = SemanticTokenModifiers::LEGEND
            .iter()
            .position(|modifier| *modifier == name)
            .unwrap_or_else(|| panic!("semantic token modifier legend should include {name}"));
        assert_eq!(SemanticTokenModifiers::FALLBACKS[index], expected_fallback);
    }
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
        "missing token at {line}:{character} len {length} as {token_type:?} with {modifiers:?}; tokens: {:?}",
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
