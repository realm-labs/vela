use std::time::Duration;

use crossbeam_channel::{Receiver, TryRecvError, unbounded};
use lsp_server::Response;
use vela_language_service::{
    DocumentId, SchemaConfig, SourceVersion, WorkspaceConfig, WorkspaceRoot,
};

use crate::task::{TaskLane, TaskOutcome};

use super::*;

mod edits;
mod queries;
mod state;
mod tasks;
fn typed_navigation_response(
    state: &mut GlobalState,
    id: i32,
    method: &str,
    document: &DocumentId,
    line: u32,
    character: usize,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: method.to_owned(),
        params: serde_json::to_value(lsp_types::GotoDefinitionParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                position: lsp_types::Position::new(
                    line,
                    u32::try_from(character).expect("position should fit in u32"),
                ),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        })
        .expect("goto params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = response_message(result, "typed navigation should return a response");
    response_json(response)
}

fn typed_prepare_call_hierarchy_response(
    state: &mut GlobalState,
    id: i32,
    document: &DocumentId,
    line: u32,
    character: usize,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/prepareCallHierarchy".to_owned(),
        params: serde_json::to_value(lsp_types::CallHierarchyPrepareParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                position: lsp_types::Position::new(
                    line,
                    u32::try_from(character).expect("position should fit in u32"),
                ),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        })
        .expect("prepareCallHierarchy params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = response_message(
        result,
        "typed prepareCallHierarchy should return a response",
    );
    response_json(response)
}

fn typed_incoming_calls_response(
    state: &mut GlobalState,
    id: i32,
    item: lsp_types::CallHierarchyItem,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "callHierarchy/incomingCalls".to_owned(),
        params: serde_json::to_value(lsp_types::CallHierarchyIncomingCallsParams {
            item,
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        })
        .expect("incomingCalls params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = response_message(result, "typed incomingCalls should return a response");
    response_json(response)
}

fn typed_outgoing_calls_response(
    state: &mut GlobalState,
    id: i32,
    item: lsp_types::CallHierarchyItem,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "callHierarchy/outgoingCalls".to_owned(),
        params: serde_json::to_value(lsp_types::CallHierarchyOutgoingCallsParams {
            item,
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        })
        .expect("outgoingCalls params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = response_message(result, "typed outgoingCalls should return a response");
    response_json(response)
}

fn typed_rename_response(
    state: &mut GlobalState,
    id: i32,
    document: &DocumentId,
    line: u32,
    character: usize,
    new_name: &str,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/rename".to_owned(),
        params: serde_json::to_value(lsp_types::RenameParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                position: lsp_types::Position::new(
                    line,
                    u32::try_from(character).expect("position should fit in u32"),
                ),
            },
            new_name: new_name.to_owned(),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        })
        .expect("rename params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = response_message(result, "typed rename should return a response");
    response_json(response)
}

fn typed_prepare_rename_response(
    state: &mut GlobalState,
    id: i32,
    document: &DocumentId,
    line: u32,
    character: usize,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/prepareRename".to_owned(),
        params: serde_json::to_value(lsp_types::TextDocumentPositionParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse(document.as_str()).expect("document URI should parse"),
            },
            position: lsp_types::Position::new(
                line,
                u32::try_from(character).expect("position should fit in u32"),
            ),
        })
        .expect("prepareRename params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = response_message(result, "typed prepareRename should return a response");
    response_json(response)
}

fn typed_references_response(
    state: &mut GlobalState,
    id: i32,
    document: &DocumentId,
    line: u32,
    character: usize,
    include_declaration: bool,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/references".to_owned(),
        params: serde_json::to_value(lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                position: lsp_types::Position::new(
                    line,
                    u32::try_from(character).expect("position should fit in u32"),
                ),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
            context: lsp_types::ReferenceContext {
                include_declaration,
            },
        })
        .expect("reference params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = response_message(result, "typed references should return a response");
    response_json(response)
}

fn typed_document_highlight_response(
    state: &mut GlobalState,
    id: i32,
    document: &DocumentId,
    line: u32,
    character: usize,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/documentHighlight".to_owned(),
        params: serde_json::to_value(lsp_types::DocumentHighlightParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                position: lsp_types::Position::new(
                    line,
                    u32::try_from(character).expect("position should fit in u32"),
                ),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        })
        .expect("documentHighlight params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = response_message(result, "typed documentHighlight should return a response");
    response_json(response)
}

fn typed_scheduled_response(
    state: &mut GlobalState,
    receiver: &Receiver<Message>,
    request: Message,
    lane: TaskLane,
    label: &str,
) -> serde_json::Value {
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    assert_no_messages(result);
    let task = match lane {
        TaskLane::Latency => state.task_scheduler().latency_results(),
        TaskLane::Worker => state.task_scheduler().worker_results(),
        TaskLane::Formatting => state.task_scheduler().formatting_results(),
        TaskLane::Main => unreachable!("main-thread requests are not scheduled"),
    }
    .recv_timeout(Duration::from_secs(1))
    .unwrap_or_else(|_| panic!("{label} task should complete"));
    assert!(task.retry().is_some());
    let task_summary = state
        .send_task_result(task)
        .unwrap_or_else(|error| panic!("{label} task response should send: {error}"));
    assert_eq!(task_summary.outcome(), TaskOutcome::Completed);
    let response = receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap_or_else(|_| panic!("{label} should send a response"));
    let Message::Response(response) = response else {
        panic!("{label} should send response");
    };
    assert!(response.error.is_none(), "{response:?}");
    serde_json::to_value(response).expect("response should serialize")
}

fn typed_document_symbol_response(
    state: &mut GlobalState,
    receiver: &Receiver<Message>,
    id: i32,
    document: &DocumentId,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/documentSymbol".to_owned(),
        params: serde_json::to_value(lsp_types::DocumentSymbolParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse(document.as_str()).expect("document URI should parse"),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        })
        .expect("documentSymbol params should serialize"),
    });
    typed_scheduled_response(
        state,
        receiver,
        request,
        TaskLane::Worker,
        "typed documentSymbol",
    )
}

fn typed_workspace_symbol_response(
    state: &mut GlobalState,
    receiver: &Receiver<Message>,
    id: i32,
    query: &str,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "workspace/symbol".to_owned(),
        params: serde_json::to_value(lsp_types::WorkspaceSymbolParams {
            query: query.to_owned(),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        })
        .expect("workspaceSymbol params should serialize"),
    });
    typed_scheduled_response(
        state,
        receiver,
        request,
        TaskLane::Worker,
        "typed workspaceSymbol",
    )
}

fn typed_folding_range_response(
    state: &mut GlobalState,
    receiver: &Receiver<Message>,
    id: i32,
    document: &DocumentId,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/foldingRange".to_owned(),
        params: serde_json::to_value(lsp_types::FoldingRangeParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse(document.as_str()).expect("document URI should parse"),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        })
        .expect("foldingRange params should serialize"),
    });
    typed_scheduled_response(
        state,
        receiver,
        request,
        TaskLane::Worker,
        "typed foldingRange",
    )
}

fn typed_selection_range_response(
    state: &mut GlobalState,
    id: i32,
    document: &DocumentId,
    line: u32,
    character: u32,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/selectionRange".to_owned(),
        params: serde_json::to_value(lsp_types::SelectionRangeParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse(document.as_str()).expect("document URI should parse"),
            },
            positions: vec![lsp_types::Position::new(line, character)],
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        })
        .expect("selectionRange params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = response_message(result, "typed selectionRange should return a response");
    response_json(response)
}

fn typed_semantic_tokens_full_response(
    state: &mut GlobalState,
    receiver: &Receiver<Message>,
    id: i32,
    document: &DocumentId,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/semanticTokens/full".to_owned(),
        params: serde_json::to_value(lsp_types::SemanticTokensParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse(document.as_str()).expect("document URI should parse"),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        })
        .expect("semanticTokens/full params should serialize"),
    });
    typed_scheduled_response(
        state,
        receiver,
        request,
        TaskLane::Latency,
        "typed semanticTokens/full",
    )
}

fn typed_semantic_tokens_delta_response(
    state: &mut GlobalState,
    id: i32,
    document: &DocumentId,
    previous_result_id: &str,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/semanticTokens/full/delta".to_owned(),
        params: serde_json::to_value(lsp_types::SemanticTokensDeltaParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse(document.as_str()).expect("document URI should parse"),
            },
            previous_result_id: previous_result_id.to_owned(),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        })
        .expect("semanticTokens/full/delta params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = response_message(
        result,
        "typed semanticTokens/full/delta should return a response",
    );
    response_json(response)
}

fn typed_semantic_tokens_range_response(
    state: &mut GlobalState,
    id: i32,
    document: &DocumentId,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/semanticTokens/range".to_owned(),
        params: serde_json::to_value(lsp_types::SemanticTokensRangeParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse(document.as_str()).expect("document URI should parse"),
            },
            range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 42),
            ),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        })
        .expect("semanticTokens/range params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = response_message(
        result,
        "typed semanticTokens/range should return a response",
    );
    response_json(response)
}

fn typed_code_action_response(
    state: &mut GlobalState,
    id: i32,
    document: &DocumentId,
    start_character: u32,
    end_character: u32,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/codeAction".to_owned(),
        params: serde_json::to_value(lsp_types::CodeActionParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse(document.as_str()).expect("document URI should parse"),
            },
            range: lsp_types::Range::new(
                lsp_types::Position::new(0, start_character),
                lsp_types::Position::new(0, end_character),
            ),
            context: lsp_types::CodeActionContext::default(),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        })
        .expect("codeAction params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = response_message(result, "typed codeAction should return a response");
    response_json(response)
}

fn typed_inlay_hint_response(
    state: &mut GlobalState,
    id: i32,
    document: &DocumentId,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/inlayHint".to_owned(),
        params: serde_json::to_value(lsp_types::InlayHintParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse(document.as_str()).expect("document URI should parse"),
            },
            range: lsp_types::Range::new(
                lsp_types::Position::new(1, 0),
                lsp_types::Position::new(1, 80),
            ),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        })
        .expect("inlayHint params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = response_message(result, "typed inlayHint should return a response");
    response_json(response)
}

fn json_selection_chain(range: &serde_json::Value) -> Vec<&serde_json::Value> {
    let mut ranges = Vec::new();
    let mut current = Some(range);
    while let Some(selection) = current {
        ranges.push(&selection["range"]);
        current = selection.get("parent");
    }
    ranges
}

fn typed_formatting_response(
    state: &mut GlobalState,
    id: i32,
    document: &DocumentId,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/formatting".to_owned(),
        params: serde_json::to_value(lsp_types::DocumentFormattingParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse(document.as_str()).expect("document URI should parse"),
            },
            options: lsp_formatting_options(),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        })
        .expect("formatting params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response =
        formatting_task_response(state, result, "typed formatting should return a response");
    response_string_json(&response)
}

fn typed_range_formatting_response(
    state: &mut GlobalState,
    id: i32,
    document: &DocumentId,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/rangeFormatting".to_owned(),
        params: serde_json::to_value(lsp_types::DocumentRangeFormattingParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse(document.as_str()).expect("document URI should parse"),
            },
            range: lsp_types::Range::new(
                lsp_types::Position::new(1, 0),
                lsp_types::Position::new(2, 0),
            ),
            options: lsp_formatting_options(),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        })
        .expect("rangeFormatting params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = formatting_task_response(
        state,
        result,
        "typed rangeFormatting should return a response",
    );
    response_string_json(&response)
}

fn typed_on_type_formatting_response(
    state: &mut GlobalState,
    id: i32,
    document: &DocumentId,
) -> serde_json::Value {
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(id),
        method: "textDocument/onTypeFormatting".to_owned(),
        params: serde_json::to_value(lsp_types::DocumentOnTypeFormattingParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                position: lsp_types::Position::new(2, 1),
            },
            ch: "}".to_owned(),
            options: lsp_formatting_options(),
        })
        .expect("onTypeFormatting params should serialize"),
    });
    let result = state
        .handle_message(&request)
        .expect("message should dispatch");
    let response = formatting_task_response(
        state,
        result,
        "typed onTypeFormatting should return a response",
    );
    response_string_json(&response)
}

fn formatting_task_response(
    state: &mut GlobalState,
    result: Vec<Message>,
    expected: &str,
) -> String {
    if let Some(response) = response_message_opt(result, expected) {
        return crate::rpc::serialize_message(&Message::Response(response));
    }
    let task = state
        .task_scheduler()
        .formatting_results()
        .recv_timeout(Duration::from_secs(1))
        .expect(expected);
    assert_eq!(task.lane(), TaskLane::Formatting);
    let messages = task.into_messages();
    let [message] = messages.as_slice() else {
        panic!("{expected}");
    };
    crate::rpc::serialize_message(message)
}

fn response_message(messages: Vec<Message>, expected: &str) -> Response {
    response_message_opt(messages, expected).expect(expected)
}

fn response_message_opt(messages: Vec<Message>, expected: &str) -> Option<Response> {
    match messages.as_slice() {
        [] => None,
        [Message::Response(response)] => Some(response.clone()),
        _ => panic!("{expected}: {messages:?}"),
    }
}

fn response_json(response: Response) -> serde_json::Value {
    serde_json::from_str(&crate::rpc::serialize_message(&Message::Response(response)))
        .expect("response should be JSON")
}

fn response_string_json(response: &str) -> serde_json::Value {
    serde_json::from_str(response).expect("response should be JSON")
}

fn assert_no_messages(messages: Vec<Message>) {
    assert!(
        messages.is_empty(),
        "expected no messages, got {messages:?}"
    );
}

fn assert_has_messages(messages: &[Message]) {
    assert!(!messages.is_empty(), "expected at least one message");
}

fn single_message_value(messages: Vec<Message>) -> serde_json::Value {
    let [message] = messages.as_slice() else {
        panic!("expected exactly one message, got {messages:?}");
    };
    serde_json::from_str(&crate::rpc::serialize_message(message))
        .expect("message should serialize as JSON")
}

fn lsp_formatting_options() -> lsp_types::FormattingOptions {
    lsp_types::FormattingOptions {
        tab_size: 4,
        insert_spaces: true,
        properties: Default::default(),
        trim_trailing_whitespace: None,
        insert_final_newline: None,
        trim_final_newlines: None,
    }
}

fn workspace_config_with_schema(root: &str, schema: &str) -> WorkspaceConfig {
    let mut config = WorkspaceConfig::workspace([WorkspaceRoot::from(root)]);
    config.set_schema(SchemaConfig::from_path(schema));
    config
}
