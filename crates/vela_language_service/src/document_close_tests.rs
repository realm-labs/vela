use crate::document_sync_test_support::{diagnostics, id, inputs, update};
use crate::matrix_fixture::{Document, Spec, document_open as oracle, load, parse_markers};
use crate::{
    DiagnosticStatus, LanguageServiceDatabases, Position, SourceFileSnapshot, SourceVersion,
    Workspace,
};

fn spec(crlf: bool) -> Spec {
    let mut spec = load("document-change");
    if crlf {
        for text in spec.files.values_mut() {
            *text = text.replace('\n', "\r\n");
        }
    }
    spec
}
fn doc(text: &str, crlf: bool) -> Document {
    parse_markers(&text.replace('\n', if crlf { "\r\n" } else { "\n" })).expect("authored input")
}

#[test]
fn document_close_matrix_restores_full_disk_facts_removes_scratch_and_preserves_frozen_overlays() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let files = inputs(&s);
        for case in s.oracle["cases"].as_array().expect("cases") {
            let file = case["file"].as_str().expect("file");
            let uri = id(file);
            let disk = parse_markers(&s.files[file]).expect("disk");
            let mut workspace = Workspace::new();
            let mut db = LanguageServiceDatabases::new();
            update(&mut db, &files, &workspace);
            for phase in &case["steps"].as_array().expect("phases")[1..] {
                let overlay = doc(phase["text"].as_str().expect("text"), crlf);
                workspace.open_document(
                    uri.clone(),
                    overlay.text.as_str(),
                    SourceVersion::new(u32::MAX.into()),
                );
                update(&mut db, &files, &workspace);
                assert_eq!(
                    diagnostics(&db, &uri),
                    oracle::expected(&overlay, phase, uri.as_str())
                );
                let frozen = db.clone();
                let before = workspace.snapshot();
                let generation = db.generation();
                workspace.close_document(&uri);
                update(&mut db, &files, &workspace);
                assert!(workspace.snapshot().open_document_ids().next().is_none());
                assert!(workspace.document(&uri).is_none());
                assert_eq!(
                    before.document(&uri).expect("frozen overlay").text(),
                    overlay.text
                );
                assert_eq!(frozen.generation(), generation);
                assert_eq!(
                    diagnostics(&frozen, &uri),
                    oracle::expected(&overlay, phase, uri.as_str())
                );
                let present = file.ends_with(".vela") && !oracle::missing_disk(&s, file);
                assert_eq!(db.source_db().records().contains_key(&uri), present);
                let expected = if present {
                    oracle::expected(&disk, case, uri.as_str())
                } else {
                    Vec::new()
                };
                if present {
                    let source = &db.source_db().records()[&uri];
                    assert_eq!(
                        (source.text(), source.version()),
                        (disk.text.as_str(), SourceVersion::INITIAL)
                    );
                }
                assert_eq!(diagnostics(&db, &uri), expected);
                assert_eq!(diagnostics(&db, &uri), expected, "repeat");
                if file.ends_with(".vela") {
                    assert_eq!(
                        db.diagnostics_for_document_at_generation(&uri, generation)
                            .status(),
                        DiagnosticStatus::Stale
                    );
                }
                let mut fresh = LanguageServiceDatabases::new();
                update(&mut fresh, &files, &workspace);
                assert_eq!(diagnostics(&fresh, &uri), expected);
                let workspace_generation = workspace.generation();
                workspace.close_document(&uri);
                workspace.close_document(&id("scripts/unopened.vela"));
                assert_eq!(workspace.generation(), workspace_generation);
                update(&mut db, &files, &workspace);
                assert_eq!(diagnostics(&db, &uri), expected);
            }
        }
    }
}

#[test]
fn document_close_uses_supplied_current_disk_revisions_deletion_and_recreation() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let case = s.oracle["cases"]
            .as_array()
            .expect("cases")
            .iter()
            .find(|c| c["file"] == "scripts/invalid.vela")
            .expect("case");
        let uri = id("scripts/invalid.vela");
        let repaired = doc(case["steps"][1]["text"].as_str().expect("repair"), crlf);
        let shifted = doc(case["steps"][0]["text"].as_str().expect("shifted"), crlf);
        let mut files = inputs(&s);
        let mut workspace = Workspace::new();
        let mut db = LanguageServiceDatabases::new();
        workspace.open_document(uri.clone(), repaired.text.as_str(), SourceVersion::new(9));
        update(&mut db, &files, &workspace);
        let frozen = db.clone();
        assert!(diagnostics(&db, &uri).is_empty());
        files.retain(|f| f.document_id() != &uri);
        files.push(SourceFileSnapshot::new(uri.clone(), shifted.text.as_str()));
        workspace.close_document(&uri);
        update(&mut db, &files, &workspace);
        assert_eq!(db.source_db().records()[&uri].text(), shifted.text);
        assert_eq!(
            diagnostics(&db, &uri),
            oracle::expected(&shifted, &case["steps"][0], uri.as_str())
        );
        assert!(diagnostics(&frozen, &uri).is_empty());
        workspace.open_document(uri.clone(), shifted.text.as_str(), SourceVersion::new(1));
        update(&mut db, &files, &workspace);
        files.retain(|f| f.document_id() != &uri);
        workspace.close_document(&uri);
        update(&mut db, &files, &workspace);
        assert!(!db.source_db().records().contains_key(&uri));
        assert!(diagnostics(&db, &uri).is_empty());
        files.push(SourceFileSnapshot::new(uri.clone(), repaired.text.as_str()));
        update(&mut db, &files, &workspace);
        assert_eq!(db.source_db().records()[&uri].text(), repaired.text);
        assert!(diagnostics(&db, &uri).is_empty());
        let mut fresh = LanguageServiceDatabases::new();
        update(&mut fresh, &files, &workspace);
        assert_eq!(diagnostics(&db, &uri), diagnostics(&fresh, &uri));
    }
}

#[test]
fn document_close_rebinds_importer_hover_to_disk_and_keeps_overlay_snapshots() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let files = inputs(&s);
        let caller = parse_markers(&s.files["scripts/open_caller.vela"]).expect("caller");
        let api = id("scripts/open_api.vela");
        let caller_id = id("scripts/open_caller.vela");
        let marker = caller.markers["call"].start;
        let column = marker.byte - caller.text[..marker.byte].rfind('\n').map_or(0, |p| p + 1);
        let point = Position::new(marker.line, column);
        let mut workspace = Workspace::new();
        let mut db = LanguageServiceDatabases::new();
        workspace.open_document(
            caller_id.clone(),
            caller.text.as_str(),
            SourceVersion::new(7),
        );
        update(&mut db, &files, &workspace);
        let disk_hover = db.hover(&caller_id, point).expect("disk target");
        assert_eq!(
            format!(
                "```vela\n{}\n```\n\n_function_: {}\n\n{}",
                disk_hover.label(),
                disk_hover.detail(),
                disk_hover.docs().expect("docs")
            ),
            s.oracle["dependency"][0]["hover"]
        );
        for phase in &s.oracle["dependency"].as_array().expect("phases")[1..] {
            let overlay = doc(phase["text"].as_str().expect("text"), crlf);
            workspace.open_document(
                api.clone(),
                overlay.text.as_str(),
                SourceVersion::new(u32::MAX.into()),
            );
            update(&mut db, &files, &workspace);
            let frozen = db.clone();
            let old = frozen.hover(&caller_id, point).expect("overlay target");
            assert_eq!(
                format!(
                    "```vela\n{}\n```\n\n_function_: {}\n\n{}",
                    old.label(),
                    old.detail(),
                    old.docs().expect("docs")
                ),
                phase["hover"]
            );
            workspace.close_document(&api);
            update(&mut db, &files, &workspace);
            assert_eq!(
                workspace.snapshot().open_document_ids().collect::<Vec<_>>(),
                vec![caller_id.clone()]
            );
            assert_eq!(
                workspace.document(&caller_id).expect("caller").version(),
                SourceVersion::new(7)
            );
            assert_eq!(db.hover(&caller_id, point), Some(disk_hover.clone()));
            assert_eq!(frozen.hover(&caller_id, point), Some(old));
            assert!(diagnostics(&db, &caller_id).is_empty());
            let mut fresh = LanguageServiceDatabases::new();
            update(&mut fresh, &files, &workspace);
            assert_eq!(db.hover(&caller_id, point), fresh.hover(&caller_id, point));
        }
    }
}
