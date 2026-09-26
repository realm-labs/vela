use crate::matrix_fixture::{Document, semantic_tokens as oracle};
use crate::tests::{TestServer, request, response_value};
use lsp_types::request as r;
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn start(root: &Path) -> (TestServer, Value) {
    let mut server = TestServer::new();
    let result = response_value(request::<r::Initialize>(
        &mut server,
        1,
        json!({"processId":null,"rootUri":lsp_types::Url::from_file_path(root).expect("root URI"),"capabilities":{}}),
    ));
    (
        server,
        result["result"]["capabilities"]["semanticTokensProvider"]["legend"].clone(),
    )
}

pub(super) fn full(server: &mut TestServer, uri: &str) -> Value {
    let response = response_value(request::<r::SemanticTokensFullRequest>(
        server,
        2,
        json!({"textDocument":{"uri":uri}}),
    ));
    assert!(response.get("error").is_none(), "{response}");
    response["result"].clone()
}

pub(super) fn rows(document: &Document, data: &Value, legend: &Value) -> Vec<oracle::Row> {
    let data = data.as_array().expect("encoded data");
    assert_eq!(data.len() % 5, 0);
    let types = legend["tokenTypes"].as_array().expect("types");
    let modifiers = legend["tokenModifiers"].as_array().expect("modifiers");
    let mut line = 0;
    let mut column = 0;
    data.as_chunks::<5>()
        .0
        .iter()
        .map(|token| {
            let integer =
                |index: usize| usize::try_from(token[index].as_u64().expect("u32")).expect("usize");
            let line_delta = integer(0);
            line += line_delta;
            column = if line_delta == 0 {
                column + integer(1)
            } else {
                integer(1)
            };
            let bits = integer(4);
            assert!(bits >> modifiers.len() == 0, "unknown modifier bits");
            oracle::row(
                document,
                (line, column, integer(2)),
                types[integer(3)].as_str().expect("type"),
                modifiers
                    .iter()
                    .enumerate()
                    .filter(|(bit, _)| bits & (1 << bit) != 0)
                    .map(|(_, name)| name.as_str().expect("modifier").to_owned())
                    .collect(),
                true,
            )
        })
        .collect()
}

pub(super) fn apply_delta(previous: &Value, delta: &Value) -> Value {
    let mut applied = previous.as_array().expect("previous data").clone();
    for edit in delta["edits"].as_array().expect("edits").iter().rev() {
        let start = edit["start"].as_u64().expect("start") as usize;
        let count = edit["deleteCount"].as_u64().expect("count") as usize;
        assert!(start + count <= applied.len());
        applied.splice(
            start..start + count,
            edit["data"]
                .as_array()
                .expect("replacement data")
                .iter()
                .cloned(),
        );
    }
    json!(applied)
}

pub(super) fn delta(server: &mut TestServer, uri: &str, previous_id: &Value) -> Value {
    let response = response_value(request::<r::SemanticTokensFullDeltaRequest>(
        server,
        3,
        json!({"textDocument":{"uri":uri},"previousResultId":previous_id}),
    ));
    assert!(response.get("error").is_none(), "{response}");
    response["result"].clone()
}
