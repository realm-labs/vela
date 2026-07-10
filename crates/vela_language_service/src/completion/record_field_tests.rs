use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;
use vela_hir::body::HirBodyOwner;

use super::*;
use crate::{
    QueryContext, SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};

#[test]
fn record_field_completion_uses_root_hir_body() {
    let (databases, document, text) = record_project(
        "pub struct Player { id: String level: i64 }\n\
         pub fn main() { let player = Player { id: \"p1\", le } }",
    );
    let position = position_at(&text, 1, "le }");
    let query = QueryContext::from_databases(&databases, &document, position).expect("query");

    assert!(matches!(
        query.body().map(|body| &body.owner),
        Some(HirBodyOwner::Declaration(_))
    ));
    assert_hir_record_at_query(&query);

    let completions = databases.completion_items(&document, position);
    assert_record_completions(&completions);
}

#[test]
fn record_field_completion_uses_nested_lambda_hir_body() {
    let (databases, document, text) = record_project(
        "pub struct Player { id: String level: i64 }\n\
         pub fn main() { let make = |value| Player { id: value, le }; }",
    );
    let position = position_at(&text, 1, "le }");
    let query = QueryContext::from_databases(&databases, &document, position).expect("query");

    assert!(matches!(
        query.body().map(|body| &body.owner),
        Some(HirBodyOwner::Lambda { .. })
    ));
    assert_hir_record_at_query(&query);

    let completions = databases.completion_items(&document, position);
    assert_record_completions(&completions);
}

#[test]
fn record_field_completion_uses_parameter_default_hir_body() {
    let (databases, document, text) = record_project(
        "pub struct Player { id: String level: i64 }\n\
         pub fn main(player = Player { id: \"p1\", le }) { return player }",
    );
    let position = position_at(&text, 1, "le }");
    let query = QueryContext::from_databases(&databases, &document, position).expect("query");

    assert!(matches!(
        query.body().map(|body| &body.owner),
        Some(HirBodyOwner::ParameterDefault { .. })
    ));
    assert_hir_record_at_query(&query);

    let completions = databases.completion_items(&document, position);
    assert_record_completions(&completions);
}

#[test]
fn malformed_record_field_completion_uses_syntax_recovery() {
    let (databases, document, text) = record_project(
        "pub struct Player { id: String level: i64 }\n\
         pub fn () { let player = Player { id: \"p1\", le } }",
    );
    let position = position_at(&text, 1, "le }");
    let query = QueryContext::from_databases(&databases, &document, position).expect("query");

    assert!(
        query.body().is_none(),
        "a function without a name should not lower an executable HIR body"
    );

    let completions = databases.completion_items(&document, position);
    assert_record_completions(&completions);
}

#[test]
fn record_field_completion_requires_known_type() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn helper() { return 1 }\npub fn main() { let player = Missing { le } }";
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);

    let completions = databases.completion_items(&document, position_at(text, 1, "le }"));

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
    schema.insert_field_docs("Player", "level", "Current player level.");
    schema.insert_field("Player", "name", TypeFact::STRING);
    databases.set_schema_facts(schema);
    databases.update(&project);

    let completions = databases.completion_items(&document, position_at(text, 0, "le }"));

    assert_eq!(
        completions.context().kind(),
        CompletionContextKind::RecordField
    );
    assert_completion(&completions, "level", CompletionKind::Field);
    assert_no_completion(&completions, "name");
    let level = completion(&completions, "level");
    assert_eq!(level.documentation(), None);
    assert_eq!(
        level.symbol(),
        Some(&CompletionSymbol::Schema("Player.level".to_owned()))
    );
}

#[test]
fn record_field_completion_finds_constructor_nested_in_index() {
    let (databases, document, text) = record_project(
        "pub struct Player { id: String level: i64 }\n\
         pub fn main(players: Array<i64>) { let value = players[Player { le }]; }",
    );
    let position = position_at(&text, 1, "le }");

    let completions = databases.completion_items(&document, position);

    assert_record_completions(&completions);
}

fn record_project(text: &str) -> (LanguageServiceDatabases, DocumentId, String) {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let files = vec![SourceFileSnapshot::new(document.clone(), text)];
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    (databases, document, text.to_owned())
}

fn position_at(text: &str, line: usize, needle: &str) -> Position {
    Position::new(
        line,
        text.lines()
            .nth(line)
            .expect("completion line")
            .find(needle)
            .expect("record field prefix")
            + "le".len(),
    )
}

fn assert_hir_record_at_query(query: &QueryContext<'_>) {
    let body = query.body().expect("production query HIR body");
    let source = query.source_id().expect("query source");
    let offset = query.cursor().replace_range().end;
    let constructor = record_field::hir_record_constructor_at(body, source, offset)
        .expect("selected production HIR body should own the record constructor");
    assert_eq!(constructor.path, ["Player"]);
}

fn assert_record_completions(completions: &CompletionList) {
    assert_eq!(
        completions.context().kind(),
        CompletionContextKind::RecordField
    );
    assert_completion(completions, "level", CompletionKind::Field);
    assert_no_completion(completions, "id");
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
