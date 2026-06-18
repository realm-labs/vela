use super::*;
use crate::{
    SourceFileSnapshot, SourceVersion, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};
use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};

mod schema_collision_tests;

#[test]
fn prepare_rename_rejects_keywords_and_literals() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(amount: i64) -> i64 {
    return amount + 1
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    assert_eq!(
        databases.prepare_rename(
            &document,
            Position::new(1, line(text, 1).find("return").expect("return keyword"))
        ),
        None
    );
    assert_eq!(
        databases.prepare_rename(
            &document,
            Position::new(1, line(text, 1).find('1').expect("literal"))
        ),
        None
    );
}

#[test]
fn prepare_rename_rejects_non_source_query_targets() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(value: Any) -> i64 {
    let first = max(1, 2)
    value.level
    missing
    return grant()
}";
    let mut schema = RegistryFacts::default();
    schema.insert_function(
        "game::rewards::grant",
        TypeFact::function(Vec::new(), TypeFact::I64),
    );
    schema.insert_function(
        "game::quests::grant",
        TypeFact::function(Vec::new(), TypeFact::I64),
    );
    let databases = databases_for_with_schema(
        vec![SourceFileSnapshot::new(document.clone(), text)],
        schema,
    );

    assert_eq!(
        databases.prepare_rename(
            &document,
            Position::new(1, line(text, 1).find("max").expect("stdlib function"))
        ),
        None
    );
    assert_eq!(
        databases.prepare_rename(
            &document,
            Position::new(2, line(text, 2).find("level").expect("dynamic member"))
        ),
        None
    );
    assert_eq!(
        databases.prepare_rename(
            &document,
            Position::new(3, line(text, 3).find("missing").expect("unresolved name"))
        ),
        None
    );
    assert_eq!(
        databases.prepare_rename(
            &document,
            Position::new(
                4,
                line(text, 4).find("grant").expect("ambiguous schema call")
            )
        ),
        None
    );
}

#[test]
fn local_rename_updates_all_function_uses() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    next += amount
    return next
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let prepare = databases
        .prepare_rename(
            &document,
            Position::new(2, line(text, 2).find("next").expect("next write")),
        )
        .expect("local binding should be renameable");

    assert_eq!(prepare.document_id(), &document);
    assert_eq!(prepare.placeholder(), "next");
    assert_eq!(prepare.range().start(), Position::new(2, 4));
    let symbol = SymbolRef::local_at("next", document.clone(), TextRange::new(42, 46));
    assert_eq!(prepare.symbol(), &symbol);

    let edit = databases
        .rename(
            &document,
            Position::new(2, line(text, 2).find("next").expect("next write")),
            "score",
        )
        .expect("local rename should produce edits");
    assert_eq!(edit.symbol(), Some(&symbol));

    let document_edit = edit
        .document_edits()
        .first()
        .expect("rename should edit one document");
    assert_eq!(edit.edit_plan().document_edits(), edit.document_edits());
    assert_eq!(document_edit.document_id(), &document);
    assert_eq!(document_edit.edits().len(), 3);
    assert_edit_at(document_edit.edits(), 1, 8, "score");
    assert_edit_at(document_edit.edits(), 2, 4, "score");
    assert_edit_at(document_edit.edits(), 3, 11, "score");
}

#[test]
fn rename_workspace_edits_carry_document_versions() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let initial = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next
}";
    let changed = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 2
    return next
}";
    let config = WorkspaceConfig::scratch(document.clone());
    let mut workspace = Workspace::new();
    workspace.set_disk_snapshot(document.clone(), initial, SourceVersion::new(1));
    workspace.open_document(document.clone(), changed, SourceVersion::new(2));
    let project = assemble_project_sources(&config, &[], &workspace.snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);

    let edit = databases
        .rename(
            &document,
            Position::new(2, line(changed, 2).find("next").expect("next use")),
            "score",
        )
        .expect("local rename should produce edits");

    let document_edit = document_edit(&edit, &document);
    assert_eq!(
        document_edit.document_version(),
        Some(SourceVersion::new(2))
    );
    assert_eq!(document_edit.edits().len(), 2);
    assert_edit_at(document_edit.edits(), 1, 8, "score");
    assert_edit_at(document_edit.edits(), 2, 11, "score");
}

#[test]
fn edit_plan_rejects_overlapping_ranges() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let document_edit = DocumentTextEdit::new(
        document,
        vec![
            TextEdit::new(
                DiagnosticRange::new(Position::new(0, 4), Position::new(0, 10)),
                "first",
            ),
            TextEdit::new(
                DiagnosticRange::new(Position::new(0, 8), Position::new(0, 12)),
                "second",
            ),
        ],
    );

    assert_eq!(EditPlan::new(vec![document_edit]), None);
}

#[test]
fn rename_rejects_scope_collision() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    assert_eq!(
        databases.rename(
            &document,
            Position::new(1, line(text, 1).find("next").expect("next local")),
            "amount",
        ),
        None
    );
}

#[test]
fn private_value_declaration_rename_updates_uses() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
const BONUS: i64 = 5
pub fn main() -> i64 {
    return BONUS + BONUS
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let prepare = databases
        .prepare_rename(
            &document,
            Position::new(2, line(text, 2).find("BONUS").expect("BONUS read")),
        )
        .expect("private const should be renameable from a use site");

    assert_eq!(prepare.placeholder(), "BONUS");
    assert_eq!(prepare.range().start(), Position::new(2, 11));
    assert_eq!(
        prepare.symbol(),
        &SymbolRef::Source("game::main::BONUS".into())
    );

    let edit = databases
        .rename(
            &document,
            Position::new(2, line(text, 2).find("BONUS").expect("BONUS read")),
            "BASE",
        )
        .expect("private const rename should produce edits");
    assert_eq!(
        edit.symbol(),
        Some(&SymbolRef::Source("game::main::BONUS".into()))
    );

    let document_edit = document_edit(&edit, &document);
    assert_eq!(document_edit.edits().len(), 3);
    assert_edit_at(document_edit.edits(), 0, 6, "BASE");
    assert_edit_at(document_edit.edits(), 2, 11, "BASE");
    assert_edit_at(document_edit.edits(), 2, 19, "BASE");
    assert!(edit.risks().is_empty());
}

#[test]
fn private_type_declaration_rename_updates_type_hints() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
struct Reward {
    amount: i64
}

fn grant(reward: Reward) -> Reward {
    let next: Reward = reward
    return next
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let prepare = databases
        .prepare_rename(
            &document,
            Position::new(4, line(text, 4).rfind("Reward").expect("return type")),
        )
        .expect("private type should be renameable from a type hint");

    assert_eq!(prepare.placeholder(), "Reward");
    assert_eq!(prepare.range().start(), Position::new(4, 28));
    assert_eq!(
        prepare.symbol(),
        &SymbolRef::Source("game::main::Reward".into())
    );

    let edit = databases
        .rename(
            &document,
            Position::new(4, line(text, 4).rfind("Reward").expect("return type")),
            "Prize",
        )
        .expect("private type rename should produce edits");

    let document_edit = document_edit(&edit, &document);
    assert_eq!(document_edit.edits().len(), 4);
    assert_edit_at(document_edit.edits(), 0, 7, "Prize");
    assert_edit_at(document_edit.edits(), 4, 17, "Prize");
    assert_edit_at(document_edit.edits(), 4, 28, "Prize");
    assert_edit_at(document_edit.edits(), 5, 14, "Prize");
    assert!(edit.risks().is_empty());
}

#[test]
fn private_struct_field_rename_updates_member_uses() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
struct Player {
    level: i64
    xp: i64
}

fn bump(player: Player) -> i64 {
    player.level += 1
    return player.level + player.xp
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let prepare = databases
        .prepare_rename(
            &document,
            Position::new(6, line(text, 6).find("level").expect("level write")),
        )
        .expect("private struct field should be renameable from a member use");

    assert_eq!(prepare.placeholder(), "level");
    assert_eq!(prepare.range().start(), Position::new(6, 11));
    assert_eq!(
        prepare.symbol(),
        &SymbolRef::Source("game::main::Player.level".into())
    );

    let edit = databases
        .rename(
            &document,
            Position::new(6, line(text, 6).find("level").expect("level write")),
            "rank",
        )
        .expect("private struct field rename should produce edits");
    assert_eq!(
        edit.symbol(),
        Some(&SymbolRef::Source("game::main::Player.level".into()))
    );

    let document_edit = document_edit(&edit, &document);
    assert_eq!(document_edit.edits().len(), 3);
    assert_edit_at(document_edit.edits(), 1, 4, "rank");
    assert_edit_at(document_edit.edits(), 6, 11, "rank");
    assert_edit_at(document_edit.edits(), 7, 18, "rank");
    assert!(edit.risks().is_empty());
}

#[test]
fn private_method_rename_updates_typed_receiver_calls() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
struct Reward {
    amount: i64
}

impl Reward {
    fn grant(self, amount: i64) -> i64 { return amount }
    fn preview(self) -> i64 { return self.grant(1) }
}

fn main(reward: Reward) -> i64 {
    let first = reward.grant(1)
    return reward.grant(first)
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let prepare = databases
        .prepare_rename(
            &document,
            Position::new(10, line(text, 10).find("grant").expect("grant call")),
        )
        .expect("private method should be renameable from a typed call");

    assert_eq!(prepare.placeholder(), "grant");
    assert_eq!(prepare.range().start(), Position::new(10, 23));

    let edit = databases
        .rename(
            &document,
            Position::new(10, line(text, 10).find("grant").expect("grant call")),
            "award",
        )
        .expect("private method rename should produce edits");

    let document_edit = document_edit(&edit, &document);
    assert_eq!(document_edit.edits().len(), 4);
    assert_edit_at(document_edit.edits(), 5, 7, "award");
    assert_edit_at(document_edit.edits(), 6, 42, "award");
    assert_edit_at(document_edit.edits(), 10, 23, "award");
    assert_edit_at(document_edit.edits(), 11, 18, "award");
    assert!(edit.risks().is_empty());
}

#[test]
fn private_enum_variant_rename_updates_constructors_and_patterns() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
enum QuestState {
    Active { count: i64 },
    Done
}

fn active(count: i64) -> QuestState {
    return QuestState::Active { count: count }
}

fn main(state: QuestState) -> i64 {
    match state {
        QuestState::Active { count } => { return count }
        QuestState::Done => { return 0 }
    }
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let prepare = databases
        .prepare_rename(
            &document,
            Position::new(6, line(text, 6).find("Active").expect("constructor")),
        )
        .expect("private enum variant should be renameable from a constructor");

    assert_eq!(prepare.placeholder(), "Active");
    assert_eq!(prepare.range().start(), Position::new(6, 23));

    let edit = databases
        .rename(
            &document,
            Position::new(6, line(text, 6).find("Active").expect("constructor")),
            "Running",
        )
        .expect("private enum variant rename should produce edits");

    let document_edit = document_edit(&edit, &document);
    assert_eq!(document_edit.edits().len(), 3);
    assert_edit_at(document_edit.edits(), 1, 4, "Running");
    assert_edit_at(document_edit.edits(), 6, 23, "Running");
    assert_edit_at(document_edit.edits(), 11, 20, "Running");
    assert!(edit.risks().is_empty());
}

#[test]
fn private_enum_variant_rename_rejects_variant_collision() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
enum QuestState {
    Active,
    Done
}

fn main() -> QuestState {
    return QuestState::Active
}";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    assert_eq!(
        databases.rename(
            &document,
            Position::new(6, line(text, 6).find("Active").expect("constructor")),
            "Done",
        ),
        None
    );
}

#[test]
fn source_backed_schema_type_rename_updates_type_hints() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/_schema_defs.vela");
    let main_text = "\
pub fn spawn(player: Player) -> Player {
    return player
}";
    let schema_text = "pub fn Player() { return 1 }";
    let mut databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(schema.clone(), schema_text),
    ]);
    let schema_record = databases
        .source_db()
        .records()
        .get(&schema)
        .expect("schema source should be indexed");
    let target_start = schema_text.find("Player").expect("schema marker");
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "Player",
                    "fact": { "kind": "host", "name": "Player" },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": target_start,
                        "end": target_start + "Player".len()
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

    let prepare = databases
        .prepare_rename(
            &main,
            Position::new(
                0,
                line(main_text, 0).find("Player").expect("parameter type"),
            ),
        )
        .expect("source-backed schema type should be renameable from a type hint");

    assert_eq!(prepare.placeholder(), "Player");
    assert_eq!(prepare.range().start(), Position::new(0, 21));
    assert_eq!(prepare.symbol(), &SymbolRef::Schema("Player".into()));

    let edit = databases
        .rename(
            &main,
            Position::new(
                0,
                line(main_text, 0).find("Player").expect("parameter type"),
            ),
            "Actor",
        )
        .expect("source-backed schema type rename should produce edits");

    let schema_edit = document_edit(&edit, &schema);
    assert_eq!(schema_edit.edits().len(), 1);
    assert_edit_at(schema_edit.edits(), 0, 7, "Actor");

    let main_edit = document_edit(&edit, &main);
    assert_eq!(main_edit.edits().len(), 2);
    assert_edit_at(main_edit.edits(), 0, 21, "Actor");
    assert_edit_at(main_edit.edits(), 0, 32, "Actor");
    assert!(edit.risks().is_empty());
}

#[test]
fn source_backed_schema_function_rename_updates_call_sites() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/_schema_defs.vela");
    let main_text = "\
pub fn main(amount: i64) -> i64 {
    let first = grant(amount)
    return game::reward::grant(first)
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
    let target_start = schema_text.find("grant").expect("schema marker");
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "functions": [
                {
                    "name": "game::reward::grant",
                    "fact": {
                        "kind": "function",
                        "params": [{ "kind": "primitive", "name": "i64" }],
                        "returns": { "kind": "primitive", "name": "i64" }
                    },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": target_start,
                        "end": target_start + "grant".len()
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

    let prepare = databases
        .prepare_rename(
            &main,
            Position::new(1, line(main_text, 1).find("grant").expect("short call")),
        )
        .expect("source-backed schema function should be renameable from a call");

    assert_eq!(prepare.placeholder(), "game::reward::grant");
    assert_eq!(prepare.range().start(), Position::new(1, 16));
    assert_eq!(
        prepare.symbol(),
        &SymbolRef::Schema("game::reward::grant".into())
    );

    let edit = databases
        .rename(
            &main,
            Position::new(1, line(main_text, 1).find("grant").expect("short call")),
            "award",
        )
        .expect("source-backed schema function rename should produce edits");

    let schema_edit = document_edit(&edit, &schema);
    assert_eq!(schema_edit.edits().len(), 1);
    assert_edit_at(schema_edit.edits(), 0, 7, "award");

    let main_edit = document_edit(&edit, &main);
    assert_eq!(main_edit.edits().len(), 2);
    assert_edit_at(main_edit.edits(), 1, 16, "award");
    assert_edit_at(main_edit.edits(), 2, 25, "award");
    assert!(edit.risks().is_empty());
}

#[test]
fn source_backed_schema_variant_rename_updates_constructors_and_patterns() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/_schema_defs.vela");
    let main_text = "\
pub fn main(state: QuestState) -> i64 {
    let next = QuestState::Active
    match state {
        QuestState::Active => { return 1 }
        QuestState::Done => { return 2 }
    }
    return 0
}";
    let schema_text = "pub fn Active() { return 1 }\npub fn Done() { return 2 }";
    let mut databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(schema.clone(), schema_text),
    ]);
    let schema_record = databases
        .source_db()
        .records()
        .get(&schema)
        .expect("schema source should be indexed");
    let target_start = schema_text.find("Active").expect("schema marker");
    let done_start = schema_text.find("Done").expect("schema Done marker");
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "QuestState",
                    "fact": { "kind": "enum", "name": "QuestState", "variant": null }
                }
            ],
            "variants": [
                {
                    "owner": "QuestState",
                    "name": "Active",
                    "fact": {
                        "kind": "enum",
                        "name": "QuestState",
                        "variant": "Active"
                    },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": target_start,
                        "end": target_start + "Active".len()
                    }
                },
                {
                    "owner": "QuestState",
                    "name": "Done",
                    "fact": {
                        "kind": "enum",
                        "name": "QuestState",
                        "variant": "Done"
                    },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": done_start,
                        "end": done_start + "Done".len()
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

    let prepare = databases
        .prepare_rename(
            &main,
            Position::new(1, line(main_text, 1).find("Active").expect("constructor")),
        )
        .expect("source-backed schema variant should be renameable from a constructor");

    assert_eq!(prepare.placeholder(), "Active");
    assert_eq!(prepare.range().start(), Position::new(1, 27));
    assert_eq!(
        prepare.symbol(),
        &SymbolRef::Schema("QuestState::Active".into())
    );

    assert_eq!(
        databases.rename(
            &main,
            Position::new(1, line(main_text, 1).find("Active").expect("constructor")),
            "Done",
        ),
        None
    );

    let edit = databases
        .rename(
            &main,
            Position::new(1, line(main_text, 1).find("Active").expect("constructor")),
            "Running",
        )
        .expect("source-backed schema variant rename should produce edits");

    let schema_edit = document_edit(&edit, &schema);
    assert_eq!(schema_edit.edits().len(), 1);
    assert_edit_at(schema_edit.edits(), 0, 7, "Running");

    let main_edit = document_edit(&edit, &main);
    assert_eq!(main_edit.edits().len(), 2);
    assert_edit_at(main_edit.edits(), 1, 27, "Running");
    assert_edit_at(main_edit.edits(), 3, 20, "Running");
    assert!(edit.risks().is_empty());
}

#[test]
fn source_backed_schema_field_rename_updates_member_uses() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/_schema_defs.vela");
    let main_text = "\
pub fn main(player: Player) -> i64 {
    let first = player.level
    player.level += 1
    return player.level + first
}";
    let schema_text = "pub fn level() { return 1 }";
    let mut databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(schema.clone(), schema_text),
    ]);
    let schema_record = databases
        .source_db()
        .records()
        .get(&schema)
        .expect("schema source should be indexed");
    let target_start = schema_text.find("level").expect("schema marker");
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "Player",
                    "fact": { "kind": "host", "name": "Player" }
                }
            ],
            "fields": [
                {
                    "owner": "Player",
                    "name": "level",
                    "fact": { "kind": "primitive", "name": "i64" },
                    "sourceSpan": {
                        "source": schema_record.source_id().get(),
                        "start": target_start,
                        "end": target_start + "level".len()
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

    let prepare = databases
        .prepare_rename(
            &main,
            Position::new(1, line(main_text, 1).find("level").expect("field read")),
        )
        .expect("source-backed schema field should be renameable from a use");

    assert_eq!(prepare.placeholder(), "level");
    assert_eq!(prepare.range().start(), Position::new(1, 23));
    assert_eq!(prepare.symbol(), &SymbolRef::Schema("Player.level".into()));

    let edit = databases
        .rename(
            &main,
            Position::new(1, line(main_text, 1).find("level").expect("field read")),
            "rank",
        )
        .expect("source-backed schema field rename should produce edits");
    assert_eq!(
        edit.symbol(),
        Some(&SymbolRef::Schema("Player.level".into()))
    );

    let schema_edit = document_edit(&edit, &schema);
    assert_eq!(schema_edit.edits().len(), 1);
    assert_edit_at(schema_edit.edits(), 0, 7, "rank");

    let main_edit = document_edit(&edit, &main);
    assert_eq!(main_edit.edits().len(), 3);
    assert_edit_at(main_edit.edits(), 1, 23, "rank");
    assert_edit_at(main_edit.edits(), 2, 11, "rank");
    assert_edit_at(main_edit.edits(), 3, 18, "rank");
    assert!(edit.risks().is_empty());
}

#[test]
fn source_backed_schema_method_rename_updates_member_calls() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let schema = DocumentId::from("/workspace/scripts/_schema_defs.vela");
    let main_text = "\
pub fn main(player: Player) -> i64 {
    let first = player.grant(1)
    return player.grant(first)
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
    let target_start = schema_text.find("grant").expect("schema marker");
    let artifact = serde_json::json!({
        "formatVersion": 1,
        "facts": {
            "types": [
                {
                    "name": "Player",
                    "fact": { "kind": "host", "name": "Player" }
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
                        "start": target_start,
                        "end": target_start + "grant".len()
                    }
                }
            ]
        }
    })
    .to_string();
    databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

    let prepare = databases
        .prepare_rename(
            &main,
            Position::new(1, line(main_text, 1).find("grant").expect("method call")),
        )
        .expect("source-backed schema method should be renameable from a call");

    assert_eq!(prepare.placeholder(), "grant");
    assert_eq!(prepare.range().start(), Position::new(1, 23));
    assert_eq!(prepare.symbol(), &SymbolRef::Schema("Player.grant".into()));

    let edit = databases
        .rename(
            &main,
            Position::new(1, line(main_text, 1).find("grant").expect("method call")),
            "award",
        )
        .expect("source-backed schema method rename should produce edits");

    let schema_edit = document_edit(&edit, &schema);
    assert_eq!(schema_edit.edits().len(), 1);
    assert_edit_at(schema_edit.edits(), 0, 7, "award");

    let main_edit = document_edit(&edit, &main);
    assert_eq!(main_edit.edits().len(), 2);
    assert_edit_at(main_edit.edits(), 1, 23, "award");
    assert_edit_at(main_edit.edits(), 2, 18, "award");
    assert!(edit.risks().is_empty());
}

#[test]
fn private_function_rename_updates_imports() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
    let main_text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    return grant(amount)
}";
    let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(helper.clone(), helper_text),
    ]);

    let prepare = databases
        .prepare_rename(
            &main,
            Position::new(2, line(main_text, 2).find("grant").expect("grant call")),
        )
        .expect("script function should be renameable from call site");

    assert_eq!(prepare.placeholder(), "grant");
    assert_eq!(prepare.range().start(), Position::new(2, 11));

    let edit = databases
        .rename(
            &main,
            Position::new(2, line(main_text, 2).find("grant").expect("grant call")),
            "award",
        )
        .expect("script function rename should produce workspace edits");

    let main_edit = document_edit(&edit, &main);
    assert_eq!(main_edit.edits().len(), 2);
    assert_edit_at(main_edit.edits(), 0, 18, "award");
    assert_edit_at(main_edit.edits(), 2, 11, "award");

    let helper_edit = document_edit(&edit, &helper);
    assert_eq!(helper_edit.edits().len(), 1);
    assert_edit_at(helper_edit.edits(), 0, 7, "award");
}

#[test]
fn private_function_rename_updates_aliased_import_path() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
    let main_text = "\
use game::reward::grant as award
pub fn main(amount: i64) -> i64 {
    return award(amount)
}";
    let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(helper.clone(), helper_text),
    ]);

    let edit = databases
        .rename(
            &helper,
            Position::new(0, line(helper_text, 0).find("grant").expect("declaration")),
            "grant_reward",
        )
        .expect("script function rename should update aliased import path");

    let main_edit = document_edit(&edit, &main);
    assert_eq!(main_edit.edits().len(), 1);
    assert_edit_at(main_edit.edits(), 0, 18, "grant_reward");

    let helper_edit = document_edit(&edit, &helper);
    assert_eq!(helper_edit.edits().len(), 1);
    assert_edit_at(helper_edit.edits(), 0, 7, "grant_reward");
}

#[test]
fn function_rename_rejects_import_alias_collision() {
    let main = DocumentId::from("/workspace/scripts/game/main.vela");
    let reward = DocumentId::from("/workspace/scripts/game/reward.vela");
    let bonus = DocumentId::from("/workspace/scripts/game/bonus.vela");
    let main_text = "\
use game::reward::grant
use game::bonus::score as award
pub fn main() -> i64 {
    return grant() + award()
}";
    let reward_text = "pub fn grant() -> i64 { return 1 }";
    let bonus_text = "pub fn score() -> i64 { return 2 }";
    let databases = databases_for(vec![
        SourceFileSnapshot::new(main.clone(), main_text),
        SourceFileSnapshot::new(reward, reward_text),
        SourceFileSnapshot::new(bonus, bonus_text),
    ]);

    assert_eq!(
        databases.rename(
            &main,
            Position::new(3, line(main_text, 3).find("grant").expect("grant call")),
            "award",
        ),
        None
    );
}

#[test]
fn host_schema_rename_is_not_editable() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "pub fn main(player: Player) { return player.level }";
    let mut schema = RegistryFacts::default();
    schema.insert_type("Player", TypeFact::host("Player"));
    schema.insert_field("Player", "level", TypeFact::I64);
    let databases = databases_for_with_schema(
        vec![SourceFileSnapshot::new(document.clone(), text)],
        schema,
    );

    assert!(
        databases
            .hover(
                &document,
                Position::new(0, text.find("level").expect("schema field"))
            )
            .is_some(),
        "fixture should prove schema-backed member resolution is active"
    );

    assert_eq!(
        databases.prepare_rename(
            &document,
            Position::new(0, text.find("Player").expect("schema type"))
        ),
        None
    );
    assert_eq!(
        databases.rename(
            &document,
            Position::new(0, text.find("Player").expect("schema type")),
            "Actor",
        ),
        None
    );
    assert_eq!(
        databases.prepare_rename(
            &document,
            Position::new(0, text.find("level").expect("schema field"))
        ),
        None
    );
    assert_eq!(
        databases.rename(
            &document,
            Position::new(0, text.find("level").expect("schema field")),
            "rank",
        ),
        None
    );
}

#[test]
fn public_export_rename_reports_hot_reload_risk() {
    let document = DocumentId::from("/workspace/scripts/game/reward.vela");
    let text = "pub fn grant(amount: i64) -> i64 { return amount }";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    let edit = databases
        .rename(
            &document,
            Position::new(0, line(text, 0).find("grant").expect("grant declaration")),
            "award",
        )
        .expect("public function rename should still produce edits");

    assert_eq!(edit.risks().len(), 1);
    assert_eq!(edit.risks()[0].kind(), RenameRiskKind::HotReloadAbi);
    assert!(
        edit.risks()[0]
            .message()
            .contains("public function `grant`")
    );
    let document_edit = document_edit(&edit, &document);
    assert_eq!(document_edit.edits().len(), 1);
    assert_edit_at(document_edit.edits(), 0, 7, "award");
}

#[test]
fn rename_rejects_module_declaration_collision() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let text = "\
pub fn grant(amount: i64) -> i64 { return amount }
pub fn award(amount: i64) -> i64 { return amount + 1 }";
    let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

    assert_eq!(
        databases.rename(
            &document,
            Position::new(0, line(text, 0).find("grant").expect("grant declaration")),
            "award",
        ),
        None
    );
}

fn assert_edit_at(edits: &[TextEdit], line: usize, character: usize, new_text: &str) {
    assert!(
        edits.iter().any(|edit| {
            edit.range().start() == Position::new(line, character) && edit.new_text() == new_text
        }),
        "{edits:?}"
    );
}

fn document_edit<'a>(edit: &'a WorkspaceEdit, document_id: &DocumentId) -> &'a DocumentTextEdit {
    edit.document_edits()
        .iter()
        .find(|document_edit| document_edit.document_id() == document_id)
        .unwrap_or_else(|| panic!("workspace edit should contain {document_id:?}"))
}

fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}

fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
    databases_for_with_schema(files, RegistryFacts::default())
}

fn databases_for_with_schema(
    files: Vec<SourceFileSnapshot>,
    schema: RegistryFacts,
) -> LanguageServiceDatabases {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.set_schema_facts(schema);
    databases.update(&project);
    databases
}
