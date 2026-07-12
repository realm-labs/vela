#[test]
fn typed_completion_resolve_dispatch_projects_completion_item() {
    let (sender, receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(7),
        method: "completionItem/resolve".to_owned(),
        params: serde_json::to_value(lsp_types::CompletionItem {
            label: "plain".to_owned(),
            kind: Some(lsp_types::CompletionItemKind::VARIABLE),
            data: Some(serde_json::json!({ "source": "vela" })),
            ..lsp_types::CompletionItem::default()
        })
        .expect("completion item should serialize"),
    });

    let result = state
        .handle_message(&request)
        .expect("message should dispatch");

    assert_no_messages(result);
    let task = state
        .task_scheduler()
        .latency_results()
        .recv_timeout(Duration::from_secs(1))
        .expect("completion resolve task should complete");
    assert!(task.retry().is_some());
    let task_summary = state
        .send_task_result(task)
        .expect("completion resolve task response should send");
    assert_eq!(task_summary.outcome(), TaskOutcome::Completed);
    let response = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("completion resolve should send response");
    let Message::Response(response) = response else {
        panic!("completion resolve should send response");
    };
    assert!(response.error.is_none(), "{response:?}");
    let response = serde_json::to_value(response).expect("response should serialize");
    assert_eq!(response["id"], 7);
    assert_eq!(response["result"]["label"], "plain");
    assert_eq!(response["result"]["kind"], 6);
    assert!(response["result"].get("documentation").is_none());
}

#[test]
fn typed_hover_dispatch_projects_hover_response() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "pub fn main(amount: i64) -> i64 { return amount }";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(8),
        method: "textDocument/hover".to_owned(),
        params: serde_json::to_value(lsp_types::HoverParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                position: lsp_types::Position::new(
                    0,
                    u32::try_from(
                        text.rfind("amount")
                            .expect("hover fixture should contain amount use"),
                    )
                    .expect("position should fit in u32"),
                ),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        })
        .expect("hover params should serialize"),
    });

    let result = state
        .handle_message(&request)
        .expect("message should dispatch");

    let response = response_message(result, "typed hover should return a response");
    let response: serde_json::Value = response_json(response);
    assert_eq!(response["id"], 8);
    assert_eq!(response["result"]["contents"]["kind"], "markdown");
    let value = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be markdown");
    assert!(value.contains("amount"), "{value}");
    assert!(value.contains("_parameter_: i64"), "{value}");
}

#[test]
fn typed_signature_help_dispatch_projects_signature_response() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "pub fn grant(amount: i64, bonus: i64) -> bool { return true } pub fn main() { grant(1, 2) }";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);
    let request = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from(9),
        method: "textDocument/signatureHelp".to_owned(),
        params: serde_json::to_value(lsp_types::SignatureHelpParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Url::parse(document.as_str())
                        .expect("document URI should parse"),
                },
                position: lsp_types::Position::new(
                    0,
                    u32::try_from(
                        text.find("2)")
                            .expect("signature fixture should contain second argument"),
                    )
                    .expect("position should fit in u32"),
                ),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            context: None,
        })
        .expect("signatureHelp params should serialize"),
    });

    let result = state
        .handle_message(&request)
        .expect("message should dispatch");

    let response = response_message(result, "typed signatureHelp should return a response");
    let response: serde_json::Value = response_json(response);
    assert_eq!(response["id"], 9);
    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 1);
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "grant(amount: i64, bonus: i64) -> bool"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["label"],
        "bonus: i64"
    );
}

#[test]
fn typed_navigation_dispatch_projects_location_responses() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "\
struct Inventory {
    slots: i64,
}

struct Player {
    inventory: Inventory,
}

fn grant() -> i64 { return 1 }
fn main(player: Player) { grant(); return player.inventory }";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);

    for (id, method) in [
        (10, "textDocument/definition"),
        (11, "textDocument/declaration"),
    ] {
        let response = typed_navigation_response(
            &mut state,
            id,
            method,
            &document,
            9,
            text.lines()
                .nth(9)
                .expect("main line should exist")
                .find("grant")
                .expect("call should contain grant"),
        );
        assert_eq!(response["result"]["uri"], document.as_str());
        assert_eq!(response["result"]["range"]["start"]["line"], 8);
        assert_eq!(response["result"]["range"]["start"]["character"], 3);
        assert_eq!(response["result"]["range"]["end"]["character"], 8);
    }

    let response = typed_navigation_response(
        &mut state,
        12,
        "textDocument/typeDefinition",
        &document,
        9,
        text.lines()
            .nth(9)
            .expect("main line should exist")
            .rfind("inventory")
            .expect("field use should contain inventory"),
    );
    assert_eq!(response["result"]["uri"], document.as_str());
    assert_eq!(response["result"]["range"]["start"]["line"], 0);
    assert_eq!(response["result"]["range"]["start"]["character"], 7);
    assert_eq!(response["result"]["range"]["end"]["character"], 16);
}

#[test]
fn typed_references_dispatch_projects_location_array() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next + amount
}";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);
    let line = text.lines().nth(2).expect("return line should exist");
    let character = line
        .find("amount")
        .expect("return line should contain amount");

    let response = typed_references_response(&mut state, 13, &document, 2, character, true);
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");
    assert_eq!(references.len(), 3, "{references:?}");
    assert_eq!(references[0]["uri"], document.as_str());
    assert_eq!(references[0]["range"]["start"]["line"], 0);
    assert_eq!(references[0]["range"]["start"]["character"], 12);
    assert_eq!(references[2]["range"]["start"]["line"], 2);
    assert_eq!(references[2]["range"]["start"]["character"], 18);

    let response = typed_references_response(&mut state, 14, &document, 2, character, false);
    let references = response["result"]
        .as_array()
        .expect("references response should be an array");
    assert_eq!(references.len(), 2, "{references:?}");
    assert!(
        references
            .iter()
            .all(|reference| reference["range"]["start"]["line"] != 0)
    );
}

#[test]
fn typed_document_highlight_dispatch_projects_highlights() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next + amount
}";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);
    let line = text.lines().nth(2).expect("return line should exist");
    let character = line
        .find("amount")
        .expect("return line should contain amount");

    let response = typed_document_highlight_response(&mut state, 15, &document, 2, character);
    let highlights = response["result"]
        .as_array()
        .expect("documentHighlight response should be an array");

    assert_eq!(highlights.len(), 3, "{highlights:?}");
    assert_eq!(highlights[0]["kind"], 1);
    assert_eq!(highlights[1]["kind"], 2);
    assert_eq!(highlights[0]["range"]["start"]["line"], 0);
    assert_eq!(highlights[0]["range"]["start"]["character"], 12);
    assert_eq!(highlights[2]["range"]["start"]["line"], 2);
    assert_eq!(highlights[2]["range"]["start"]["character"], 18);
}

#[test]
fn typed_document_symbol_dispatch_projects_nested_symbols() {
    let (sender, receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "\
struct Player {
    level: i64,
}

pub fn main(player: Player) -> i64 {
    return player.level
}";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);

    let response = typed_document_symbol_response(&mut state, &receiver, 16, &document);
    let symbols = response["result"]
        .as_array()
        .expect("documentSymbol response should be an array");

    let player = symbols
        .iter()
        .find(|symbol| symbol["name"] == "Player")
        .expect("Player symbol should project");
    assert_eq!(player["kind"], 23);
    assert!(
        player["children"]
            .as_array()
            .expect("Player should include field children")
            .iter()
            .any(|child| child["name"] == "level" && child["kind"] == 8)
    );
    assert!(
        symbols
            .iter()
            .any(|symbol| symbol["name"] == "main" && symbol["kind"] == 12)
    );
}

#[test]
fn typed_workspace_symbol_dispatch_projects_symbols() {
    let (sender, receiver) = unbounded();
    let mut launch_configuration = LaunchConfiguration::new();
    launch_configuration.add_workspace_root("/workspace/scripts");
    let mut state = GlobalState::new(sender, launch_configuration);
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/game/reward.vela");
    let text = "pub fn grant() -> i64 { return 1 }";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);

    let response = typed_workspace_symbol_response(&mut state, &receiver, 17, "reward.vela");
    let symbols = response["result"]
        .as_array()
        .expect("workspaceSymbol response should be an array");
    let reward = symbols
        .iter()
        .find(|symbol| symbol["name"] == "reward.vela")
        .expect("file symbol should project");

    assert_eq!(reward["kind"], 1);
    assert_eq!(reward["data"]["detail"], "game::reward");
    assert_eq!(reward["location"]["uri"], document.as_str());
}

#[test]
fn typed_folding_range_dispatch_projects_ranges() {
    let (sender, receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "\
use game::reward
use game::player

pub fn main() {
    if true {
        return
    }
}";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);

    let response = typed_folding_range_response(&mut state, &receiver, 18, &document);
    let ranges = response["result"]
        .as_array()
        .expect("foldingRange response should be an array");

    assert!(ranges.iter().any(|range| {
        range["kind"] == "imports" && range["startLine"] == 0 && range["endLine"] == 1
    }));
    assert!(ranges.iter().any(|range| {
        range["kind"] == "region" && range["startLine"] == 3 && range["endLine"] == 7
    }));
}

#[test]
fn typed_selection_range_dispatch_projects_parent_chain() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "\
pub fn main(player: Player) -> i64 {
    let next = player.level + 1
    return next
}";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);

    let response = typed_selection_range_response(&mut state, 19, &document, 1, 22);
    let ranges = response["result"]
        .as_array()
        .expect("selectionRange response should be an array");
    assert_eq!(ranges.len(), 1);
    let chain = json_selection_chain(&ranges[0]);

    assert!(chain.iter().any(|range| {
        range["start"]["line"] == 1
            && range["start"]["character"] == 22
            && range["end"]["character"] == 27
    }));
    assert!(chain.iter().any(|range| {
        range["start"]["line"] == 1
            && range["start"]["character"] == 15
            && range["end"]["character"] == 27
    }));
}

#[test]
fn typed_semantic_token_dispatch_projects_full_delta_and_range() {
    let (sender, receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "pub fn main() { let value = 1 return value }";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);

    let full_response = typed_semantic_tokens_full_response(&mut state, &receiver, 20, &document);
    let full_data = full_response["result"]["data"]
        .as_array()
        .expect("semanticTokens/full response should include data");
    assert!(!full_data.is_empty());
    let result_id = full_response["result"]["resultId"]
        .as_str()
        .expect("semanticTokens/full response should include resultId")
        .to_owned();

    let delta_response =
        typed_semantic_tokens_delta_response(&mut state, 21, &document, &result_id);
    assert_eq!(delta_response["result"]["edits"], serde_json::json!([]));

    let range_response = typed_semantic_tokens_range_response(&mut state, 22, &document);
    let range_data = range_response["result"]["data"]
        .as_array()
        .expect("semanticTokens/range response should include data");
    assert!(!range_data.is_empty());
}

#[test]
fn typed_code_action_dispatch_projects_quickfix_edits() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "pub fn main(scores: Array<i64>) { return scores.frist() }";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);
    let typo_start = text.find("frist").expect("fixture should contain typo");

    let response = typed_code_action_response(
        &mut state,
        23,
        &document,
        u32::try_from(typo_start).expect("position should fit in u32"),
        u32::try_from(typo_start + "frist".len()).expect("position should fit in u32"),
    );
    let actions = response["result"]
        .as_array()
        .expect("codeAction response should be an array");
    let action = actions
        .iter()
        .find(|action| action["title"] == "Replace with `first`")
        .expect("quickfix should project");

    assert_eq!(action["kind"], "quickfix");
    assert_eq!(
        action["edit"]["changes"][document.as_str()][0]["newText"],
        "first"
    );
}

#[test]
fn typed_inlay_hint_dispatch_projects_parameter_hints() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "pub fn grant(amount: i64, reason: String) -> i64 { return amount }\npub fn main() { return grant(10, \"quest\") }";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);

    let response = typed_inlay_hint_response(&mut state, 24, &document);
    let hints = response["result"]
        .as_array()
        .expect("inlayHint response should be an array");

    assert_eq!(hints.len(), 2);
    assert_eq!(hints[0]["label"], "amount:");
    assert_eq!(hints[0]["kind"], 2);
    assert_eq!(hints[0]["paddingRight"], true);
    assert_eq!(hints[1]["label"], "reason:");
}

#[test]
fn typed_formatting_dispatch_projects_text_edits() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "pub fn main() {   \n    return 1   \n}\n";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);

    let document_response = typed_formatting_response(&mut state, 20, &document);
    let document_edits = document_response["result"]
        .as_array()
        .expect("formatting response should be an array");
    assert_eq!(document_edits.len(), 1);
    assert_eq!(
        document_edits[0]["newText"],
        "pub fn main() {\n    return 1\n}\n"
    );

    let range_response = typed_range_formatting_response(&mut state, 21, &document);
    let range_edits = range_response["result"]
        .as_array()
        .expect("rangeFormatting response should be an array");
    assert_eq!(range_edits.len(), 1);
    assert_eq!(range_edits[0]["range"]["start"]["line"], 1);
    assert_eq!(range_edits[0]["newText"], "");

    let on_type_response = typed_on_type_formatting_response(&mut state, 22, &document);
    let on_type_edits = on_type_response["result"]
        .as_array()
        .expect("onTypeFormatting response should be an array");
    assert_eq!(on_type_edits.len(), 1);
    assert_eq!(on_type_edits[0]["range"]["start"]["line"], 0);
    assert_eq!(
        on_type_edits[0]["newText"],
        "pub fn main() {\n    return 1\n}\n"
    );
}
use super::*;
