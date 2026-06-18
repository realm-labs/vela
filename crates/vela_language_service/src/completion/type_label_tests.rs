use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};

use super::{CompletionContextKind, CompletionItem, CompletionKind, CompletionList};
use crate::{
    CompletionSymbol, DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot,
    Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn completion_uses_short_type_labels_with_owner_details() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let reward = DocumentId::from("/workspace/scripts/game/reward.vela");
    let files = vec![
        SourceFileSnapshot::new(main.clone(), "pub fn main() { Re }"),
        SourceFileSnapshot::new(reward, "pub struct Reward { amount: i64 }"),
    ];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    let mut schema = RegistryFacts::default();
    schema.insert_type(
        "game::schema::Region",
        TypeFact::host("game::schema::Region"),
    );
    databases.set_schema_facts(schema);
    databases.update(&project);

    let completions =
        databases.completion_items(&main, Position::new(0, "pub fn main() { Re".len()));

    assert_eq!(
        completions.context().kind(),
        CompletionContextKind::Expression
    );
    let reward = completion(&completions, "Reward");
    assert_eq!(reward.kind(), CompletionKind::Type);
    assert_eq!(reward.lookup(), "game::reward::Reward");
    assert_eq!(reward.filter_text(), "game::reward::Reward");
    assert_eq!(reward.insert_text(), Some("Reward"));
    assert_eq!(reward.label_details().description(), Some("game::reward"));
    assert_eq!(
        reward.symbol(),
        Some(&CompletionSymbol::Source("game::reward::Reward".to_owned()))
    );
    assert_no_completion(&completions, "game::reward::Reward");

    let region = completion(&completions, "Region");
    assert_eq!(region.kind(), CompletionKind::Type);
    assert_eq!(region.lookup(), "game::schema::Region");
    assert_eq!(region.filter_text(), "game::schema::Region");
    assert_eq!(region.insert_text(), Some("Region"));
    assert_eq!(region.label_details().description(), Some("game::schema"));
    assert_eq!(
        region.symbol(),
        Some(&CompletionSymbol::Schema("game::schema::Region".to_owned()))
    );
    assert_no_completion(&completions, "game::schema::Region");
}

fn completion<'a>(list: &'a CompletionList, label: &str) -> &'a CompletionItem {
    list.items()
        .iter()
        .find(|item| item.label() == label)
        .unwrap_or_else(|| panic!("completion {label} should exist in {list:?}"))
}

fn assert_no_completion(list: &CompletionList, label: &str) {
    assert!(
        list.items().iter().all(|item| item.label() != label),
        "{list:?}"
    );
}
