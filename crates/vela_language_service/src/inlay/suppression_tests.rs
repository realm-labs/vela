use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};
use vela_analysis::type_fact::TypeFact;

#[test]
fn inlay_hints_suppress_any_schema_function_parameters() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"pub fn main(player: Player) {
    host_dynamic(player, 10)
    host_stable(player, 10)
    player.grant(player, 10)
}"#;
    let mut databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let mut schema = vela_analysis::registry::RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    schema.insert_function(
        "host_dynamic",
        TypeFact::function(vec![TypeFact::Any, TypeFact::I64], TypeFact::I64),
    );
    schema.insert_function(
        "host_stable",
        TypeFact::function(vec![TypeFact::host("Player"), TypeFact::I64], TypeFact::I64),
    );
    schema.insert_method(
        "Player",
        "grant",
        TypeFact::function(vec![TypeFact::Any, TypeFact::I64], TypeFact::I64),
    );
    databases.set_schema_facts(schema);

    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(0, 0), Position::new(5, 0)),
    );

    assert_eq!(
        hint_labels(&hints),
        vec![
            (Position::new(1, 25), "arg1:".to_owned()),
            (Position::new(2, 16), "arg0:".to_owned()),
            (Position::new(2, 24), "arg1:".to_owned()),
            (Position::new(3, 25), "arg1:".to_owned())
        ]
    );
}

#[test]
fn inlay_hints_suppress_any_source_function_and_method_parameters() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"struct Player { level: i64 }
fn dynamic(raw: Any, count: i64) -> i64 { return count }
impl Player {
    fn grant(self, raw: Any, count: i64) -> i64 { return count }
}
pub fn main(player: Player) {
    dynamic("raw", 1)
    player.grant("raw", 2)
}"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(0, 0), Position::new(9, 0)),
    );

    assert_eq!(
        hint_labels(&hints),
        vec![
            (Position::new(6, 19), "count:".to_owned()),
            (Position::new(7, 24), "count:".to_owned())
        ]
    );
}

#[test]
fn inlay_hints_suppress_any_enum_variant_payloads() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"enum Payload {
    Dynamic(raw: Any, count: i64),
    Stable(name: String, count: i64),
}
pub fn main() {
    Payload::Dynamic("raw", 1)
    Payload::Stable("ok", 2)
}"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(0, 0), Position::new(8, 0)),
    );

    assert_eq!(
        hint_labels(&hints),
        vec![
            (Position::new(5, 28), "count:".to_owned()),
            (Position::new(6, 20), "name:".to_owned()),
            (Position::new(6, 26), "count:".to_owned())
        ]
    );
}

fn hint_labels(hints: &[InlayHint]) -> Vec<(Position, String)> {
    hints
        .iter()
        .map(|hint| (hint.position(), hint.label().to_owned()))
        .collect()
}

fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases
}
