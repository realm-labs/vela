use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;

fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases
}

#[test]
fn inlay_hints_show_parameter_names() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn grant(amount: i64, reason: String) -> i64 { return amount }\npub fn main() { return grant(10, \"quest\") }";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(1, 0), Position::new(1, 80)),
    );

    assert_eq!(
        hint_labels(&hints),
        vec![
            (Position::new(1, 29), "amount:".to_owned()),
            (Position::new(1, 33), "reason:".to_owned())
        ]
    );
    assert!(
        hints
            .iter()
            .all(|hint| hint.kind() == InlayHintKind::Parameter)
    );
}

#[test]
fn inlay_hints_skip_named_arguments_and_unknown_calls() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn grant(amount: i64) -> i64 { return amount }\npub fn main() { return grant(amount = 10) + missing(1) }";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(1, 0), Position::new(1, 90)),
    );

    assert!(hints.is_empty());
}

#[test]
fn inlay_hints_show_source_method_parameter_names() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"struct Player { level: i64 }
impl Player {
    fn grant(self, amount: i64, bonus: i64) -> i64 { return amount + bonus }
}
pub fn main(player: Player) { player.grant(1, 2) }"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let main_line = text.lines().nth(4).expect("main line should exist");

    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(0, 0), Position::new(4, main_line.len())),
    );

    assert_eq!(
        hint_labels(&hints),
        vec![
            (
                Position::new(4, main_line.find("1,").expect("first arg")),
                "amount:".to_owned(),
            ),
            (
                Position::new(4, main_line.find("2)").expect("second arg")),
                "bonus:".to_owned(),
            )
        ]
    );
    assert!(
        hints
            .iter()
            .all(|hint| hint.kind() == InlayHintKind::Parameter)
    );
}

#[test]
fn inlay_hints_show_stable_local_typefacts() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"const BONUS: i64 = 10
pub fn main() {
    let total = 1 + 2;
    let next = total + 1;
    let scripted = BONUS;
    let explicit: i64 = 3;
    let dynamic = host_any();
}"#;
    let mut databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let mut schema = vela_analysis::registry::RegistryFacts::default();
    schema.insert_function("host_any", TypeFact::function(Vec::new(), TypeFact::Any));
    databases.set_schema_facts(schema);

    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(0, 0), Position::new(7, 0)),
    );

    assert_eq!(
        hint_labels(&hints),
        vec![
            (Position::new(2, 13), ": i64".to_owned()),
            (Position::new(3, 12), ": i64".to_owned()),
            (Position::new(4, 16), ": i64".to_owned())
        ]
    );
    assert!(hints.iter().all(|hint| hint.kind() == InlayHintKind::Type));
}

#[test]
fn inlay_hints_show_lambda_parameter_facts() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"pub fn main() {
    let scores: Array<i64> = [1, 2, 3];
    let doubled: Array<i64> = scores.map(|score| score + 1);
    let rewards: Map<String, i64> = {"gold": 1};
    let mapped: Map<String, i64> = rewards.map_values(|value| value + 1);
    let filtered: Map<String, i64> = rewards.filter(|key, value| key.len() > value);
}"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(0, 0), Position::new(7, 0)),
    );

    assert_eq!(
        hint_labels(&hints),
        vec![
            (Position::new(2, 47), ": i64".to_owned()),
            (Position::new(4, 60), ": i64".to_owned()),
            (Position::new(5, 56), ": String".to_owned()),
            (Position::new(5, 63), ": i64".to_owned())
        ]
    );
    assert!(hints.iter().all(|hint| hint.kind() == InlayHintKind::Type));
}

#[test]
fn inlay_hints_show_host_path_typefacts() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"pub fn main(player: Player) {
    let next = player.level + 1;
    player.level += next;
    let dynamic = player.mystery;
    player.grant(next);
}"#;
    let mut databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let mut schema = vela_analysis::registry::RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    schema.insert_field("Player", "level", TypeFact::I64);
    schema.insert_field("Player", "mystery", TypeFact::Any);
    schema.insert_method(
        "Player",
        "grant",
        TypeFact::function(vec![TypeFact::I64], TypeFact::I64),
    );
    databases.set_schema_facts(schema);

    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(0, 0), Position::new(6, 0)),
    );

    assert_eq!(
        hint_labels(&hints),
        vec![
            (Position::new(1, 12), ": i64".to_owned()),
            (Position::new(1, 27), ": i64".to_owned()),
            (Position::new(2, 16), ": i64".to_owned()),
            (Position::new(4, 17), "arg0:".to_owned())
        ]
    );
}

#[test]
fn inlay_hints_show_enum_variant_payload_names() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"enum QuestProgress {
    Active(quest_id: String, count: i64),
    Done,
}
pub fn main() {
    let active = QuestProgress::Active("quest-1", 3);
}"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(0, 0), Position::new(7, 0)),
    );

    assert_eq!(
        hint_labels(&hints),
        vec![
            (Position::new(5, 39), "quest_id:".to_owned()),
            (Position::new(5, 50), "count:".to_owned())
        ]
    );
    assert!(
        hints
            .iter()
            .all(|hint| hint.kind() == InlayHintKind::Parameter)
    );
}

#[test]
fn inlay_hints_show_schema_tuple_variant_payload_names() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"pub fn main() { QuestState::Active("quest-1", 3) }"#;
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(
        &config,
        &[SourceFileSnapshot::new(document.clone(), text)],
        &Workspace::new().snapshot(),
    );
    let mut databases = LanguageServiceDatabases::new();
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
    schema.insert_field("QuestState::Active", "0", TypeFact::STRING);
    schema.insert_field("QuestState::Active", "1", TypeFact::I64);
    databases.set_schema_facts(schema);
    databases.update(&project);

    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(0, 0), Position::new(0, text.len())),
    );

    assert_eq!(
        hint_labels(&hints),
        vec![
            (
                Position::new(0, text.find("\"quest-1\"").expect("first argument")),
                "arg0:".to_owned()
            ),
            (
                Position::new(0, text.find(", 3").expect("second argument") + 2),
                "arg1:".to_owned()
            )
        ]
    );
    assert!(
        hints
            .iter()
            .all(|hint| hint.kind() == InlayHintKind::Parameter)
    );
}

#[test]
fn inlay_hints_degrade_to_any_without_schema() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main() { return host_grant(10) }";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(0, 0), Position::new(0, 80)),
    );

    assert!(hints.is_empty());
}

#[test]
fn inlay_hints_use_schema_function_names() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main() { return host_grant(10) }";
    let mut databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let mut schema = vela_analysis::registry::RegistryFacts::default();
    schema.insert_function(
        "host_grant",
        TypeFact::function(vec![TypeFact::I64], TypeFact::I64),
    );
    databases.set_schema_facts(schema);

    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(0, 0), Position::new(0, 80)),
    );

    assert_eq!(
        hint_labels(&hints),
        vec![(Position::new(0, 34), "arg0:".to_owned())]
    );
}

fn hint_labels(hints: &[InlayHint]) -> Vec<(Position, String)> {
    hints
        .iter()
        .map(|hint| (hint.position(), hint.label().to_owned()))
        .collect()
}
