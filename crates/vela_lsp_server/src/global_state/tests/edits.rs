#[test]
fn typed_prepare_rename_dispatch_projects_placeholder_range() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "\
pub fn main(amount: i64) -> i64 {
    return amount
}";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);
    let line = text.lines().nth(1).expect("return line should exist");
    let character = line
        .find("amount")
        .expect("return line should contain amount");

    let response = typed_prepare_rename_response(&mut state, 15, &document, 1, character);

    assert_eq!(response["result"]["placeholder"], "amount");
    assert_eq!(response["result"]["range"]["start"]["line"], 1);
    assert_eq!(response["result"]["range"]["start"]["character"], 11);
    assert_eq!(response["result"]["range"]["end"]["character"], 17);
}

#[test]
fn typed_rename_dispatch_projects_workspace_edit() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "\
pub fn main(amount: i64) -> i64 {
    return amount
}";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(2));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);
    let line = text.lines().nth(1).expect("return line should exist");
    let character = line
        .find("amount")
        .expect("return line should contain amount");

    let response = typed_rename_response(&mut state, 16, &document, 1, character, "total");

    let edits = response["result"]["changes"][document.as_str()]
        .as_array()
        .expect("rename changes should contain document edits");
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0]["newText"], "total");
    assert_eq!(
        response["result"]["documentChanges"][0]["textDocument"]["uri"],
        document.as_str()
    );
    assert_eq!(
        response["result"]["documentChanges"][0]["textDocument"]["version"],
        2
    );
    assert_eq!(
        response["result"]["documentChanges"][0]["edits"][0]["newText"],
        "total"
    );
}

#[test]
fn typed_prepare_call_hierarchy_dispatch_projects_items() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "pub fn grant() -> i64 { return 1 }\npub fn main() { return grant() }";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);
    let line = text.lines().nth(1).expect("main line should exist");
    let character = line.find("grant").expect("main line should contain grant");

    let response = typed_prepare_call_hierarchy_response(&mut state, 17, &document, 1, character);
    let items = response["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array");

    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0]["name"], "grant");
    assert_eq!(items[0]["kind"], 12);
    assert_eq!(items[0]["uri"], document.as_str());
    assert_eq!(items[0]["selectionRange"]["start"]["line"], 0);
    assert_eq!(items[0]["selectionRange"]["start"]["character"], 7);
    assert!(items[0]["data"].is_object());
}

#[test]
fn typed_call_hierarchy_incoming_and_outgoing_dispatch_project_calls() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    state.initialized = true;
    state.initialized = true;
    let document = DocumentId::from("file:///workspace/scripts/main.vela");
    let text = "pub fn grant() -> i64 { return 1 }\npub fn main() { return grant() }";
    state
        .project
        .workspace
        .open_document(document.clone(), text, SourceVersion::new(1));
    state.project.open_documents.insert(document.clone());
    state.project.refresh_document_databases(&document);
    let main_line = text.lines().nth(1).expect("main line should exist");
    let grant_character = main_line
        .find("grant")
        .expect("main line should contain grant");
    let main_character = main_line
        .find("main")
        .expect("main line should contain main");
    let grant_item: lsp_types::CallHierarchyItem =
            serde_json::from_value(
                typed_prepare_call_hierarchy_response(
                    &mut state,
                    18,
                    &document,
                    1,
                    grant_character,
                )["result"][0]
                    .clone(),
            )
            .expect("grant item should deserialize");
    let main_item: lsp_types::CallHierarchyItem =
            serde_json::from_value(
                typed_prepare_call_hierarchy_response(&mut state, 19, &document, 1, main_character)
                    ["result"][0]
                    .clone(),
            )
            .expect("main item should deserialize");

    let incoming = typed_incoming_calls_response(&mut state, 20, grant_item);
    let outgoing = typed_outgoing_calls_response(&mut state, 21, main_item);

    let incoming = incoming["result"]
        .as_array()
        .expect("incomingCalls response should be an array");
    assert_eq!(incoming.len(), 1, "{incoming:?}");
    assert_eq!(incoming[0]["from"]["name"], "main");
    assert_eq!(
        incoming[0]["fromRanges"]
            .as_array()
            .expect("incomingCalls should contain fromRanges")
            .len(),
        1
    );

    let outgoing = outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(outgoing.len(), 1, "{outgoing:?}");
    assert_eq!(outgoing[0]["to"]["name"], "grant");
    assert_eq!(
        outgoing[0]["fromRanges"]
            .as_array()
            .expect("outgoingCalls should contain fromRanges")
            .len(),
        1
    );
}

#[test]
fn typed_cancellation_is_tracked_by_global_request_queue() {
    let (sender, _receiver) = unbounded();
    let mut state = GlobalState::new(sender, LaunchConfiguration::new());
    let request_id = RequestId::from(7);
    state.request_queue.start(request_id.clone());

    let result = state.cancel_request(lsp_types::CancelParams {
        id: lsp_types::NumberOrString::Number(7),
    });

    assert_no_messages(result);
    assert!(state.take_cancelled_request(&request_id));
    assert!(!state.take_cancelled_request(&request_id));
}

#[test]
fn request_queue_tracks_typed_request_ids() {
    let mut queue = RequestQueue::default();
    let numeric = RequestId::from(7);
    let string = RequestId::from("hover-1".to_owned());

    queue.start(numeric.clone());
    queue.start(string.clone());
    assert!(queue.incoming.contains(&numeric));
    assert!(queue.incoming.contains(&string));

    queue.finish(&numeric);
    assert!(!queue.incoming.contains(&numeric));
    assert!(queue.incoming.contains(&string));

    let message = Message::Request(lsp_server::Request {
        id: lsp_server::RequestId::from("hover-1".to_owned()),
        method: "textDocument/hover".to_owned(),
        params: serde_json::json!({}),
    });
    assert_eq!(RequestQueue::request_id(&message), Some(string));
}

#[test]
fn request_queue_stores_in_flight_cancellation_handles() {
    let mut queue = RequestQueue::default();
    let id = RequestId::from(9);
    let databases = LanguageServiceDatabases::new();
    let (token, handle) = databases.begin_cancellable_background_request();

    queue.start_in_flight(id.clone(), handle);
    assert!(queue.in_flight.contains_key(&id));

    queue.cancel(id.clone());
    assert!(token.is_cancelled());
    assert!(queue.finish_in_flight(&id).is_some());
    assert!(!queue.in_flight.contains_key(&id));
}

#[test]
fn request_queue_ignores_unknown_and_completed_cancels() {
    let mut queue = RequestQueue::default();
    let unknown = RequestId::from(404);
    let completed = RequestId::from("done".to_owned());

    queue.cancel(unknown.clone());
    assert!(!queue.take_cancelled(&unknown));

    queue.start(completed.clone());
    queue.finish(&completed);
    queue.cancel(completed.clone());
    assert!(!queue.take_cancelled(&completed));
}
use super::*;
