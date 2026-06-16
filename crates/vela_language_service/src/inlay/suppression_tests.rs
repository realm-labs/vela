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
    databases.set_schema_facts(schema);

    let hints = databases.inlay_hints(
        &document,
        DiagnosticRange::new(Position::new(0, 0), Position::new(4, 0)),
    );

    assert_eq!(
        hint_labels(&hints),
        vec![
            (Position::new(1, 25), "arg1:".to_owned()),
            (Position::new(2, 16), "arg0:".to_owned()),
            (Position::new(2, 24), "arg1:".to_owned())
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
