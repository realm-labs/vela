use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

const HIGHLIGHTING_SHOWCASE: &str =
    include_str!("../../../../tests/fixtures/lsp_highlighting/showcase.vela");

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
    let schema_host = SemanticTokenModifiers::HOST.union(SemanticTokenModifiers::SCHEMA);

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
        schema_host,
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
        41,
        line(HIGHLIGHTING_SHOWCASE, 41)
            .rfind("amount")
            .expect("source field from constructor local"),
        "amount".len(),
        SemanticTokenType::Property,
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
        schema_host,
    );
    assert_token_at(
        &tokens,
        43,
        line(HIGHLIGHTING_SHOWCASE, 43)
            .find("grant")
            .expect("host method"),
        "grant".len(),
        SemanticTokenType::Method,
        schema_host,
    );
    assert_token_at(
        &tokens,
        44,
        line(HIGHLIGHTING_SHOWCASE, 44)
            .rfind("preview")
            .expect("schema trait method"),
        "preview".len(),
        SemanticTokenType::Method,
        schema_host,
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
            .expect("source method"),
        "bonus".len(),
        SemanticTokenType::Method,
        SemanticTokenModifiers::SOURCE,
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
        63,
        line(HIGHLIGHTING_SHOWCASE, 63)
            .find("unknown_call")
            .expect("unresolved call"),
        "unknown_call".len(),
        SemanticTokenType::UnresolvedReference,
        SemanticTokenModifiers::UNRESOLVED,
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
