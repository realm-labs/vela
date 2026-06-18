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
fn call_hierarchy_uses_imported_function_alias_calls() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
    let main_text = "\
use game::reward::grant as award
pub fn main(amount: i64) -> i64 {
    let first = award(amount)
    return award(first)
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

    let prepared_from_import_alias = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            0,
            line(main_text, 0)
                .find("award")
                .expect("import alias should exist"),
        ),
    );
    assert_eq!(prepared_from_import_alias, prepared);

    let prepared_from_import_path = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            0,
            line(main_text, 0)
                .find("grant")
                .expect("import path function should exist"),
        ),
    );
    assert_eq!(prepared_from_import_path, prepared);

    let prepared_from_alias_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            2,
            line(main_text, 2).find("award").expect("first alias call"),
        ),
    );
    assert_eq!(prepared_from_alias_call, prepared);

    let incoming = databases.incoming_calls(&prepared[0]);
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].from().name(), "main");
    assert_eq!(incoming[0].from().document_id(), &main);
    assert_eq!(incoming[0].from_ranges().len(), 2);
    assert_range(
        incoming[0].from_ranges(),
        2,
        line(main_text, 2).find("award").expect("first alias call"),
    );
    assert_range(
        incoming[0].from_ranges(),
        3,
        line(main_text, 3).find("award").expect("second alias call"),
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
    assert_range(
        outgoing[0].from_ranges(),
        2,
        line(main_text, 2).find("award").expect("first alias call"),
    );
    assert_range(
        outgoing[0].from_ranges(),
        3,
        line(main_text, 3).find("award").expect("second alias call"),
    );
}

#[test]
fn call_hierarchy_returns_empty_for_unresolved_dynamic_and_non_callable_targets() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(player) {
    missing(1)
    player.grant(1)
    let amount = 1
    return amount
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let unresolved = databases.prepare_call_hierarchy(
        &document,
        Position::new(
            1,
            line(text, 1)
                .find("missing")
                .expect("unresolved call should exist"),
        ),
    );
    assert!(
        unresolved.is_empty(),
        "unresolved calls must not produce speculative call hierarchy items"
    );

    let dynamic_receiver = databases.prepare_call_hierarchy(
        &document,
        Position::new(
            2,
            line(text, 2)
                .find("grant")
                .expect("dynamic receiver call should exist"),
        ),
    );
    assert!(
        dynamic_receiver.is_empty(),
        "dynamic receiver calls must not invent method call hierarchy items"
    );

    let non_callable = databases.prepare_call_hierarchy(
        &document,
        Position::new(
            4,
            line(text, 4)
                .find("amount")
                .expect("non-callable local use should exist"),
        ),
    );
    assert!(
        non_callable.is_empty(),
        "non-callable symbols must not produce call hierarchy items"
    );
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
fn call_hierarchy_cross_file_source_method_calls() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let types = DocumentId::from("/workspace/scripts/game/types.vela");
    let main_text = "\
use game::types::Reward
pub fn first(reward: Reward) -> i64 {
    return reward.grant(1)
}

pub fn second(reward: Reward) -> i64 {
    return reward.grant(2)
}";
    let types_text = "\
pub struct Reward {
    amount: i64
}

impl Reward {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(types.clone(), types_text),
    ]);

    let prepared_from_declaration = databases.prepare_call_hierarchy(
        &types,
        Position::new(
            5,
            line(types_text, 5)
                .find("grant")
                .expect("method declaration should exist"),
        ),
    );
    let prepared_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            2,
            line(main_text, 2)
                .find("grant")
                .expect("method call should exist"),
        ),
    );

    assert_eq!(prepared_from_declaration.len(), 1);
    assert_eq!(prepared_from_declaration[0].name(), "grant");
    assert_eq!(prepared_from_declaration[0].document_id(), &types);
    assert_eq!(prepared_from_call, prepared_from_declaration);

    let incoming = databases.incoming_calls(&prepared_from_declaration[0]);
    assert_eq!(incoming.len(), 2, "{incoming:?}");
    assert_eq!(incoming[0].from().name(), "first");
    assert_eq!(incoming[0].from().document_id(), &main);
    assert_range(
        incoming[0].from_ranges(),
        2,
        line(main_text, 2).find("grant").expect("first method call"),
    );
    assert_eq!(incoming[1].from().name(), "second");
    assert_eq!(incoming[1].from().document_id(), &main);
    assert_range(
        incoming[1].from_ranges(),
        6,
        line(main_text, 6)
            .find("grant")
            .expect("second method call"),
    );

    let first_item = databases
        .prepare_call_hierarchy(
            &main,
            Position::new(1, line(main_text, 1).find("first").expect("first")),
        )
        .pop()
        .expect("first should prepare a call hierarchy item");
    let outgoing = databases.outgoing_calls(&first_item);
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].to().name(), "grant");
    assert_eq!(outgoing[0].to().document_id(), &types);
    assert_range(
        outgoing[0].from_ranges(),
        2,
        line(main_text, 2).find("grant").expect("first method call"),
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
fn call_hierarchy_cross_file_trait_impl_method_calls() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let math = DocumentId::from("/workspace/scripts/game/math.vela");
    let types = DocumentId::from("/workspace/scripts/game/types.vela");
    let main_text = "\
use game::types::Player
pub fn first(player: Player) -> i64 {
    return player.grant(1)
}

pub fn second(player: Player) -> i64 {
    return player.grant(2)
}";
    let math_text = "pub fn clamp(value: i64) -> i64 { return value }";
    let types_text = "\
use game::math::clamp
pub trait Rewardable {
    fn grant(self, amount: i64) -> i64;
}

pub struct Player { level: i64 }

impl Rewardable for Player {
    fn grant(self, amount: i64) -> i64 { return clamp(amount) }
}";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(math.clone(), math_text),
        SourceFileSnapshot::new(types.clone(), types_text),
    ]);

    let prepared_from_declaration = databases.prepare_call_hierarchy(
        &types,
        Position::new(
            8,
            line(types_text, 8)
                .find("grant")
                .expect("trait impl method declaration should exist"),
        ),
    );
    let prepared_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            2,
            line(main_text, 2)
                .find("grant")
                .expect("trait impl method call should exist"),
        ),
    );

    assert_eq!(prepared_from_declaration.len(), 1);
    assert_eq!(prepared_from_declaration[0].name(), "grant");
    assert_eq!(prepared_from_declaration[0].document_id(), &types);
    assert_eq!(prepared_from_call, prepared_from_declaration);

    let incoming = databases.incoming_calls(&prepared_from_declaration[0]);
    assert_eq!(incoming.len(), 2, "{incoming:?}");
    assert_eq!(incoming[0].from().name(), "first");
    assert_eq!(incoming[0].from().document_id(), &main);
    assert_range(
        incoming[0].from_ranges(),
        2,
        line(main_text, 2).find("grant").expect("first method call"),
    );
    assert_eq!(incoming[1].from().name(), "second");
    assert_eq!(incoming[1].from().document_id(), &main);
    assert_range(
        incoming[1].from_ranges(),
        6,
        line(main_text, 6)
            .find("grant")
            .expect("second method call"),
    );

    let first_item = databases
        .prepare_call_hierarchy(
            &main,
            Position::new(1, line(main_text, 1).find("first").expect("first")),
        )
        .pop()
        .expect("first should prepare a call hierarchy item");
    let first_outgoing = databases.outgoing_calls(&first_item);
    assert_eq!(first_outgoing.len(), 1);
    assert_eq!(first_outgoing[0].to().name(), "grant");
    assert_eq!(first_outgoing[0].to().document_id(), &types);
    assert_range(
        first_outgoing[0].from_ranges(),
        2,
        line(main_text, 2).find("grant").expect("first method call"),
    );

    let method_outgoing = databases.outgoing_calls(&prepared_from_declaration[0]);
    assert_eq!(method_outgoing.len(), 1);
    assert_eq!(method_outgoing[0].to().name(), "clamp");
    assert_eq!(method_outgoing[0].to().document_id(), &math);
    assert_range(
        method_outgoing[0].from_ranges(),
        8,
        line(types_text, 8)
            .find("clamp")
            .expect("imported helper call"),
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

#[test]
fn call_hierarchy_cross_file_trait_default_and_interface_methods() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let math = DocumentId::from("/workspace/scripts/game/math.vela");
    let traits = DocumentId::from("/workspace/scripts/game/traits.vela");
    let main_text = "\
use game::traits::Rewardable
pub fn main(rewardable: Rewardable) -> i64 {
    let first = rewardable.grant(1)
    return rewardable.preview(first)
}";
    let math_text = "pub fn clamp(value: i64) -> i64 { return value }";
    let traits_text = "\
use game::math::clamp
pub trait Rewardable {
    fn grant(self, amount: i64) -> i64 { return clamp(amount) }
    fn preview(self, amount: i64) -> i64;
}";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(math.clone(), math_text),
        SourceFileSnapshot::new(traits.clone(), traits_text),
    ]);

    let grant_from_declaration = databases.prepare_call_hierarchy(
        &traits,
        Position::new(
            2,
            line(traits_text, 2)
                .find("grant")
                .expect("trait default method declaration"),
        ),
    );
    let grant_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            2,
            line(main_text, 2)
                .find("grant")
                .expect("trait default method call"),
        ),
    );
    let preview_from_declaration = databases.prepare_call_hierarchy(
        &traits,
        Position::new(
            3,
            line(traits_text, 3)
                .find("preview")
                .expect("trait interface method declaration"),
        ),
    );
    let preview_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(
            3,
            line(main_text, 3)
                .find("preview")
                .expect("trait interface method call"),
        ),
    );

    assert_eq!(grant_from_declaration.len(), 1);
    assert_eq!(grant_from_declaration[0].name(), "grant");
    assert_eq!(grant_from_declaration[0].document_id(), &traits);
    assert_eq!(grant_from_call, grant_from_declaration);
    assert_eq!(preview_from_declaration.len(), 1);
    assert_eq!(preview_from_declaration[0].name(), "preview");
    assert_eq!(preview_from_declaration[0].document_id(), &traits);
    assert_eq!(preview_from_call, preview_from_declaration);

    let grant_incoming = databases.incoming_calls(&grant_from_declaration[0]);
    assert_eq!(grant_incoming.len(), 1);
    assert_eq!(grant_incoming[0].from().name(), "main");
    assert_eq!(grant_incoming[0].from().document_id(), &main);
    assert_range(
        grant_incoming[0].from_ranges(),
        2,
        line(main_text, 2)
            .find("grant")
            .expect("trait default method call"),
    );

    let preview_incoming = databases.incoming_calls(&preview_from_declaration[0]);
    assert_eq!(preview_incoming.len(), 1);
    assert_eq!(preview_incoming[0].from().name(), "main");
    assert_eq!(preview_incoming[0].from().document_id(), &main);
    assert_range(
        preview_incoming[0].from_ranges(),
        3,
        line(main_text, 3)
            .find("preview")
            .expect("trait interface method call"),
    );

    let main_item = databases
        .prepare_call_hierarchy(
            &main,
            Position::new(1, line(main_text, 1).find("main").expect("main")),
        )
        .pop()
        .expect("main should prepare a call hierarchy item");
    let main_outgoing = databases.outgoing_calls(&main_item);
    assert_eq!(main_outgoing.len(), 2, "{main_outgoing:?}");
    assert_outgoing_call(
        &main_outgoing,
        "grant",
        &traits,
        2,
        line(main_text, 2)
            .find("grant")
            .expect("trait default method call"),
    );
    assert_outgoing_call(
        &main_outgoing,
        "preview",
        &traits,
        3,
        line(main_text, 3)
            .find("preview")
            .expect("trait interface method call"),
    );

    let grant_outgoing = databases.outgoing_calls(&grant_from_declaration[0]);
    assert_eq!(grant_outgoing.len(), 1);
    assert_eq!(grant_outgoing[0].to().name(), "clamp");
    assert_eq!(grant_outgoing[0].to().document_id(), &math);
    assert_range(
        grant_outgoing[0].from_ranges(),
        2,
        line(traits_text, 2)
            .find("clamp")
            .expect("imported helper call"),
    );
    assert!(
        databases
            .outgoing_calls(&preview_from_declaration[0])
            .is_empty()
    );
}

#[test]
fn call_hierarchy_uses_schema_method_and_trait_method_calls() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "\
pub fn main(player: Player, rewardable: Rewardable) -> i64 {
    let first = player.grant(1)
    return rewardable.preview(first)
}";
    let schema_text = "\
pub fn grant() { return 1 }
pub fn preview() { return 1 }";
    let mut databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(schema.clone(), schema_text),
    ]);
    let schema_record = databases
        .source_db()
        .records()
        .get(&schema)
        .expect("schema source should be indexed");
    let grant_start = schema_text.find("grant").expect("grant marker");
    let preview_start = schema_text.find("preview").expect("preview marker");
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "Player",
                    "fact": { "kind": "host", "name": "Player" }
                }
            ],
            "traits": [
                {
                    "name": "Rewardable",
                    "fact": { "kind": "trait", "name": "Rewardable" }
                }
            ],
            "methods": [
                {
                    "owner": "Player",
                    "name": "grant",
                    "fact": {
                        "kind": "function",
                        "params": [{ "kind": "primitive", "name": "i64" }],
                        "returns": { "kind": "primitive", "name": "i64" }
                    },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": grant_start,
                        "end": grant_start + "grant".len()
                    }
                }
            ],
            "traitMethods": [
                {
                    "owner": "Rewardable",
                    "name": "preview",
                    "fact": {
                        "kind": "function",
                        "params": [{ "kind": "primitive", "name": "i64" }],
                        "returns": { "kind": "primitive", "name": "i64" }
                    },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": preview_start,
                        "end": preview_start + "preview".len()
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

    let grant_from_declaration = databases.prepare_call_hierarchy(
        &schema,
        Position::new(0, line(schema_text, 0).find("grant").expect("grant")),
    );
    let grant_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(1, line(main_text, 1).find("grant").expect("grant call")),
    );
    let preview_from_declaration = databases.prepare_call_hierarchy(
        &schema,
        Position::new(
            1,
            line(schema_text, 1)
                .find("preview")
                .expect("preview declaration"),
        ),
    );
    let preview_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(2, line(main_text, 2).find("preview").expect("preview call")),
    );

    assert_eq!(grant_from_declaration.len(), 1);
    assert_eq!(grant_from_declaration[0].name(), "grant");
    assert_eq!(grant_from_declaration[0].document_id(), &schema);
    assert_eq!(grant_from_call, grant_from_declaration);
    assert_eq!(preview_from_declaration.len(), 1);
    assert_eq!(preview_from_declaration[0].name(), "preview");
    assert_eq!(preview_from_declaration[0].document_id(), &schema);
    assert_eq!(preview_from_call, preview_from_declaration);

    let grant_incoming = databases.incoming_calls(&grant_from_declaration[0]);
    assert_eq!(grant_incoming.len(), 1);
    assert_eq!(grant_incoming[0].from().name(), "main");
    assert_range(
        grant_incoming[0].from_ranges(),
        1,
        line(main_text, 1).find("grant").expect("grant call"),
    );
    assert!(
        databases
            .outgoing_calls(&grant_from_declaration[0])
            .is_empty()
    );

    let preview_incoming = databases.incoming_calls(&preview_from_declaration[0]);
    assert_eq!(preview_incoming.len(), 1);
    assert_eq!(preview_incoming[0].from().name(), "main");
    assert_range(
        preview_incoming[0].from_ranges(),
        2,
        line(main_text, 2).find("preview").expect("preview call"),
    );
    assert!(
        databases
            .outgoing_calls(&preview_from_declaration[0])
            .is_empty()
    );

    let main_item = databases
        .prepare_call_hierarchy(
            &main,
            Position::new(0, line(main_text, 0).find("main").expect("main")),
        )
        .pop()
        .expect("main should prepare a call hierarchy item");
    let outgoing = databases.outgoing_calls(&main_item);
    assert_eq!(outgoing.len(), 2, "{outgoing:?}");
    assert_outgoing_call(
        &outgoing,
        "grant",
        &schema,
        1,
        line(main_text, 1).find("grant").expect("grant call"),
    );
    assert_outgoing_call(
        &outgoing,
        "preview",
        &schema,
        2,
        line(main_text, 2).find("preview").expect("preview call"),
    );
}

#[test]
fn call_hierarchy_uses_schema_method_calls_on_schema_function_return_receivers() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
    let main_text = "\
pub fn main() -> i64 {
    let first = current_player().grant(1)
    return current_player().grant(first)
}";
    let schema_text = "pub fn grant() { return 1 }";
    let mut databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(schema.clone(), schema_text),
    ]);
    let schema_record = databases
        .source_db()
        .records()
        .get(&schema)
        .expect("schema source should be indexed");
    let grant_start = schema_text.find("grant").expect("grant marker");
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "Player",
                    "fact": { "kind": "host", "name": "Player" }
                }
            ],
            "functions": [
                {
                    "name": "current_player",
                    "fact": {
                        "kind": "function",
                        "params": [],
                        "returns": { "kind": "host", "name": "Player" }
                    }
                }
            ],
            "methods": [
                {
                    "owner": "Player",
                    "name": "grant",
                    "fact": {
                        "kind": "function",
                        "params": [{ "kind": "primitive", "name": "i64" }],
                        "returns": { "kind": "primitive", "name": "i64" }
                    },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": grant_start,
                        "end": grant_start + "grant".len()
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

    let grant_from_declaration = databases.prepare_call_hierarchy(
        &schema,
        Position::new(0, line(schema_text, 0).find("grant").expect("grant")),
    );
    let grant_from_call = databases.prepare_call_hierarchy(
        &main,
        Position::new(1, line(main_text, 1).find("grant").expect("grant call")),
    );

    assert_eq!(grant_from_declaration.len(), 1);
    assert_eq!(grant_from_declaration[0].name(), "grant");
    assert_eq!(grant_from_declaration[0].document_id(), &schema);
    assert_eq!(grant_from_call, grant_from_declaration);

    let incoming = databases.incoming_calls(&grant_from_declaration[0]);
    assert_eq!(incoming.len(), 1, "{incoming:?}");
    assert_eq!(incoming[0].from().name(), "main");
    assert_range(
        incoming[0].from_ranges(),
        1,
        line(main_text, 1).find("grant").expect("first grant call"),
    );
    assert_range(
        incoming[0].from_ranges(),
        2,
        line(main_text, 2).find("grant").expect("second grant call"),
    );

    let main_item = databases
        .prepare_call_hierarchy(
            &main,
            Position::new(0, line(main_text, 0).find("main").expect("main")),
        )
        .pop()
        .expect("main should prepare a call hierarchy item");
    let outgoing = databases.outgoing_calls(&main_item);
    assert_eq!(outgoing.len(), 1, "{outgoing:?}");
    assert_outgoing_call(
        &outgoing,
        "grant",
        &schema,
        1,
        line(main_text, 1).find("grant").expect("first grant call"),
    );
    assert_outgoing_call(
        &outgoing,
        "grant",
        &schema,
        2,
        line(main_text, 2).find("grant").expect("second grant call"),
    );
}

fn assert_outgoing_call(
    calls: &[OutgoingCall],
    name: &str,
    document_id: &DocumentId,
    line: usize,
    character: usize,
) {
    assert!(
        calls.iter().any(|call| {
            call.to().name() == name
                && call.to().document_id() == document_id
                && call
                    .from_ranges()
                    .iter()
                    .any(|range| range.start().line == line && range.start().character == character)
        }),
        "{calls:?}"
    );
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
