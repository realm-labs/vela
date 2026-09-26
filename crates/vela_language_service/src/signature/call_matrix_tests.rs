use crate::matrix_fixture::{FixtureWorkspace, hover_signature, signature_calls};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, QueryContext, SourceFileSnapshot, SymbolRef,
    Workspace, WorkspaceConfig, assemble_project_sources,
};
use serde_json::{Value, json};

mod type_oracle;

fn uri(file: &str) -> DocumentId {
    DocumentId::from(format!("/workspace/{file}"))
}

#[test]
fn signature_call_matrix_preserves_full_parameters_owners_and_static_boundaries() {
    verify_fixture("signature-s5", 161);
}

#[test]
fn signature_type_matrix_preserves_complete_hints_and_unknown_boundaries() {
    verify_fixture("signature-s3", 102);
}

fn verify_fixture(name: &str, expected_count: usize) {
    for crlf in [false, true] {
        let mut total = 0;
        for spec in signature_calls::specs(name, crlf) {
            let fixture = FixtureWorkspace::new(&spec).expect("fixture");
            let sources = fixture
                .disk
                .iter()
                .filter(|(file, _)| file.ends_with(".vela"))
                .map(|(file, document)| SourceFileSnapshot::new(uri(file), document.text.as_str()))
                .collect::<Vec<_>>();
            let mut db = LanguageServiceDatabases::new();
            let config =
                WorkspaceConfig::from_vela_toml("/workspace", &fixture.disk["vela.toml"].text);
            assert!(config.diagnostics.is_empty(), "{}", spec.id);
            db.update(&assemble_project_sources(
                &config.config,
                &sources,
                &Workspace::new().snapshot(),
            ));
            db.load_schema_artifact_json(
                "/workspace/schema.json",
                &fixture.disk["schema.json"].text,
            );
            assert!(db.schema_db().diagnostics().is_empty(), "{}", spec.id);
            for case in spec.oracle["queries"].as_array().expect("queries") {
                total += 1;
                let file = case["file"].as_str().expect("file");
                let point = hover_signature::position(
                    &fixture.disk[file],
                    case["marker"].as_str().expect("marker"),
                    false,
                    0,
                );
                let position = Position::new(
                    point["line"].as_u64().expect("line") as usize,
                    point["character"].as_u64().expect("column") as usize,
                );
                let help = db.signature_help(&uri(file), position);
                let actual = help.as_ref().map_or(Value::Null, |help| {
                    json!({"activeSignature":help.active_signature(),"activeParameter":help.active_parameter(),
                        "signatures":help.signatures().iter().map(|signature|json!({"label":signature.label(),
                            "parameters":signature.parameters().iter().map(|parameter|json!({
                                "name":parameter.name(),"label":parameter.label(),"type":parameter.type_fact().display_name()
                            })).collect::<Vec<_>>()
                        })).collect::<Vec<_>>()
                    })
                });
                assert_eq!(
                    actual,
                    hover_signature::signature_result(case, false),
                    "{}/{}, CRLF={crlf}",
                    spec.id,
                    case["id"]
                );
                assert_eq!(db.signature_help(&uri(file), position), help, "repeat");
                let query = QueryContext::from_databases(&db, &uri(file), position).expect("query");
                let callables = query.call_target_facts(&db);
                let owners = callables
                    .iter()
                    .map(|callable| {
                        let (kind, name) = match callable.symbol() {
                            SymbolRef::Source(name) => ("Source", name),
                            SymbolRef::Schema(name) => ("Schema", name),
                            SymbolRef::Builtin(name) => ("Builtin", name),
                            other => panic!("unreviewed callable owner {other:?}"),
                        };
                        json!({"kind":kind,"name":name})
                    })
                    .collect::<Vec<_>>();
                let expected = if case["result"].is_null() {
                    Vec::new()
                } else {
                    vec![case["owner"].clone()]
                };
                assert_eq!(owners, expected, "{}/{}, CRLF={crlf}", spec.id, case["id"]);
                if let Some(callable) = callables.first() {
                    if name == "signature-s3" {
                        let facts = case["parameterFacts"].as_array().expect("authored facts");
                        let parameters = &help.as_ref().expect("signature").signatures()[0];
                        assert_eq!(parameters.parameters().len(), facts.len());
                        for (parameter, expected) in parameters.parameters().iter().zip(facts) {
                            assert_eq!(
                                parameter.type_fact(),
                                &type_oracle::expected(expected),
                                "{}/{} parameter {}",
                                spec.id,
                                case["id"],
                                parameter.name()
                            );
                        }
                        assert_eq!(
                            callable.returns(),
                            &type_oracle::expected(&case["returnsFact"]),
                            "{}/{} return",
                            spec.id,
                            case["id"]
                        );
                    }
                    assert_eq!(
                        callable.supports_named_arguments(),
                        case["named"].as_bool().expect("named policy"),
                        "{}/{}",
                        spec.id,
                        case["id"]
                    );
                }
            }
        }
        assert_eq!(total, expected_count, "every reviewed position must run");
    }
}
