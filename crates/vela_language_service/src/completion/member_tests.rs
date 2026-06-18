use super::{CompletionContextKind, CompletionItem, CompletionKind, CompletionList};
use crate::{
    DocumentId, LanguageServiceDatabases, LineIndex, SourceFileSnapshot, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};

#[test]
fn member_completion_includes_source_impl_and_trait_methods() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"
struct Player { level: i64 }
trait Rewardable {
    fn preview(self, amount: i64) -> bool { return amount > 0 }
    fn grant(self, amount: i64) -> bool { return amount > 0 }
}
impl Player {
    fn level_up(self, amount: i64) -> bool { return amount > 0 }
}
impl Rewardable for Player {
    fn grant(self, amount: i64) -> bool { return amount > 0 }
}
pub fn main(player: Player, rewardable: Rewardable) {
    player.
}"#;

    let player_completions = completions_for(document.clone(), text, "player.");

    assert_eq!(
        player_completions.context().kind(),
        CompletionContextKind::Member
    );
    assert_completion(&player_completions, "level", CompletionKind::Field);
    assert_completion(&player_completions, "level_up", CompletionKind::Method);
    assert_completion(&player_completions, "grant", CompletionKind::Method);
    assert_completion(&player_completions, "preview", CompletionKind::Method);
    assert_no_completion(&player_completions, "Rewardable");

    let text = r#"
trait Rewardable {
    fn preview(self, amount: i64) -> bool { return amount > 0 }
    fn grant(self, amount: i64) -> bool { return amount > 0 }
}
pub fn main(rewardable: Rewardable) {
    rewardable.
}"#;

    let rewardable_completions = completions_for(document, text, "rewardable.");

    assert_eq!(
        rewardable_completions.context().kind(),
        CompletionContextKind::Member
    );
    assert_completion(&rewardable_completions, "preview", CompletionKind::Method);
    assert_completion(&rewardable_completions, "grant", CompletionKind::Method);
    assert_no_completion(&rewardable_completions, "level_up");
}

#[test]
fn member_completion_uses_schema_function_return_receiver_facts() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main() { current_player(). }";
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    schema.insert_function(
        "current_player",
        TypeFact::function(Vec::new(), TypeFact::host("Player")),
    );
    schema.insert_field("Player", "level", TypeFact::I64);
    schema.insert_method(
        "Player",
        "grant",
        TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
    );
    schema.insert_function(
        "global_grant",
        TypeFact::function(Vec::new(), TypeFact::BOOL),
    );

    let completions = completions_for_with_schema(document, text, "current_player().", schema);

    assert_eq!(completions.context().kind(), CompletionContextKind::Member);
    assert_completion(&completions, "level", CompletionKind::Field);
    assert_completion(&completions, "grant", CompletionKind::Method);
    assert_no_completion(&completions, "current_player");
    assert_no_completion(&completions, "global_grant");
}

fn completions_for(document: DocumentId, text: &str, needle: &str) -> CompletionList {
    completions_for_with_schema(document, text, needle, RegistryFacts::default())
}

fn completions_for_with_schema(
    document: DocumentId,
    text: &str,
    needle: &str,
    schema: RegistryFacts,
) -> CompletionList {
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.set_schema_facts(schema);
    databases.update(&project);
    let line_index = LineIndex::new(text);
    let offset = text.find(needle).expect("completion needle") + needle.len();
    databases.completion_items(&document, line_index.position(offset))
}

fn completion<'a>(list: &'a CompletionList, label: &str) -> &'a CompletionItem {
    list.items()
        .iter()
        .find(|item| item.label() == label)
        .unwrap_or_else(|| panic!("completion {label} should exist in {list:?}"))
}

fn assert_completion(list: &CompletionList, label: &str, kind: CompletionKind) {
    let item = completion(list, label);
    assert_eq!(item.kind(), kind, "{list:?}");
}

fn assert_no_completion(list: &CompletionList, label: &str) {
    assert!(
        list.items().iter().all(|item| item.label() != label),
        "{list:?}"
    );
}
