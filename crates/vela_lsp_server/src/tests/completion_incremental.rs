use crate::matrix_fixture::{Edit, FixtureWorkspace, apply_edits, load, parse_markers};
use crate::tests::{TestServer, notify, request, response_value};
use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use std::path::Path;
use vela_language_service::{DocumentId, LanguageServiceDatabases};

#[test]
fn incremental_completion_links_fingerprints_invalidation_and_current_owned_edits() {
    for crlf in [false, true] {
        let mut spec = load("completion-incremental-ownership");
        if crlf {
            for source in spec.files.values_mut() {
                *source = source.replace('\n', "\r\n");
            }
        }
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let temp = crate::tests::support::unique_temp_root("completion-incremental");
        let root = temp.join("中文 % workspace");
        fixture.materialize(&root).expect("workspace");
        let mut live = start(&root, &fixture);
        assert_candidates(&mut live, &root, &fixture, &spec.oracle["initial"]);
        for (index, step) in spec.oracle["steps"]
            .as_array()
            .expect("steps")
            .iter()
            .enumerate()
        {
            let file = step["file"].as_str().expect("file");
            let document = DocumentId::from(uri(&root, file));
            let before = live.snapshot();
            let before = before.databases();
            let key = before.source_db().records()[&document].module_key();
            let fingerprint = before
                .parse_db()
                .module_fingerprint(key)
                .expect("fingerprint");
            let source = step["source"]
                .as_str()
                .expect("source")
                .replace('\n', if crlf { "\r\n" } else { "\n" });
            fixture
                .disk
                .insert(file.to_owned(), parse_markers(&source).expect("source"));
            let _ = notify::<n::DidChangeTextDocument>(
                &mut live,
                json!({"textDocument":{"uri":uri(&root,file),"version":index+2},"contentChanges":[{"text":fixture.disk[file].text}]}),
            );
            let after = live.snapshot();
            let after = after.databases();
            assert_eq!(
                after.parse_db().parse_count(),
                before.parse_db().parse_count() + 1,
                "{step}"
            );
            assert_eq!(
                after.hir_db().rebuild_count(),
                before.hir_db().rebuild_count() + 1
            );
            assert_eq!(
                after.project_db().rebuild_count() - before.project_db().rebuild_count(),
                usize::from(step["declarationChanged"] == true || step["importChanged"] == true)
            );
            let current = after
                .parse_db()
                .module_fingerprint(key)
                .expect("fingerprint");
            assert_eq!(
                current.declaration() != fingerprint.declaration(),
                step["declarationChanged"] == true,
                "{step}"
            );
            assert_eq!(
                current.import() != fingerprint.import(),
                step["importChanged"] == true,
                "{step}"
            );
            let invalidated = after
                .analysis_db()
                .invalidated_modules()
                .iter()
                .map(|key| key.path.join())
                .collect::<Vec<_>>();
            assert_eq!(json!(invalidated), step["invalidated"], "{step}");
            assert!(after.generation() > before.generation());
            assert_candidates(&mut live, &root, &fixture, &step["items"]);
        }
        drop(live);
        std::fs::remove_dir_all(temp).expect("cleanup");
    }
}

fn assert_candidates(
    live: &mut TestServer,
    root: &Path,
    fixture: &FixtureWorkspace,
    expected: &Value,
) {
    let source = fixture.document("scripts/main.vela").expect("main");
    let point = source.markers["cursor"].start;
    let params = json!({"textDocument":{"uri":uri(root,"scripts/main.vela")},"position":{"line":point.line,"character":point.character}});
    let before = counters(live.snapshot().databases());
    let result = completion(live, &params);
    assert_eq!(result, completion(live, &params));
    let mut fresh = start(root, fixture);
    assert_eq!(result, completion(&mut fresh, &params), "fresh analysis");
    let items = result["items"].as_array().expect("items");
    let expected = expected.as_array().expect("expected");
    assert_eq!(items.len(), expected.len());
    for (item, expected) in items.iter().zip(expected) {
        assert_eq!(item["label"], expected["label"]);
        assert_eq!(item["kind"], 5);
        assert_eq!(item["detail"], expected["detail"]);
        assert!(item.get("documentation").is_none());
        assert_eq!(
            item["data"]["resolve"],
            json!({"kind":"documentation","symbol":{"kind":"source","name":expected["symbol"]}})
        );
        let resolved = response_value(request::<r::ResolveCompletionItem>(live, 12, item.clone()));
        assert!(resolved["error"].is_null(), "{resolved}");
        assert_eq!(&resolved["result"], item);
        let range = source.markers["replace"];
        assert_eq!(
            item["textEdit"],
            json!({"range":{"start":{"line":range.start.line,"character":range.start.character},"end":{"line":range.end.line,"character":range.end.character}},"newText":expected["label"]})
        );
        let text = apply_edits(
            &source.text,
            &[Edit {
                start: (range.start.line, range.start.character),
                end: (range.end.line, range.end.character),
                text: item["textEdit"]["newText"].as_str().expect("insertion"),
            }],
        )
        .expect("edit");
        assert!(
            vela_syntax::parse::parse_source(&text)
                .diagnostics()
                .is_empty()
        );
        let mut applied = fixture.clone();
        applied
            .disk
            .get_mut("scripts/main.vela")
            .expect("main")
            .text = text;
        let mut applied_server = start(root, &applied);
        let definition = response_value(request::<r::GotoDefinition>(
            &mut applied_server,
            13,
            json!({"textDocument":{"uri":uri(root,"scripts/main.vela")},"position":{"line":range.start.line,"character":range.start.character+1}}),
        ));
        assert!(definition["error"].is_null(), "{definition}");
        let file = expected["file"].as_str().expect("target");
        let marker = fixture.document(file).expect("target").markers
            [expected["marker"].as_str().expect("marker")];
        assert_eq!(
            definition["result"],
            json!({"uri":uri(root,file),"range":{"start":{"line":marker.start.line,"character":marker.start.character},"end":{"line":marker.end.line,"character":marker.end.character}}})
        );
        let repeated = completion(
            &mut applied_server,
            &json!({"textDocument":{"uri":uri(root,"scripts/main.vela")},"position":{"line":range.start.line,"character":range.start.character+expected["label"].as_str().expect("label").encode_utf16().count()}}),
        );
        let repeated = repeated["items"].as_array().expect("applied items");
        assert_eq!(repeated.len(), 1);
        assert_eq!(repeated[0]["data"], item["data"]);
        assert_eq!(repeated[0]["detail"], item["detail"]);
    }
    assert_eq!(
        counters(live.snapshot().databases()),
        before,
        "queries and resolve must not rebuild"
    );
}

fn completion(live: &mut TestServer, params: &Value) -> Value {
    let response = response_value(request::<r::Completion>(live, 10, params.clone()));
    assert!(response["error"].is_null(), "{response}");
    response["result"].clone()
}
fn counters(db: &LanguageServiceDatabases) -> (usize, usize, usize, u64) {
    (
        db.parse_db().parse_count(),
        db.project_db().rebuild_count(),
        db.hir_db().rebuild_count(),
        db.generation().get(),
    )
}
fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("URI")
        .to_string()
}
fn start(root: &Path, fixture: &FixtureWorkspace) -> TestServer {
    let mut live = TestServer::new();
    let result = response_value(request::<r::Initialize>(
        &mut live,
        1,
        json!({"processId":null,"rootUri":uri(root,""),"capabilities":{"textDocument":{"completion":{"completionItem":{"snippetSupport":true,"labelDetailsSupport":true,"resolveSupport":{"properties":["documentation"]}}}}}}),
    ));
    assert!(result["error"].is_null(), "{result}");
    for (file, source) in fixture
        .disk
        .iter()
        .filter(|(file, _)| file.ends_with(".vela"))
    {
        let _ = notify::<n::DidOpenTextDocument>(
            &mut live,
            json!({"textDocument":{"uri":uri(root,file),"languageId":"vela","version":1,"text":source.text}}),
        );
    }
    live
}
