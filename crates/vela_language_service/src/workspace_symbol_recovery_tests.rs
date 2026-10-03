use crate::matrix_fixture::{FixtureWorkspace, parse_markers, workspace_symbols as oracle};
use crate::{
    DocumentId, LanguageServiceDatabases, SourceFileSnapshot, Workspace, WorkspaceConfig,
    WorkspaceRoot, assemble_project_sources,
};
use serde_json::{Value, json};

fn uri(file: &str) -> String {
    format!("/workspace/中文 % workspace recovery/{file}")
}

fn update(db: &mut LanguageServiceDatabases, fixture: &FixtureWorkspace) {
    let sources = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, doc)| SourceFileSnapshot::new(DocumentId::from(uri(file)), doc.text.as_str()))
        .collect::<Vec<_>>();
    db.update(&assemble_project_sources(
        &WorkspaceConfig::workspace([WorkspaceRoot::from(uri("scripts"))]),
        &sources,
        &Workspace::new().snapshot(),
    ));
}

#[test]
fn workspace_symbol_recovery_pins_whole_query_sets_through_damage_repair_and_unicode_shifts() {
    for crlf in [false, true] {
        for shifted in [false, true] {
            let (fixture, authored) = oracle::recovery(crlf, shifted);
            let file = authored["file"].as_str().expect("file");
            let schema = json!({"formatVersion":1,"facts":authored["schema"]}).to_string();
            let mut db = LanguageServiceDatabases::new();
            update(&mut db, &fixture);
            db.load_schema_artifact_json(&uri("schema.json"), &schema);
            for case in authored["cases"].as_array().expect("cases") {
                let damage = parse_markers(case["source"].as_str().expect("source"))
                    .expect("damage markers");
                for (doc, expected, parse_error) in [
                    (
                        &damage,
                        case,
                        case["parseError"].as_bool().expect("parse policy"),
                    ),
                    (&fixture.disk[file], &authored, false),
                    (
                        &damage,
                        case,
                        case["parseError"].as_bool().expect("parse policy"),
                    ),
                ] {
                    let mut current = fixture.clone();
                    current.disk.insert(file.to_owned(), doc.clone());
                    update(&mut db, &current);
                    let mut fresh = LanguageServiceDatabases::new();
                    update(&mut fresh, &current);
                    fresh.load_schema_artifact_json(&uri("schema.json"), &schema);
                    for actual in [&db, &fresh] {
                        assert!(
                            actual.schema_db().diagnostics().is_empty(),
                            "healthy static schema"
                        );
                        assert_eq!(actual.source_db().records().len(), 2);
                        assert_eq!(
                            actual
                                .parse_db()
                                .parse_diagnostics(&DocumentId::from(uri(file)))
                                .expect("parsed")
                                .iter()
                                .any(|d| d.code.as_deref() == Some("E_PARSE")),
                            parse_error,
                            "parse policy {}",
                            case["id"]
                        );
                        let before = actual.workspace_symbols("");
                        for query in expected["queries"].as_array().expect("queries") {
                            let wanted = oracle::expected(
                                &current,
                                expected,
                                &query["symbols"],
                                false,
                                &uri,
                            );
                            for repeat in 0..3 {
                                assert_eq!(
                                    Value::Array(
                                        actual
                                            .workspace_symbols(
                                                query["query"].as_str().expect("query")
                                            )
                                            .iter()
                                            .map(crate::workspace_symbol_ownership_tests::project)
                                            .collect()
                                    ),
                                    wanted,
                                    "case={} query={} crlf={crlf} shifted={shifted} repeat={repeat}",
                                    case["id"],
                                    query["id"]
                                );
                            }
                        }
                        assert_eq!(
                            actual.workspace_symbols(""),
                            before,
                            "immutable source/schema results"
                        );
                        for (file, source) in current
                            .disk
                            .iter()
                            .filter(|(file, _)| file.ends_with(".vela"))
                        {
                            assert_eq!(
                                actual.source_db().records()[&DocumentId::from(uri(file))].text(),
                                source.text
                            );
                        }
                    }
                }
            }
        }
    }
}
