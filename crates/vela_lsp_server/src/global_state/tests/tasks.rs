#[test]
fn typed_formatting_dispatch_registers_in_flight_cancellation_handle() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    state.project.workspace.open_document(
        document.clone(),
        "pub fn main(){return 1}",
        SourceVersion::new(1),
    );
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);
    let request_id = RequestId::from(30);

    let result = state
        .handle_message(&Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(30),
            method: "textDocument/formatting".to_owned(),
            params: serde_json::to_value(lsp_types::DocumentFormattingParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                options: lsp_formatting_options(),
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            })
            .expect("formatting params should serialize"),
        }))
        .expect("message should dispatch");

    assert_no_messages(result);
    assert!(state.request_queue.in_flight.contains_key(&request_id));
    let task = state
        .task_scheduler()
        .formatting_results()
        .recv_timeout(Duration::from_secs(1))
        .expect("formatting task should complete");
    assert_eq!(task.document_uri(), Some(document.as_str()));
    assert_eq!(task.request_id(), Some(&request_id));
    let generation = task
        .generation_token()
        .expect("formatting task should carry generation token");
    assert_eq!(
        generation.generation(),
        state.project.databases.generation()
    );
    assert!(!generation.is_cancelled());

    let task_summary = state
        .send_task_result(task)
        .expect("formatting task response should send");
    assert_eq!(task_summary.outcome(), TaskOutcome::Completed);
    assert!(!state.request_queue.in_flight.contains_key(&request_id));
}

#[test]
fn send_task_result_returns_content_modified_for_stale_non_retryable_response() {
    let (sender, receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    state.project.workspace.open_document(
        document.clone(),
        "pub fn main(){return 1}",
        SourceVersion::new(1),
    );
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);
    let request_id = RequestId::from(31);

    let result = state
        .handle_message(&Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(31),
            method: "textDocument/formatting".to_owned(),
            params: serde_json::to_value(lsp_types::DocumentFormattingParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                options: lsp_formatting_options(),
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            })
            .expect("formatting params should serialize"),
        }))
        .expect("message should dispatch");

    assert_no_messages(result);
    assert!(state.request_queue.in_flight.contains_key(&request_id));
    let task = state
        .task_scheduler()
        .formatting_results()
        .recv_timeout(Duration::from_secs(1))
        .expect("formatting task should complete");
    state.project.databases.invalidate_project_config();

    let task_summary = state
        .send_task_result(task)
        .expect("stale formatting task response should be handled");
    assert_eq!(task_summary.outcome(), TaskOutcome::StaleDiscarded);

    assert!(!state.request_queue.in_flight.contains_key(&request_id));
    let response = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("stale formatting should send ContentModified");
    let Message::Response(response) = response else {
        panic!("stale formatting should send response");
    };
    assert_eq!(response.id, lsp_server::RequestId::from(31));
    let error = response
        .error
        .expect("stale formatting should return an error");
    assert_eq!(error.code, -32801);
    assert_eq!(
        error.message,
        "request result is stale because the document was modified"
    );
}

#[test]
fn send_task_result_returns_request_cancelled_for_cancelled_in_flight_response() {
    let (sender, receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    state.project.workspace.open_document(
        document.clone(),
        "pub fn main(){return 1}",
        SourceVersion::new(1),
    );
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);
    let request_id = RequestId::from(33);

    let result = state
        .handle_message(&Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(33),
            method: "textDocument/formatting".to_owned(),
            params: serde_json::to_value(lsp_types::DocumentFormattingParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                options: lsp_formatting_options(),
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            })
            .expect("formatting params should serialize"),
        }))
        .expect("message should dispatch");

    assert_no_messages(result);
    assert!(state.request_queue.in_flight.contains_key(&request_id));
    let task = state
        .task_scheduler()
        .formatting_results()
        .recv_timeout(Duration::from_secs(1))
        .expect("formatting task should complete");
    let generation = task
        .generation_token()
        .expect("formatting task should carry generation token")
        .clone();
    assert!(!generation.is_cancelled());

    assert_no_messages(state.cancel_request(lsp_types::CancelParams {
        id: lsp_types::NumberOrString::Number(33),
    }));
    assert!(generation.is_cancelled());

    let task_summary = state
        .send_task_result(task)
        .expect("cancelled formatting task response should be handled");
    assert_eq!(task_summary.outcome(), TaskOutcome::Cancelled);

    assert!(!state.request_queue.in_flight.contains_key(&request_id));
    let response = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("cancelled formatting should send RequestCancelled");
    let Message::Response(response) = response else {
        panic!("cancelled formatting should send response");
    };
    assert_eq!(response.id, lsp_server::RequestId::from(33));
    let error = response
        .error
        .expect("cancelled formatting should return an error");
    assert_eq!(error.code, -32800);
    assert_eq!(error.message, "request was cancelled before processing");
}

#[test]
fn send_task_result_retries_stale_retryable_completion_once() {
    let (sender, receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    state.project.workspace.open_document(
        document.clone(),
        "pub fn old_only() { return 1 }",
        SourceVersion::new(1),
    );
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);
    let request_id = RequestId::from(32);

    let result = state
        .handle_message(&Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(32),
            method: "textDocument/completion".to_owned(),
            params: serde_json::to_value(lsp_types::CompletionParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: lsp_types::Url::parse(document.as_str())
                            .expect("document URI should parse"),
                    },
                    position: lsp_types::Position {
                        line: 0,
                        character: 7,
                    },
                },
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
                context: None,
            })
            .expect("completion params should serialize"),
        }))
        .expect("message should dispatch");

    assert_no_messages(result);
    assert!(state.request_queue.in_flight.contains_key(&request_id));
    let stale_task = state
        .task_scheduler()
        .latency_results()
        .recv_timeout(Duration::from_secs(1))
        .expect("completion task should complete");
    assert_eq!(stale_task.request_id(), Some(&request_id));
    assert!(stale_task.retry().is_some());

    state.project.workspace.open_document(
        document.clone(),
        "pub fn new_only() { return 2 }",
        SourceVersion::new(2),
    );
    state.project.refresh_document_databases(&document);

    let stale_summary = state
        .send_task_result(stale_task)
        .expect("stale completion task should schedule retry");
    assert_eq!(stale_summary.outcome(), TaskOutcome::Retried);

    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
    assert!(state.request_queue.in_flight.contains_key(&request_id));
    let retry_task = state
        .task_scheduler()
        .latency_results()
        .recv_timeout(Duration::from_secs(1))
        .expect("retry completion task should complete");
    assert_eq!(retry_task.request_id(), Some(&request_id));

    let retry_summary = state
        .send_task_result(retry_task)
        .expect("fresh retry response should send");
    assert_eq!(retry_summary.outcome(), TaskOutcome::Completed);

    assert!(!state.request_queue.in_flight.contains_key(&request_id));
    let response = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("retry should send completion response");
    let Message::Response(response) = response else {
        panic!("retry should send response");
    };
    assert!(response.error.is_none(), "{response:?}");
    let result = response
        .result
        .expect("completion retry should contain result");
    let labels: Vec<_> = result["items"]
        .as_array()
        .expect("completion response should contain items")
        .iter()
        .filter_map(|item| item["label"].as_str())
        .collect();
    assert!(labels.contains(&"new_only"), "{labels:?}");
    assert!(!labels.contains(&"old_only"), "{labels:?}");
}
use super::*;
