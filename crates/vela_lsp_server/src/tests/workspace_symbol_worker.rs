use super::document_symbol_lifecycle::{Harness, check_snapshot};
use super::document_sync_matrix_support::send;
use super::workspace_symbol_lifecycle::check;
use super::{message_value, notify};
use crate::matrix_fixture::{
    FixtureWorkspace, document_symbol_lifecycle as states, workspace_symbols as oracle,
};
use crate::task::TaskOutcome;
use lsp_server::RequestId;
use lsp_types::notification as n;
use serde_json::{Value, json};

fn contracts() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tests/lsp_matrix/fixtures/workspace-symbol-worker.json"
    ))
    .expect("authored worker contracts")
}

fn queue(harness: &mut Harness, id: &Value, query: &str) {
    harness.test_server.queue_request_with_id(
        serde_json::from_value::<RequestId>(id.clone()).expect("typed ID"),
        "workspace/symbol",
        json!({"query":query}),
    );
}

fn cancel(harness: &mut Harness, params: Value) {
    assert!(
        send(
            &mut harness.test_server,
            json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":params})
        )
        .is_empty()
    );
}

fn expected(
    harness: &Harness,
    fixture: &FixtureWorkspace,
    phase: &Value,
    query: &str,
    id: Value,
) -> Value {
    let mut effective = fixture.clone();
    for (file, doc) in &fixture.open {
        effective.disk.insert(file.clone(), doc.clone());
    }
    let workspace = &phase["workspace"];
    let ids = &workspace["queries"]
        .as_array()
        .expect("queries")
        .iter()
        .find(|row| row["query"] == query)
        .expect("authored query")["symbols"];
    json!({"jsonrpc":"2.0","id":id,"result":oracle::expected(&effective,workspace,ids,true,&|file|harness.uri(file))})
}

fn fresh_request(
    harness: &mut Harness,
    fixture: &FixtureWorkspace,
    phase: &Value,
    query: &str,
    id: Value,
) {
    let wanted = expected(harness, fixture, phase, query, id.clone());
    assert_eq!(
        send(
            &mut harness.test_server,
            json!({"jsonrpc":"2.0","id":id,"method":"workspace/symbol","params":{"query":query}})
        ),
        vec![wanted]
    );
}

fn error(id: Value, contract: &Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":contract})
}

#[test]
fn lsp_workspace_symbol_stale_versions_keep_current_whole_sets_and_pending_workers_publishable() {
    let worker = contracts();
    for crlf in [false, true] {
        for shifted in [false, true] {
            for complete_first in [false, true] {
                let spec = oracle::lifecycle(crlf, shifted);
                let phases = spec.oracle["phases"].as_array().expect("phases");
                let phase = &phases[1];
                let mut fixture = states::fresh_fixture(&spec, phase);
                let mut harness = Harness::new(&fixture, &spec, phase);
                check(&mut harness, &fixture, &spec, phase);
                let before = harness.test_server.snapshot();
                let uri = harness.uri("scripts/main.vela");
                let rejected = states::document(&spec, "main-second");
                for version in worker["ignoredVersions"].as_array().expect("versions") {
                    queue(&mut harness, &json!(41), "");
                    let completed = complete_first.then(|| harness.test_server.receive_task());
                    let messages = notify::<n::DidChangeTextDocument>(
                        &mut harness.test_server,
                        json!({"textDocument":{"uri":uri,"version":version},"contentChanges":[{"text":rejected.text}]}),
                    );
                    assert!(messages.is_empty(), "ignored version has no publications");
                    assert_eq!(
                        harness.test_server.snapshot().generation(),
                        before.generation()
                    );
                    let task = completed.unwrap_or_else(|| harness.test_server.receive_task());
                    let (outcome, replies) = harness.test_server.publish_task(task);
                    assert_eq!(outcome, TaskOutcome::Completed);
                    assert_eq!(
                        replies.iter().map(message_value).collect::<Vec<_>>(),
                        vec![expected(&harness, &fixture, phase, "", json!(41))]
                    );
                    check(&mut harness, &fixture, &spec, phase);
                    check_snapshot(&before, &harness, &spec, phase);
                }
                // A strictly newer version changes ownership and ranges normally.
                let next = &phases[2];
                for action in states::actions(&spec, next) {
                    fixture.apply(&action).expect("accepted source edit");
                    harness.apply(&fixture, &action, 2);
                }
                assert!(harness.test_server.snapshot().generation() > before.generation());
                check(&mut harness, &fixture, &spec, next);
                check_snapshot(&before, &harness, &spec, phase);
                let fresh_fixture = states::fresh_fixture(&spec, next);
                let mut fresh = Harness::new(&fresh_fixture, &spec, next);
                check(&mut fresh, &fresh_fixture, &spec, next);
            }
        }
    }
}

#[test]
fn lsp_workspace_symbol_cancellation_discards_queued_and_completed_workers_without_mutation_or_id_poisoning()
 {
    let worker = contracts();
    for crlf in [false, true] {
        for shifted in [false, true] {
            let spec = oracle::lifecycle(crlf, shifted);
            let phase = &spec.oracle["phases"][0];
            let fixture = states::fresh_fixture(&spec, phase);
            let mut harness = Harness::new(&fixture, &spec, phase);
            check(&mut harness, &fixture, &spec, phase);
            for id in worker["ids"].as_array().expect("IDs") {
                for query in worker["queries"].as_array().expect("queries") {
                    for complete_first in [false, true] {
                        let query = query.as_str().expect("query");
                        let before = harness.test_server.snapshot();
                        let before_symbols = before.databases().workspace_symbols("");
                        queue(&mut harness, id, query);
                        let completed = complete_first.then(|| harness.test_server.receive_task());
                        cancel(&mut harness, json!({"id":id}));
                        let task = completed.unwrap_or_else(|| harness.test_server.receive_task());
                        cancel(&mut harness, json!({"id":id}));
                        let (outcome, replies) = harness.test_server.publish_task(task);
                        assert_eq!(outcome, TaskOutcome::Cancelled);
                        assert_eq!(
                            replies.iter().map(message_value).collect::<Vec<_>>(),
                            vec![error(id.clone(), &worker["errors"]["cancelled"])]
                        );
                        assert_eq!(
                            harness.test_server.snapshot().generation(),
                            before.generation()
                        );
                        assert_eq!(
                            harness
                                .test_server
                                .snapshot()
                                .databases()
                                .workspace_symbols(""),
                            before_symbols
                        );
                        check_snapshot(&before, &harness, &spec, phase);
                        fresh_request(&mut harness, &fixture, phase, query, id.clone());
                        cancel(&mut harness, json!({"id":id}));
                        fresh_request(&mut harness, &fixture, phase, query, id.clone());
                    }
                }
            }
            check(&mut harness, &fixture, &spec, phase);
        }
    }
}

#[test]
fn lsp_workspace_symbol_unknown_malformed_and_different_typed_cancel_ids_preserve_pending_results()
{
    let worker = contracts();
    for crlf in [false, true] {
        for shifted in [false, true] {
            let spec = oracle::lifecycle(crlf, shifted);
            let phase = &spec.oracle["phases"][0];
            let fixture = states::fresh_fixture(&spec, phase);
            let mut harness = Harness::new(&fixture, &spec, phase);
            let before = harness.test_server.snapshot();
            for (id, other) in [(json!(20), json!("20")), (json!("20"), json!(20))] {
                queue(&mut harness, &id, "");
                let task = harness.test_server.receive_task();
                cancel(&mut harness, json!({"id":other}));
                cancel(&mut harness, json!({"id":"unknown 中😀"}));
                for params in worker["ignoredCancelParams"]
                    .as_array()
                    .expect("malformed params")
                {
                    cancel(&mut harness, params.clone());
                }
                assert_eq!(
                    harness.test_server.snapshot().generation(),
                    before.generation()
                );
                let (outcome, replies) = harness.test_server.publish_task(task);
                assert_eq!(outcome, TaskOutcome::Completed);
                assert_eq!(
                    replies.iter().map(message_value).collect::<Vec<_>>(),
                    vec![expected(&harness, &fixture, phase, "", id.clone())]
                );
                fresh_request(&mut harness, &fixture, phase, "", other);
                fresh_request(&mut harness, &fixture, phase, "", id);
            }
            check(&mut harness, &fixture, &spec, phase);
            check_snapshot(&before, &harness, &spec, phase);
        }
    }
}

#[test]
fn lsp_workspace_symbol_cancel_priority_and_bounded_retry_cover_all_source_schema_lifecycle_mutations()
 {
    let worker = contracts();
    for crlf in [false, true] {
        for shifted in [false, true] {
            for complete_first in [false, true] {
                let spec = oracle::lifecycle(crlf, shifted);
                let phases = spec.oracle["phases"].as_array().expect("phases");
                let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
                let mut harness = Harness::new(&fixture, &spec, &phases[0]);
                check(&mut harness, &fixture, &spec, &phases[0]);
                let mut frozen = Vec::new();
                for (index, phase) in phases.iter().enumerate().skip(1) {
                    let before = harness.test_server.snapshot();
                    let old_symbols = before.databases().workspace_symbols("");
                    queue(&mut harness, &json!(20), "");
                    queue(&mut harness, &json!("20"), "");
                    let completed = complete_first.then(|| {
                        [
                            harness.test_server.receive_task(),
                            harness.test_server.receive_task(),
                        ]
                    });
                    if index % 2 == 0 {
                        cancel(&mut harness, json!({"id":20}));
                    }
                    for action in states::actions(&spec, phase) {
                        fixture.apply(&action).expect("finite source action");
                        harness.apply(&fixture, &action, index as i32 + 1);
                    }
                    if !phase["schemaAction"].is_null() {
                        harness.schema(&fixture, &spec, &phase["schema"]);
                    }
                    if index % 2 != 0 {
                        cancel(&mut harness, json!({"id":20}));
                    }
                    assert!(
                        harness.test_server.snapshot().generation() > before.generation(),
                        "phase {} invalidates workspace-wide results",
                        phase["id"]
                    );
                    let tasks = completed.unwrap_or_else(|| {
                        [
                            harness.test_server.receive_task(),
                            harness.test_server.receive_task(),
                        ]
                    });
                    let mut replies = Vec::new();
                    let mut outcomes = Vec::new();
                    for task in tasks {
                        let (outcome, messages) = harness.test_server.publish_task(task);
                        outcomes.push(outcome);
                        replies.extend(messages.iter().map(message_value));
                    }
                    assert!(outcomes.contains(&TaskOutcome::Cancelled));
                    assert!(outcomes.contains(&TaskOutcome::Retried));
                    let mut wanted = vec![error(json!(20), &worker["errors"]["cancelled"])];
                    replies.sort_by_key(Value::to_string);
                    wanted.sort_by_key(Value::to_string);
                    assert_eq!(replies, wanted, "phase {}", phase["id"]);
                    let retry = harness.test_server.receive_task();
                    let (outcome, messages) = harness.test_server.publish_task(retry);
                    assert_eq!(outcome, TaskOutcome::Completed);
                    assert_eq!(
                        messages.iter().map(message_value).collect::<Vec<_>>(),
                        vec![expected(&harness, &fixture, phase, "", json!("20"))],
                        "one retry publishes only current whole workspace rows {}",
                        phase["id"]
                    );
                    assert_eq!(before.databases().workspace_symbols(""), old_symbols);
                    check_snapshot(&before, &harness, &spec, &phases[index - 1]);
                    check(&mut harness, &fixture, &spec, phase);
                    fresh_request(&mut harness, &fixture, phase, "", json!(20));
                    fresh_request(&mut harness, &fixture, phase, "", json!("20"));
                    let fresh_fixture = states::fresh_fixture(&spec, phase);
                    let mut fresh = Harness::new(&fresh_fixture, &spec, phase);
                    check(&mut fresh, &fresh_fixture, &spec, phase);
                    for (snapshot, old_phase, old_rows) in &frozen {
                        check_snapshot(snapshot, &harness, &spec, old_phase);
                        assert_eq!(snapshot.databases().workspace_symbols(""), *old_rows);
                    }
                    let snapshot = harness.test_server.snapshot();
                    let rows = snapshot.databases().workspace_symbols("");
                    frozen.push((snapshot, phase.clone(), rows));
                }
            }
        }
    }
}

#[test]
fn lsp_workspace_symbol_retry_exhaustion_returns_one_exact_error_and_cancellation_still_wins() {
    let worker = contracts();
    assert_eq!(worker["retryLimit"], 1);
    for crlf in [false, true] {
        for shifted in [false, true] {
            for complete_first in [false, true] {
                for cancel_retry in [false, true] {
                    for id in [json!(-7), json!("retry 中😀")] {
                        let spec = oracle::lifecycle(crlf, shifted);
                        let phases = spec.oracle["phases"].as_array().expect("phases");
                        let mut fixture = FixtureWorkspace::new(&spec).expect("fixture");
                        let mut harness = Harness::new(&fixture, &spec, &phases[0]);
                        check(&mut harness, &fixture, &spec, &phases[0]);
                        let before = harness.test_server.snapshot();
                        queue(&mut harness, &id, "");
                        let completed = complete_first.then(|| harness.test_server.receive_task());
                        for action in states::actions(&spec, &phases[1]) {
                            fixture.apply(&action).expect("first mutation");
                            harness.apply(&fixture, &action, 1);
                        }
                        assert!(harness.test_server.snapshot().generation() > before.generation());
                        let task = completed.unwrap_or_else(|| harness.test_server.receive_task());
                        let (outcome, replies) = harness.test_server.publish_task(task);
                        assert_eq!(outcome, TaskOutcome::Retried);
                        assert!(replies.is_empty());
                        let retry = harness.test_server.receive_task();
                        let first_generation = harness.test_server.snapshot().generation();
                        if cancel_retry {
                            cancel(&mut harness, json!({"id":id}));
                        }
                        for action in states::actions(&spec, &phases[2]) {
                            fixture.apply(&action).expect("second mutation");
                            harness.apply(&fixture, &action, 2);
                        }
                        assert!(harness.test_server.snapshot().generation() > first_generation);
                        let (outcome, replies) = harness.test_server.publish_task(retry);
                        assert_eq!(
                            outcome,
                            if cancel_retry {
                                TaskOutcome::Cancelled
                            } else {
                                TaskOutcome::StaleDiscarded
                            }
                        );
                        assert_eq!(
                            replies.iter().map(message_value).collect::<Vec<_>>(),
                            vec![error(
                                id.clone(),
                                &worker["errors"][if cancel_retry { "cancelled" } else { "stale" }]
                            )]
                        );
                        check_snapshot(&before, &harness, &spec, &phases[0]);
                        check(&mut harness, &fixture, &spec, &phases[2]);
                        fresh_request(&mut harness, &fixture, &phases[2], "", id);
                        let fresh_fixture = states::fresh_fixture(&spec, &phases[2]);
                        let mut fresh = Harness::new(&fresh_fixture, &spec, &phases[2]);
                        check(&mut fresh, &fresh_fixture, &spec, &phases[2]);
                    }
                }
            }
        }
    }
}
