use crate::tests::{LspServer, notification, notification_value, request, response_value};

use super::{assert_call_range, assert_outgoing_call, line};

#[test]
fn lsp_call_hierarchy_uses_source_method_calls_on_source_method_return_receivers() {
    let (mut server, uri, text) = open_source_method_return_method_fixture();

    let grant_items = prepare_items(&mut server, uri, 13, col(text, 13, "grant"));
    let call_items = prepare_items(&mut server, uri, 17, col(text, 17, "grant"));
    assert_eq!(grant_items.len(), 1);
    assert_eq!(grant_items[0]["name"], "grant");
    assert_eq!(call_items, grant_items);

    assert_incoming_ranges(
        &mut server,
        &grant_items[0],
        &[(17, col(text, 17, "grant")), (18, col(text, 18, "grant"))],
    );
    assert_main_outgoing(
        &mut server,
        uri,
        text,
        16,
        &[
            ("inventory", 17, col(text, 17, "inventory")),
            ("grant", 17, col(text, 17, "grant")),
            ("inventory", 18, col(text, 18, "inventory")),
            ("grant", 18, col(text, 18, "grant")),
        ],
    );
}

#[test]
fn lsp_call_hierarchy_uses_source_trait_default_method_calls_on_source_method_return_receivers() {
    let (mut server, uri, text) = open_source_method_return_trait_fixture();

    let preview_items = prepare_items(&mut server, uri, 1, col(text, 1, "preview"));
    let call_items = prepare_items(&mut server, uri, 19, col(text, 19, "preview"));
    assert_eq!(preview_items.len(), 1);
    assert_eq!(preview_items[0]["name"], "preview");
    assert_eq!(call_items, preview_items);

    assert_incoming_ranges(
        &mut server,
        &preview_items[0],
        &[
            (19, col(text, 19, "preview")),
            (20, col(text, 20, "preview")),
        ],
    );
    assert_main_outgoing(
        &mut server,
        uri,
        text,
        18,
        &[
            ("inventory", 19, col(text, 19, "inventory")),
            ("preview", 19, col(text, 19, "preview")),
            ("inventory", 20, col(text, 20, "inventory")),
            ("preview", 20, col(text, 20, "preview")),
        ],
    );
}

fn col(text: &str, line_number: usize, needle: &str) -> usize {
    line(text, line_number)
        .find(needle)
        .unwrap_or_else(|| panic!("expected line {line_number} to contain {needle:?}"))
}

fn prepare_items(
    server: &mut LspServer,
    uri: &str,
    line: usize,
    character: usize,
) -> Vec<serde_json::Value> {
    response_value(server.handle_json(&request(
        2,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        }),
    )))["result"]
        .as_array()
        .expect("prepareCallHierarchy response should be an array")
        .clone()
}

fn assert_incoming_ranges(
    server: &mut LspServer,
    item: &serde_json::Value,
    expected: &[(usize, usize)],
) {
    let incoming = response_value(server.handle_json(&request(
        3,
        "callHierarchy/incomingCalls",
        serde_json::json!({ "item": item.clone() }),
    )));
    let incoming_calls = incoming["result"]
        .as_array()
        .expect("incomingCalls response should be an array");
    assert_eq!(incoming_calls.len(), 1, "{incoming_calls:?}");
    assert_eq!(incoming_calls[0]["from"]["name"], "main");
    let ranges = incoming_calls[0]["fromRanges"]
        .as_array()
        .expect("incoming call should include ranges");
    for (line, character) in expected {
        assert_call_range(ranges, *line, *character);
    }
}

fn assert_main_outgoing(
    server: &mut LspServer,
    uri: &str,
    text: &str,
    main_line: usize,
    expected: &[(&str, usize, usize)],
) {
    let main_items = prepare_items(server, uri, main_line, col(text, main_line, "main"));
    assert_eq!(main_items.len(), 1);

    let outgoing = response_value(server.handle_json(&request(
        4,
        "callHierarchy/outgoingCalls",
        serde_json::json!({ "item": main_items[0].clone() }),
    )));
    let outgoing_calls = outgoing["result"]
        .as_array()
        .expect("outgoingCalls response should be an array");
    assert_eq!(outgoing_calls.len(), 2, "{outgoing_calls:?}");
    for (name, line, character) in expected {
        assert_outgoing_call(outgoing_calls, name, uri, *line, *character);
    }
}

fn open_source_method_return_method_fixture() -> (LspServer, &'static str, &'static str) {
    let text = "\
pub struct Player {
    level: i64
}

pub struct Inventory {
    count: i64
}

impl Player {
    pub fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}

impl Inventory {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

pub fn main(player: Player) -> i64 {
    let first = player.inventory().grant(1)
    return player.inventory().grant(first)
}";
    open_fixture(text)
}

fn open_source_method_return_trait_fixture() -> (LspServer, &'static str, &'static str) {
    let text = "\
pub trait Rewardable {
    fn preview(self, amount: i64) -> i64 { return amount }
}

pub struct Player {
    level: i64
}

pub struct Inventory {
    count: i64
}

impl Player {
    pub fn inventory(self) -> Inventory { return Inventory { count: 1 } }
}

impl Rewardable for Inventory {}

pub fn main(player: Player) -> i64 {
    let first = player.inventory().preview(1)
    return player.inventory().preview(first)
}";
    open_fixture(text)
}

fn open_fixture(text: &'static str) -> (LspServer, &'static str, &'static str) {
    let mut server = LspServer::new();
    let initialize = response_value(server.handle_json(&request(
        1,
        "initialize",
        serde_json::json!({
            "processId": null,
            "rootUri": "file:///workspace/scripts",
            "capabilities": {}
        }),
    )));
    assert_eq!(
        initialize["result"]["capabilities"]["callHierarchyProvider"],
        true
    );
    let uri = "file:///workspace/scripts/game/main.vela";
    let _ = notification_value(server.handle_json(&notification(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": "vela",
                "version": 1,
                "text": text
            }
        }),
    )));
    (server, uri, text)
}
