use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn type_definition_returns_none_for_dynamic_receiver_member() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"fn main(value: Any) {
return value.level;
}"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let use_line = text.lines().nth(1).expect("member use line");

    let definition = databases.type_definition(
        &document,
        Position::new(1, use_line.find("level").expect("dynamic member use")),
    );

    assert!(definition.is_none());
}

fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases
}
