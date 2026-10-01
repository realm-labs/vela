use crate::document_sync_test_support::{diagnostics, id, inputs, update};
use crate::matrix_fixture::{document_open as oracle, load, parse_markers};
use crate::{DiagnosticStatus, LanguageServiceDatabases, Position, SourceVersion, Workspace};

#[test]
fn document_change_matrix_preserves_complete_sources_repairs_restoration_and_stale_generations() {
    for crlf in [false, true] {
        let mut spec = load("document-change");
        if crlf {
            for text in spec.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let files = inputs(&spec);
        for case in spec.oracle["cases"].as_array().expect("cases") {
            let file = case["file"].as_str().expect("file");
            let doc_id = id(file);
            let initial = parse_markers(&spec.files[file]).expect("initial");
            let mut workspace = Workspace::new();
            let mut db = LanguageServiceDatabases::new();
            workspace.open_document(
                doc_id.clone(),
                initial.text.as_str(),
                SourceVersion::new(u64::from(u32::MAX - 6)),
            );
            update(&mut db, &files, &workspace);
            assert_eq!(
                diagnostics(&db, &doc_id),
                oracle::expected(&initial, case, doc_id.as_str())
            );
            for phase in case["steps"].as_array().expect("phases") {
                let source = phase["text"]
                    .as_str()
                    .expect("phase")
                    .replace('\n', if crlf { "\r\n" } else { "\n" });
                let doc = parse_markers(&source).expect("authored markers");
                let signed =
                    i32::try_from(phase["version"].as_i64().expect("version")).expect("i32");
                let version =
                    SourceVersion::new(u64::from(u32::from_ne_bytes(signed.to_ne_bytes())));
                let before = workspace.snapshot();
                let old_text = before
                    .document(&doc_id)
                    .expect("old source")
                    .text()
                    .to_owned();
                let previous = db.clone();
                let generation = db.generation();
                workspace.change_document(doc_id.clone(), doc.text.as_str(), version);
                update(&mut db, &files, &workspace);
                let current = workspace.document(&doc_id).expect("changed buffer");
                assert_eq!(
                    (current.text(), current.version()),
                    (doc.text.as_str(), version)
                );
                assert_eq!(
                    before.document(&doc_id).expect("frozen source").text(),
                    old_text
                );
                assert_eq!(previous.generation(), generation);
                assert_ne!(db.generation(), generation);
                assert_eq!(
                    workspace.snapshot().open_document_ids().collect::<Vec<_>>(),
                    vec![doc_id.clone()]
                );
                assert_eq!(
                    db.source_db().records().contains_key(&doc_id),
                    file.ends_with(".vela")
                );
                if let Some(record) = db.source_db().records().get(&doc_id) {
                    assert_eq!(
                        (record.text(), record.version()),
                        (doc.text.as_str(), version)
                    );
                }
                let expected = oracle::expected(&doc, phase, doc_id.as_str());
                assert_eq!(
                    diagnostics(&db, &doc_id),
                    expected,
                    "{file}/{}, CRLF={crlf}",
                    phase["id"]
                );
                assert_eq!(diagnostics(&db, &doc_id), expected, "repeat");
                let stale = db.diagnostics_for_document_at_generation(&doc_id, generation);
                assert_eq!(stale.status(), DiagnosticStatus::Stale);
                assert!(stale.diagnostics().is_empty());
                let mut fresh = LanguageServiceDatabases::new();
                update(&mut fresh, &files, &workspace);
                assert_eq!(diagnostics(&fresh, &doc_id), expected, "fresh");
            }
        }
    }
}

#[test]
fn document_change_dependency_phases_preserve_body_indexes_current_hover_and_frozen_snapshots() {
    for crlf in [false, true] {
        let mut s = load("document-change");
        if crlf {
            for text in s.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let files = inputs(&s);
        let caller = parse_markers(&s.files["scripts/open_caller.vela"]).expect("caller");
        let api = parse_markers(&s.files["scripts/open_api.vela"]).expect("api");
        let caller_id = id("scripts/open_caller.vela");
        let api_id = id("scripts/open_api.vela");
        let mut workspace = Workspace::new();
        workspace.open_document(
            caller_id.clone(),
            caller.text.as_str(),
            SourceVersion::new(1),
        );
        workspace.open_document(api_id.clone(), api.text.as_str(), SourceVersion::new(1));
        let mut db = LanguageServiceDatabases::new();
        update(&mut db, &files, &workspace);
        let frozen = db.clone();
        let marker = caller.markers["call"].start;
        let column = marker.byte - caller.text[..marker.byte].rfind('\n').map_or(0, |p| p + 1);
        let point = Position::new(marker.line, column);
        let old = frozen.hover(&caller_id, point).expect("original target");
        for (index, phase) in s.oracle["dependency"]
            .as_array()
            .expect("phases")
            .iter()
            .enumerate()
        {
            let source = parse_markers(
                &phase["text"]
                    .as_str()
                    .expect("text")
                    .replace('\n', if crlf { "\r\n" } else { "\n" }),
            )
            .expect("phase");
            let parses = db.parse_db().parse_count();
            let projects = db.project_db().rebuild_count();
            let hirs = db.hir_db().rebuild_count();
            workspace.change_document(
                api_id.clone(),
                source.text.as_str(),
                SourceVersion::new(index as u64 + 2),
            );
            update(&mut db, &files, &workspace);
            if phase["id"] == "body" {
                assert_eq!(db.parse_db().parse_count(), parses + 1);
                assert_eq!(db.project_db().rebuild_count(), projects);
                assert_eq!(db.hir_db().rebuild_count(), hirs + 1);
            }
            let hover = db.hover(&caller_id, point).expect("current target");
            assert_eq!(
                format!(
                    "```vela\n{}\n```\n\n_function_: {}\n\n{}",
                    hover.label(),
                    hover.detail(),
                    hover.docs().expect("docs")
                ),
                phase["hover"]
            );
            assert_eq!(db.hover(&caller_id, point), Some(hover.clone()));
            assert_eq!(frozen.hover(&caller_id, point), Some(old.clone()));
            assert!(diagnostics(&db, &caller_id).is_empty());
            let mut fresh = LanguageServiceDatabases::new();
            update(&mut fresh, &files, &workspace);
            assert_eq!(fresh.hover(&caller_id, point), Some(hover));
        }
    }
}
