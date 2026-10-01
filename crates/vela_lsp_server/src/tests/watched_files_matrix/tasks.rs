use super::support::*;
use crate::task::TaskOutcome;
use lsp_server::RequestId;
use serde_json::{Value, json};

fn references_params(layout: &Layout, caller: &Document) -> Value {
    let mut value = params(&layout.uri("scripts/main.vela"), caller, "call");
    value["context"] = json!({"includeDeclaration":true});
    value
}

#[test]
fn watched_mutations_reject_real_old_tasks_before_and_after_worker_completion() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let caller = doc(&s.files["scripts/main.vela"], false);
        let old = doc(&s.files["scripts/defs.vela"], false);
        let changed = variant(&s, "shifted");
        for complete_first in [false, true] {
            layout.write_disk("scripts/defs.vela", &old.text);
            let mut server = layout.server(json!({}));
            open(
                &mut server,
                &layout.uri("scripts/main.vela"),
                &caller.text,
                9,
            );
            let frozen = server.snapshot();
            server.queue_request(
                41,
                "textDocument/references",
                references_params(&layout, &caller),
            );
            let task = complete_first.then(|| server.receive_task());
            layout.write_disk("scripts/defs.vela", &changed.text);
            watch(&mut server, &layout, &[("scripts/defs.vela", 2)]);
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
                frozen.databases().source_db().records()[&layout.id("scripts/defs.vela")].text(),
                old.text
            );
            definition(
                &mut server,
                &layout,
                &caller,
                Some(("scripts/defs.vela", &changed)),
            );
        }
    }
}

#[test]
fn watched_reloads_preserve_typed_cancellation_priority_without_poisoning_new_queries() {
    let s = spec(false);
    let layout = Layout::new(&s);
    let caller = doc(&s.files["scripts/main.vela"], false);
    let original = doc(&s.files["scripts/defs.vela"], false);
    let changed = variant(&s, "shifted");
    for (id, other) in [(json!(41), json!("41")), (json!("41"), json!(41))] {
        for matching in [false, true] {
            layout.write_disk("scripts/defs.vela", &original.text);
            let mut server = layout.server(json!({}));
            open(
                &mut server,
                &layout.uri("scripts/main.vela"),
                &caller.text,
                1,
            );
            server.queue_request_with_id(
                serde_json::from_value::<RequestId>(id.clone()).expect("ID"),
                "textDocument/references",
                references_params(&layout, &caller),
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
            layout.write_disk("scripts/defs.vela", &changed.text);
            watch(&mut server, &layout, &[("scripts/defs.vela", 2)]);
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
                messages.iter().map(message_value).collect::<Vec<Value>>(),
                vec![json!({"jsonrpc":"2.0","id":id,"error":{
                "code":if matching{-32800}else{-32801},"message":if matching{"request was cancelled before processing"}else{"request result is stale because the document was modified"}}})]
            );
            definition(
                &mut server,
                &layout,
                &caller,
                Some(("scripts/defs.vela", &changed)),
            );
        }
    }
}
