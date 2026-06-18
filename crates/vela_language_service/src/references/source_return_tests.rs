use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn references_find_source_method_calls_on_source_function_return_receivers() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Player {
    level: i64
}

impl Player {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

fn current_player() -> Player { return Player { level: 1 } }

pub fn main() -> i64 {
    let first = current_player().grant(1)
    return current_player().grant(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let references = databases.references(
        &document,
        Position::new(
            11,
            line(text, 11)
                .find("grant")
                .expect("first returned receiver method call"),
        ),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        &references,
        5,
        line(text, 5).find("grant").expect("method declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &references,
        11,
        line(text, 11).find("grant").expect("first method call"),
        ReferenceKind::Call,
    );
    assert_reference(
        &references,
        12,
        line(text, 12).find("grant").expect("second method call"),
        ReferenceKind::Call,
    );
    assert_all_symbols(
        &references,
        &SymbolRef::Source("game::main::Player.grant".into()),
    );
}

#[test]
fn references_find_source_trait_default_method_calls_on_source_function_return_receivers() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub trait Rewardable {
    fn grant(self, amount: i64) -> i64 { return amount }
}

pub struct Player {
    level: i64
}

impl Rewardable for Player {}

fn current_player() -> Player { return Player { level: 1 } }

pub fn main() -> i64 {
    let first = current_player().grant(1)
    return current_player().grant(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let references = databases.references(
        &document,
        Position::new(
            13,
            line(text, 13)
                .find("grant")
                .expect("first returned receiver trait method call"),
        ),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        &references,
        1,
        line(text, 1)
            .find("grant")
            .expect("trait method declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &references,
        13,
        line(text, 13)
            .find("grant")
            .expect("first trait method call"),
        ReferenceKind::Call,
    );
    assert_reference(
        &references,
        14,
        line(text, 14)
            .find("grant")
            .expect("second trait method call"),
        ReferenceKind::Call,
    );
    assert_all_symbols(
        &references,
        &SymbolRef::Source("game::main::Rewardable.grant".into()),
    );
}

#[test]
fn references_find_source_method_calls_on_source_method_return_receivers() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Player {
    level: i64
}

pub struct Inventory {
    count: i64
}

impl Player {
    pub fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}

impl Inventory {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

pub fn main(player: Player) -> i64 {
    let first = player.inventory().grant(1)
    return player.inventory().grant(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let references = databases.references(
        &document,
        Position::new(
            17,
            line(text, 17)
                .find("grant")
                .expect("first method-return receiver method call"),
        ),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        &references,
        13,
        line(text, 13).find("grant").expect("method declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &references,
        17,
        line(text, 17).find("grant").expect("first method call"),
        ReferenceKind::Call,
    );
    assert_reference(
        &references,
        18,
        line(text, 18).find("grant").expect("second method call"),
        ReferenceKind::Call,
    );
    assert_all_symbols(
        &references,
        &SymbolRef::Source("game::main::Inventory.grant".into()),
    );
}

#[test]
fn references_find_source_trait_default_method_calls_on_source_method_return_receivers() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub trait Rewardable {
    fn preview(self, amount: i64) -> i64 { return amount }
}

pub struct Player {
    level: i64
}

pub struct Inventory {
    count: i64
}

impl Player {
    pub fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}

impl Rewardable for Inventory {}

pub fn main(player: Player) -> i64 {
    let first = player.inventory().preview(1)
    return player.inventory().preview(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let references = databases.references(
        &document,
        Position::new(
            19,
            line(text, 19)
                .find("preview")
                .expect("first method-return receiver trait method call"),
        ),
        true,
    );

    assert_eq!(references.len(), 3, "{references:?}");
    assert_reference(
        &references,
        1,
        line(text, 1)
            .find("preview")
            .expect("trait method declaration"),
        ReferenceKind::Declaration,
    );
    assert_reference(
        &references,
        19,
        line(text, 19)
            .find("preview")
            .expect("first trait method call"),
        ReferenceKind::Call,
    );
    assert_reference(
        &references,
        20,
        line(text, 20)
            .find("preview")
            .expect("second trait method call"),
        ReferenceKind::Call,
    );
    assert_all_symbols(
        &references,
        &SymbolRef::Source("game::main::Rewardable.preview".into()),
    );
}

#[test]
fn document_highlight_marks_source_method_calls_on_source_function_return_receivers() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Player {
    level: i64
}

impl Player {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

fn current_player() -> Player { return Player { level: 1 } }

pub fn main() -> i64 {
    let first = current_player().grant(1)
    return current_player().grant(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let highlights = databases.document_highlights(
        &document,
        Position::new(
            11,
            line(text, 11)
                .find("grant")
                .expect("first returned receiver method call"),
        ),
    );

    assert_eq!(highlights.len(), 3, "{highlights:?}");
    assert_highlight(
        &highlights,
        5,
        line(text, 5).find("grant").expect("method declaration"),
        DocumentHighlightKind::Text,
    );
    assert_highlight(
        &highlights,
        11,
        line(text, 11).find("grant").expect("first method call"),
        DocumentHighlightKind::Call,
    );
    assert_highlight(
        &highlights,
        12,
        line(text, 12).find("grant").expect("second method call"),
        DocumentHighlightKind::Call,
    );
}

#[test]
fn document_highlight_marks_source_trait_default_method_calls_on_source_function_return_receivers()
{
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub trait Rewardable {
    fn grant(self, amount: i64) -> i64 { return amount }
}

pub struct Player {
    level: i64
}

impl Rewardable for Player {}

fn current_player() -> Player { return Player { level: 1 } }

pub fn main() -> i64 {
    let first = current_player().grant(1)
    return current_player().grant(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let highlights = databases.document_highlights(
        &document,
        Position::new(
            13,
            line(text, 13)
                .find("grant")
                .expect("first returned receiver trait method call"),
        ),
    );

    assert_eq!(highlights.len(), 3, "{highlights:?}");
    assert_highlight(
        &highlights,
        1,
        line(text, 1)
            .find("grant")
            .expect("trait method declaration"),
        DocumentHighlightKind::Text,
    );
    assert_highlight(
        &highlights,
        13,
        line(text, 13)
            .find("grant")
            .expect("first trait method call"),
        DocumentHighlightKind::Call,
    );
    assert_highlight(
        &highlights,
        14,
        line(text, 14)
            .find("grant")
            .expect("second trait method call"),
        DocumentHighlightKind::Call,
    );
}

#[test]
fn document_highlight_marks_source_method_calls_on_source_method_return_receivers() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Player {
    level: i64
}

pub struct Inventory {
    count: i64
}

impl Player {
    pub fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}

impl Inventory {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

pub fn main(player: Player) -> i64 {
    let first = player.inventory().grant(1)
    return player.inventory().grant(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let highlights = databases.document_highlights(
        &document,
        Position::new(
            17,
            line(text, 17)
                .find("grant")
                .expect("first method-return receiver method call"),
        ),
    );

    assert_eq!(highlights.len(), 3, "{highlights:?}");
    assert_highlight(
        &highlights,
        13,
        line(text, 13).find("grant").expect("method declaration"),
        DocumentHighlightKind::Text,
    );
    assert_highlight(
        &highlights,
        17,
        line(text, 17).find("grant").expect("first method call"),
        DocumentHighlightKind::Call,
    );
    assert_highlight(
        &highlights,
        18,
        line(text, 18).find("grant").expect("second method call"),
        DocumentHighlightKind::Call,
    );
}

#[test]
fn document_highlight_marks_source_trait_default_method_calls_on_source_method_return_receivers() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub trait Rewardable {
    fn preview(self, amount: i64) -> i64 { return amount }
}

pub struct Player {
    level: i64
}

pub struct Inventory {
    count: i64
}

impl Player {
    pub fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}

impl Rewardable for Inventory {}

pub fn main(player: Player) -> i64 {
    let first = player.inventory().preview(1)
    return player.inventory().preview(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let highlights = databases.document_highlights(
        &document,
        Position::new(
            19,
            line(text, 19)
                .find("preview")
                .expect("first method-return receiver trait method call"),
        ),
    );

    assert_eq!(highlights.len(), 3, "{highlights:?}");
    assert_highlight(
        &highlights,
        1,
        line(text, 1)
            .find("preview")
            .expect("trait method declaration"),
        DocumentHighlightKind::Text,
    );
    assert_highlight(
        &highlights,
        19,
        line(text, 19)
            .find("preview")
            .expect("first trait method call"),
        DocumentHighlightKind::Call,
    );
    assert_highlight(
        &highlights,
        20,
        line(text, 20)
            .find("preview")
            .expect("second trait method call"),
        DocumentHighlightKind::Call,
    );
}

fn assert_all_symbols(references: &[Reference], symbol: &SymbolRef) {
    assert!(
        references
            .iter()
            .all(|reference| reference.symbol() == symbol),
        "{references:?}"
    );
}

fn assert_reference(references: &[Reference], line: usize, character: usize, kind: ReferenceKind) {
    assert!(
        references.iter().any(|reference| {
            reference.range().start() == Position::new(line, character) && reference.kind() == kind
        }),
        "{references:?}"
    );
}

fn assert_highlight(
    highlights: &[DocumentHighlight],
    line: usize,
    character: usize,
    kind: DocumentHighlightKind,
) {
    assert!(
        highlights.iter().any(|highlight| {
            highlight.range().start() == Position::new(line, character) && highlight.kind() == kind
        }),
        "{highlights:?}"
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
