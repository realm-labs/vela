use vela_analysis::registry::RegistryFacts;

use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn hover_reports_imported_function_const_and_global_facts() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let rewards = DocumentId::from("/workspace/scripts/game/rewards.vela");
    let main_text = "\
use game::rewards::BASE_REWARD
use game::rewards::reward_scale
use game::rewards::reward_bonus
pub fn main(amount: i64) -> i64 {
    let first = BASE_REWARD
    let scaled = reward_bonus(first, reward_scale)
    return scaled + amount
}";
    let rewards_text = r#"#[doc("Base reward amount")]
pub const BASE_REWARD: i64 = 4
#[doc("Current reward scale")]
pub global reward_scale: i64
#[doc("Compute reward bonus")]
pub fn reward_bonus(amount: i64, scale: i64 = reward_scale) -> i64 {
    return amount * scale
}"#;
    let databases = databases_for_files(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(rewards, rewards_text),
    ]);

    let const_hover = databases
        .hover(
            &main,
            Position::new(
                4,
                line(main_text, 4)
                    .find("BASE_REWARD")
                    .expect("const use should exist"),
            ),
        )
        .expect("hover should resolve imported const use");

    assert_eq!(const_hover.kind(), HoverKind::Const);
    assert_eq!(const_hover.label(), "game::rewards::BASE_REWARD");
    assert_eq!(const_hover.detail(), "i64");
    assert_eq!(const_hover.docs(), Some("Base reward amount"));
    assert_eq!(
        const_hover.symbol(),
        Some(&SymbolRef::Source("game::rewards::BASE_REWARD".to_owned()))
    );

    let function_hover = databases
        .hover(
            &main,
            Position::new(
                5,
                line(main_text, 5)
                    .find("reward_bonus")
                    .expect("function call should exist"),
            ),
        )
        .expect("hover should resolve imported function call");

    assert_eq!(function_hover.kind(), HoverKind::Function);
    assert_eq!(function_hover.label(), "game::rewards::reward_bonus");
    assert_eq!(function_hover.detail(), "(amount: i64, scale: i64) -> i64");
    assert_eq!(function_hover.docs(), Some("Compute reward bonus"));
    assert_eq!(
        function_hover.symbol(),
        Some(&SymbolRef::Source("game::rewards::reward_bonus".to_owned()))
    );

    let global_hover = databases
        .hover(
            &main,
            Position::new(
                5,
                line(main_text, 5)
                    .find("reward_scale")
                    .expect("global use should exist"),
            ),
        )
        .expect("hover should resolve imported global use");

    assert_eq!(global_hover.kind(), HoverKind::Global);
    assert_eq!(global_hover.label(), "game::rewards::reward_scale");
    assert_eq!(global_hover.detail(), "i64");
    assert_eq!(global_hover.docs(), Some("Current reward scale"));
    assert_eq!(
        global_hover.symbol(),
        Some(&SymbolRef::Source("game::rewards::reward_scale".to_owned()))
    );
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}

fn databases_for_files(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases.set_schema_facts(RegistryFacts::default());
    databases
}
