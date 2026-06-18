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

#[test]
fn definition_follows_source_method_on_source_function_return_receiver() {
    assert_source_method_navigation_on_source_function_return_receiver(NavigationKind::Definition);
}

#[test]
fn declaration_follows_source_method_on_source_function_return_receiver() {
    assert_source_method_navigation_on_source_function_return_receiver(NavigationKind::Declaration);
}

#[test]
fn definition_follows_source_method_on_source_method_return_receiver() {
    assert_source_method_navigation_on_source_method_return_receiver(NavigationKind::Definition);
}

#[test]
fn declaration_follows_source_method_on_source_method_return_receiver() {
    assert_source_method_navigation_on_source_method_return_receiver(NavigationKind::Declaration);
}

#[test]
fn definition_follows_source_trait_default_method_on_source_method_return_receiver() {
    assert_source_trait_default_method_navigation_on_source_method_return_receiver(
        NavigationKind::Definition,
    );
}

#[test]
fn declaration_follows_source_trait_default_method_on_source_method_return_receiver() {
    assert_source_trait_default_method_navigation_on_source_method_return_receiver(
        NavigationKind::Declaration,
    );
}

#[derive(Clone, Copy)]
enum NavigationKind {
    Definition,
    Declaration,
}

fn assert_source_method_navigation_on_source_function_return_receiver(kind: NavigationKind) {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"struct Player {
    level: i64,
}
impl Player {
    fn grant(self, amount: i64) -> bool {
        return amount > 0
    }
}
fn current_player() -> Player { return Player { level: 1 } }
pub fn main() {
    return current_player().grant(3)
}"#;
    assert_navigation(
        kind,
        &document,
        text,
        NavigationExpectation {
            call_line: 10,
            call_name: "grant",
            declaration_line: 4,
            declaration_name: "grant",
            symbol: "game::main::Player.grant",
        },
    );
}

fn assert_source_method_navigation_on_source_method_return_receiver(kind: NavigationKind) {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = r#"struct Player {
    level: i64,
}
struct Inventory {
    slots: i64,
}
impl Player {
    fn inventory(self) -> Inventory { return Inventory { slots: 1 } }
}
impl Inventory {
    fn grant(self, amount: i64) -> bool {
        return amount > 0
    }
}
pub fn main(player: Player) {
    return player.inventory().grant(3)
}"#;
    assert_navigation(
        kind,
        &document,
        text,
        NavigationExpectation {
            call_line: 15,
            call_name: "grant",
            declaration_line: 10,
            declaration_name: "grant",
            symbol: "game::main::Inventory.grant",
        },
    );
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
    assert_navigation(
        kind,
        &document,
        text,
        NavigationExpectation {
            call_line: 10,
            call_name: "preview",
            declaration_line: 2,
            declaration_name: "preview",
            symbol: "game::main::Rewardable.preview",
        },
    );
}

fn assert_source_trait_default_method_navigation_on_source_method_return_receiver(
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
struct Inventory {
    slots: i64,
}
impl Player {
    fn inventory(self) -> Inventory { return Inventory { slots: 1 } }
}
impl Rewardable for Inventory {}
pub fn main(player: Player) {
    return player.inventory().preview(1)
}"#;
    assert_navigation(
        kind,
        &document,
        text,
        NavigationExpectation {
            call_line: 15,
            call_name: "preview",
            declaration_line: 2,
            declaration_name: "preview",
            symbol: "game::main::Rewardable.preview",
        },
    );
}

struct NavigationExpectation {
    call_line: usize,
    call_name: &'static str,
    declaration_line: usize,
    declaration_name: &'static str,
    symbol: &'static str,
}

fn assert_navigation(
    kind: NavigationKind,
    document: &DocumentId,
    text: &str,
    expectation: NavigationExpectation,
) {
    let call_line = line(text, expectation.call_line);
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
    let position = Position::new(
        expectation.call_line,
        call_line
            .find(expectation.call_name)
            .unwrap_or_else(|| panic!("{} call should exist", expectation.call_name)),
    );

    let definition = match kind {
        NavigationKind::Definition => databases.definition(document, position),
        NavigationKind::Declaration => databases.declaration(document, position),
    }
    .unwrap_or_else(|| panic!("navigation should resolve {}", expectation.call_name));

    assert_eq!(definition.document_id(), document);
    assert_eq!(
        definition.range().start().line,
        expectation.declaration_line
    );
    assert_eq!(
        definition.range().start().character,
        line(text, expectation.declaration_line)
            .find(expectation.declaration_name)
            .unwrap_or_else(|| panic!("{} declaration should exist", expectation.declaration_name))
    );
    assert_eq!(
        definition.symbol(),
        Some(&SymbolRef::Source(expectation.symbol.to_owned()))
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
