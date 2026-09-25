use std::{fs, path::Path};

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};
use vela_language_service::DocumentId;

use crate::matrix_fixture::{Document, FixtureWorkspace, Marker, load, parse_markers};
use crate::tests::{
    TestServer, notification_values, notify, protocol_notification, request, response_value,
    sync_diagnostics,
};

fn uri(root: &Path, file: &str) -> String {
    lsp_types::Url::from_file_path(root.join(file))
        .expect("URI")
        .to_string()
}

fn range(marker: Marker) -> Value {
    json!({"start":{"line":marker.start.line,"character":marker.start.character},
        "end":{"line":marker.end.line,"character":marker.end.character}})
}

fn diagnostic(
    code: &str,
    message: &str,
    span: Value,
    labels: Vec<Value>,
    candidates: &[&str],
) -> Value {
    json!({"code":code,"message":message,"severity":1,"source":"vela","range":span,
        "data":{"labels":labels,"repairHints":[],"candidates":candidates.iter()
            .map(|replacement| json!({"replacement":replacement})).collect::<Vec<_>>()}})
}

fn expected_main(root: &Path, document: &Document, unresolved: bool) -> Value {
    let main = uri(root, "scripts/main.vela");
    let mut expected = Vec::new();
    if unresolved {
        let span = range(document.markers["import"]);
        expected.push(diagnostic(
            "hir::unresolved_import",
            "unresolved import `make` in module `helper`",
            span.clone(),
            vec![json!({"uri":main,"range":span,"message":"no similar declarations found"})],
            &[],
        ));
    }
    let span = range(document.markers["typo"]);
    expected.push(diagnostic(
        "analysis::unknown_method",
        "unknown method `frist` for `Array(i64)`",
        span.clone(),
        [
            "unknown member access",
            "did you mean `first`?",
            "similar candidates: first, find, last",
        ]
        .iter()
        .map(|message| json!({"uri":main,"range":span,"message":message}))
        .collect(),
        &["first", "find", "last"],
    ));
    json!(expected)
}

fn start(root: &Path, fixture: &FixtureWorkspace) -> TestServer {
    let mut server = TestServer::new();
    let _ = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":uri(root,""),"capabilities":{}}),
    ));
    for file in [
        "scripts/main.vela",
        "scripts/relay.vela",
        "scripts/helper.vela",
        "scripts/unrelated.vela",
    ] {
        let source = &fixture.disk[file].text;
        let _ = sync_diagnostics::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri(root,file),"languageId":"vela","version":1,"text":source}}),
        );
    }
    server
}

fn publication<'a>(messages: &'a [Value], target: &str) -> Option<&'a Value> {
    let found = messages
        .iter()
        .filter(|message| message["params"]["uri"] == target)
        .collect::<Vec<_>>();
    assert!(found.len() <= 1, "one publication per document");
    found.first().copied()
}

#[test]
fn incremental_diagnostics_publish_current_reverse_dependencies_only() {
    let spec = load("diagnostic-incremental");
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        if crlf {
            for (file, source) in &mut fixture.disk {
                *source =
                    parse_markers(&spec.files[file].replace('\n', "\r\n")).expect("CRLF source");
            }
        }
        let parent = crate::tests::support::unique_temp_root("diagnostic-incremental");
        let root = parent.join("中文 % incremental");
        fixture.materialize(&root).expect("workspace");
        let main = uri(&root, "scripts/main.vela");
        let helper = uri(&root, "scripts/helper.vela");
        let relay = uri(&root, "scripts/relay.vela");
        let unrelated = uri(&root, "scripts/unrelated.vela");
        assert!(main.contains('%'));
        let mut live = start(&root, &fixture);
        let baseline = sync_diagnostics::<n::DidChangeTextDocument>(
            &mut live,
            json!({"textDocument":{"uri":main,"version":2},
                "contentChanges":[{"text":fixture.disk["scripts/main.vela"].text}]}),
        );
        assert_eq!(
            baseline["params"]["diagnostics"],
            expected_main(&root, &fixture.disk["scripts/main.vela"], false)
        );
        let mut main_version = 2;
        let mut helper_version = 1;
        for step in spec.oracle["steps"].as_array().expect("steps") {
            let file = step["file"].as_str().expect("file");
            let source = step["source"]
                .as_str()
                .expect("source")
                .replace('\n', if crlf { "\r\n" } else { "\n" });
            fixture.disk.insert(
                file.to_owned(),
                parse_markers(&source).expect("phase source"),
            );
            let version = if file == "scripts/main.vela" {
                main_version += 1;
                main_version
            } else {
                helper_version += 1;
                helper_version
            };
            let target = uri(&root, file);
            let changed = notification_values(notify::<n::DidChangeTextDocument>(
                &mut live,
                json!({"textDocument":{"uri":target,"version":version},
                    "contentChanges":[{"text":fixture.disk[file].text}]}),
            ));
            let mut published = changed
                .iter()
                .map(|message| message["params"]["uri"].as_str().expect("URI").to_owned())
                .collect::<Vec<_>>();
            published.sort();
            let mut expected_published = step["invalidated"]
                .as_array()
                .expect("invalidated")
                .iter()
                .map(|name| {
                    uri(
                        &root,
                        &format!("scripts/{}.vela", name.as_str().expect("module")),
                    )
                })
                .collect::<Vec<_>>();
            expected_published.sort();
            assert_eq!(
                published, expected_published,
                "{} published documents",
                step["id"]
            );
            assert!(
                publication(&changed, &target).is_some(),
                "changed document publishes"
            );
            assert!(
                publication(&changed, &unrelated).is_none(),
                "unrelated document does not republish"
            );
            assert!(
                changed
                    .iter()
                    .all(|message| message["method"] == "textDocument/publishDiagnostics")
            );
            let unresolved = step["id"] == "declaration_renamed";
            let main_expected =
                expected_main(&root, &fixture.disk["scripts/main.vela"], unresolved);
            if let Some(main_publication) = publication(&changed, &main) {
                assert_eq!(
                    main_publication["params"]["diagnostics"], main_expected,
                    "{}",
                    step["id"]
                );
            }
            for document in [&helper, &relay] {
                if let Some(value) = publication(&changed, document) {
                    assert_eq!(value["params"]["diagnostics"], json!([]), "{document}");
                }
            }
            let duplicate = notify::<n::DidChangeTextDocument>(
                &mut live,
                json!({"textDocument":{"uri":target,"version":version},
                    "contentChanges":[{"text":"invalid replacement"}]}),
            );
            assert!(duplicate.is_empty(), "stale version must not publish");
            let older = notify::<n::DidChangeTextDocument>(
                &mut live,
                json!({"textDocument":{"uri":target,"version":version-1},
                    "contentChanges":[{"text":"older invalid replacement"}]}),
            );
            assert!(older.is_empty(), "older version must not publish");
            let cancel = protocol_notification(&mut live, "$/cancelRequest", json!({"id":99999}));
            assert!(cancel.is_empty(), "unknown cancellation has no response");
            let repeat_version = version + 1;
            let repeated_live = sync_diagnostics::<n::DidChangeTextDocument>(
                &mut live,
                json!({"textDocument":{"uri":target,"version":repeat_version},
                    "contentChanges":[{"text":fixture.disk[file].text}]}),
            );
            assert_eq!(
                repeated_live["params"]["diagnostics"],
                if file == "scripts/main.vela" {
                    main_expected.clone()
                } else {
                    json!([])
                },
                "identical text after stale/cancel {}",
                step["id"]
            );
            if file == "scripts/main.vela" {
                main_version = repeat_version;
            } else {
                helper_version = repeat_version;
            }
            let mut fresh = start(&root, &fixture);
            let repeated = sync_diagnostics::<n::DidChangeTextDocument>(
                &mut fresh,
                json!({"textDocument":{"uri":main,"version":2},
                    "contentChanges":[{"text":fixture.disk["scripts/main.vela"].text}]}),
            );
            assert_eq!(
                repeated["params"]["diagnostics"], main_expected,
                "fresh {}",
                step["id"]
            );
            assert_eq!(
                live.snapshot()
                    .databases()
                    .diagnostics_for_document(&DocumentId::from(main.clone()))
                    .diagnostics(),
                fresh
                    .snapshot()
                    .databases()
                    .diagnostics_for_document(&DocumentId::from(main.clone()))
                    .diagnostics(),
                "live diagnostics survive stale version and cancellation {}",
                step["id"]
            );
            if unresolved {
                assert!(
                    publication(&changed, &main).is_some(),
                    "reverse dependent must republish"
                );
            }
        }
        fs::remove_dir_all(parent).expect("cleanup");
    }
}
