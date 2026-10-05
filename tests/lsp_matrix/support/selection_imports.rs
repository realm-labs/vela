//! Independent complete token/ancestor chains and point-only results.
use super::{Document, Point, Spec, load, parse_markers};
use serde_json::{Value, json};

pub(crate) fn spec(crlf: bool, shifted: bool) -> Spec {
    let mut spec = load("selection-imports");
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
    for case in spec.oracle["cases"].as_array_mut().expect("sixteen cases") {
        case["source"] = json!(transform(case["source"].as_str().expect("source")));
    }
    let cases = spec.oracle["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 16);
    assert_eq!(
        cases
            .iter()
            .map(|c| c["queries"].as_array().expect("queries").len())
            .sum::<usize>(),
        52
    );
    let first = document(&cases[0]);
    let token = first.markers["token"];
    assert_eq!(
        (
            token.start.line,
            token.start.character,
            token.end.line,
            token.end.character
        ),
        (usize::from(shifted) * 2, 20, usize::from(shifted) * 2, 25)
    );
    assert_eq!(
        (
            byte_column(&first, token.start),
            byte_column(&first, token.end)
        ),
        (24, 29)
    );
    let item = first.markers["item"];
    assert_eq!((item.start.character, item.end.character), (8, 35));
    assert_eq!(
        (
            byte_column(&first, item.start),
            byte_column(&first, item.end)
        ),
        (12, 39)
    );
    assert_eq!(first.markers["file"].start.byte, 0);
    spec
}

pub(crate) fn document(case: &Value) -> Document {
    parse_markers(case["source"].as_str().expect("source")).expect("authored selection source")
}
pub(crate) fn byte_column(doc: &Document, point: Point) -> usize {
    point.byte
        - doc.text[..point.byte]
            .rfind('\n')
            .map_or(0, |offset| offset + 1)
}
fn point(doc: &Document, p: Point, protocol: bool) -> Value {
    json!({"line":p.line,"character":if protocol {p.character} else {byte_column(doc,p)}})
}
pub(crate) fn positions(doc: &Document, queries: &Value, protocol: bool) -> Value {
    Value::Array(
        queries
            .as_array()
            .expect("query vector")
            .iter()
            .map(|query| {
                point(
                    doc,
                    doc.markers[query["position"].as_str().expect("position")].start,
                    protocol,
                )
            })
            .collect(),
    )
}
pub(crate) fn expected(doc: &Document, queries: &Value, protocol: bool) -> Value {
    Value::Array(queries.as_array().expect("query vector").iter().map(|query| {
        let chain = query["chain"].as_array().expect("complete chain");
        if chain.is_empty() {
            let p = point(doc, doc.markers[query["position"].as_str().expect("position")].start, protocol);
            return json!({"range":{"start":p,"end":p}});
        }
        chain.iter().rev().fold(None, |parent: Option<Value>, name| {
            let range = doc.markers[name.as_str().expect("range name")];
            let mut row = json!({"range":{"start":point(doc,range.start,protocol),"end":point(doc,range.end,protocol)}});
            if let Some(parent) = parent { row["parent"] = parent; }
            Some(row)
        }).expect("nonempty chain")
    }).collect())
}
