use super::*;
use crate::{
    Position, SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};
use std::collections::BTreeSet;

fn file(path: &str, text: &str) -> SourceFileSnapshot {
    SourceFileSnapshot::new(path, text)
}

fn scaled_file(index: usize, lines: usize, value: usize) -> SourceFileSnapshot {
    let padding = (1..lines)
        .map(|line| format!("// scale padding {index}:{line}"))
        .collect::<Vec<_>>()
        .join("\n");
    file(
        &format!("/workspace/scripts/mod_{index}.vela"),
        &format!("pub fn value_{index}() {{ return {value} }}\n{padding}"),
    )
}

fn project(files: &[SourceFileSnapshot]) -> ProjectSources {
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    assemble_project_sources(&config, files, &Workspace::new().snapshot())
}

fn project_with_roots(files: &[SourceFileSnapshot], roots: &[&str]) -> ProjectSources {
    let config = WorkspaceConfig::workspace(roots.iter().copied().map(WorkspaceRoot::from));
    assemble_project_sources(&config, files, &Workspace::new().snapshot())
}

#[test]
fn language_service_symbols_keep_package_ownership() {
    let first = PackageId::new("com.example.first").expect("first package");
    let second = PackageId::new("com.example.second").expect("second package");
    let config = WorkspaceConfig::workspace([
        WorkspaceRoot::for_package("/workspace/first", first.clone()),
        WorkspaceRoot::for_package("/workspace/second", second.clone()),
    ]);
    let project = assemble_project_sources(
        &config,
        &[
            file(
                "/workspace/first/shared.vela",
                "pub fn value() { return 1; }",
            ),
            file(
                "/workspace/second/shared.vela",
                "pub fn value() { return 2; }",
            ),
        ],
        &Workspace::new().snapshot(),
    );
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);

    let module_symbols = databases
        .workspace_symbols("shared")
        .into_iter()
        .filter(|symbol| symbol.name() == "shared")
        .map(|symbol| symbol.symbol().clone())
        .collect::<Vec<_>>();
    assert!(module_symbols.contains(&SymbolRef::Source(format!("{first}::shared"))));
    assert!(module_symbols.contains(&SymbolRef::Source(format!("{second}::shared"))));
}

fn module(name: &str) -> ModuleKey {
    ModuleKey::new(PackageId::anonymous(), ModulePath::from_qualified(name))
}

#[test]
fn function_body_edit_does_not_invalidate_unrelated_modules() {
    let mut db = LanguageServiceDatabases::new();
    let main_document = DocumentId::from("/workspace/scripts/game/main.vela");
    db.update(&project(&[
        file(
            "/workspace/scripts/game/main.vela",
            "use game::reward::grant\npub fn main() { return grant() }",
        ),
        file(
            "/workspace/scripts/game/reward.vela",
            "pub fn grant() { return 1 }",
        ),
        file("/workspace/scripts/game/config.vela", "pub const value = 1"),
    ]));
    let before_parse_count = db.parse_db().parse_count();
    let before_project_rebuild_count = db.project_db().rebuild_count();
    let before_fingerprint = db
        .parse_db()
        .module_fingerprint(&module("game::main"))
        .expect("main fingerprint");

    let report = db.update(&project(&[
        file(
            "/workspace/scripts/game/main.vela",
            "use game::reward::grant\npub fn main() { return grant() + 1 }",
        ),
        file(
            "/workspace/scripts/game/reward.vela",
            "pub fn grant() { return 1 }",
        ),
        file("/workspace/scripts/game/config.vela", "pub const value = 1"),
    ]));

    assert_eq!(db.parse_db().parse_count() - before_parse_count, 1);
    assert!(report.changed_modules().contains(&module("game::main")));
    assert!(
        report
            .hir_invalidated_modules()
            .contains(&module("game::main"))
    );
    assert!(
        !report
            .hir_invalidated_modules()
            .contains(&module("game::reward"))
    );
    assert!(
        !report
            .hir_invalidated_modules()
            .contains(&module("game::config"))
    );
    assert!(report.declaration_changed_modules().is_empty());
    assert!(report.import_changed_modules().is_empty());
    assert_eq!(
        db.parse_db()
            .module_fingerprint(&module("game::main"))
            .expect("main fingerprint after body edit"),
        before_fingerprint
    );
    let syntax_parse = db
        .parse_db()
        .syntax_parse(&main_document)
        .expect("main syntax parse");
    assert!(syntax_parse.diagnostics().is_empty());
    assert_eq!(syntax_parse.tree().functions().count(), 1);
    assert_eq!(
        db.project_db().rebuild_count(),
        before_project_rebuild_count
    );
    assert_eq!(report.metrics().project_rebuild_count(), 0);
    assert_eq!(report.metrics().hir_rebuild_count(), 1);
}

#[test]
fn declaration_and_import_fingerprints_invalidate_project_indexes() {
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[
        file(
            "/workspace/scripts/game/main.vela",
            "use game::reward::grant\npub fn main() { return grant() }",
        ),
        file(
            "/workspace/scripts/game/reward.vela",
            "pub fn grant() { return 1 }",
        ),
        file(
            "/workspace/scripts/game/bonus.vela",
            "pub fn grant() { return 2 }",
        ),
    ]));
    let initial = db
        .parse_db()
        .module_fingerprint(&module("game::main"))
        .expect("main fingerprint");

    let declaration_report = db.update(&project(&[
        file(
            "/workspace/scripts/game/main.vela",
            "use game::reward::grant\npub fn main(amount: i64) { return grant() + amount }",
        ),
        file(
            "/workspace/scripts/game/reward.vela",
            "pub fn grant() { return 1 }",
        ),
        file(
            "/workspace/scripts/game/bonus.vela",
            "pub fn grant() { return 2 }",
        ),
    ]));
    let after_declaration = db
        .parse_db()
        .module_fingerprint(&module("game::main"))
        .expect("main fingerprint after declaration edit");

    assert_ne!(after_declaration.declaration(), initial.declaration());
    assert_eq!(after_declaration.import(), initial.import());
    assert!(
        declaration_report
            .declaration_changed_modules()
            .contains(&module("game::main"))
    );
    assert!(declaration_report.import_changed_modules().is_empty());
    assert_eq!(declaration_report.metrics().project_rebuild_count(), 1);

    let import_report = db.update(&project(&[
        file(
            "/workspace/scripts/game/main.vela",
            "use game::bonus::grant\npub fn main(amount: i64) { return grant() + amount }",
        ),
        file(
            "/workspace/scripts/game/reward.vela",
            "pub fn grant() { return 1 }",
        ),
        file(
            "/workspace/scripts/game/bonus.vela",
            "pub fn grant() { return 2 }",
        ),
    ]));
    let after_import = db
        .parse_db()
        .module_fingerprint(&module("game::main"))
        .expect("main fingerprint after import edit");

    assert_eq!(after_import.declaration(), after_declaration.declaration());
    assert_ne!(after_import.import(), after_declaration.import());
    assert!(import_report.declaration_changed_modules().is_empty());
    assert!(
        import_report
            .import_changed_modules()
            .contains(&module("game::main"))
    );
    assert_eq!(import_report.metrics().project_rebuild_count(), 1);
}

#[test]
fn import_edit_invalidates_reverse_dependencies() {
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
        file(
            "/workspace/scripts/game/bonus.vela",
            "pub fn grant() { return 2 }",
        ),
    ]));

    let report = db.update(&project(&[
        file(
            "/workspace/scripts/game/main.vela",
            "use game::bonus::grant\npub fn main() { return grant() }",
        ),
        file(
            "/workspace/scripts/game/wrapper.vela",
            "use game::main::main\npub fn wrapped() { return main() }",
        ),
        file(
            "/workspace/scripts/game/reward.vela",
            "pub fn grant() { return 1 }",
        ),
        file(
            "/workspace/scripts/game/bonus.vela",
            "pub fn grant() { return 2 }",
        ),
    ]));

    assert!(
        report
            .import_changed_modules()
            .contains(&module("game::main"))
    );
    assert!(
        report
            .hir_invalidated_modules()
            .contains(&module("game::main"))
    );
    assert!(
        report
            .hir_invalidated_modules()
            .contains(&module("game::wrapper"))
    );
    assert!(
        !report
            .hir_invalidated_modules()
            .contains(&module("game::reward"))
    );
    assert_eq!(report.metrics().hir_rebuild_count(), 1);
}

#[test]
fn declaration_edit_invalidates_dependent_modules() {
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[
        file(
            "/workspace/scripts/game/main.vela",
            "use game::reward::grant\npub fn main() { return grant() }",
        ),
        file(
            "/workspace/scripts/game/reward.vela",
            "pub fn grant() { return 1 }",
        ),
    ]));

    let report = db.update(&project(&[
        file(
            "/workspace/scripts/game/main.vela",
            "use game::reward::grant\npub fn main() { return grant() }",
        ),
        file(
            "/workspace/scripts/game/reward.vela",
            "pub fn grant_bonus() { return 1 }",
        ),
    ]));

    assert!(
        report
            .declaration_changed_modules()
            .contains(&module("game::reward"))
    );
    assert!(
        report
            .hir_invalidated_modules()
            .contains(&module("game::reward"))
    );
    assert!(
        report
            .hir_invalidated_modules()
            .contains(&module("game::main"))
    );
}

#[test]
fn module_path_change_invalidates_hir_without_text_reparse() {
    let files = [
        file(
            "/workspace/scripts/game/main.vela",
            "use game::reward::grant\npub fn main() { return grant() }",
        ),
        file(
            "/workspace/scripts/game/reward.vela",
            "pub fn grant() { return 1 }",
        ),
    ];
    let mut db = LanguageServiceDatabases::new();
    db.update(&project_with_roots(&files, &["/workspace/scripts/game"]));
    let before_parse_count = db.parse_db().parse_count();

    let report = db.update(&project_with_roots(&files, &["/workspace/scripts"]));

    assert_eq!(db.parse_db().parse_count(), before_parse_count);
    assert!(report.changed_modules().contains(&module("main")));
    assert!(report.changed_modules().contains(&module("game::main")));
    assert!(
        report
            .hir_invalidated_modules()
            .contains(&module("game::main"))
    );
    assert!(
        report
            .hir_invalidated_modules()
            .contains(&module("game::reward"))
    );
    assert_eq!(report.metrics().hir_rebuild_count(), 1);
}

#[test]
fn project_config_invalidation_rebuilds_module_paths() {
    let files = [
        file(
            "/workspace/scripts/game/main.vela",
            "use game::reward::grant\npub fn main() { return grant() }",
        ),
        file(
            "/workspace/scripts/game/reward.vela",
            "pub fn grant() { return 1 }",
        ),
    ];
    let reward = DocumentId::from("/workspace/scripts/game/reward.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&project_with_roots(&files, &["/workspace"]));
    assert_eq!(
        db.project_db().module_by_document().get(&reward),
        Some(&module("scripts::game::reward"))
    );
    let before_generation = db.generation();

    db.invalidate_project_config();
    assert!(db.generation() > before_generation);
    db.update(&project_with_roots(&files, &["/workspace/scripts"]));

    assert_eq!(
        db.project_db().module_by_document().get(&reward),
        Some(&module("game::reward"))
    );
    assert_eq!(db.parse_db().parse_count(), 2);
}

#[test]
fn project_config_update_rebuilds_with_one_generation_commit() {
    let files = [file(
        "/workspace/scripts/game/main.vela",
        "pub fn main() { return 1 }",
    )];
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&project_with_roots(&files, &["/workspace"]));
    let before_generation = db.generation();

    db.update_after_project_config_change_with_open_documents(
        &project_with_roots(&files, &["/workspace/scripts"]),
        &BTreeSet::new(),
    );

    assert_eq!(db.generation().get(), before_generation.get() + 1);
    assert_eq!(
        db.project_db().module_by_document().get(&document),
        Some(&module("game::main"))
    );
    assert_eq!(db.parse_db().parse_count(), 1);
}

#[test]
fn stale_background_diagnostics_are_not_published() {
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[file(
        "/workspace/scripts/game/main.vela",
        "pub fn main() { return 1 }",
    )]));
    let stale = db.begin_background_request();

    db.update(&project(&[file(
        "/workspace/scripts/game/main.vela",
        "pub fn main() { return 2 }",
    )]));

    let result = BackgroundResult::new(stale, vec!["old diagnostic"]);
    assert_eq!(db.accept_background_result(result), None);
    let fresh = db.begin_background_request();
    let result = BackgroundResult::new(fresh, vec!["current diagnostic"]);
    assert_eq!(
        db.accept_background_result(result),
        Some(vec!["current diagnostic"])
    );
}

#[test]
fn cancelled_background_diagnostics_are_not_published() {
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&[file(
        "/workspace/scripts/game/main.vela",
        "pub fn main() { return 1 }",
    )]));
    let (token, cancellation) = db.begin_cancellable_background_request();
    cancellation.cancel();

    let result = BackgroundResult::new(token, vec!["cancelled diagnostic"]);

    assert_eq!(db.accept_background_result(result), None);
}

#[test]
fn open_file_recomputation_is_scheduled_before_workspace_work() {
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
    let open_documents = BTreeSet::from([DocumentId::from("/workspace/scripts/game/wrapper.vela")]);

    let report = db.update_with_open_documents(
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

    assert_eq!(
        report.scheduled_modules()[0].module(),
        &module("game::wrapper")
    );
    assert_eq!(report.scheduled_modules()[0].priority(), WorkPriority::Open);
    assert!(
        report
            .scheduled_modules()
            .iter()
            .skip(1)
            .any(|scheduled| scheduled.module() == &module("game::reward")
                && scheduled.priority() == WorkPriority::Workspace)
    );
}

#[test]
fn scale_fixture_reparses_single_body_edit_and_refreshes_hir() {
    let mut files = (0..128)
        .map(|index| {
            file(
                &format!("/workspace/scripts/mod_{index}.vela"),
                &format!("pub fn value_{index}() {{ return {index} }}"),
            )
        })
        .collect::<Vec<_>>();
    let mut db = LanguageServiceDatabases::new();
    db.update(&project(&files));
    let before_parse_count = db.parse_db().parse_count();

    files[42] = file(
        "/workspace/scripts/mod_42.vela",
        "pub fn value_42() { return 4200 }",
    );
    let report = db.update(&project(&files));

    assert_eq!(db.parse_db().parse_count() - before_parse_count, 1);
    assert_eq!(report.reparsed_documents().len(), 1);
    assert!(report.changed_modules().contains(&module("mod_42")));
    assert_eq!(report.hir_invalidated_modules().len(), 1);
    assert_eq!(report.metrics().source_count(), 128);
    assert_eq!(report.metrics().parsed_document_count(), 128);
    assert_eq!(report.metrics().reparsed_document_count(), 1);
    assert_eq!(report.metrics().hir_rebuild_count(), 1);
    assert!(report.metrics().total_lines() >= 128);
    assert!(report.metrics().total_bytes() > 0);
}

#[test]
fn larger_synthetic_workspace_reports_indexing_metrics() {
    const MODULES: usize = 512;
    const LINES_PER_MODULE: usize = 64;

    let mut files = (0..MODULES)
        .map(|index| scaled_file(index, LINES_PER_MODULE, index))
        .collect::<Vec<_>>();
    let mut db = LanguageServiceDatabases::new();

    let initial = db.update(&project(&files));

    assert_eq!(initial.metrics().source_count(), MODULES);
    assert_eq!(initial.metrics().parsed_document_count(), MODULES);
    assert_eq!(initial.metrics().reparsed_document_count(), MODULES);
    assert_eq!(initial.metrics().hir_rebuild_count(), 1);
    assert!(initial.metrics().elapsed_micros() > 0);
    assert!(initial.metrics().total_lines() >= MODULES * LINES_PER_MODULE);
    assert!(initial.metrics().total_bytes() >= MODULES * LINES_PER_MODULE);

    files[300] = scaled_file(300, LINES_PER_MODULE, 3000);
    let report = db.update(&project(&files));

    assert_eq!(report.metrics().source_count(), MODULES);
    assert_eq!(report.metrics().parsed_document_count(), MODULES);
    assert_eq!(report.metrics().reparsed_document_count(), 1);
    assert_eq!(report.metrics().hir_rebuild_count(), 1);
    assert_eq!(report.reparsed_documents().len(), 1);
    assert_eq!(report.hir_invalidated_modules().len(), 1);
}

#[test]
#[ignore = "explicit Phase 18 scale checkpoint for roughly one million lines"]
fn million_line_synthetic_workspace_checkpoint_refreshes_hir_for_body_edit() {
    const MODULES: usize = 2_048;
    const LINES_PER_MODULE: usize = 512;

    let mut files = (0..MODULES)
        .map(|index| scaled_file(index, LINES_PER_MODULE, index))
        .collect::<Vec<_>>();
    let mut db = LanguageServiceDatabases::new();

    let initial = db.update(&project(&files));

    assert_eq!(initial.metrics().source_count(), MODULES);
    assert_eq!(initial.metrics().parsed_document_count(), MODULES);
    assert_eq!(initial.metrics().reparsed_document_count(), MODULES);
    assert_eq!(initial.metrics().hir_rebuild_count(), 1);
    assert!(initial.metrics().total_lines() >= 1_000_000);
    assert!(initial.metrics().total_bytes() >= initial.metrics().total_lines());

    files[1_337] = scaled_file(1_337, LINES_PER_MODULE, 13_370);
    let report = db.update(&project(&files));

    assert_eq!(report.metrics().source_count(), MODULES);
    assert_eq!(report.metrics().parsed_document_count(), MODULES);
    assert_eq!(report.metrics().reparsed_document_count(), 1);
    assert_eq!(report.metrics().hir_rebuild_count(), 1);
    assert_eq!(report.reparsed_documents().len(), 1);
    assert_eq!(report.hir_invalidated_modules().len(), 1);
    assert!(report.metrics().total_lines() >= 1_000_000);
}

fn memoization_project(main_body: &str) -> ProjectSources {
    project(&[
        file(
            "/workspace/scripts/game/main.vela",
            &format!("use game::reward::grant\npub fn main() {{ {main_body} }}"),
        ),
        file(
            "/workspace/scripts/game/reward.vela",
            "pub fn grant() { return 1 }",
        ),
    ])
}

#[test]
fn requests_in_one_generation_share_a_single_analysis_facts_build() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&memoization_project(
        "let total = grant()\n    return total",
    ));

    assert_eq!(db.analysis_facts_build_count(), 0);
    let _ = db.diagnostics_for_document(&document);
    assert!(db.analysis_facts_build_count() > 0);

    let _ = db.hover(&document, Position::new(1, 30));
    let _ = db.completion_items(&document, Position::new(1, 30));
    let _ = db.semantic_tokens(&document);
    let _ = db.diagnostics_for_document(&document);

    // Only the schema-free and the schema-backed whole-workspace builds exist,
    // however many requests the editor sends within the generation.
    assert_eq!(db.analysis_facts_build_count(), 2);
}

#[test]
fn editing_a_module_rebuilds_analysis_facts_and_reports_the_new_body() {
    // `total` sits at line 1, column 21 of `pub fn main() { let total = ...`.
    let caret = Position::new(1, 21);
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&memoization_project("let total = 1\n    return total"));
    let before_hover = db
        .hover(&document, caret)
        .expect("hover on the numeric let");
    let before = db.analysis_facts_build_count();

    db.update(&memoization_project(
        "let total = \"text\"\n    return total",
    ));
    let after_hover = db.hover(&document, caret).expect("hover on the string let");

    assert!(db.analysis_facts_build_count() > before);
    assert!(
        before_hover.detail().contains("i64"),
        "unexpected detail before the edit: {}",
        before_hover.detail()
    );
    assert!(
        after_hover.detail().contains("String"),
        "the rebuilt facts should describe the edited body, got: {}",
        after_hover.detail()
    );
}

#[test]
fn schema_reload_rebuilds_only_schema_backed_analysis_facts() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&memoization_project("return grant()"));

    let _ = db.hover(&document, Position::new(1, 22));
    let _ = db.diagnostics_for_document(&document);
    assert_eq!(db.analysis_facts_build_count(), 2);

    db.set_schema_facts(RegistryFacts::default());

    let _ = db.hover(&document, Position::new(1, 22));
    assert_eq!(
        db.analysis_facts_build_count(),
        2,
        "schema-free facts do not read the schema and stay valid"
    );

    let _ = db.diagnostics_for_document(&document);
    assert_eq!(db.analysis_facts_build_count(), 3);
}

#[test]
fn a_database_snapshot_reuses_the_facts_its_source_already_built() {
    let document = DocumentId::from("/workspace/scripts/game/main.vela");
    let mut db = LanguageServiceDatabases::new();
    db.update(&memoization_project("return grant()"));
    let _ = db.diagnostics_for_document(&document);
    let built = db.analysis_facts_build_count();

    let snapshot = db.clone();
    let _ = snapshot.diagnostics_for_document(&document);

    assert_eq!(snapshot.analysis_facts_build_count(), built);
}
