use super::document_sync_matrix_support::{Layout, open, profiles, published, send};
use super::{TestServer, message_value};
use crate::matrix_fixture::{Document, document_open as oracle, load, parse_markers};
use serde_json::{Value, json};
use vela_language_service::SourceVersion;

fn spec(crlf: bool) -> crate::matrix_fixture::Spec {
    let mut s = load("document-change");
    if crlf {
        for text in s.files.values_mut() {
            *text = text.replace('\n', "\r\n");
        }
    }
    s
}
fn doc(text: &str, crlf: bool) -> Document {
    parse_markers(&text.replace('\n', if crlf { "\r\n" } else { "\n" })).expect("authored source")
}
fn change(server: &mut TestServer, uri: &str, version: i32, changes: Value) -> Vec<Value> {
    send(
        server,
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":version},"contentChanges":changes}}),
    )
}
fn current(server: &TestServer, layout: &Layout, file: &str, text: &str, version: i32) {
    let snapshot = server.snapshot();
    let id = layout.id(file);
    let buffer = snapshot.workspace().document(&id).expect("buffer");
    let bits = SourceVersion::new(u64::from(u32::from_ne_bytes(version.to_ne_bytes())));
    assert_eq!((buffer.text(), buffer.version()), (text, bits));
    if file.ends_with(".vela") {
        let source = &snapshot.databases().source_db().records()[&id];
        assert_eq!((source.text(), source.version()), (text, bits));
    }
}

#[test]
fn document_change_unopened_ranges_and_malformed_payloads_preserve_controlled_source_ownership() {
    let s = spec(false);
    let layout = Layout::new(&s);
    let file = "scripts/scratch.vela";
    let uri = layout.uri(file);
    let mut server = layout.server(json!({}));
    let old = server.snapshot();
    let error = change(
        &mut server,
        &uri,
        1,
        json!([{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"text":"fn poisoned("}]),
    );
    assert_eq!(
        error,
        vec![
            json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{
        "uri":uri,"diagnostics":[],"error":"ranged didChange requires an open document"}})
        ]
    );
    assert_eq!(server.snapshot().generation(), old.generation());
    assert!(
        server
            .snapshot()
            .workspace()
            .document(&layout.id(file))
            .is_none()
    );
    let source = doc(
        s.oracle["ranges"]["initial"].as_str().expect("source"),
        false,
    );
    assert!(
        published(
            &change(&mut server, &uri, 1, json!([{"text":source.text}])),
            &uri
        )
        .is_empty()
    );
    current(&server, &layout, file, &source.text, 1);
    let generation = server.snapshot().generation();
    for params in [
        Value::Null,
        json!({}),
        json!({"textDocument":{"uri":uri,"version":2}}),
        json!({"textDocument":{"uri":uri,"version":"2"},"contentChanges":[{"text":"poison"}]}),
        json!({"textDocument":{"uri":uri,"version":2},"contentChanges":false}),
        json!({"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":false}]}),
        json!({"textDocument":{"uri":uri,"version":2},"contentChanges":[{"range":{"start":{"line":0,"character":-1},"end":{"line":0,"character":0}},"text":"poison"}]}),
    ] {
        assert!(
            send(
                &mut server,
                json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":params})
            )
            .is_empty()
        );
        assert_eq!(server.snapshot().generation(), generation);
        current(&server, &layout, file, &source.text, 1);
    }
    layout.check_disk(&s);
}

#[test]
fn document_change_matrix_preserves_full_repair_restore_facts_and_signed_stale_version_policy() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        for capabilities in profiles() {
            for case in s.oracle["cases"].as_array().expect("cases") {
                let file = case["file"].as_str().expect("file");
                let uri = layout.uri(file);
                let initial = parse_markers(&s.files[file]).expect("initial");
                let mut server = layout.server(capabilities.clone());
                assert_eq!(
                    published(&open(&mut server, &uri, &initial.text, -7), &uri),
                    oracle::expected(&initial, case, &uri)
                );
                for phase in case["steps"].as_array().expect("phases") {
                    let source = doc(phase["text"].as_str().expect("source"), crlf);
                    let version =
                        i32::try_from(phase["version"].as_i64().expect("version")).expect("i32");
                    let old = server.snapshot();
                    let previous = old
                        .workspace()
                        .document(&layout.id(file))
                        .expect("previous")
                        .text()
                        .to_owned();
                    let messages =
                        change(&mut server, &uri, version, json!([{"text":source.text}]));
                    assert_eq!(messages.len(), 1);
                    assert_eq!(
                        published(&messages, &uri),
                        oracle::expected(&source, phase, &uri)
                    );
                    current(&server, &layout, file, &source.text, version);
                    assert_ne!(server.snapshot().generation(), old.generation());
                    assert_eq!(
                        old.workspace()
                            .document(&layout.id(file))
                            .expect("immutable")
                            .text(),
                        previous
                    );
                    let generation = server.snapshot().generation();
                    let parse = server.snapshot().databases().parse_db().parse_count();
                    for stale in [version, version.saturating_sub(1), i32::MIN] {
                        assert!(
                            change(&mut server, &uri, stale, json!([{"text":"fn poisoned("}]))
                                .is_empty()
                        );
                        assert_eq!(server.snapshot().generation(), generation);
                        assert_eq!(
                            server.snapshot().databases().parse_db().parse_count(),
                            parse
                        );
                        current(&server, &layout, file, &source.text, version);
                    }
                    let mut fresh = layout.server(capabilities.clone());
                    assert_eq!(
                        published(&open(&mut fresh, &uri, &source.text, version), &uri),
                        oracle::expected(&source, phase, &uri)
                    );
                    layout.check_disk(&s);
                }
            }
        }
    }
}

#[test]
fn document_change_ranges_apply_in_message_order_and_mixed_replacements_preserve_exact_unicode_bytes()
 {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let r = &s.oracle["ranges"];
        let file = "scripts/scratch.vela";
        let uri = layout.uri(file);
        let initial = doc(r["initial"].as_str().expect("initial"), crlf);
        let first = doc(r["afterFirst"].as_str().expect("first"), crlf);
        let sequential = doc(r["sequential"].as_str().expect("sequential"), crlf);
        let mut server = layout.server(json!({}));
        assert!(published(&open(&mut server, &uri, &initial.text, 1), &uri).is_empty());
        let changes = json!([
            {"range":oracle::span(&initial,"value"),"rangeLength":999999,"text":"22"},
            {"range":oracle::span(&first,"value"),"text":"333"},
            {"range":{"start":{"line":1,"character":0},"end":{"line":1,"character":0}},"text":if crlf{"// tail 中😀 %\r\n"}else{"// tail 中😀 %\n"}}
        ]);
        assert!(published(&change(&mut server, &uri, 2, changes), &uri).is_empty());
        current(&server, &layout, file, &sequential.text, 2);
        let replacement = doc(r["replacement"].as_str().expect("replacement"), crlf);
        let mixed = doc(r["mixed"].as_str().expect("mixed"), crlf);
        let changes = json!([{"text":replacement.text},{"range":oracle::span(&replacement,"name"),"text":"changed"},
            {"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"text":if crlf{"// prefix 中😀 %\r\n"}else{"// prefix 中😀 %\n"}}]);
        assert!(published(&change(&mut server, &uri, 3, changes), &uri).is_empty());
        current(&server, &layout, file, &mixed.text, 3);
        layout.check_disk(&s);
    }
}

#[test]
fn document_change_invalid_ranges_rollback_atomically_and_keep_current_diagnostics() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let r = &s.oracle["ranges"];
        let file = "scripts/scratch.vela";
        let uri = layout.uri(file);
        let source = doc(r["invalid"].as_str().expect("invalid"), crlf);
        let mut server = layout.server(json!({}));
        let expected = oracle::expected(&source, r, &uri);
        assert_eq!(
            published(&open(&mut server, &uri, &source.text, 1), &uri),
            expected
        );
        let emoji = source.markers["emoji"].start;
        let visible_end = source
            .text
            .split('\n')
            .next()
            .expect("line")
            .trim_end_matches('\r')
            .encode_utf16()
            .count();
        let bad = [
            (json!([]), "didChange requires at least one content change"),
            (
                json!([{"range":{"start":{"line":99,"character":0},"end":{"line":99,"character":0}},"text":"poison"}]),
                "LSP position line is outside the document",
            ),
            (
                json!([{"range":{"start":{"line":0,"character":9999},"end":{"line":0,"character":9999}},"text":"poison"}]),
                "LSP position character is outside the line",
            ),
            (
                json!([{"range":{"start":{"line":emoji.line,"character":emoji.character+1},"end":{"line":emoji.line,"character":emoji.character+1}},"text":"poison"}]),
                "LSP position splits a UTF-16 character",
            ),
            (
                json!([{"range":{"start":{"line":0,"character":10},"end":{"line":0,"character":0}},"text":"poison"}]),
                "didChange range start must not be after the end",
            ),
            (
                json!([{"range":oracle::span(&source,"typo"),"text":"first"},{"range":{"start":{"line":99,"character":0},"end":{"line":99,"character":0}},"text":"poison"}]),
                "LSP position line is outside the document",
            ),
        ];
        let bad=bad.into_iter().chain([
            (json!([{"text":"fn replacement() {}"},{"range":{"start":{"line":99,"character":0},"end":{"line":99,"character":0}},"text":"poison"}]),"LSP position line is outside the document"),
            (json!([{"range":{"start":{"line":0,"character":visible_end+1},"end":{"line":0,"character":visible_end+1}},"text":"poison"}]),"LSP position character is outside the line"),
        ]);
        for (changes, error) in bad {
            let before = server.snapshot();
            let messages = change(&mut server, &uri, 2, changes);
            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0]["params"]["error"], error);
            assert_eq!(
                published(&messages, &uri),
                expected,
                "invalid edit must not clear current diagnostics"
            );
            assert_eq!(server.snapshot().generation(), before.generation());
            current(&server, &layout, file, &source.text, 1);
        }
        let repaired = doc(r["repaired"].as_str().expect("repair"), crlf);
        assert!(
            published(
                &change(&mut server, &uri, 2, json!([{"text":repaired.text}])),
                &uri
            )
            .is_empty()
        );
        current(&server, &layout, file, &repaired.text, 2);
        layout.check_disk(&s);
    }
}

#[test]
fn document_change_dependency_updates_discard_stale_or_cancelled_tasks_and_keep_current_hover() {
    for crlf in [false, true] {
        for cancelled in [false, true] {
            let s = spec(crlf);
            let layout = Layout::new(&s);
            let mut server = layout.server(json!({}));
            let caller_file = "scripts/open_caller.vela";
            let api_file = "scripts/open_api.vela";
            let caller = parse_markers(&s.files[caller_file]).expect("caller");
            let caller_uri = layout.uri(caller_file);
            let api_uri = layout.uri(api_file);
            assert!(
                published(
                    &open(&mut server, &caller_uri, &caller.text, 1),
                    &caller_uri
                )
                .is_empty()
            );
            let initial = parse_markers(&s.files[api_file]).expect("api");
            assert!(published(&open(&mut server, &api_uri, &initial.text, 1), &api_uri).is_empty());
            let point = caller.markers["call"].start;
            for (index, phase) in s.oracle["dependency"]
                .as_array()
                .expect("phases")
                .iter()
                .enumerate()
            {
                let source = doc(phase["text"].as_str().expect("text"), crlf);
                server.queue_request(20,"textDocument/references",json!({"textDocument":{"uri":caller_uri},
                "position":{"line":point.line,"character":point.character},"context":{"includeDeclaration":true}}));
                let old_task = server.receive_task();
                let before = server.snapshot();
                let messages = change(
                    &mut server,
                    &api_uri,
                    index as i32 + 2,
                    json!([{"text":source.text}]),
                );
                assert!(published(&messages, &api_uri).is_empty());
                if phase["id"] == "signature" {
                    assert_eq!(messages.len(), 2);
                    assert!(published(&messages, &caller_uri).is_empty());
                }
                assert_ne!(server.snapshot().generation(), before.generation());
                if cancelled {
                    assert!(
                        send(
                            &mut server,
                            json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":20}})
                        )
                        .is_empty()
                    );
                }
                let (outcome, replies) = server.publish_task(old_task);
                assert_eq!(
                    outcome,
                    if cancelled {
                        crate::task::TaskOutcome::Cancelled
                    } else {
                        crate::task::TaskOutcome::StaleDiscarded
                    }
                );
                let replies = replies.iter().map(message_value).collect::<Vec<_>>();
                assert_eq!(replies.len(), 1);
                assert_eq!(replies[0]["id"], 20);
                assert_eq!(
                    replies[0]["error"]["code"],
                    if cancelled { -32800 } else { -32801 }
                );
                assert!(replies[0].get("result").is_none());
                assert_eq!(
                    replies[0]["error"]["message"],
                    if cancelled {
                        "request was cancelled before processing"
                    } else {
                        "request result is stale because the document was modified"
                    }
                );
                for request_id in [30, 31] {
                    assert_eq!(
                        send(
                            &mut server,
                            json!({"jsonrpc":"2.0","id":request_id,"method":"textDocument/hover","params":{
                "textDocument":{"uri":caller_uri},"position":{"line":point.line,"character":point.character}}})
                        ),
                        vec![json!({
                    "jsonrpc":"2.0","id":request_id,"result":{"contents":{"kind":"markdown","value":phase["hover"]},
                        "range":oracle::span(&caller,"call")}})]
                    );
                }
                current(&server, &layout, api_file, &source.text, index as i32 + 2);
                layout.check_disk(&s);
            }
        }
    }
}
