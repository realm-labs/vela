use super::support::*;
use crate::{task::TaskOutcome, tests::message_value};
use lsp_server::RequestId;
use serde_json::{Value, json};

fn params(layout: &Layout, caller: &Document) -> Value {
    let point = caller.markers["call"].start;
    json!({"textDocument":{"uri":layout.uri("roots/left/caller.vela")},
        "position":{"line":point.line,"character":point.character},"context":{"includeDeclaration":true}})
}

fn mutate(server: &mut TestServer, layout: &Layout, configuration: bool) {
    if configuration {
        change(
            server,
            json!({"workspace":{"roots":[layout.uri("roots/right")]}}),
        );
    } else {
        folders(server, layout, &["roots/right"], &["roots/left"]);
    }
}

#[test]
fn workspace_mutations_reject_old_workers_before_and_after_completion() {
    for crlf in [false, true] {
        let s = fallback(crlf);
        let layout = Layout::new(&s);
        let caller = doc(&s, "roots/left/caller.vela");
        let alpha = doc(&s, "roots/left/alpha.vela");
        for configuration in [false, true] {
            for completed in [false, true] {
                let mut server = session(&layout, json!({}), &["roots/left"], Value::Null);
                open(
                    &mut server,
                    &layout.uri("roots/left/caller.vela"),
                    &caller.text,
                    73,
                );
                let frozen = server.snapshot();
                server.queue_request(41, "textDocument/references", params(&layout, &caller));
                let task = completed.then(|| server.receive_task());
                mutate(&mut server, &layout, configuration);
                let task = task.unwrap_or_else(|| server.receive_task());
                let (outcome, messages) = server.publish_task(task);
                assert_eq!(outcome, TaskOutcome::StaleDiscarded);
                assert_eq!(
                    messages.iter().map(message_value).collect::<Vec<_>>(),
                    vec![
                        json!({"jsonrpc":"2.0","id":41,"error":{"code":-32801,"message":"request result is stale because the document was modified"}})
                    ]
                );
                assert!(server.snapshot().generation() > frozen.generation());
                assert_eq!(
                    frozen.databases().source_db().records()[&layout.id("roots/left/alpha.vela")]
                        .text(),
                    alpha.text
                );
                definition(
                    &mut server,
                    &layout,
                    "roots/left/caller.vela",
                    &caller,
                    "call",
                    None,
                );
            }
        }
        layout.check_disk(&s);
    }
}

#[test]
fn workspace_mutations_preserve_typed_cancellation_priority_and_new_queries() {
    let s = fallback(false);
    let layout = Layout::new(&s);
    let caller = doc(&s, "roots/left/caller.vela");
    for configuration in [false, true] {
        for (id, other) in [(json!(41), json!("41")), (json!("41"), json!(41))] {
            for matching in [false, true] {
                let mut server = session(&layout, json!({}), &["roots/left"], Value::Null);
                open(
                    &mut server,
                    &layout.uri("roots/left/caller.vela"),
                    &caller.text,
                    73,
                );
                server.queue_request_with_id(
                    serde_json::from_value::<RequestId>(id.clone()).expect("typed ID"),
                    "textDocument/references",
                    params(&layout, &caller),
                );
                let task = server.receive_task();
                let cancel = if matching { id.clone() } else { other.clone() };
                assert!(
                    send(
                        &mut server,
                        json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":cancel}})
                    )
                    .is_empty()
                );
                mutate(&mut server, &layout, configuration);
                let (outcome, messages) = server.publish_task(task);
                assert_eq!(
                    outcome,
                    if matching {
                        TaskOutcome::Cancelled
                    } else {
                        TaskOutcome::StaleDiscarded
                    }
                );
                assert_eq!(
                    messages.iter().map(message_value).collect::<Vec<_>>(),
                    vec![json!({"jsonrpc":"2.0","id":id,"error":{
                    "code":if matching {-32800} else {-32801},"message":if matching {"request was cancelled before processing"} else {"request result is stale because the document was modified"}}})]
                );
                definition(
                    &mut server,
                    &layout,
                    "roots/left/caller.vela",
                    &caller,
                    "call",
                    None,
                );
            }
        }
    }
    layout.check_disk(&s);
}

#[test]
fn workspace_membership_noop_preserves_held_reference_results() {
    let s = fallback(false);
    let layout = Layout::new(&s);
    let caller = doc(&s, "roots/left/caller.vela");
    let alpha = doc(&s, "roots/left/alpha.vela");
    let mut server = session(&layout, json!({}), &["roots/left"], Value::Null);
    open(
        &mut server,
        &layout.uri("roots/left/caller.vela"),
        &caller.text,
        73,
    );
    server.queue_request(41, "textDocument/references", params(&layout, &caller));
    let task = server.receive_task();
    folders(&mut server, &layout, &["roots/left"], &["roots/left"]);
    let (outcome, messages) = server.publish_task(task);
    assert_eq!(outcome, TaskOutcome::Completed);
    assert_eq!(
        messages.iter().map(message_value).collect::<Vec<_>>(),
        vec![json!({"jsonrpc":"2.0","id":41,"result":[
            {"uri":layout.uri("roots/left/alpha.vela"),"range":span(&alpha,"decl")},
            {"uri":layout.uri("roots/left/caller.vela"),"range":s.oracle["importNameRange"]},
            {"uri":layout.uri("roots/left/caller.vela"),"range":span(&caller,"call")},
            {"uri":layout.uri("roots/left/cross.vela"),"range":s.oracle["importNameRange"]},
            {"uri":layout.uri("roots/left/cross.vela"),"range":span(&doc(&s,"roots/left/cross.vela"),"alpha")}
        ]})]
    );
    layout.check_disk(&s);
}
