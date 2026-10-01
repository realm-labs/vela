pub(super) use crate::matrix_fixture::{Document, Spec, document_open::span, load, parse_markers};
pub(super) use crate::tests::TestServer;
pub(super) use crate::tests::document_sync_matrix_support::{Layout, open, profiles, send};
pub(super) use crate::tests::watched_files_matrix::support::{
    has_progress, metadata_error, publications, watch,
};
use serde_json::{Value, json};

pub(super) fn spec(crlf: bool) -> Spec {
    let mut s = load("workspace-configuration");
    if crlf {
        for text in s.files.values_mut() {
            *text = text.replace('\n', "\r\n");
        }
    }
    s
}
pub(super) fn doc(s: &Spec, file: &str) -> Document {
    parse_markers(&s.files[file]).expect("authored markers")
}
pub(super) fn definition(
    server: &mut TestServer,
    layout: &Layout,
    source_file: &str,
    source: &Document,
    marker: &str,
    target: Option<(&str, &Document)>,
) {
    let result = target.map_or(
        Value::Null,
        |(file, d)| json!({"uri":layout.uri(file),"range":span(d,"decl")}),
    );
    for id in [91, 92] {
        assert_eq!(
            send(
                server,
                json!({"jsonrpc":"2.0","id":id,"method":"textDocument/definition","params":{
        "textDocument":{"uri":layout.uri(source_file)},"position":{"line":source.markers[marker].start.line,"character":source.markers[marker].start.character}}})
            ),
            vec![json!({"jsonrpc":"2.0","id":id,"result":result})]
        );
    }
}
pub(super) fn missing(
    s: &Spec,
    layout: &Layout,
    file: &str,
    source: &Document,
    marker: &str,
    module: &str,
) -> Value {
    let oracle = &s.oracle["missingModule"];
    let range = span(source, marker);
    json!({"code":oracle["code"],"message":oracle["message"].as_str().expect("message").replace("{MODULE}",module),"severity":1,"source":"vela","range":range,
    "data":{"labels":[{"uri":layout.uri(file),"range":range,"message":oracle["label"]}],"candidates":[],"repairHints":[]}})
}
pub(super) fn change(server: &mut TestServer, settings: Value) -> Vec<Value> {
    send(
        server,
        json!({"jsonrpc":"2.0","method":"workspace/didChangeConfiguration","params":{"settings":settings}}),
    )
}
pub(super) fn folders(
    server: &mut TestServer,
    layout: &Layout,
    added: &[&str],
    removed: &[&str],
) -> Vec<Value> {
    let rows = |files: &[&str]| {
        files
            .iter()
            .map(|file| json!({"uri":layout.uri(file),"name":file}))
            .collect::<Vec<_>>()
    };
    send(
        server,
        json!({"jsonrpc":"2.0","method":"workspace/didChangeWorkspaceFolders","params":{"event":{"added":rows(added),"removed":rows(removed)}}}),
    )
}

pub(super) fn fallback(crlf: bool) -> Spec {
    let mut s = spec(crlf);
    s.oracle["missingDisk"]
        .as_array_mut()
        .expect("missing files")
        .push(json!("vela.toml"));
    s
}
pub(super) fn session(
    layout: &Layout,
    profile: Value,
    roots: &[&str],
    settings: Value,
) -> TestServer {
    let mut config = crate::LaunchConfiguration::new();
    config.set_watch_files_enabled(false);
    let mut server = TestServer::with_launch_configuration(config);
    let root = roots.first().map(|file| layout.uri(file));
    let list = roots
        .iter()
        .map(|file| json!({"uri":layout.uri(file),"name":file}))
        .collect::<Vec<_>>();
    let messages = send(
        &mut server,
        json!({"jsonrpc":"2.0","id":"workspace-init 中😀","method":"initialize","params":{
        "processId":null,"rootUri":root,"workspaceFolders":list,"capabilities":profile,"initializationOptions":settings}}),
    );
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["id"], "workspace-init 中😀");
    assert!(messages[0].get("error").is_none());
    assert_eq!(
        messages[0]["result"]["capabilities"]["textDocumentSync"],
        json!({"openClose":true,"change":2,"save":false})
    );
    assert_eq!(
        messages[0]["result"]["capabilities"]["workspace"],
        json!({"workspaceFolders":{"supported":true,"changeNotifications":true}})
    );
    assert!(
        send(
            &mut server,
            json!({"jsonrpc":"2.0","method":"initialized","params":{}})
        )
        .is_empty()
    );
    server
}
pub(super) fn schema_queries(
    server: &mut TestServer,
    layout: &Layout,
    source: &Document,
    fields: &Value,
) {
    for name in ["value", "gone", "rank"] {
        let point = source.markers[name].start;
        let params = json!({"textDocument":{"uri":layout.uri("roots/left/schema_caller.vela")},"position":{"line":point.line,"character":point.character}});
        let result=fields[name].as_str().map_or(Value::Null,|kind|json!({"contents":{"kind":"markdown","value":format!("```vela\nHostCell.{name}\n```\n\n_field_: {kind}")},"range":span(source,name)}));
        for id in [31, 32] {
            assert_eq!(
                send(
                    server,
                    json!({"jsonrpc":"2.0","id":id,"method":"textDocument/hover","params":params})
                ),
                vec![json!({"jsonrpc":"2.0","id":id,"result":result})]
            );
        }
        assert_eq!(
            send(
                server,
                json!({"jsonrpc":"2.0","id":33,"method":"textDocument/definition","params":params})
            ),
            vec![json!({"jsonrpc":"2.0","id":33,"result":null})]
        );
    }
}
pub(super) fn schema_warning(message: Option<&str>) -> Vec<Value> {
    message.map_or_else(Vec::new,|message|vec![json!({"code":"schema::unavailable","message":message,"severity":2,"source":"vela",
    "range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"data":{"labels":[],"candidates":[],"repairHints":[]}})])
}
pub(super) fn schema_source_diagnostics(
    s: &Spec,
    layout: &Layout,
    source: &Document,
    file: &str,
    message: Option<&str>,
) -> Vec<Value> {
    if message.is_some() {
        return schema_warning(message);
    }
    let oracle = &s.oracle["fieldErrors"][file];
    if oracle.is_null() {
        return vec![];
    }
    let field = oracle["field"].as_str().expect("missing field");
    let range = span(source, field);
    let uri = layout.uri("roots/left/schema_caller.vela");
    let names = oracle["candidates"]
        .as_array()
        .expect("authored candidates")
        .iter()
        .map(|n| n.as_str().expect("name"))
        .collect::<Vec<_>>();
    let labels = [
        "unknown member access".to_owned(),
        format!("did you mean `{}`?", names[0]),
        format!("similar candidates: {}", names.join(", ")),
    ]
    .into_iter()
    .map(|message| json!({"uri":uri,"range":range,"message":message}))
    .collect::<Vec<_>>();
    vec![
        json!({"code":"analysis::unknown_field","message":format!("unknown field `{field}` for `HostCell`"),"severity":1,"source":"vela","range":range,
        "data":{"labels":labels,"candidates":names.iter().map(|replacement|json!({"replacement":replacement})).collect::<Vec<_>>(),"repairHints":[]}}),
    ]
}
