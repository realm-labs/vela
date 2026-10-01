#[path = "support/stdio.rs"]
mod process;
#[allow(dead_code, unused_imports)]
#[path = "../../../tests/lsp_matrix/support/mod.rs"]
mod stdio_fixture;
use serde_json::{Value, json};

#[test]
fn stdio_binary_uses_typed_transport_for_initialize_and_exit() {
    let output = process::run(vec![
        frame(&initialize_request()).into_bytes(),
        frame(&exit_notification()).into_bytes(),
    ]);
    assert!(
        output.status.success(),
        "server exited with {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let messages = process::messages(&output.stdout);
    assert_eq!(messages.len(), 1, "{messages:?}");
    let response = &messages[0];
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["serverInfo"]["name"], "vela_lsp_server");
    assert_eq!(
        response["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
}

fn initialize_request() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "processId": null,
            "capabilities": {}
        }
    })
    .to_string()
}

fn exit_notification() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "exit"
    })
    .to_string()
}

fn frame(message: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{message}", message.len())
}

fn initialize(id: Value, capabilities: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"initialize","params":{"processId":null,"capabilities":capabilities}})
}
fn notification(method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","method":method,"params":params})
}
fn finish() -> Vec<u8> {
    [
        process::frame(
            json!({"jsonrpc":"2.0","id":"shutdown 中😀","method":"shutdown","params":null}),
        ),
        process::frame(notification("exit", Value::Null)),
    ]
    .concat()
}

#[test]
fn stdio_process_matrix_round_trips_fragmented_utf8_sources_and_exact_repair_diagnostics() {
    let s = stdio_fixture::load("document-change");
    let case = s.oracle["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|c| c["file"] == "scripts/invalid.vela")
        .expect("invalid declarations");
    let uri = "file:///stdio/%E4%B8%AD%20%25%20%F0%9F%98%80/invalid.vela";
    for crlf in [false, true] {
        for capabilities in [
            json!({}),
            json!({"textDocument":{"publishDiagnostics":{"versionSupport":true,"relatedInformation":false}}}),
            json!({"textDocument":{"publishDiagnostics":{"relatedInformation":true,"tagSupport":{"valueSet":[1,2]}}}}),
        ] {
            let source = stdio_fixture::parse_markers(
                &s.files["scripts/invalid.vela"].replace('\n', if crlf { "\r\n" } else { "\n" }),
            )
            .expect("source");
            let repaired = stdio_fixture::parse_markers(
                &case["steps"][1]["text"]
                    .as_str()
                    .expect("repair")
                    .replace('\n', if crlf { "\r\n" } else { "\n" }),
            )
            .expect("repair");
            let payload=[process::frame(initialize(json!("init 中😀"),capabilities)),process::frame(notification("initialized",json!({}))),
                process::frame(notification("textDocument/didOpen",json!({"textDocument":{"uri":uri,"languageId":"vela","version":-7,"text":source.text}}))),
                process::frame(notification("textDocument/didChange",json!({"textDocument":{"uri":uri,"version":-6},"contentChanges":[{"text":repaired.text}]}))),
                process::frame(notification("textDocument/didChange",json!({"textDocument":{"uri":uri,"version":-7},"contentChanges":[{"text":"fn poisoned("}]}))),
                process::frame(notification("textDocument/didClose",json!({"textDocument":{"uri":uri}})))].concat();
            // Cut headers, bodies and multibyte UTF-8 at byte boundaries. The
            // complete terminal packet is written before the reader sees exit.
            let mut chunks = payload.chunks(7).map(<[u8]>::to_vec).collect::<Vec<_>>();
            chunks.push(finish());
            let output = process::run(chunks);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty(), "default stdio has no log noise");
            let messages = process::messages(&output.stdout);
            assert_eq!(messages.len(), 5);
            assert_eq!(messages[0]["id"], "init 中😀");
            assert_eq!(
                messages[0]["result"]["serverInfo"],
                json!({"name":"vela_lsp_server","version":env!("CARGO_PKG_VERSION")})
            );
            assert_eq!(
                messages[0]["result"]["capabilities"]["textDocumentSync"],
                json!({"openClose":true,"change":2,"save":false})
            );
            assert_eq!(messages[1]["method"], "textDocument/publishDiagnostics");
            assert_eq!(messages[1]["params"]["uri"], uri);
            assert!(messages[1]["params"].get("version").is_none());
            let facts=messages[1]["params"]["diagnostics"].as_array().expect("diagnostics").iter().map(|d| {
                assert_eq!(d["source"],"vela");
                json!({"code":d["code"],"severity":d["severity"],"message":d["message"],"range":d["range"],"labels":d["data"]["labels"],"candidates":d["data"]["candidates"].as_array().expect("candidates").iter().map(|c|c["replacement"].clone()).collect::<Vec<_>>(),"repairHints":d["data"]["repairHints"].as_array().expect("repairs").len()})
            }).collect::<Vec<_>>();
            let mut expected = stdio_fixture::document_open::expected(&source, case, uri);
            // This standalone process has no workspace root: scratch assembly
            // owns the main module, unlike the reviewed package's invalid module.
            let method = expected
                .iter_mut()
                .find(|d| d["code"] == "hir::duplicate_script_method")
                .expect("authored method fact");
            assert_eq!(
                method["message"],
                "duplicate script method `invalid::BadFields.size`"
            );
            method["message"] = json!("duplicate script method `main::BadFields.size`");
            assert_eq!(facts, expected);
            for message in &messages[2..4] {
                assert_eq!(
                    message,
                    &json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":[]}})
                );
            }
            assert_eq!(
                messages[4],
                json!({"jsonrpc":"2.0","id":"shutdown 中😀","result":null})
            );
        }
    }
}

#[test]
fn stdio_process_matrix_keeps_protocol_errors_framed_and_ignores_queued_messages_after_exit() {
    let messages = [
        json!({"jsonrpc":"2.0","id":1,"method":"textDocument/hover","params":{}}),
        initialize(json!("init😀"), json!({})),
        json!({"jsonrpc":"2.0","id":2,"method":"vela/unknown😀","params":null}),
        json!({"jsonrpc":"2.0","id":3,"method":"textDocument/didSave","params":{}}),
        json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":true}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":null}),
    ];
    let payload = [
        messages
            .into_iter()
            .flat_map(process::frame)
            .collect::<Vec<_>>(),
        finish(),
        process::frame(initialize(json!(999), json!({}))),
    ]
    .concat();
    let output = process::run(vec![payload]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let messages = process::messages(&output.stdout);
    assert_eq!(messages.len(), 5);
    assert_eq!(
        messages[0],
        json!({"jsonrpc":"2.0","id":1,"error":{"code":-32002,"message":"server has not been initialized"}})
    );
    assert_eq!(messages[1]["id"], "init😀");
    assert_eq!(
        messages[2],
        json!({"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"method `vela/unknown😀` is not implemented"}})
    );
    assert_eq!(
        messages[3],
        json!({"jsonrpc":"2.0","id":3,"error":{"code":-32600,"message":"`textDocument/didSave` must be sent as a notification"}})
    );
    assert_eq!(
        messages[4],
        json!({"jsonrpc":"2.0","id":"shutdown 中😀","result":null})
    );
}

#[test]
fn stdio_process_matrix_terminates_on_eof_invalid_headers_and_truncated_or_invalid_json_frames() {
    for input in [
        vec![],
        b"Content-Length: nope\r\n\r\n{}".to_vec(),
        b"Content-Length: 20\r\n\r\n{}".to_vec(),
        b"Content-Length: 1\r\n\r\n{".to_vec(),
        b"Wrong-Header: 2\r\n\r\n{}".to_vec(),
    ] {
        let empty = input.is_empty();
        let output = process::run(vec![input]);
        assert!(
            output.stdout.is_empty(),
            "transport failures cannot print protocol junk"
        );
        if empty {
            assert!(output.status.success());
            assert!(output.stderr.is_empty());
        } else {
            assert!(!output.status.success());
            assert!(!output.stderr.is_empty());
        }
    }
}
