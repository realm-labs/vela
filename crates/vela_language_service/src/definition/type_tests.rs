use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn type_definition_follows_imported_parameter_source_type_alias() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let inventory = DocumentId::from("/workspace/scripts/game/inventory.vela");
    let main_text = r#"use game::inventory::Inventory as Bag

fn main(bag: Bag) {
return bag;
}"#;
    let inventory_text = r#"pub struct Inventory {
slots: i64,
}"#;
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(inventory.clone(), inventory_text),
    ]);
    let use_line = main_text.lines().nth(3).expect("bag use line");

    let definition = databases
        .type_definition(
            &main,
            Position::new(3, use_line.find("bag").expect("bag use")),
        )
        .expect("type definition should resolve imported source type alias");

    assert_eq!(definition.document_id(), &inventory);
    assert_eq!(definition.range().start().line, 0);
    assert_eq!(definition.range().start().character, 11);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Source("game::inventory::Inventory".into()))
    );
}

#[test]
fn type_definition_follows_imported_source_field_type_alias() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let inventory = DocumentId::from("/workspace/scripts/game/inventory.vela");
    let main_text = r#"use game::inventory::Inventory as Bag

struct Player {
inventory: Bag,
}

fn main(player: Player) {
return player.inventory;
}"#;
    let inventory_text = r#"pub struct Inventory {
slots: i64,
}"#;
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(inventory.clone(), inventory_text),
    ]);
    let use_line = main_text.lines().nth(7).expect("field use line");

    let definition = databases
        .type_definition(
            &main,
            Position::new(7, use_line.find("inventory").expect("field use")),
        )
        .expect("type definition should resolve imported source field type alias");

    assert_eq!(definition.document_id(), &inventory);
    assert_eq!(definition.range().start().line, 0);
    assert_eq!(definition.range().start().character, 11);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Source("game::inventory::Inventory".into()))
    );
}

#[test]
fn type_definition_follows_imported_source_member_type() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let inventory = DocumentId::from("/workspace/scripts/game/inventory.vela");
    let main_text = r#"use game::inventory::Player

fn main(player: Player) {
return player.inventory;
}"#;
    let inventory_text = r#"pub struct Inventory {
slots: i64,
}

pub struct Player {
inventory: Inventory,
}"#;
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(inventory.clone(), inventory_text),
    ]);
    let use_line = main_text.lines().nth(3).expect("field use line");

    let definition = databases
        .type_definition(
            &main,
            Position::new(3, use_line.find("inventory").expect("field use")),
        )
        .expect("type definition should resolve imported source member type");

    assert_eq!(definition.document_id(), &inventory);
    assert_eq!(definition.range().start().line, 0);
    assert_eq!(definition.range().start().character, 11);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Source("game::inventory::Inventory".into()))
    );
}

#[test]
fn type_definition_follows_imported_enum_variant_constructor_type() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let rewards = DocumentId::from("/workspace/scripts/game/rewards.vela");
    let main_text = r#"use game::rewards::RewardOutcome

fn main() {
return RewardOutcome::Granted { item: "gold", count: 1 };
}"#;
    let rewards_text = r#"pub enum RewardOutcome {
Granted { item: String, count: i64 },
Skipped,
}"#;
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(rewards.clone(), rewards_text),
    ]);
    let constructor_line = main_text.lines().nth(3).expect("constructor line");

    let definition = databases
        .type_definition(
            &main,
            Position::new(
                3,
                constructor_line
                    .find("Granted")
                    .expect("variant constructor"),
            ),
        )
        .expect("type definition should resolve imported enum variant owner");

    assert_eq!(definition.document_id(), &rewards);
    assert_eq!(definition.range().start().line, 0);
    assert_eq!(definition.range().start().character, 9);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Source("game::rewards::RewardOutcome".into()))
    );
}

#[test]
fn type_definition_follows_imported_source_method_return_type() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let rewards = DocumentId::from("/workspace/scripts/game/rewards.vela");
    let main_text = r#"use game::rewards::RewardConfig

fn main(config: RewardConfig) {
return config.outcome();
}"#;
    let rewards_text = r#"pub enum RewardOutcome {
Granted,
Skipped,
}

pub struct RewardConfig {
count: i64,
}

impl RewardConfig {
pub fn outcome(self) -> RewardOutcome {
return RewardOutcome::Granted;
}
}"#;
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(rewards.clone(), rewards_text),
    ]);
    let call_line = main_text.lines().nth(3).expect("method call line");

    let definition = databases
        .type_definition(
            &main,
            Position::new(3, call_line.find("outcome").expect("method call")),
        )
        .expect("type definition should resolve imported source method return type");

    assert_eq!(definition.document_id(), &rewards);
    assert_eq!(definition.range().start().line, 0);
    assert_eq!(definition.range().start().character, 9);
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Source("game::rewards::RewardOutcome".into()))
    );
}

#[test]
fn type_definition_follows_imported_const_and_global_source_types() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let rewards = DocumentId::from("/workspace/scripts/game/rewards.vela");
    let main_text = r#"use game::rewards::DEFAULT_CONFIG
use game::rewards::active_config

fn main() {
return DEFAULT_CONFIG.count + active_config.count;
}"#;
    let rewards_text = r#"pub struct RewardConfig {
count: i64,
}

pub const DEFAULT_CONFIG: RewardConfig = RewardConfig { count: 1 }
pub global active_config: RewardConfig"#;
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(rewards.clone(), rewards_text),
    ]);
    let return_line = main_text.lines().nth(4).expect("return line");

    let const_definition = databases
        .type_definition(
            &main,
            Position::new(
                4,
                return_line
                    .find("DEFAULT_CONFIG")
                    .expect("imported const use"),
            ),
        )
        .expect("type definition should resolve imported const type");
    assert_eq!(const_definition.document_id(), &rewards);
    assert_eq!(const_definition.range().start().line, 0);
    assert_eq!(const_definition.range().start().character, 11);
    assert_eq!(
        const_definition.symbol(),
        Some(&SymbolRef::Source("game::rewards::RewardConfig".into()))
    );

    let global_definition = databases
        .type_definition(
            &main,
            Position::new(
                4,
                return_line
                    .find("active_config")
                    .expect("imported global use"),
            ),
        )
        .expect("type definition should resolve imported global type");
    assert_eq!(global_definition.document_id(), &rewards);
    assert_eq!(global_definition.range().start().line, 0);
    assert_eq!(global_definition.range().start().character, 11);
    assert_eq!(
        global_definition.symbol(),
        Some(&SymbolRef::Source("game::rewards::RewardConfig".into()))
    );
}

fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases
}
