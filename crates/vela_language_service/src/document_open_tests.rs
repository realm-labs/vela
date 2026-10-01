use crate::matrix_fixture::{FixtureWorkspace, document_open as oracle, parse_markers};
use crate::{
    DiagnosticRange, DiagnosticStatus, DocumentId, LanguageServiceDatabases, Position,
    SourceFileSnapshot, SourceVersion, Workspace, WorkspaceConfig, WorkspaceRoot,
    assemble_project_sources,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn id(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/中文 % open/{file}"))
}
fn range(text: &str, r: DiagnosticRange) -> Value {
    let point = |p: Position| {
        let line = text
            .split('\n')
            .nth(p.line)
            .expect("existing diagnostic line");
        json!({"line":p.line,"character":line[..p.character].encode_utf16().count()})
    };
    json!({"start":point(r.start()),"end":point(r.end())})
}
fn diagnostics(db: &LanguageServiceDatabases, document: &DocumentId) -> Vec<Value> {
    let text = db
        .source_db()
        .records()
        .get(document)
        .map_or("", |r| r.text());
    db.diagnostics_for_document(document).diagnostics().iter().map(|d| json!({
        "code":d.code(),"message":d.message(),"severity":match d.severity() {
            crate::ServiceDiagnosticSeverity::Error=>1, crate::ServiceDiagnosticSeverity::Warning=>2,
            crate::ServiceDiagnosticSeverity::Note=>3,crate::ServiceDiagnosticSeverity::Help=>4 },
        "range":d.range().map(|r|range(text,r)),
        "labels":d.labels().iter().map(|l|json!({"uri":l.document_id().as_str(),
            "range":range(db.source_db().records()[l.document_id()].text(),l.range()),"message":l.message()})).collect::<Vec<_>>(),
        "candidates":d.candidates().iter().map(|c|c.replacement()).collect::<Vec<_>>(),"repairHints":d.repair_hints().len()
    })).collect()
}
fn inputs(spec: &crate::matrix_fixture::Spec) -> Vec<SourceFileSnapshot> {
    let fixture = FixtureWorkspace::new(spec).expect("fixture");
    fixture
        .disk
        .iter()
        .filter(|(f, _)| f.ends_with(".vela") && !oracle::missing_disk(spec, f))
        .map(|(f, d)| SourceFileSnapshot::new(id(f), d.text.as_str()))
        .collect()
}
fn update(db: &mut LanguageServiceDatabases, files: &[SourceFileSnapshot], workspace: &Workspace) {
    let config =
        WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/中文 % open/scripts")]);
    db.update_with_open_documents(
        &assemble_project_sources(&config, files, &workspace.snapshot()),
        &workspace
            .snapshot()
            .open_document_ids()
            .collect::<BTreeSet<_>>(),
    );
}

#[test]
fn document_open_matrix_preserves_exact_sources_versions_diagnostics_and_old_snapshots() {
    for crlf in [false, true] {
        let spec = oracle::spec(crlf);
        let files = inputs(&spec);
        for case in spec.oracle["cases"].as_array().expect("cases") {
            let file = case["file"].as_str().expect("file");
            let doc_id = id(file);
            let mut workspace = Workspace::new();
            let mut db = LanguageServiceDatabases::new();
            for source in &files {
                workspace.set_disk_snapshot(
                    source.document_id().clone(),
                    source.text(),
                    SourceVersion::INITIAL,
                );
            }
            update(&mut db, &files, &workspace);
            for (index, signed) in [0_i32, -1, i32::MAX, i32::MIN].into_iter().enumerate() {
                let doc = oracle::document(&spec, file, index % 2 == 1);
                let before = workspace.snapshot();
                let before_db = db.clone();
                let generation = db.generation();
                let old = before
                    .document(&doc_id)
                    .map(|d| (d.text().to_owned(), d.version()));
                let version =
                    SourceVersion::new(u64::from(u32::from_ne_bytes(signed.to_ne_bytes())));
                workspace.open_document(doc_id.clone(), doc.text.as_str(), version);
                update(&mut db, &files, &workspace);
                let current = workspace.snapshot();
                let actual = current.document(&doc_id).expect("opened text");
                assert_eq!(
                    (actual.text(), actual.version()),
                    (doc.text.as_str(), version)
                );
                assert_eq!(
                    current.open_document_ids().collect::<Vec<_>>(),
                    vec![doc_id.clone()]
                );
                assert_eq!(
                    before
                        .document(&doc_id)
                        .map(|d| (d.text().to_owned(), d.version())),
                    old
                );
                assert_eq!(before_db.generation(), generation);
                assert_ne!(db.generation(), generation);
                let stale = db.diagnostics_for_document_at_generation(&doc_id, generation);
                assert_eq!(stale.status(), DiagnosticStatus::Stale);
                assert!(stale.diagnostics().is_empty());
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
                let expected = oracle::expected(&doc, case, doc_id.as_str());
                assert_eq!(
                    diagnostics(&db, &doc_id),
                    expected,
                    "{file}, CRLF={crlf}, version={signed}"
                );
                assert_eq!(diagnostics(&db, &doc_id), expected, "repeat");
                let mut fresh = LanguageServiceDatabases::new();
                update(&mut fresh, &files, &workspace);
                assert_eq!(diagnostics(&fresh, &doc_id), expected, "fresh");
            }
        }
    }
}

#[test]
fn document_open_dependency_body_and_signature_changes_preserve_current_and_frozen_facts() {
    for crlf in [false, true] {
        let spec = oracle::spec(crlf);
        let files = inputs(&spec);
        let mut workspace = Workspace::new();
        let caller = oracle::document(&spec, "scripts/open_caller.vela", false);
        let caller_id = id("scripts/open_caller.vela");
        let api_id = id("scripts/open_api.vela");
        workspace.open_document(
            caller_id.clone(),
            caller.text.as_str(),
            SourceVersion::new(1),
        );
        let mut db = LanguageServiceDatabases::new();
        update(&mut db, &files, &workspace);
        let frozen = db.clone();
        let point = caller.markers["call"].start;
        let byte_col = point.byte - caller.text[..point.byte].rfind('\n').map_or(0, |p| p + 1);
        let query = Position::new(point.line, byte_col);
        let old = frozen.hover(&caller_id, query).expect("disk target");
        for (index, phase) in spec.oracle["dependency"]
            .as_array()
            .expect("phases")
            .iter()
            .enumerate()
        {
            let doc = parse_markers(
                &phase["text"]
                    .as_str()
                    .expect("phase text")
                    .replace('\n', if crlf { "\r\n" } else { "\n" }),
            )
            .expect("markers");
            workspace.open_document(
                api_id.clone(),
                doc.text.as_str(),
                SourceVersion::new(index as u64 + 2),
            );
            update(&mut db, &files, &workspace);
            let hover = db.hover(&caller_id, query).expect("current target");
            let markdown = format!(
                "```vela\n{}\n```\n\n_function_: {}\n\n{}",
                hover.label(),
                hover.detail(),
                hover.docs().expect("docs")
            );
            assert_eq!(markdown, phase["hover"]);
            assert_eq!(db.hover(&caller_id, query), Some(hover.clone()));
            assert_eq!(frozen.hover(&caller_id, query), Some(old.clone()));
            assert!(diagnostics(&db, &caller_id).is_empty());
            let mut fresh = LanguageServiceDatabases::new();
            update(&mut fresh, &files, &workspace);
            assert_eq!(fresh.hover(&caller_id, query), Some(hover));
        }
    }
}
