use std::collections::BTreeSet;

use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};
use vela_common::{Diagnostic, Span};

use super::*;
use crate::{
    DisplayPartKind, SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};

fn file(path: &str, text: &str) -> SourceFileSnapshot {
    SourceFileSnapshot::new(path, text)
}

fn project(files: &[SourceFileSnapshot]) -> crate::ProjectSources {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    assemble_project_sources(&config, files, &Workspace::new().snapshot())
}

#[test]
fn syntax_diagnostics_map_to_document_ranges() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[file(
        document.as_str(),
        "pub fn main( { return 1 }",
    )]));

    let diagnostics = db.diagnostics_for_document(&document);

    assert_eq!(diagnostics.document_id(), &document);
    assert_eq!(diagnostics.status(), DiagnosticStatus::Partial);
    assert!(!diagnostics.diagnostics().is_empty());
    assert!(
        diagnostics
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.range().is_some()
                && diagnostic.severity() == ServiceDiagnosticSeverity::Error)
    );
}

#[test]
fn open_file_diagnostics_are_prioritized() {
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[
        file(
            "/workspace/scripts/game/main.vela",
            "use game::reward::grant\npub fn main() { return grant() }",
        ),
        file(
            "/workspace/scripts/game/wrapper.vela",
            "use game::main::main\npub fn wrapped() { return main() }",
        ),
        file(
            "/workspace/scripts/game/reward.vela",
            "pub fn grant() { return 1 }",
        ),
    ]));
    let open_document = DocumentId::from("/workspace/scripts/game/wrapper.vela");
    let open_documents = BTreeSet::from([open_document.clone()]);
    db.update_with_open_documents(
        &project(&[
            file(
                "/workspace/scripts/game/main.vela",
                "use game::reward::grant\npub fn main() { return grant() }",
            ),
            file(
                "/workspace/scripts/game/wrapper.vela",
                "use game::main::main\npub fn wrapped() { return main() }",
            ),
            file(
                "/workspace/scripts/game/reward.vela",
                "pub fn grant_bonus() { return 1 }",
            ),
        ]),
        &open_documents,
    );

    let batch = db.diagnostics_for_open_documents(&open_documents);

    assert_eq!(batch.documents().len(), 1);
    assert_eq!(batch.documents()[0].document_id(), &open_document);
    assert_eq!(batch.documents()[0].status(), DiagnosticStatus::Partial);
    assert!(
        batch
            .pending_workspace_documents()
            .iter()
            .any(|document| document.as_str() == "/workspace/scripts/game/reward.vela")
    );
}

#[test]
fn hir_diagnostics_survive_multi_file_workspace() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[
        file(
            document.as_str(),
            "use game::reward::grant_bonus\npub fn main() { return 1 }",
        ),
        file(helper.as_str(), "pub fn grant() { return 1 }"),
    ]));

    let diagnostics = db.diagnostics_for_document(&document);
    let helper_diagnostics = db.diagnostics_for_document(&helper);

    assert!(
        diagnostics.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == Some("hir::unresolved_import")
                && diagnostic.range().is_some()
                && diagnostic
                    .labels()
                    .iter()
                    .any(|label| label.document_id() == &document)
        }),
        "{:?}",
        diagnostics.diagnostics()
    );
    assert!(helper_diagnostics.diagnostics().is_empty());
}

#[test]
fn unused_import_reports_warning() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[
        file(
            document.as_str(),
            "use game::reward::grant\npub fn main() { return 1 }",
        ),
        file(
            "/workspace/scripts/game/reward.vela",
            "pub fn grant() { return 1 }",
        ),
    ]));

    let diagnostics = db.diagnostics_for_document(&document);

    assert!(
        diagnostics.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == Some(UNUSED_IMPORT_CODE)
                && diagnostic.severity() == ServiceDiagnosticSeverity::Warning
                && diagnostic.range().is_some()
                && diagnostic.message() == "unused import `grant`"
                && diagnostic.message_parts().render() == "unused import `grant`"
                && diagnostic.symbol() == Some(&SymbolRef::Source("game::reward::grant".to_owned()))
        }),
        "{:?}",
        diagnostics.diagnostics()
    );
}

#[test]
fn unused_import_ignores_type_hint_use() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[
        file(
            document.as_str(),
            "use game::types::Reward\npub fn main(reward: Reward) { return reward.amount }",
        ),
        file(
            "/workspace/scripts/game/types.vela",
            "pub struct Reward { amount: i64 }",
        ),
    ]));

    let diagnostics = db.diagnostics_for_document(&document);

    assert!(
        diagnostics
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code() != Some(UNUSED_IMPORT_CODE)),
        "{:?}",
        diagnostics.diagnostics()
    );
}

#[test]
fn analysis_diagnostics_map_to_document_ranges() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[file(
        document.as_str(),
        "pub fn main(scores: Array<i64>) { return scores.frist() }",
    )]));

    let diagnostics = db.diagnostics_for_document(&document);

    assert!(
        diagnostics.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == Some("analysis::unknown_method")
                && diagnostic.range().is_some()
                && diagnostic
                    .labels()
                    .iter()
                    .any(|label| label.document_id() == &document)
        }),
        "{:?}",
        diagnostics.diagnostics()
    );
}

#[test]
fn analysis_diagnostics_report_unknown_match_pattern_variants() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[file(
        document.as_str(),
        "\
pub fn main(maybe: Option<i64>) {
    match maybe {
        Option::Smoe(value) => value,
        Option::None => 0,
    }
}",
    )]));

    let diagnostics = db.diagnostics_for_document(&document);

    assert!(
        diagnostics.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == Some("analysis::unknown_variant")
                && diagnostic
                    .message()
                    .contains("unknown variant `Smoe` for `Option`")
                && diagnostic.range().is_some()
                && diagnostic
                    .labels()
                    .iter()
                    .any(|label| label.message().contains("available variants: Some"))
        }),
        "{:?}",
        diagnostics.diagnostics()
    );
}

#[test]
fn analysis_diagnostics_report_schema_enum_match_pattern_variants() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut schema = RegistryFacts::default();
    schema.insert_type(
        "QuestState",
        TypeFact::enum_type("QuestState", None::<String>),
    );
    schema.insert_variant(
        "QuestState",
        "Active",
        TypeFact::enum_type("QuestState", Some("Active")),
    );
    schema.insert_variant(
        "QuestState",
        "Finished",
        TypeFact::enum_type("QuestState", Some("Finished")),
    );
    let mut db = LanguageServiceDatabases::new();
    db.set_schema_facts(schema);
    db.update(&project(&[file(
        document.as_str(),
        "\
pub fn main(quest: QuestState) {
    match quest {
        QuestState::Activ => 1,
        QuestState::Finished => 0,
    }
}",
    )]));

    let diagnostics = db.diagnostics_for_document(&document);

    assert!(
        diagnostics.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == Some("analysis::unknown_variant")
                && diagnostic
                    .labels()
                    .iter()
                    .any(|label| label.message().contains("Active"))
        }),
        "{:?}",
        diagnostics.diagnostics()
    );
}

#[test]
fn analysis_diagnostics_ignore_different_match_pattern_owners() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut schema = RegistryFacts::default();
    schema.insert_type(
        "QuestState",
        TypeFact::enum_type("QuestState", None::<String>),
    );
    schema.insert_variant(
        "QuestState",
        "Active",
        TypeFact::enum_type("QuestState", Some("Active")),
    );
    schema.insert_variant(
        "QuestState",
        "Finished",
        TypeFact::enum_type("QuestState", Some("Finished")),
    );
    let mut db = LanguageServiceDatabases::new();
    db.set_schema_facts(schema);
    db.update(&project(&[file(
        document.as_str(),
        "\
pub fn main(quest: QuestState) {
    match quest {
        Other::Activ => 1,
        QuestState::Active => 2,
        QuestState::Finished => 0,
    }
}",
    )]));

    let diagnostics = db.diagnostics_for_document(&document);

    assert!(
        diagnostics
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code() != Some("analysis::unknown_variant")),
        "{:?}",
        diagnostics.diagnostics()
    );
}

#[test]
fn syntax_errors_do_not_block_unaffected_module_diagnostics() {
    let broken_document = DocumentId::from("/workspace/scripts/game/broken.vela");
    let healthy_document = DocumentId::from("/workspace/scripts/game/reward.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[
        file(broken_document.as_str(), "pub fn broken( { return 1 }"),
        file(
            healthy_document.as_str(),
            "pub fn reward(scores: Array<i64>) { return scores.frist() }",
        ),
    ]));

    let broken_diagnostics = db.diagnostics_for_document(&broken_document);
    let healthy_diagnostics = db.diagnostics_for_document(&healthy_document);

    assert!(
        broken_diagnostics
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.severity() == ServiceDiagnosticSeverity::Error),
        "{:?}",
        broken_diagnostics.diagnostics()
    );
    assert!(
        healthy_diagnostics
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == Some("analysis::unknown_method")),
        "{:?}",
        healthy_diagnostics.diagnostics()
    );
}

#[test]
fn schema_diagnostics_degrade_to_any() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.mark_schema_missing("/workspace/target/vela/schema.json");
    db.update(&project(&[file(
        document.as_str(),
        "pub fn main(player: Player, scores: Array<i64>) {
                player.level
                scores.frist()
            }",
    )]));

    let diagnostics = db.diagnostics_for_document(&document);

    assert!(
        diagnostics
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == Some("schema::unavailable")),
        "{:?}",
        diagnostics.diagnostics()
    );
    assert!(
        diagnostics
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == Some("analysis::unknown_method")),
        "{:?}",
        diagnostics.diagnostics()
    );
    assert!(
        diagnostics
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code() != Some("analysis::unknown_field")),
        "{:?}",
        diagnostics.diagnostics()
    );
}

#[test]
fn analysis_diagnostics_report_missing_required_record_fields() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[file(
        document.as_str(),
        "\
struct Reward {
    amount: i64,
    reason: String = \"quest\",
}

pub fn main() {
    return Reward { reason: \"bonus\" }
}",
    )]));

    let diagnostics = db.diagnostics_for_document(&document);

    assert!(
        diagnostics.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == Some("analysis::missing_constructor_field")
                && diagnostic
                    .message()
                    .contains("missing constructor field `amount`")
                && diagnostic.range().is_some()
        }),
        "{:?}",
        diagnostics.diagnostics()
    );
    assert!(
        diagnostics
            .diagnostics()
            .iter()
            .all(|diagnostic| !diagnostic
                .message()
                .contains("missing constructor field `reason`")),
        "{:?}",
        diagnostics.diagnostics()
    );
}

#[test]
fn missing_schema_keeps_syntax_diagnostics_available() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.mark_schema_missing("/workspace/target/vela/schema.json");
    db.update(&project(&[file(
        document.as_str(),
        "pub fn main(player: Player) { return player.level ",
    )]));

    let diagnostics = db.diagnostics_for_document(&document);

    assert!(
        diagnostics
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == Some("schema::unavailable")),
        "{:?}",
        diagnostics.diagnostics()
    );
    assert!(
        diagnostics.diagnostics().iter().any(|diagnostic| {
            diagnostic.severity() == ServiceDiagnosticSeverity::Error
                && diagnostic.code() != Some("schema::unavailable")
                && diagnostic.range().is_some()
        }),
        "{:?}",
        diagnostics.diagnostics()
    );
}

#[test]
fn structured_diagnostics_preserve_candidates_and_repair_hints() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[file(
        document.as_str(),
        "pub fn main() { return levle }",
    )]));
    let source = db.parse_db().source_id(&document).expect("source exists");
    let diagnostic = Diagnostic::error("unknown name `levle`")
        .with_code("hir::unresolved_name")
        .with_span(Span::new(source, 23, 28))
        .with_label(Span::new(source, 23, 28), "unresolved binding")
        .with_candidate("level")
        .with_repair("replace with `level`", Span::new(source, 23, 28), "level");

    let converted = db.convert_diagnostic(&diagnostic);

    assert_eq!(converted.message(), "unknown name `levle`");
    assert_eq!(converted.message_parts().render(), "unknown name `levle`");
    assert_eq!(converted.symbol(), None);
    assert_eq!(
        converted.message_parts().parts()[0].kind(),
        DisplayPartKind::Text
    );
    assert_eq!(converted.labels().len(), 1);
    assert_eq!(converted.labels()[0].message(), "unresolved binding");
    assert_eq!(
        converted.labels()[0].message_parts().render(),
        "unresolved binding"
    );
    assert_eq!(converted.candidates().len(), 1);
    assert_eq!(converted.candidates()[0].replacement(), "level");
    assert_eq!(
        converted.candidates()[0].replacement_parts().render(),
        "level"
    );
    assert_eq!(converted.repair_hints().len(), 1);
    assert_eq!(converted.repair_hints()[0].document_id(), &document);
    assert_eq!(converted.repair_hints()[0].title(), "replace with `level`");
    assert_eq!(
        converted.repair_hints()[0].title_parts().render(),
        "replace with `level`"
    );
    assert_eq!(converted.repair_hints()[0].replacement(), "level");
    assert_eq!(
        converted.repair_hints()[0].replacement_parts().render(),
        "level"
    );
    assert_eq!(
        converted.repair_hints()[0].range().start(),
        Position::new(0, 23)
    );
    assert_eq!(
        converted.repair_hints()[0].range().end(),
        Position::new(0, 28)
    );
}

#[test]
fn partial_diagnostics_report_stale_generation() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[file(
        document.as_str(),
        "pub fn main(scores: Array<i64>) { return scores.frist() }",
    )]));
    let stale_generation = db.generation();

    db.update(&project(&[file(
        document.as_str(),
        "pub fn main(scores: Array<i64>) { return scores.first() }",
    )]));

    let stale = db.diagnostics_for_document_at_generation(&document, stale_generation);
    let current = db.diagnostics_for_document_at_generation(&document, db.generation());

    assert_eq!(stale.document_id(), &document);
    assert_eq!(stale.generation(), stale_generation);
    assert_eq!(stale.status(), DiagnosticStatus::Stale);
    assert!(stale.diagnostics().is_empty());
    assert_eq!(current.status(), DiagnosticStatus::Partial);
    assert_eq!(current.generation(), db.generation());
}

#[test]
fn workspace_diagnostics_include_background_documents() {
    let open_document = DocumentId::from("/workspace/scripts/game/main.vela");
    let workspace_document = DocumentId::from("/workspace/scripts/game/reward.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[
        file(open_document.as_str(), "pub fn main() { return 1 }"),
        file(
            workspace_document.as_str(),
            "pub fn reward(scores: Array<i64>) { return scores.frist() }",
        ),
    ]));
    let open_documents = BTreeSet::from([open_document.clone()]);

    let open_batch = db.diagnostics_for_open_documents(&open_documents);
    let workspace_batch = db.diagnostics_for_workspace_documents(&open_documents);

    assert_eq!(open_batch.documents().len(), 1);
    assert_eq!(open_batch.documents()[0].document_id(), &open_document);
    assert_eq!(workspace_batch.generation(), db.generation());
    assert_eq!(workspace_batch.documents().len(), 1);
    assert_eq!(
        workspace_batch.documents()[0].document_id(),
        &workspace_document
    );
    assert!(
        workspace_batch.documents()[0]
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == Some("analysis::unknown_method"))
    );
}
