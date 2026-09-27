use crate::matrix_fixture::{FixtureWorkspace, hover_signature as oracle, load};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, SymbolRef, TextRange,
    Workspace, WorkspaceConfig, assemble_project_sources,
};
use serde_json::{Value, json};

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/中文 % hover/{file}"))
}

#[test]
fn hover_type_matrix_preserves_complete_scoped_hints_and_degraded_boundaries() {
    let mut positions = 0;
    for crlf in [false, true] {
        for missing_schema in [false, true] {
            let mut spec = load("hover-s3");
            if crlf {
                for source in spec.files.values_mut() {
                    *source = source.replace('\n', "\r\n");
                }
            }
            let fixture = FixtureWorkspace::new(&spec).expect("fixture");
            let sources = fixture
                .disk
                .iter()
                .filter(|(file, _)| file.ends_with(".vela"))
                .map(|(file, document)| SourceFileSnapshot::new(uri(file), document.text.as_str()))
                .collect::<Vec<_>>();
            let config = WorkspaceConfig::from_vela_toml(
                "/workspace/中文 % hover",
                &fixture.disk["vela.toml"].text,
            );
            assert!(config.diagnostics.is_empty());
            let mut db = LanguageServiceDatabases::new();
            db.update(&assemble_project_sources(
                &config.config,
                &sources,
                &Workspace::new().snapshot(),
            ));
            if !missing_schema {
                db.load_schema_artifact_json(
                    "/workspace/中文 % hover/schema.json",
                    &fixture.disk["schema.json"].text,
                );
                assert!(db.schema_db().diagnostics().is_empty());
            }
            let queries = spec.oracle["queries"].as_array().expect("queries");
            assert_eq!(queries.len(), 245);
            for authored in queries {
                let mut case = authored.clone();
                if missing_schema && case.get("missingResult").is_some() {
                    case["result"] = case["missingResult"].clone();
                }
                let file = case["file"].as_str().expect("file");
                let document = &fixture.disk[file];
                for offset in 0..if case["result"].is_null() { 1 } else { 2 } {
                    positions += 1;
                    let point = oracle::position(
                        document,
                        case["marker"].as_str().expect("marker"),
                        false,
                        offset,
                    );
                    let position = Position::new(
                        point["line"].as_u64().expect("line") as usize,
                        point["character"].as_u64().expect("column") as usize,
                    );
                    let actual = db.hover(&uri(file), position);
                    let normalized = actual.as_ref().map_or(Value::Null, |hover| json!({
                        "label":hover.label(),"kind":format!("{:?}",hover.kind()),"detail":hover.detail(),"docs":hover.docs(),
                        "range":{"start":{"line":hover.range().start().line,"character":hover.range().start().character},
                            "end":{"line":hover.range().end().line,"character":hover.range().end().character}}
                    }));
                    assert_eq!(
                        normalized,
                        oracle::hover_result(document, &case, false),
                        "{}, missing={missing_schema}, CRLF={crlf}",
                        case["id"]
                    );
                    if let Some(hover) = &actual {
                        let symbol = &case["result"]["symbol"];
                        let expected = if symbol.is_null() {
                            None
                        } else {
                            let name = symbol["name"].as_str().expect("name").to_owned();
                            Some(match symbol["kind"].as_str().expect("kind") {
                                "source" => SymbolRef::Source(name),
                                "schema" => SymbolRef::Schema(name),
                                "builtin" => SymbolRef::Builtin(name),
                                "local" => {
                                    let local_file = symbol["file"].as_str().expect("local file");
                                    let declaration = fixture.disk[local_file].markers
                                        [symbol["marker"].as_str().expect("local marker")];
                                    SymbolRef::local_at(
                                        name,
                                        uri(local_file),
                                        TextRange::new(
                                            declaration.start.byte,
                                            declaration.end.byte,
                                        ),
                                    )
                                }
                                other => panic!("unreviewed owner {other}"),
                            })
                        };
                        assert_eq!(hover.symbol(), expected.as_ref(), "{} identity", case["id"]);
                    }
                    assert_eq!(db.hover(&uri(file), position), actual, "repeat hover");
                }
            }
        }
    }
    assert_eq!(
        positions, 1924,
        "every authored token position and schema variant"
    );
}
