use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;

use super::*;
use crate::{
    SourceFileSnapshot, SourceVersion, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};

#[test]
fn completion_uses_open_overlay_facts() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let files = vec![SourceFileSnapshot::new(
        document.clone(),
        "pub fn disk_only() { return 1 }",
    )];
    let mut workspace = Workspace::new();
    workspace.open_document(
        document.clone(),
        "pub fn overlay_only() { return 2 }",
        SourceVersion::new(2),
    );
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &workspace.snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);

    let completions = databases.completion_items(&document, Position::new(0, 7));

    assert_completion(&completions, "overlay_only", CompletionKind::Function);
    assert_no_completion(&completions, "game::main::disk_only");
}

#[test]
fn global_completion_uses_schema_facts() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let files = vec![SourceFileSnapshot::new(
        document.clone(),
        "pub fn main() { Pla }",
    )];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    schema.insert_function(
        "spawn_player",
        TypeFact::function(vec![TypeFact::STRING], TypeFact::host("Player")),
    );
    databases.set_schema_facts(schema);
    databases.update(&project);

    let completions = databases.completion_items(&document, Position::new(0, 18));

    assert_completion(&completions, "Player", CompletionKind::Type);
    assert_no_completion(&completions, "spawn_player");
}

#[test]
fn item_boundary_completion_ranks_fn_keyword_before_callables() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "f";
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    let mut schema = RegistryFacts::default();
    schema.insert_function(
        "fetch_player",
        TypeFact::function(Vec::new(), TypeFact::host("Player")),
    );
    databases.set_schema_facts(schema);
    databases.update(&project);

    let completions = databases.completion_items(&document, Position::new(0, text.len()));

    assert_eq!(completions.context().kind(), CompletionContextKind::Item);
    assert_completion(&completions, "fn", CompletionKind::Keyword);
    assert_no_completion(&completions, "fetch_player");
    assert_eq!(completions.items()[0].label(), "fn");
}

#[test]
fn item_boundary_completion_ranks_struct_keyword_before_globals() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "st";
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    let mut schema = RegistryFacts::default();
    schema.insert_function("stabilize", TypeFact::function(Vec::new(), TypeFact::BOOL));
    databases.set_schema_facts(schema);
    databases.update(&project);

    let completions = databases.completion_items(&document, Position::new(0, text.len()));

    assert_eq!(completions.context().kind(), CompletionContextKind::Item);
    assert_completion(&completions, "struct", CompletionKind::Keyword);
    assert_no_completion(&completions, "stabilize");
    assert_eq!(completions.items()[0].label(), "struct");
}

#[test]
fn member_completion_uses_host_schema_facts() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main(player: Player) { player.le }";
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    schema.insert_field("Player", "level", TypeFact::I64);
    schema.insert_method(
        "Player",
        "level_up",
        TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
    );
    databases.set_schema_facts(schema);
    databases.update(&project);

    let completions = databases.completion_items(
        &document,
        Position::new(0, text.find("le }").expect("member prefix") + "le".len()),
    );

    assert_eq!(completions.context().kind(), CompletionContextKind::Member);
    assert_completion(&completions, "level", CompletionKind::Field);
    assert_completion(&completions, "level_up", CompletionKind::Method);
}

#[test]
fn member_completion_uses_schema_trait_method_facts() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main(rewardable: Rewardable) { rewardable.pr }";
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    let mut schema = RegistryFacts::default();
    schema.insert_trait("Rewardable", TypeFact::trait_type("Rewardable"));
    schema.insert_trait_method(
        "Rewardable",
        "preview",
        TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
    );
    databases.set_schema_facts(schema);
    databases.update(&project);

    let completions = databases.completion_items(
        &document,
        Position::new(0, text.find("pr }").expect("member prefix") + "pr".len()),
    );

    assert_eq!(completions.context().kind(), CompletionContextKind::Member);
    assert_completion(&completions, "preview", CompletionKind::Method);
}

#[test]
fn record_field_completion_requires_known_type() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub struct Player { id: String level: i64 }\npub fn main() { let player = Player { id: \"p1\", le } }";
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);

    let completions = databases.completion_items(
        &document,
        Position::new(
            1,
            text.lines()
                .nth(1)
                .expect("second line")
                .find("le }")
                .expect("record prefix")
                + "le".len(),
        ),
    );

    assert_eq!(
        completions.context().kind(),
        CompletionContextKind::RecordField
    );
    assert_completion(&completions, "level", CompletionKind::Field);
    assert_no_completion(&completions, "id");

    let unknown_text =
        "pub fn helper() { return 1 }\npub fn main() { let player = Missing { le } }";
    let files = vec![SourceFileSnapshot::new(document.clone(), unknown_text)];
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    databases.update(&project);

    let completions = databases.completion_items(
        &document,
        Position::new(
            1,
            unknown_text
                .lines()
                .nth(1)
                .expect("second line")
                .find("le }")
                .expect("unknown prefix")
                + "le".len(),
        ),
    );

    assert_eq!(
        completions.context().kind(),
        CompletionContextKind::RecordField
    );
    assert!(completions.items().is_empty(), "{completions:?}");
}

#[test]
fn record_field_completion_uses_schema_facts() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main() { let player = Player { le } }";
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    schema.insert_field("Player", "level", TypeFact::I64);
    schema.insert_field("Player", "name", TypeFact::STRING);
    databases.set_schema_facts(schema);
    databases.update(&project);

    let completions = databases.completion_items(
        &document,
        Position::new(0, text.find("le }").expect("record prefix") + "le".len()),
    );

    assert_eq!(
        completions.context().kind(),
        CompletionContextKind::RecordField
    );
    assert_completion(&completions, "level", CompletionKind::Field);
    assert_no_completion(&completions, "name");
}

#[test]
fn module_completion_follows_import_context() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let reward = DocumentId::from("/workspace/scripts/game/reward.vela");
    let files = vec![
        SourceFileSnapshot::new(main.clone(), "use game::r"),
        SourceFileSnapshot::new(reward, "pub fn grant() { return 1 }"),
    ];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);

    let completions = databases.completion_items(&main, Position::new(0, "use game::r".len()));

    assert_eq!(
        completions.context().kind(),
        CompletionContextKind::ModulePath
    );
    assert_eq!(completions.context().module_base(), Some("game"));
    assert_completion(&completions, "reward", CompletionKind::Module);
    assert_no_completion(&completions, "main");
}

#[test]
fn module_path_completion_uses_stdlib_function_segments() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main() { math:: }";
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);

    let completions = databases.completion_items(
        &document,
        Position::new(0, text.find(" }").expect("completion point")),
    );

    assert_eq!(
        completions.context().kind(),
        CompletionContextKind::ModulePath
    );
    assert_eq!(completions.context().module_base(), Some("math"));
    assert_completion(&completions, "max", CompletionKind::Function);
    assert_completion(&completions, "sqrt", CompletionKind::Function);
    assert_no_completion(&completions, "math::max");
}

#[test]
fn global_completion_prefers_current_module_declarations_and_locals() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub struct Player { level: i64 }\npub fn main(amount: i64) { let ammo = 1; am }";
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);

    let main_line = text.lines().nth(1).expect("main line");
    let completions = databases.completion_items(
        &document,
        Position::new(
            1,
            main_line.find("am }").expect("local prefix") + "am".len(),
        ),
    );

    assert_completion(&completions, "amount", CompletionKind::Parameter);
    assert_completion(&completions, "ammo", CompletionKind::Binding);
    assert_eq!(completions.items()[0].label(), "amount");
}

#[test]
fn global_completion_uses_unqualified_current_module_structs() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub struct Player { level: i64 }\npub fn main() { Pla }";
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let main_line = text.lines().nth(1).expect("main line");

    let completions = databases.completion_items(
        &document,
        Position::new(
            1,
            main_line.find("Pla").expect("struct prefix") + "Pla".len(),
        ),
    );

    assert_completion(&completions, "Player", CompletionKind::Type);
    assert_no_completion(&completions, "game::main::Player");
}

#[test]
fn named_argument_completion_suggests_unused_script_parameters() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"
pub fn grant(player: Player, amount: i64, reason: String = "quest") -> bool { return true }
pub fn main(player: Player) { grant(player: player, ) }
"#;
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    databases.set_schema_facts(schema);
    databases.update(&project);

    let main_line = text.lines().nth(2).expect("main line should exist");
    let position = Position::new(
        2,
        main_line
            .find(", )")
            .expect("call should contain empty argument")
            + ", ".len(),
    );
    let completions = databases.completion_items(&document, position);

    assert_eq!(
        completions.context().kind(),
        CompletionContextKind::NamedArgument
    );
    assert_no_completion(&completions, "player");
    assert_completion(&completions, "amount", CompletionKind::Parameter);
    assert_completion(&completions, "reason", CompletionKind::Parameter);
    let amount = completion(&completions, "amount");
    assert_eq!(amount.detail(), "i64");
    assert_eq!(amount.insert_text(), Some("amount: "));
    let reason = completion(&completions, "reason");
    assert_eq!(reason.detail(), "String (defaulted)");
    assert_eq!(reason.insert_text(), Some("reason: "));
}

#[test]
fn named_argument_completion_uses_parameter_prefix() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"
pub fn grant(player: Player, amount: i64, reason: String = "quest") -> bool { return true }
pub fn main(player: Player) { grant(player: player, am) }
"#;
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    databases.set_schema_facts(schema);
    databases.update(&project);

    let main_line = text.lines().nth(2).expect("main line should exist");
    let position = Position::new(
        2,
        main_line.find("am)").expect("call should contain prefix") + "am".len(),
    );
    let completions = databases.completion_items(&document, position);

    assert_eq!(
        completions.context().kind(),
        CompletionContextKind::NamedArgument
    );
    assert_completion(&completions, "amount", CompletionKind::Parameter);
    assert_no_completion(&completions, "reason");
    assert_no_completion(&completions, "player");
}

#[test]
fn map_key_completion_suggests_typed_enum_variants() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"
pub enum QuestState { Started, Completed, Failed }
pub fn main() {
    let rewards: Map<QuestState, i64> = {
        Started: 1,
        Co: 2,
    }
}
"#;
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let main_line = text.lines().nth(5).expect("map key line should exist");

    let completions = databases.completion_items(
        &document,
        Position::new(
            5,
            main_line.find("Co:").expect("map key prefix") + "Co".len(),
        ),
    );

    assert_eq!(completions.context().kind(), CompletionContextKind::MapKey);
    assert_completion(&completions, "Completed", CompletionKind::Variant);
    assert_no_completion(&completions, "Started");
    assert_no_completion(&completions, "Failed");
}

#[test]
fn map_key_completion_suppresses_untyped_global_fallback() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"
pub fn helper() { return 1 }
pub fn main() {
    let rewards = {
        he: 1,
    }
}
"#;
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    let main_line = text.lines().nth(4).expect("map key line should exist");

    let completions = databases.completion_items(
        &document,
        Position::new(
            4,
            main_line.find("he:").expect("map key prefix") + "he".len(),
        ),
    );

    assert_eq!(completions.context().kind(), CompletionContextKind::MapKey);
    assert!(completions.items().is_empty(), "{completions:?}");
}

#[test]
fn member_context_is_detected_without_global_fallback() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let files = vec![SourceFileSnapshot::new(
        document.clone(),
        "pub fn main(player) { player.le }",
    )];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);

    let completions = databases.completion_items(&document, Position::new(0, 31));

    assert_eq!(completions.context().kind(), CompletionContextKind::Member);
    assert!(completions.items().is_empty(), "{completions:?}");
}

fn completion<'a>(list: &'a CompletionList, label: &str) -> &'a CompletionItem {
    list.items()
        .iter()
        .find(|item| item.label() == label)
        .unwrap_or_else(|| panic!("completion {label} should exist in {list:?}"))
}

fn assert_completion(list: &CompletionList, label: &str, kind: CompletionKind) {
    assert!(
        list.items()
            .iter()
            .any(|item| item.label() == label && item.kind() == kind),
        "{list:?}"
    );
}

fn assert_no_completion(list: &CompletionList, label: &str) {
    assert!(
        list.items().iter().all(|item| item.label() != label),
        "{list:?}"
    );
}
