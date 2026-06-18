use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

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

fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases
}
