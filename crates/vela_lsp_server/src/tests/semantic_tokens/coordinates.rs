use super::support::{apply_delta, delta, full, rows, start};
use std::{fs, path::Path};

use lsp_types::{notification as n, request as r};
use serde_json::{Value, json};

use crate::matrix_fixture::{
    Document, FixtureWorkspace, load, parse_markers, semantic_tokens as oracle,
};
use crate::tests::{TestServer, notify, request, response_value, sync_diagnostics};

fn uri(root: &Path) -> String {
    lsp_types::Url::from_file_path(root.join("scripts/main.vela"))
        .expect("URI")
        .to_string()
}

fn check_ranges(
    server: &mut TestServer,
    uri: &str,
    document: &Document,
    expected: &Value,
    legend: &Value,
) {
    let current = full(server, uri);
    let selected = &oracle::expected(document, expected, true)[0];
    for (start, end, wanted) in [
        (1, 2, vec![selected.clone()]),
        (0, 0, vec![]),
        (1, 1, vec![]),
    ] {
        let response = response_value(request::<r::SemanticTokensRangeRequest>(
            server,
            4,
            json!({"textDocument":{"uri":uri},"range":{"start":{"line":0,"character":start},"end":{"line":0,"character":end}}}),
        ));
        oracle::assert_stream(
            &rows(document, &response["result"]["data"], legend),
            &wanted,
        );
        let repeated = response_value(request::<r::SemanticTokensRangeRequest>(
            server,
            5,
            json!({"textDocument":{"uri":uri},"range":{"start":{"line":0,"character":start},"end":{"line":0,"character":end}}}),
        ));
        assert_eq!(repeated["result"], response["result"]);
    }
    for name in ["main", "string"] {
        let marker = document.markers[name];
        let span = json!({"start":{"line":marker.start.line,"character":marker.start.character},
            "end":{"line":marker.end.line,"character":marker.end.character}});
        let response = response_value(request::<r::SemanticTokensRangeRequest>(
            server,
            6,
            json!({"textDocument":{"uri":uri},"range":span}),
        ));
        let entry = expected
            .as_array()
            .expect("oracle")
            .iter()
            .find(|entry| entry["marker"] == name)
            .expect("token oracle");
        oracle::assert_stream(
            &rows(document, &response["result"]["data"], legend),
            &oracle::expected(document, &json!([entry]), true),
        );
    }
    let emoji_middle = if document.markers.contains_key("header") {
        6
    } else {
        5
    };
    for (start, end) in [(emoji_middle, emoji_middle + 1), (2, 1)] {
        let response = response_value(request::<r::SemanticTokensRangeRequest>(
            server,
            7,
            json!({"textDocument":{"uri":uri},"range":{"start":{"line":0,"character":start},"end":{"line":0,"character":end}}}),
        ));
        assert_eq!(response["error"]["code"], -32600);
        assert!(response.get("result").is_none());
        assert_eq!(
            full(server, uri),
            current,
            "bad request cannot poison token queries"
        );
    }
}

#[test]
fn coordinate_streams_and_applied_deltas_keep_exact_utf16_tokens() {
    let spec = load("semantic-token-coordinates");
    for crlf in [false, true] {
        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
        let source = |text: &str| {
            parse_markers(&if crlf {
                text.replace('\n', "\r\n")
            } else {
                text.to_owned()
            })
            .expect("document")
        };
        fixture.disk.insert(
            "scripts/main.vela".into(),
            source(&spec.files["scripts/main.vela"]),
        );
        let disk = &fixture.disk["scripts/main.vela"];
        let dirty = source(spec.oracle["dirty"]["source"].as_str().expect("dirty"));
        let same_bytes = source(
            spec.oracle["sameByteLength"]["source"]
                .as_str()
                .expect("same bytes"),
        );
        assert_eq!(same_bytes.text.len(), disk.text.len());
        let parent = crate::tests::support::unique_temp_root("semantic-coordinates");
        let root = parent.join("中文 % tokens");
        fixture.materialize(&root).expect("workspace");
        let uri = uri(&root);
        assert!(uri.contains('%'));
        let (mut server, legend) = start(&root);
        let _ = sync_diagnostics::<n::DidOpenTextDocument>(
            &mut server,
            json!({"textDocument":{"uri":uri,"languageId":"vela","version":1,"text":disk.text}}),
        );
        let original = full(&mut server, &uri);
        let mut previous = original.clone();
        for (index, (document, expected)) in [
            (disk, &spec.oracle["disk"]),
            (&same_bytes, &spec.oracle["sameByteLength"]["tokens"]),
            (&dirty, &spec.oracle["dirty"]["tokens"]),
            (disk, &spec.oracle["disk"]),
        ]
        .into_iter()
        .enumerate()
        {
            if matches!(index, 1 | 2) {
                let _ = sync_diagnostics::<n::DidChangeTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri,"version":index + 1},"contentChanges":[{"text":document.text}]}),
                );
            } else if index == 3 {
                let _ = notify::<n::DidCloseTextDocument>(
                    &mut server,
                    json!({"textDocument":{"uri":uri}}),
                );
            }
            let current = full(&mut server, &uri);
            oracle::assert_stream(
                &rows(document, &current["data"], &legend),
                &oracle::expected(document, expected, true),
            );
            let changed = delta(&mut server, &uri, &previous["resultId"]);
            assert_eq!(apply_delta(&previous["data"], &changed), current["data"]);
            assert_eq!(changed["resultId"], current["resultId"]);
            let repeated = delta(&mut server, &uri, &current["resultId"]);
            assert_eq!(repeated["edits"], json!([]));
            assert_eq!(repeated["resultId"], current["resultId"]);
            assert_eq!(full(&mut server, &uri), current);
            let (mut fresh, fresh_legend) = start(&root);
            let _ = sync_diagnostics::<n::DidOpenTextDocument>(
                &mut fresh,
                json!({"textDocument":{"uri":uri,"languageId":"vela","version":2,"text":document.text}}),
            );
            assert_eq!(fresh_legend, legend);
            assert_eq!(full(&mut fresh, &uri), current);
            check_ranges(&mut server, &uri, document, expected, &legend);
            previous = current;
        }
        assert_eq!(previous, original, "close restores disk stream CRLF={crlf}");
        fs::remove_dir_all(parent).expect("cleanup own fixture");
    }
}
