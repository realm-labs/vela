//! Shared independent marker projection; never consumes provider output.
use super::{Document, Point, parse_markers};
use serde_json::{Value, json};

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
