use std::collections::BTreeMap;

use serde_json::{Value, json};

pub(super) use crate::matrix_fixture::{Document, Spec, document_open::span, load, parse_markers};
pub(super) use crate::tests::document_sync_matrix_support::{Layout, open, profiles, send};
pub(super) use crate::tests::{TestServer, message_value};

pub(super) fn spec(crlf: bool) -> Spec {
    let mut spec = load("watched-files");
    if crlf {
        for text in spec.files.values_mut() {
            *text = text.replace('\n', "\r\n");
        }
    }
    spec
}
pub(super) fn doc(text: &str, crlf: bool) -> Document {
    parse_markers(&text.replace('\n', if crlf { "\r\n" } else { "\n" })).expect("authored source")
}
pub(super) fn variant(spec: &Spec, name: &str) -> Document {
    doc(
        spec.oracle[name].as_str().expect("authored variant"),
        spec.files["vela.toml"].contains("\r\n"),
    )
}
pub(super) fn watch(
    server: &mut TestServer,
    layout: &Layout,
    events: &[(&str, i32)],
) -> Vec<Value> {
    send(
        server,
        json!({"jsonrpc":"2.0","method":"workspace/didChangeWatchedFiles","params":{
        "changes":events.iter().map(|(file,typ)|json!({"uri":layout.uri(file),"type":typ})).collect::<Vec<_>>()}}),
    )
}
pub(super) fn params(uri: &str, document: &Document, marker: &str) -> Value {
    let point = document.markers[marker].start;
    json!({"textDocument":{"uri":uri},"position":{"line":point.line,"character":point.character}})
}
pub(super) fn definition(
    server: &mut TestServer,
    layout: &Layout,
    caller: &Document,
    target: Option<(&str, &Document)>,
) {
    let expected = target.map_or(
        Value::Null,
        |(file, document)| json!({"uri":layout.uri(file),"range":span(document,"decl")}),
    );
    for id in [71, 72] {
        assert_eq!(
            send(
                server,
                json!({"jsonrpc":"2.0","id":id,"method":"textDocument/definition",
            "params":params(&layout.uri("scripts/main.vela"),caller,"call")})
            ),
            vec![json!({"jsonrpc":"2.0","id":id,"result":expected})]
        );
    }
}
pub(super) fn source_error(spec: &Spec, document: &Document, uri: &str, kind: &str) -> Vec<Value> {
    let oracle = &spec.oracle[kind];
    let range = span(document, "import");
    vec![
        json!({"code":oracle["code"],"message":oracle["message"],"severity":1,"source":"vela","range":range,
        "data":{"labels":[{"uri":uri,"range":range,"message":oracle["label"]}],"candidates":[],"repairHints":[]}}),
    ]
}
pub(super) fn metadata_error(code: &str, message: &str) -> Vec<Value> {
    vec![
        json!({"code":code,"message":message,"severity":1,"source":"vela",
        "range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},
        "data":{"labels":[],"candidates":[],"repairHints":[]}}),
    ]
}
pub(super) fn publications(
    messages: &[Value],
    expected: Vec<(String, Vec<Value>)>,
    progress: bool,
) {
    let mut actual = BTreeMap::new();
    let mut extras = Vec::new();
    for message in messages {
        if message["method"] == "textDocument/publishDiagnostics" {
            assert_eq!(message["jsonrpc"], "2.0");
            assert!(message["params"].get("version").is_none());
            assert!(message["params"].get("error").is_none());
            let uri = message["params"]["uri"].as_str().expect("URI").to_owned();
            assert!(
                actual
                    .insert(uri, message["params"]["diagnostics"].clone())
                    .is_none(),
                "one publication per document"
            );
        } else {
            extras.push(message.clone());
        }
    }
    assert_eq!(
        actual,
        expected
            .into_iter()
            .map(|(uri, d)| (uri, json!(d)))
            .collect()
    );
    let expected_progress = if progress {
        vec![
            json!({"jsonrpc":"2.0","method":"$/progress","params":{"token":"vela/workspace-diagnostics","value":{"kind":"begin","title":"Vela workspace diagnostics","message":"updating open-file diagnostics"}}}),
            json!({"jsonrpc":"2.0","method":"$/progress","params":{"token":"vela/workspace-diagnostics","value":{"kind":"end","message":"workspace diagnostics updated"}}}),
        ]
    } else {
        vec![]
    };
    assert_eq!(extras, expected_progress);
}
pub(super) fn has_progress(profile: &Value) -> bool {
    profile["window"]["workDoneProgress"] == true
}
pub(super) fn close(server: &mut TestServer, uri: &str) -> Vec<Value> {
    send(
        server,
        json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":uri}}}),
    )
}
pub(super) fn equivalent(
    layout: &Layout,
    profile: &Value,
    caller: &Document,
    target: Option<(&str, &Document)>,
    expected: Vec<Value>,
    live: &mut TestServer,
) {
    let mut fresh = layout.server(profile.clone());
    publications(
        &open(
            &mut fresh,
            &layout.uri("scripts/main.vela"),
            &caller.text,
            7,
        ),
        vec![(layout.uri("scripts/main.vela"), expected)],
        false,
    );
    definition(&mut fresh, layout, caller, target);
    definition(live, layout, caller, target);
}
