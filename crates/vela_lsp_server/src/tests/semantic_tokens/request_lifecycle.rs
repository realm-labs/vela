use lsp_types::notification as n;
use serde_json::{Value, json};

use super::incremental_support::Driver;
use crate::task::TaskOutcome;
use crate::tests::{notify, response_value};

const METHODS: [&str; 3] = [
    "textDocument/semanticTokens/full",
    "textDocument/semanticTokens/full/delta",
    "textDocument/semanticTokens/range",
];

fn successful(messages: Vec<lsp_server::Message>, id: i32) -> Value {
    let response = response_value(messages);
    assert_eq!(response["id"], id);
    assert!(response.get("error").is_none(), "{response}");
    response["result"].clone()
}

fn error(messages: Vec<lsp_server::Message>, id: i32, code: i32) {
    let response = response_value(messages);
    assert_eq!(response["id"], id);
    assert_eq!(response["error"]["code"], code);
    assert!(
        response.get("result").is_none(),
        "stale/cancelled tokens cannot be published"
    );
}

#[test]
fn cancelled_token_requests_discard_results_and_allow_later_queries() {
    for crlf in [false, true] {
        for method in METHODS {
            for collect_first in [false, true] {
                let mut driver = Driver::new(crlf);
                let uri = driver.uri("scripts/main.vela");
                let previous = super::support::full(&mut driver.live, &uri);
                let params = driver.params(method, &previous);
                driver.live.queue_request(80, method, params.clone());
                let held = collect_first.then(|| driver.live.receive_task());
                assert!(notify::<n::Cancel>(&mut driver.live, json!({"id":80})).is_empty());
                driver.change(0);
                let task = held.unwrap_or_else(|| driver.live.receive_task());
                let (outcome, messages) = driver.live.publish_task(task);
                assert_eq!(outcome, TaskOutcome::Cancelled);
                error(messages, 80, -32800);
                assert!(notify::<n::Cancel>(&mut driver.live, json!({"id":80})).is_empty());
                assert!(notify::<n::Cancel>(&mut driver.live, json!({"id":81})).is_empty());
                driver.live.queue_request(81, method, params);
                let task = driver.live.receive_task();
                let (outcome, messages) = driver.live.publish_task(task);
                assert_eq!(outcome, TaskOutcome::Completed);
                let result = successful(messages, 81);
                let queries = driver.spec.oracle["steps"][0]["queries"].clone();
                driver.assert_current_response(method, &previous, &result, &queries);
            }
        }
    }
}

#[test]
fn stale_token_requests_discard_deltas_ranges_and_bound_full_retry() {
    for crlf in [false, true] {
        for method in METHODS {
            for invalidate_retry in [false, true] {
                let mut driver = Driver::new(crlf);
                let uri = driver.uri("scripts/main.vela");
                let previous = super::support::full(&mut driver.live, &uri);
                let params = driver.params(method, &previous);
                driver.live.queue_request(90, method, params.clone());
                let held = driver.live.receive_task();
                driver.change(0);
                let (outcome, messages) = driver.live.publish_task(held);
                if method == "textDocument/semanticTokens/full" {
                    assert_eq!(outcome, TaskOutcome::Retried);
                    assert!(messages.is_empty(), "old facts cannot escape before retry");
                    let retry = driver.live.receive_task();
                    if invalidate_retry {
                        driver.change(1);
                    }
                    let (outcome, messages) = driver.live.publish_task(retry);
                    if invalidate_retry {
                        assert_eq!(outcome, TaskOutcome::StaleDiscarded);
                        error(messages, 90, -32801);
                    } else {
                        assert_eq!(outcome, TaskOutcome::Completed);
                        let result = successful(messages, 90);
                        let queries = driver.spec.oracle["steps"][0]["queries"].clone();
                        driver.assert_current_response(method, &previous, &result, &queries);
                    }
                } else {
                    assert_eq!(outcome, TaskOutcome::StaleDiscarded);
                    error(messages, 90, -32801);
                    if invalidate_retry {
                        driver.change(1);
                    }
                }
                driver.live.queue_request(91, method, params);
                let task = driver.live.receive_task();
                let (outcome, messages) = driver.live.publish_task(task);
                assert_eq!(outcome, TaskOutcome::Completed);
                let result = successful(messages, 91);
                let queries =
                    driver.spec.oracle["steps"][usize::from(invalidate_retry)]["queries"].clone();
                driver.assert_current_response(method, &previous, &result, &queries);
            }
        }
    }
}
