use super::document_sync_matrix_support::{Layout, open, profiles, send};
use super::{TestServer, message_value};
use crate::matrix_fixture::{Document, document_open::span, load, parse_markers};
use lsp_server::RequestId;
use serde_json::{Value, json};

fn params(uri: &str, doc: &Document, marker: &str) -> Value {
    let point = doc.markers[marker].start;
    json!({"textDocument":{"uri":uri},"position":{"line":point.line,"character":point.character},"context":{"includeDeclaration":true}})
}
fn cancel(server: &mut TestServer, id: Value) {
    assert!(
        send(
            server,
            json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":id}})
        )
        .is_empty()
    );
}
fn error(id: Value, cancelled: bool) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":if cancelled {-32800} else {-32801},"message":if cancelled {"request was cancelled before processing"} else {"request result is stale because the document was modified"}}})
}
fn references(server: &mut TestServer, id: Value, uri: &str, doc: &Document, marker: &str) {
    let mut responses = send(
        server,
        json!({"jsonrpc":"2.0","id":id,"method":"textDocument/references","params":params(uri,doc,marker)}),
    );
    assert_eq!(responses.len(), 1);
    let result = responses[0]["result"]
        .as_array_mut()
        .expect("reference result");
    result.sort_by_key(Value::to_string);
    let mut expected = if marker == "unknown" {
        vec![]
    } else {
        vec![
            json!({"uri":uri,"range":span(doc,"decl")}),
            json!({"uri":uri,"range":span(doc,"call")}),
        ]
    };
    expected.sort_by_key(Value::to_string);
    assert_eq!(
        responses,
        vec![json!({"jsonrpc":"2.0","id":id,"result":expected})]
    );
}
fn spec(crlf: bool) -> crate::matrix_fixture::Spec {
    let mut s = load("request-cancellation");
    if crlf {
        for text in s.files.values_mut() {
            *text = text.replace('\n', "\r\n");
        }
    }
    s
}

#[test]
fn cancellation_matrix_discards_real_queued_and_completed_tasks_preserves_facts_and_allows_id_reuse()
 {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let uri = layout.uri("scripts/main.vela");
        for profile in profiles() {
            for negative in [false, true] {
                let doc = if negative {
                    parse_markers(
                        &s.oracle["negative"]
                            .as_str()
                            .expect("negative")
                            .replace('\n', if crlf { "\r\n" } else { "\n" }),
                    )
                    .expect("negative")
                } else {
                    parse_markers(&s.files["scripts/main.vela"]).expect("source")
                };
                let marker = if negative { "unknown" } else { "call" };
                for id in [
                    json!(i32::MIN),
                    json!(0),
                    json!(i32::MAX),
                    json!("cancel 中😀"),
                    json!("20"),
                    json!(""),
                ] {
                    for complete_first in [false, true] {
                        let mut server = layout.server(profile.clone());
                        open(&mut server, &uri, &doc.text, -7);
                        let before = server.snapshot();
                        server.queue_request_with_id(
                            serde_json::from_value::<RequestId>(id.clone()).expect("typed ID"),
                            "textDocument/references",
                            params(&uri, &doc, marker),
                        );
                        let task = if complete_first {
                            let task = server.receive_task();
                            cancel(&mut server, id.clone());
                            task
                        } else {
                            cancel(&mut server, id.clone());
                            server.receive_task()
                        };
                        cancel(&mut server, id.clone());
                        let (outcome, replies) = server.publish_task(task);
                        assert_eq!(outcome, crate::task::TaskOutcome::Cancelled);
                        assert_eq!(
                            replies.iter().map(message_value).collect::<Vec<_>>(),
                            vec![error(id.clone(), true)]
                        );
                        let after = server.snapshot();
                        assert_eq!(after.generation(), before.generation());
                        assert_eq!(
                            after.databases().parse_db().parse_count(),
                            before.databases().parse_db().parse_count()
                        );
                        let buffer = after
                            .workspace()
                            .document(&layout.id("scripts/main.vela"))
                            .expect("overlay");
                        assert_eq!(
                            (buffer.text(), buffer.version()),
                            (
                                doc.text.as_str(),
                                vela_language_service::SourceVersion::new(u64::from(u32::MAX - 6))
                            )
                        );
                        assert_eq!(
                            after.databases().source_db().records()
                                [&layout.id("scripts/main.vela")]
                                .text(),
                            doc.text
                        );
                        cancel(&mut server, id.clone());
                        references(&mut server, id.clone(), &uri, &doc, marker);
                        references(&mut server, id.clone(), &uri, &doc, marker);
                    }
                }
            }
        }
        layout.check_disk(&s);
    }
}

#[test]
fn cancellation_matrix_ignores_unknown_late_malformed_and_different_typed_ids_without_poisoning_requests()
 {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let uri = layout.uri("scripts/main.vela");
        let doc = parse_markers(&s.files["scripts/main.vela"]).expect("source");
        for profile in profiles() {
            let mut server = layout.server(profile);
            open(&mut server, &uri, &doc.text, 1);
            let before = server.snapshot();
            for payload in [
                Value::Null,
                json!({}),
                json!({"id":null}),
                json!({"id":true}),
                json!({"id":[]}),
                json!({"id":{}}),
                json!({"id":1.25}),
            ] {
                assert!(
                    send(
                        &mut server,
                        json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":payload})
                    )
                    .is_empty()
                );
                assert_eq!(server.snapshot().generation(), before.generation());
            }
            for id in [json!(20), json!("20"), json!("late 中😀")] {
                cancel(&mut server, id.clone());
                references(&mut server, id.clone(), &uri, &doc, "call");
                cancel(&mut server, id.clone());
                references(&mut server, id, &uri, &doc, "call");
            }
            for (id, other) in [(json!(20), json!("20")), (json!("20"), json!(20))] {
                server.queue_request_with_id(
                    serde_json::from_value(id.clone()).expect("ID"),
                    "textDocument/references",
                    params(&uri, &doc, "call"),
                );
                let task = server.receive_task();
                cancel(&mut server, other);
                let (outcome, replies) = server.publish_task(task);
                assert_eq!(outcome, crate::task::TaskOutcome::Completed);
                let mut actual = replies.iter().map(message_value).collect::<Vec<_>>();
                actual[0]["result"]
                    .as_array_mut()
                    .expect("locations")
                    .sort_by_key(Value::to_string);
                let mut expected = vec![
                    json!({"uri":uri,"range":span(&doc,"decl")}),
                    json!({"uri":uri,"range":span(&doc,"call")}),
                ];
                expected.sort_by_key(Value::to_string);
                assert_eq!(
                    actual,
                    vec![json!({"jsonrpc":"2.0","id":id,"result":expected})]
                );
            }
            assert_eq!(server.snapshot().generation(), before.generation());
            layout.check_disk(&s);
        }
    }
}

#[test]
fn cancellation_matrix_keeps_typed_target_isolation_and_cancel_priority_across_stale_generations() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let uri = layout.uri("scripts/main.vela");
        let old = parse_markers(&s.files["scripts/main.vela"]).expect("old");
        let shifted = parse_markers(
            &s.oracle["shifted"]
                .as_str()
                .expect("new")
                .replace('\n', if crlf { "\r\n" } else { "\n" }),
        )
        .expect("new");
        for profile in profiles() {
            for cancel_first in [false, true] {
                let mut server = layout.server(profile.clone());
                open(&mut server, &uri, &old.text, 1);
                server.queue_request(20, "textDocument/references", params(&uri, &old, "call"));
                server.queue_request_with_id(
                    RequestId::from("20".to_owned()),
                    "textDocument/references",
                    params(&uri, &old, "call"),
                );
                let tasks = [server.receive_task(), server.receive_task()];
                if cancel_first {
                    cancel(&mut server, json!(20));
                }
                send(
                    &mut server,
                    json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":shifted.text}]}}),
                );
                if !cancel_first {
                    cancel(&mut server, json!(20));
                }
                let mut replies = vec![];
                let mut outcomes = vec![];
                for task in tasks {
                    let (outcome, messages) = server.publish_task(task);
                    outcomes.push(outcome);
                    replies.extend(messages.iter().map(message_value));
                }
                assert!(outcomes.contains(&crate::task::TaskOutcome::Cancelled));
                assert!(outcomes.contains(&crate::task::TaskOutcome::StaleDiscarded));
                replies.sort_by_key(Value::to_string);
                let mut expected = vec![error(json!(20), true), error(json!("20"), false)];
                expected.sort_by_key(Value::to_string);
                assert_eq!(replies, expected);
                references(&mut server, json!(20), &uri, &shifted, "call");
                references(&mut server, json!("20"), &uri, &shifted, "call");
                layout.check_disk(&s);
            }
        }
    }
}
