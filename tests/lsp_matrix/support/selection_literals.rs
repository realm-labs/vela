//! Independent complete literal and operator ancestry.
use super::Document;
use super::selection::byte_column;
pub(crate) use super::selection::{document, positions};
use super::{Spec, load};
use serde_json::{Value, json};

pub(crate) fn spec(crlf: bool, shifted: bool) -> Spec {
    let mut spec = load("selection-literals");
    let transform = |source: &str| {
        let source = if shifted {
            source.replacen(
                "[[file:start]]",
                "[[file:start]]// shifted 中😀\n/* second 😀 */\n",
                1,
            )
        } else {
            source.to_owned()
        };
        if crlf {
            source.replace('\n', "\r\n")
        } else {
            source
        }
    };
    for source in spec.files.values_mut() {
        *source = transform(source);
    }
    for case in spec.oracle["cases"].as_array_mut().expect("117 cases") {
        case["source"] = serde_json::json!(transform(case["source"].as_str().expect("source")));
    }
    let cases = spec.oracle["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 117);
    assert_eq!(
        cases
            .iter()
            .map(|c| c["queries"].as_array().expect("queries").len())
            .sum::<usize>(),
        244
    );
    let first = document(&cases[0]);
    let name = first.markers["member"];
    assert_eq!(
        (
            name.start.line,
            name.start.character,
            name.end.line,
            name.end.character
        ),
        (usize::from(shifted) * 2, 46, usize::from(shifted) * 2, 52)
    );
    assert_eq!(
        (
            byte_column(&first, name.start),
            byte_column(&first, name.end)
        ),
        (54, 60)
    );
    assert_eq!(
        (
            first.markers["item"].start.character,
            first.markers["item"].end.character
        ),
        (8, 117)
    );
    assert_eq!(first.markers["file"].start.byte, 0);
    assert_eq!(first.markers["file"].end.byte, first.text.len());
    spec
}

// Prefix only an explicitly authored ASCII left bracket. The DSL cannot place
// an end marker immediately after '['; this does not consult CST/provider data.
pub(crate) fn expected(doc: &Document, queries: &Value, protocol: bool) -> Value {
    let mut result = super::selection::expected(doc, queries, protocol);
    for (row, query) in result
        .as_array_mut()
        .expect("vector")
        .iter_mut()
        .zip(queries.as_array().expect("queries"))
    {
        if let Some(token) = query.get("token") {
            assert_eq!(token.as_str(), Some("["));
            assert!(!query["chain"].as_array().expect("parents").is_empty());
            let p = doc.markers[query["position"].as_str().expect("position")].start;
            assert_eq!(doc.text.as_bytes()[p.byte], b'[');
            let column = if protocol {
                p.character
            } else {
                byte_column(doc, p)
            };
            let parent = std::mem::replace(row, Value::Null);
            *row = json!({"range":{"start":{"line":p.line,"character":column},
                "end":{"line":p.line,"character":column+1}},"parent":parent});
        }
    }
    result
}
