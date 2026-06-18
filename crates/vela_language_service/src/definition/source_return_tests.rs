use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn definition_follows_source_trait_default_method_on_source_function_return_receiver() {
    assert_source_trait_default_method_navigation_on_source_function_return_receiver(
        NavigationKind::Definition,
    );
}

#[test]
fn declaration_follows_source_trait_default_method_on_source_function_return_receiver() {
    assert_source_trait_default_method_navigation_on_source_function_return_receiver(
        NavigationKind::Declaration,
    );
}

#[derive(Clone, Copy)]
enum NavigationKind {
    Definition,
    Declaration,
}

fn assert_source_trait_default_method_navigation_on_source_function_return_receiver(
    kind: NavigationKind,
) {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"trait Rewardable {
    #[doc("Preview reward")]
    fn preview(self, amount: i64) -> bool { return amount > 0 }
}
struct Player {
    level: i64,
}
impl Rewardable for Player {}
fn current_player() -> Player { return Player { level: 1 } }
pub fn main() {
    return current_player().preview(1)
}"#;
    let call_line = line(text, 10);
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let position = Position::new(
        10,
        call_line
            .find("preview")
            .expect("trait default method call should exist"),
    );

    let definition = match kind {
        NavigationKind::Definition => databases.definition(&document, position),
        NavigationKind::Declaration => databases.declaration(&document, position),
    }
    .expect("navigation should resolve trait default method declaration");

    assert_eq!(definition.document_id(), &document);
    assert_eq!(definition.range().start().line, 2);
    assert_eq!(
        definition.range().start().character,
        line(text, 2)
            .find("preview")
            .expect("trait method declaration should exist")
    );
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Source("game::main::Rewardable.preview".into()))
    );
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}

fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);
    databases
}
