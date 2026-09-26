use crate::matrix_fixture::{FixtureWorkspace, Spec, hover_signature, signature_imports as oracle};
use crate::{
    DocumentId, LanguageServiceDatabases, Position, QueryContext, SourceFileSnapshot,
    SourceVersion, SymbolRef, Workspace,
};
use serde_json::{Value, json};

fn uri(layout: &oracle::Layout, file: &str) -> DocumentId {
    DocumentId::from(layout.root.join(file).to_string_lossy().replace('\\', "/"))
}

fn position(value: &Value) -> Position {
    Position::new(
        value["line"].as_u64().expect("line") as usize,
        value["character"].as_u64().expect("byte column") as usize,
    )
}

fn update(
    db: &mut LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    layout: &oracle::Layout,
    workspace: &Workspace,
) {
    let graph = vela_package::load_package_graph(
        layout.root.join("vela.toml"),
        std::slice::from_ref(&layout.root),
    )
    .expect("current package graph");
    let sources = fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
        .map(|(file, document)| SourceFileSnapshot::new(uri(layout, file), document.text.as_str()))
        .collect::<Vec<_>>();
    db.update(&crate::assemble_package_project_sources(
        &graph,
        &sources,
        &workspace.snapshot(),
    ));
}

fn load_schema(
    db: &mut LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    layout: &oracle::Layout,
) {
    let artifact = crate::matrix_fixture::schema_artifact(
        &serde_json::from_str::<Value>(&fixture.disk["schema-markers.json"].text)
            .expect("schema markers"),
        fixture,
        |file| {
            db.source_db().records()[&uri(layout, file)]
                .source_id()
                .get()
        },
    );
    db.load_schema_artifact_json(
        &layout.root.join("schema.json").to_string_lossy(),
        &artifact.to_string(),
    );
    assert!(db.schema_db().diagnostics().is_empty(), "valid schema");
}

fn signature(db: &LanguageServiceDatabases, id: &DocumentId, point: Position) -> Value {
    let help = db.signature_help(id, point);
    assert_eq!(db.signature_help(id, point), help, "repeat signature");
    help.as_ref().map_or(Value::Null, |help| {
        json!({"activeSignature":help.active_signature(),"activeParameter":help.active_parameter(),
            "signatures":help.signatures().iter().map(|signature|json!({"label":signature.label(),
                "parameters":signature.parameters().iter().map(|parameter|json!({
                    "name":parameter.name(),"label":parameter.label(),"type":parameter.type_fact().display_name()
                })).collect::<Vec<_>>()
            })).collect::<Vec<_>>()
        })
    })
}

fn definition(db: &LanguageServiceDatabases, id: &DocumentId, point: Position) -> Value {
    let target = db.definition(id, point);
    assert_eq!(db.definition(id, point), target, "repeat definition");
    target.map_or(Value::Null, |target| {
        let range = target.range();
        json!({"uri":target.document_id().as_str(),"range":{
            "start":{"line":range.start().line,"character":range.start().character},
            "end":{"line":range.end().line,"character":range.end().character}
        }})
    })
}

fn verify(
    db: &LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    layout: &oracle::Layout,
    spec: &Spec,
    phase: &Value,
) -> Vec<Value> {
    oracle::cases(spec, phase)
        .iter()
        .map(|case| {
            let file = case["file"].as_str().expect("query file");
            let source = fixture.document(file).expect("query source");
            let id = uri(layout, file);
            let point = position(&hover_signature::position(
                source,
                case["marker"].as_str().expect("argument marker"),
                false,
                0,
            ));
            let actual = signature(db, &id, point);
            assert_eq!(
                actual,
                hover_signature::signature_result(case, false),
                "{} {}",
                phase["id"],
                case["id"]
            );
            let callables = QueryContext::from_databases(db, &id, point)
                .expect("query")
                .call_target_facts(db);
            let owners = callables
                .iter()
                .map(|callable| {
                    let (kind, name) = match callable.symbol() {
                        SymbolRef::Source(name) => ("Source", name),
                        SymbolRef::Schema(name) => ("Schema", name),
                        SymbolRef::Builtin(name) => ("Builtin", name),
                        other => panic!("unexpected callable owner {other:?}"),
                    };
                    assert_eq!(
                        callable.supports_named_arguments(),
                        case["named"].as_bool().expect("named policy")
                    );
                    json!({"kind":kind,"name":name})
                })
                .collect::<Vec<_>>();
            let expected = if case["result"].is_null() {
                Vec::new()
            } else {
                vec![case["owner"].clone()]
            };
            assert_eq!(owners, expected, "{} {} ownership", phase["id"], case["id"]);
            let callee = position(&hover_signature::position(
                source,
                case["callee"].as_str().expect("callee marker"),
                false,
                1,
            ));
            let target = definition(db, &id, callee);
            assert_eq!(
                target,
                oracle::definition(fixture, case, false, |file| uri(layout, file)
                    .as_str()
                    .to_owned()),
                "{} {} target",
                phase["id"],
                case["id"]
            );
            json!({"signature":actual,"definition":target})
        })
        .collect()
}

#[test]
fn signature_package_matrix_preserves_import_ownership_and_lifecycle_facts() {
    for crlf in [false, true] {
        let spec = oracle::spec(crlf);
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let original_disk = fixture.disk.clone();
        let layout = oracle::Layout::new(&fixture);
        let mut workspace = Workspace::new();
        fixture.open.insert(
            "scripts/main.vela".into(),
            fixture.disk["scripts/main.vela"].clone(),
        );
        workspace.open_document(
            uri(&layout, "scripts/main.vela"),
            fixture.open["scripts/main.vela"].text.as_str(),
            SourceVersion::new(1),
        );
        let mut version = 1;
        let mut db = LanguageServiceDatabases::new();
        let mut initial = None;
        let mut last = Vec::new();
        let mut total = 0;
        for phase in spec.oracle["phases"].as_array().expect("phases") {
            for action in oracle::actions(phase) {
                fixture.apply(&action).expect("action");
                layout.apply(&fixture, &action);
                version += 1;
                let id = uri(&layout, &action.file);
                match action.op.as_str() {
                    "open" => workspace.open_document(
                        id,
                        fixture.open[&action.file].text.as_str(),
                        SourceVersion::new(version),
                    ),
                    "change" => workspace.change_document(
                        id,
                        fixture.open[&action.file].text.as_str(),
                        SourceVersion::new(version),
                    ),
                    "close" => workspace.close_document(&id),
                    "write" | "delete" => {}
                    _ => panic!("unsupported action"),
                }
            }
            update(&mut db, &fixture, &layout, &workspace);
            if phase["id"] == "disk" {
                load_schema(&mut db, &fixture, &layout);
            }
            let mut fresh_workspace = Workspace::new();
            for (file, source) in &fixture.open {
                fresh_workspace.open_document(
                    uri(&layout, file),
                    source.text.as_str(),
                    SourceVersion::new(1),
                );
            }
            let mut fresh = LanguageServiceDatabases::new();
            update(&mut fresh, &fixture, &layout, &fresh_workspace);
            load_schema(&mut fresh, &fixture, &layout);
            last = verify(&db, &fixture, &layout, &spec, phase);
            assert_eq!(
                last,
                verify(&fresh, &fixture, &layout, &spec, phase),
                "incremental / fresh: {} CRLF={crlf}",
                phase["id"]
            );
            total += last.len();
            initial.get_or_insert_with(|| last.clone());
            layout.assert_disk(&fixture, &spec);
        }
        assert_eq!(total, 544, "all query positions at all phases");
        assert_eq!(
            last,
            initial.expect("disk baseline"),
            "restored facts and exact targets"
        );
        assert_eq!(fixture.disk, original_disk, "restore original disk");
        assert!(fixture.open.is_empty(), "overlays closed");
    }
}
