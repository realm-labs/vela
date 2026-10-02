use crate::document_symbol_matrix_tests::project;
use crate::matrix_fixture::{
    Document, FixtureWorkspace, document_symbols as oracle, parse_markers,
};
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};
use serde_json::Value;

#[test]
fn document_symbol_recovery_preserves_exact_owned_trees_and_restores_current_ranges() {
    for crlf in [false, true] {
        let (fixture, authored) = oracle::recovery(crlf);
        let file = authored["file"].as_str().expect("file");
        let mut db = LanguageServiceDatabases::new();
        let mut failed = Vec::new();
        for case in authored["cases"].as_array().expect("cases") {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let damaged =
                    parse_markers(case["source"].as_str().expect("source")).expect("markers");
                // Damage -> clean repair -> same damage catches stale declaration
                // shapes as well as stale positions and excluded synthetic owners.
                for (doc, rows, parse_error) in [
                    (
                        &damaged,
                        &case["symbols"],
                        case["parseError"].as_bool().expect("parse policy"),
                    ),
                    (&fixture.disk[file], &authored["symbols"], false),
                    (
                        &damaged,
                        &case["symbols"],
                        case["parseError"].as_bool().expect("parse policy"),
                    ),
                ] {
                    update(&mut db, &fixture, file, doc);
                    check(
                        &db,
                        file,
                        doc,
                        rows,
                        parse_error,
                        case["id"].as_str().expect("case"),
                        crlf,
                    );
                    let mut fresh = LanguageServiceDatabases::new();
                    update(&mut fresh, &fixture, file, doc);
                    check(
                        &fresh,
                        file,
                        doc,
                        rows,
                        parse_error,
                        case["id"].as_str().expect("case"),
                        crlf,
                    );
                }
            }));
            if result.is_err() {
                failed.push(case["id"].as_str().expect("case"));
            }
        }
        assert!(failed.is_empty(), "failed recovery cases: {failed:?}");
    }
}

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/中文 % recovery/{file}"))
}

fn update(
    db: &mut LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    file: &str,
    doc: &Document,
) {
    let sources = fixture
        .disk
        .iter()
        .map(|(name, disk)| {
            SourceFileSnapshot::new(
                uri(name),
                if name == file {
                    doc.text.as_str()
                } else {
                    disk.text.as_str()
                },
            )
        })
        .collect::<Vec<_>>();
    db.update(&assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/中文 % recovery/scripts")]),
        &sources,
        &Workspace::new().snapshot(),
    ));
}

fn check(
    db: &LanguageServiceDatabases,
    file: &str,
    doc: &Document,
    rows: &Value,
    parse_error: bool,
    id: &str,
    crlf: bool,
) {
    let expected = oracle::expected(doc, rows, false);
    oracle::assert_ancestry(&expected, None);
    let actual = Value::Array(
        db.document_symbols(&uri(file))
            .iter()
            .map(project)
            .collect(),
    );
    assert_eq!(actual, expected, "complete tree {id} crlf={crlf}");
    for _ in 0..2 {
        assert_eq!(
            Value::Array(
                db.document_symbols(&uri(file))
                    .iter()
                    .map(project)
                    .collect()
            ),
            expected,
            "repeat {id}"
        );
    }
    assert_eq!(
        db.parse_db()
            .parse_diagnostics(&uri(file))
            .expect("parsed")
            .iter()
            .any(|d| d.code.as_deref() == Some("E_PARSE")),
        parse_error,
        "parse policy {id} crlf={crlf}"
    );
}
