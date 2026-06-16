use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

#[test]
fn call_hierarchy_uses_resolved_call_graph() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
    let main_text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    let first = grant(amount)
    return grant(first)
}";
    let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(helper.clone(), helper_text),
    ]);

    let prepared = databases.prepare_call_hierarchy(
        &helper,
        Position::new(0, helper_text.find("grant").expect("grant declaration")),
    );

    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].name(), "grant");
    assert_eq!(prepared[0].document_id(), &helper);

    let incoming = databases.incoming_calls(&prepared[0]);
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].from().name(), "main");
    assert_eq!(incoming[0].from().document_id(), &main);
    assert_eq!(incoming[0].from_ranges().len(), 2);
    assert_range(
        incoming[0].from_ranges(),
        2,
        line(main_text, 2).find("grant").expect("first call"),
    );
    assert_range(
        incoming[0].from_ranges(),
        3,
        line(main_text, 3).find("grant").expect("second call"),
    );

    let main_item = databases
        .prepare_call_hierarchy(
            &main,
            Position::new(
                1,
                line(main_text, 1).find("main").expect("main declaration"),
            ),
        )
        .pop()
        .expect("main should prepare a call hierarchy item");
    let outgoing = databases.outgoing_calls(&main_item);
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].to().name(), "grant");
    assert_eq!(outgoing[0].to().document_id(), &helper);
    assert_eq!(outgoing[0].from_ranges().len(), 2);
}

#[test]
fn call_hierarchy_uses_resolved_script_method_calls() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub struct Reward {
    amount: i64
}

impl Reward {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

pub fn main(reward: Reward) -> i64 {
    let first = reward.grant(1)
    return reward.grant(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(main.clone(), text)]);

    let prepared_from_declaration = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            5,
            line(text, 5)
                .find("grant")
                .expect("method declaration should exist"),
        ),
    );
    let prepared_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            9,
            line(text, 9)
                .find("grant")
                .expect("method call should exist"),
        ),
    );

    assert_eq!(prepared_from_declaration.len(), 1);
    assert_eq!(prepared_from_declaration[0].name(), "grant");
    assert_eq!(prepared_from_declaration[0].document_id(), &main);
    assert_eq!(prepared_from_call, prepared_from_declaration);

    let incoming = databases.incoming_calls(&prepared_from_declaration[0]);
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].from().name(), "main");
    assert_eq!(incoming[0].from().document_id(), &main);
    assert_eq!(incoming[0].from_ranges().len(), 2);
    assert_range(
        incoming[0].from_ranges(),
        9,
        line(text, 9).find("grant").expect("first method call"),
    );
    assert_range(
        incoming[0].from_ranges(),
        10,
        line(text, 10).find("grant").expect("second method call"),
    );

    let main_item = databases
        .prepare_call_hierarchy(
            &main,
            Position::new(8, line(text, 8).find("main").expect("main declaration")),
        )
        .pop()
        .expect("main should prepare a call hierarchy item");
    let outgoing = databases.outgoing_calls(&main_item);
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].to().name(), "grant");
    assert_eq!(outgoing[0].to().document_id(), &main);
    assert_eq!(outgoing[0].from_ranges().len(), 2);
    assert_range(
        outgoing[0].from_ranges(),
        9,
        line(text, 9).find("grant").expect("first method call"),
    );
    assert_range(
        outgoing[0].from_ranges(),
        10,
        line(text, 10).find("grant").expect("second method call"),
    );
}

#[test]
fn call_hierarchy_uses_resolved_trait_impl_method_calls() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn clamp(value: i64) -> i64 { return value }

pub trait Rewardable {
    fn grant(self, amount: i64) -> i64;
}

pub struct Player { level: i64 }

impl Rewardable for Player {
    fn grant(self, amount: i64) -> i64 { return clamp(amount) }
}

pub fn main(player: Player) -> i64 {
    let first = player.grant(1)
    return player.grant(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(main.clone(), text)]);

    let prepared_from_declaration = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            9,
            line(text, 9)
                .find("grant")
                .expect("trait impl method declaration should exist"),
        ),
    );
    let prepared_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            13,
            line(text, 13)
                .find("grant")
                .expect("trait impl method call should exist"),
        ),
    );

    assert_eq!(prepared_from_declaration.len(), 1);
    assert_eq!(prepared_from_declaration[0].name(), "grant");
    assert_eq!(prepared_from_declaration[0].document_id(), &main);
    assert_eq!(prepared_from_call, prepared_from_declaration);

    let incoming = databases.incoming_calls(&prepared_from_declaration[0]);
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].from().name(), "main");
    assert_eq!(incoming[0].from().document_id(), &main);
    assert_eq!(incoming[0].from_ranges().len(), 2);
    assert_range(
        incoming[0].from_ranges(),
        13,
        line(text, 13)
            .find("grant")
            .expect("first trait method call"),
    );
    assert_range(
        incoming[0].from_ranges(),
        14,
        line(text, 14)
            .find("grant")
            .expect("second trait method call"),
    );

    let outgoing = databases.outgoing_calls(&prepared_from_declaration[0]);
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].to().name(), "clamp");
    assert_eq!(outgoing[0].to().document_id(), &main);
    assert_eq!(outgoing[0].from_ranges().len(), 1);
    assert_range(
        outgoing[0].from_ranges(),
        9,
        line(text, 9).find("clamp").expect("helper call"),
    );
}

#[test]
fn call_hierarchy_uses_trait_default_and_interface_methods() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn clamp(value: i64) -> i64 { return value }

pub trait Rewardable {
    fn grant(self, amount: i64) -> i64 { return clamp(amount) }
    fn preview(self, amount: i64) -> i64;
}

pub fn main(rewardable: Rewardable) -> i64 {
    let first = rewardable.grant(1)
    return rewardable.preview(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(main.clone(), text)]);

    let grant_from_declaration = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            3,
            line(text, 3)
                .find("grant")
                .expect("trait default method declaration"),
        ),
    );
    let grant_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            8,
            line(text, 8)
                .find("grant")
                .expect("trait default method call"),
        ),
    );
    let preview = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            4,
            line(text, 4)
                .find("preview")
                .expect("trait interface method declaration"),
        ),
    );

    assert_eq!(grant_from_declaration.len(), 1);
    assert_eq!(grant_from_declaration[0].name(), "grant");
    assert_eq!(grant_from_declaration[0].document_id(), &main);
    assert_eq!(grant_from_call, grant_from_declaration);
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].name(), "preview");
    assert_eq!(preview[0].document_id(), &main);

    let grant_incoming = databases.incoming_calls(&grant_from_declaration[0]);
    assert_eq!(grant_incoming.len(), 1);
    assert_eq!(grant_incoming[0].from().name(), "main");
    assert_range(
        grant_incoming[0].from_ranges(),
        8,
        line(text, 8)
            .find("grant")
            .expect("trait default method call"),
    );

    let grant_outgoing = databases.outgoing_calls(&grant_from_declaration[0]);
    assert_eq!(grant_outgoing.len(), 1);
    assert_eq!(grant_outgoing[0].to().name(), "clamp");
    assert_range(
        grant_outgoing[0].from_ranges(),
        3,
        line(text, 3)
            .find("clamp")
            .expect("default method helper call"),
    );

    let preview_incoming = databases.incoming_calls(&preview[0]);
    assert_eq!(preview_incoming.len(), 1);
    assert_eq!(preview_incoming[0].from().name(), "main");
    assert_range(
        preview_incoming[0].from_ranges(),
        9,
        line(text, 9)
            .find("preview")
            .expect("trait interface method call"),
    );
    assert!(databases.outgoing_calls(&preview[0]).is_empty());
}

fn assert_range(ranges: &[DiagnosticRange], line: usize, character: usize) {
    assert!(
        ranges
            .iter()
            .any(|range| range.start().line == line && range.start().character == character),
        "{ranges:?}"
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
