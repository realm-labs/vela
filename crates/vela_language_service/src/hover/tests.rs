use vela_analysis::registry::{RegistryEffectFact, RegistryFacts};

use super::*;
use crate::{
    DisplayParts, SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};

#[test]
fn hover_degrades_to_any_without_schema() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main(player: Player) { return player }";
    let databases = databases_for(&document, text, RegistryFacts::default());

    let hover = databases
        .hover(
            &document,
            Position::new(0, text.find("Player").expect("type hint")),
        )
        .expect("hover should degrade unknown type hints");

    assert_eq!(hover.kind(), HoverKind::Type);
    assert_eq!(hover.label(), "Player");
    assert_eq!(hover.detail(), "Any");
}

#[test]
fn hover_reports_effects_and_permissions() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main(player: Player) { player.grant(1) }";
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    schema.insert_method(
        "Player",
        "grant",
        TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
    );
    schema.insert_method_effect("Player", "grant", RegistryEffectFact::host_write());
    schema.insert_method_docs("Player", "grant", "Grant player rewards.");
    schema.insert_method_access(vela_analysis::registry::RegistryMethodAccessFact {
        owner: "Player".to_owned(),
        name: "grant".to_owned(),
        public: true,
        reflect_callable: true,
        required_permissions: vec!["player.reward".to_owned()],
    });
    let databases = databases_for(&document, text, schema);

    let hover = databases
        .hover(
            &document,
            Position::new(0, text.find("grant").expect("method name")),
        )
        .expect("hover should resolve schema method");

    assert_eq!(hover.kind(), HoverKind::Method);
    assert_eq!(hover.label(), "Player.grant");
    assert!(hover.detail().contains("Function(i64) -> bool"));
    assert!(hover.detail().contains("effects: writes_host"));
    assert!(hover.detail().contains("permissions: player.reward"));
    assert_eq!(hover.docs(), Some("Grant player rewards."));
    assert_eq!(
        hover.symbol(),
        Some(&SymbolRef::Schema("Player.grant".to_owned()))
    );
}

#[test]
fn hover_reports_schema_trait_method_fact() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main(rewardable: Rewardable) { rewardable.preview(1) }";
    let mut schema = RegistryFacts::default();
    schema.insert_trait("Rewardable", TypeFact::trait_type("Rewardable"));
    schema.insert_trait_method(
        "Rewardable",
        "preview",
        TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
    );
    schema.insert_trait_method_docs("Rewardable", "preview", "Preview a reward.");
    let databases = databases_for(&document, text, schema);

    let hover = databases
        .hover(
            &document,
            Position::new(0, text.find("preview").expect("trait method name")),
        )
        .expect("hover should resolve schema trait method");

    assert_eq!(hover.kind(), HoverKind::Method);
    assert_eq!(hover.label(), "Rewardable.preview");
    assert!(hover.detail().contains("Function(i64) -> bool"));
    assert_eq!(hover.docs(), Some("Preview a reward."));
    assert_eq!(
        hover.symbol(),
        Some(&SymbolRef::Schema("Rewardable.preview".to_owned()))
    );
}

#[test]
fn hover_reports_schema_trait_fact() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main(rewardable: Rewardable) { return rewardable }";
    let mut schema = RegistryFacts::default();
    schema.insert_trait("Rewardable", TypeFact::trait_type("Rewardable"));
    schema.insert_trait_docs("Rewardable", "Rewardable host trait.");
    let databases = databases_for(&document, text, schema);

    let hover = databases
        .hover(
            &document,
            Position::new(0, text.find("Rewardable").expect("trait type hint")),
        )
        .expect("hover should resolve schema trait");

    assert_eq!(hover.kind(), HoverKind::Trait);
    assert_eq!(hover.label(), "Rewardable");
    assert_eq!(hover.detail(), "Rewardable");
    assert_eq!(hover.docs(), Some("Rewardable host trait."));
    assert_eq!(
        hover.symbol(),
        Some(&SymbolRef::Schema("Rewardable".to_owned()))
    );
}

#[test]
fn hover_reports_schema_type_field_and_function_docs() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main(player: Player) {\n    player.level\n    grant(player)\n}";
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    schema.insert_type_docs("Player", "Player host object.");
    schema.insert_field("Player", "level", TypeFact::I64);
    schema.insert_field_docs("Player", "level", "Current player level.");
    schema.insert_function(
        "grant",
        TypeFact::function(vec![TypeFact::host("Player")], TypeFact::BOOL),
    );
    schema.insert_function_docs("grant", "Grant a player reward.");
    let databases = databases_for(&document, text, schema);

    let type_hover = databases
        .hover(
            &document,
            Position::new(0, text.find("Player").expect("type hint")),
        )
        .expect("hover should resolve schema type docs");
    assert_eq!(type_hover.kind(), HoverKind::Type);
    assert_eq!(type_hover.docs(), Some("Player host object."));
    assert_eq!(
        type_hover.symbol(),
        Some(&SymbolRef::Schema("Player".to_owned()))
    );

    let field_hover = databases
        .hover(
            &document,
            Position::new(
                1,
                text.lines()
                    .nth(1)
                    .expect("field line")
                    .find("level")
                    .expect("field name"),
            ),
        )
        .expect("hover should resolve schema field docs");
    assert_eq!(field_hover.kind(), HoverKind::Field);
    assert_eq!(field_hover.docs(), Some("Current player level."));
    assert_eq!(
        field_hover.symbol(),
        Some(&SymbolRef::Schema("Player.level".to_owned()))
    );

    let function_hover = databases
        .hover(
            &document,
            Position::new(
                2,
                text.lines()
                    .nth(2)
                    .expect("function line")
                    .find("grant")
                    .expect("function name"),
            ),
        )
        .expect("hover should resolve schema function docs");
    assert_eq!(function_hover.kind(), HoverKind::Function);
    assert_eq!(function_hover.docs(), Some("Grant a player reward."));
    assert_eq!(
        function_hover.symbol(),
        Some(&SymbolRef::Schema("grant".to_owned()))
    );
}

#[test]
fn hover_reports_script_parameter_fact() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main(amount: i64) -> i64 { return amount }";
    let databases = databases_for(&document, text, RegistryFacts::default());

    let hover = databases
        .hover(
            &document,
            Position::new(0, text.rfind("amount").expect("amount use")),
        )
        .expect("hover should resolve parameter use");

    assert_eq!(hover.kind(), HoverKind::Parameter);
    assert_eq!(hover.label(), "amount");
    assert_eq!(hover.detail(), "i64");
    assert_eq!(hover.detail_parts(), &DisplayParts::type_name("i64"));
    assert_eq!(
        hover.symbol(),
        Some(&SymbolRef::local_at(
            "amount",
            document,
            TextRange::new(12, 18)
        ))
    );
}

#[test]
fn hover_reports_source_global_fact() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "global score: i64\npub fn main() -> i64 { return score }";
    let databases = databases_for(&document, text, RegistryFacts::default());
    let use_line = text.lines().nth(1).expect("global use line should exist");

    let hover = databases
        .hover(
            &document,
            Position::new(1, use_line.find("score").expect("global use should exist")),
        )
        .expect("hover should resolve global use");

    assert_eq!(hover.kind(), HoverKind::Global);
    assert_eq!(hover.label(), "game::main::score");
    assert_eq!(hover.detail(), "i64");
    assert_eq!(
        hover.symbol(),
        Some(&SymbolRef::Source("game::main::score".to_owned()))
    );
}

#[test]
fn hover_reports_stdlib_function_fact() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main() { math::max(1, 2) }";
    let databases = databases_for(&document, text, RegistryFacts::default());

    let hover = databases
        .hover(
            &document,
            Position::new(0, text.find("max").expect("stdlib function")),
        )
        .expect("hover should resolve stdlib function");

    assert_eq!(hover.kind(), HoverKind::Function);
    assert_eq!(hover.label(), "math::max");
    assert_eq!(
        hover.detail(),
        "Function(i64 | f64, i64 | f64) -> i64 | f64"
    );
    assert_eq!(
        hover.symbol(),
        Some(&SymbolRef::Builtin("math::max".to_owned()))
    );
}

#[test]
fn hover_reports_stdlib_method_fact() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main(scores: Array<i64>) { scores.filter(|score| score > 0) }";
    let databases = databases_for(&document, text, RegistryFacts::default());

    let hover = databases
        .hover(
            &document,
            Position::new(0, text.find("filter").expect("stdlib method")),
        )
        .expect("hover should resolve stdlib method");

    assert_eq!(hover.kind(), HoverKind::Method);
    assert_eq!(hover.label(), "Array(i64).filter");
    assert_eq!(
        hover.detail(),
        "Function(Function(i64) -> bool) -> Array(i64)"
    );
    assert_eq!(
        hover.symbol(),
        Some(&SymbolRef::Builtin("Array(i64).filter".to_owned()))
    );
}

#[test]
fn hover_reports_imported_module_path_fact() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let reward = DocumentId::from("/workspace/scripts/game/reward.vela");
    let main_text = "use game::reward::grant\npub fn main() { return grant() }";
    let reward_text = "pub fn grant() -> i64 { return 1 }";
    let databases = databases_for_files(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(reward, reward_text),
    ]);

    let hover = databases
        .hover(
            &main,
            Position::new(0, main_text.find("reward").expect("module segment")),
        )
        .expect("hover should resolve imported module path");

    assert_eq!(hover.kind(), HoverKind::Module);
    assert_eq!(hover.label(), "game::reward");
    assert_eq!(hover.detail(), "module game::reward");
    assert_eq!(
        hover.symbol(),
        Some(&SymbolRef::Source("game::reward".to_owned()))
    );
}

#[test]
fn hover_reports_source_struct_field_fact() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"struct Player {
    #[doc("Current level")]
    level: i64,
}
pub fn main(player: Player) {
    return player.level
}"#;
    let databases = databases_for(&document, text, RegistryFacts::default());
    let use_line = text.lines().nth(5).expect("field use line should exist");

    let use_hover = databases
        .hover(
            &document,
            Position::new(5, use_line.find("level").expect("field use should exist")),
        )
        .expect("hover should resolve field use");
    assert_eq!(use_hover.kind(), HoverKind::Field);
    assert_eq!(use_hover.label(), "game::main::Player.level");
    assert_eq!(use_hover.detail(), "i64");
    assert_eq!(use_hover.docs(), Some("Current level"));
    assert_eq!(
        use_hover.symbol(),
        Some(&SymbolRef::Source("game::main::Player.level".to_owned()))
    );

    let declaration_hover = databases
        .hover(&document, Position::new(2, 4))
        .expect("hover should resolve field declaration");
    assert_eq!(declaration_hover.kind(), HoverKind::Field);
    assert_eq!(declaration_hover.label(), "game::main::Player.level");
    assert_eq!(declaration_hover.detail(), "i64");
    assert_eq!(declaration_hover.docs(), Some("Current level"));
    assert_eq!(
        declaration_hover.symbol(),
        Some(&SymbolRef::Source("game::main::Player.level".to_owned()))
    );
}

#[test]
fn hover_reports_source_method_fact() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"struct Player {
    level: i64,
}
impl Player {
    fn grant(amount: i64) -> bool {
        return amount > 0
    }
}
pub fn main(player: Player) {
    return player.grant(3)
}"#;
    let databases = databases_for(&document, text, RegistryFacts::default());
    let use_line = text.lines().nth(9).expect("method use line should exist");

    let use_hover = databases
        .hover(
            &document,
            Position::new(9, use_line.find("grant").expect("method use should exist")),
        )
        .expect("hover should resolve method use");
    assert_eq!(use_hover.kind(), HoverKind::Method);
    assert_eq!(use_hover.label(), "game::main::Player.grant");
    assert_eq!(use_hover.detail(), "(amount: i64) -> bool");
    assert_eq!(
        use_hover.symbol(),
        Some(&SymbolRef::Source("game::main::Player.grant".to_owned()))
    );

    let declaration_hover = databases
        .hover(&document, Position::new(4, 7))
        .expect("hover should resolve method declaration");
    assert_eq!(declaration_hover.kind(), HoverKind::Method);
    assert_eq!(declaration_hover.label(), "game::main::Player.grant");
    assert_eq!(declaration_hover.detail(), "(amount: i64) -> bool");
    assert_eq!(
        declaration_hover.symbol(),
        Some(&SymbolRef::Source("game::main::Player.grant".to_owned()))
    );
}

#[test]
fn hover_reports_source_trait_method_docs() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"trait Rewardable {
    #[doc("Preview reward")]
    fn preview(amount: i64) -> bool
}"#;
    let databases = databases_for(&document, text, RegistryFacts::default());

    let hover = databases
        .hover(&document, Position::new(2, 7))
        .expect("hover should resolve trait method declaration");

    assert_eq!(hover.kind(), HoverKind::Method);
    assert_eq!(hover.label(), "game::main::Rewardable.preview");
    assert_eq!(hover.detail(), "(amount: i64) -> bool");
    assert_eq!(hover.docs(), Some("Preview reward"));
    assert_eq!(
        hover.symbol(),
        Some(&SymbolRef::Source(
            "game::main::Rewardable.preview".to_owned()
        ))
    );
}

#[test]
fn hover_reports_source_trait_fact() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"#[doc("Rewardable script trait")]
trait Rewardable {
    fn preview(amount: i64) -> bool
}
pub fn main(rewardable: Rewardable) {
    return rewardable
}"#;
    let databases = databases_for(&document, text, RegistryFacts::default());

    let declaration_hover = databases
        .hover(&document, Position::new(1, 6))
        .expect("hover should resolve trait declaration");
    assert_eq!(declaration_hover.kind(), HoverKind::Trait);
    assert_eq!(declaration_hover.label(), "game::main::Rewardable");
    assert_eq!(declaration_hover.detail(), "game::main::Rewardable");
    assert_eq!(declaration_hover.docs(), Some("Rewardable script trait"));
    assert_eq!(
        declaration_hover.symbol(),
        Some(&SymbolRef::Source("game::main::Rewardable".to_owned()))
    );

    let hint_line = text.lines().nth(4).expect("trait hint line should exist");
    let hint_hover = databases
        .hover(
            &document,
            Position::new(
                4,
                hint_line
                    .find("Rewardable")
                    .expect("trait hint should exist"),
            ),
        )
        .expect("hover should resolve trait type hint");
    assert_eq!(hint_hover.kind(), HoverKind::Trait);
    assert_eq!(hint_hover.label(), "game::main::Rewardable");
    assert_eq!(hint_hover.detail(), "game::main::Rewardable");
    assert_eq!(
        hint_hover.symbol(),
        Some(&SymbolRef::Source("game::main::Rewardable".to_owned()))
    );
}

#[test]
fn hover_reports_source_trait_receiver_method_fact() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"trait Rewardable {
    #[doc("Preview reward")]
    fn preview(amount: i64) -> bool
}
pub fn main(rewardable: Rewardable) {
    return rewardable.preview(1)
}"#;
    let databases = databases_for(&document, text, RegistryFacts::default());
    let use_line = text
        .lines()
        .nth(5)
        .expect("trait method use line should exist");

    let hover = databases
        .hover(
            &document,
            Position::new(
                5,
                use_line
                    .find("preview")
                    .expect("trait method use should exist"),
            ),
        )
        .expect("hover should resolve trait receiver method use");

    assert_eq!(hover.kind(), HoverKind::Method);
    assert_eq!(hover.label(), "game::main::Rewardable.preview");
    assert_eq!(hover.detail(), "(amount: i64) -> bool");
    assert_eq!(hover.docs(), Some("Preview reward"));
    assert_eq!(
        hover.symbol(),
        Some(&SymbolRef::Source(
            "game::main::Rewardable.preview".to_owned()
        ))
    );
}

#[test]
fn hover_reports_source_enum_variant_fact() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"enum QuestState {
    #[doc("Active quest")]
    Active(quest_id: String, count: i64),
    Done,
}
pub fn main() {
    return QuestState::Active("quest-1", 3)
}"#;
    let databases = databases_for(&document, text, RegistryFacts::default());
    let constructor_line = text.lines().nth(6).expect("constructor line should exist");

    let use_hover = databases
        .hover(
            &document,
            Position::new(
                6,
                constructor_line
                    .find("Active")
                    .expect("variant constructor should exist"),
            ),
        )
        .expect("hover should resolve variant constructor use");
    assert_eq!(use_hover.kind(), HoverKind::Variant);
    assert_eq!(use_hover.label(), "game::main::QuestState::Active");
    assert_eq!(
        use_hover.detail(),
        "game::main::QuestState::Active(quest_id, count)"
    );
    assert_eq!(
        use_hover.symbol(),
        Some(&SymbolRef::Source(
            "game::main::QuestState::Active".to_owned()
        ))
    );

    let declaration_hover = databases
        .hover(&document, Position::new(2, 4))
        .expect("hover should resolve variant declaration");
    assert_eq!(declaration_hover.kind(), HoverKind::Variant);
    assert_eq!(declaration_hover.label(), "game::main::QuestState::Active");
    assert_eq!(declaration_hover.docs(), Some("Active quest"));
    assert_eq!(
        declaration_hover.symbol(),
        Some(&SymbolRef::Source(
            "game::main::QuestState::Active".to_owned()
        ))
    );
}

#[test]
fn hover_reports_schema_enum_variant_fact() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main() { return QuestState::Active }";
    let mut schema = RegistryFacts::default();
    schema.insert_type(
        "QuestState",
        TypeFact::enum_type("QuestState", None::<String>),
    );
    schema.insert_variant(
        "QuestState",
        "Active",
        TypeFact::enum_type("QuestState", Some("Active")),
    );
    schema.insert_variant_docs("QuestState", "Active", "Active quest state.");
    let databases = databases_for(&document, text, schema);

    let hover = databases
        .hover(
            &document,
            Position::new(0, text.find("Active").expect("variant use should exist")),
        )
        .expect("hover should resolve schema enum variant");

    assert_eq!(hover.kind(), HoverKind::Variant);
    assert_eq!(hover.label(), "QuestState::Active");
    assert_eq!(hover.detail(), "QuestState::Active");
    assert_eq!(hover.docs(), Some("Active quest state."));
    assert_eq!(
        hover.symbol(),
        Some(&SymbolRef::Schema("QuestState::Active".to_owned()))
    );
}

fn databases_for(
    document: &DocumentId,
    text: &str,
    schema: RegistryFacts,
) -> LanguageServiceDatabases {
    let mut databases = databases_for_files(vec![SourceFileSnapshot::new(document.clone(), text)]);
    databases.set_schema_facts(schema);
    databases
}

fn databases_for_files(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases
}
