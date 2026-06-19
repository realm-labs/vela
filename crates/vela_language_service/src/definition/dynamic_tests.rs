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

#[test]
fn definition_returns_none_for_source_any_return_receiver_member() {
    assert_source_any_return_receiver_navigation_none(NavigationKind::Definition);
}

#[test]
fn declaration_returns_none_for_source_any_return_receiver_member() {
    assert_source_any_return_receiver_navigation_none(NavigationKind::Declaration);
}

#[test]
fn type_definition_returns_none_for_source_any_return_receiver_member() {
    assert_source_any_return_receiver_navigation_none(NavigationKind::TypeDefinition);
}

#[derive(Clone, Copy)]
enum NavigationKind {
    Definition,
    Declaration,
    TypeDefinition,
}

fn assert_source_any_return_receiver_navigation_none(kind: NavigationKind) {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"
struct Player { level: i64 }
fn source_any() -> Any { return Player { level: 1 } }
pub fn main() { return source_any().level }"#;
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let use_line = text.lines().nth(3).expect("member use line should exist");
    let position = Position::new(3, use_line.find("level").expect("member use"));

    let result = match kind {
        NavigationKind::Definition => databases.definition(&document, position),
        NavigationKind::Declaration => databases.declaration(&document, position),
        NavigationKind::TypeDefinition => databases.type_definition(&document, position),
    };

    assert!(
        result.is_none(),
        "source Any return receivers must not invent member navigation facts"
    );
}

fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases
}
