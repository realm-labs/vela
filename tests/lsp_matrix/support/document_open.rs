//! Independent open-document inputs and marked diagnostic expectations.
use super::{Document, Spec, load, parse_markers};
use serde_json::{Value, json};

pub(crate) fn spec(crlf: bool) -> Spec {
    let mut spec = load("document-open");
    if crlf {
        for text in spec.files.values_mut() {
            *text = text.replace('\n', "\r\n");
        }
    }
    spec
}
pub(crate) fn document(spec: &Spec, file: &str, prefixed: bool) -> Document {
    let prefix = if prefixed {
        spec.oracle["prefix"].as_str().expect("prefix")
    } else {
        ""
    };
    let crlf = spec.files["vela.toml"].contains("\r\n");
    parse_markers(&format!(
        "{}{}",
        prefix.replace('\n', if crlf { "\r\n" } else { "\n" }),
        spec.files[file]
    ))
    .expect("authored open text")
}
pub(crate) fn span(document: &Document, marker: &str) -> Value {
    let m = document.markers[marker];
    json!({"start":{"line":m.start.line,"character":m.start.character},
        "end":{"line":m.end.line,"character":m.end.character}})
}
pub(crate) fn expected(document: &Document, case: &Value, uri: &str) -> Vec<Value> {
    case["diagnostics"].as_array().expect("diagnostics").iter().map(|d| {
        let labels = d["labels"].as_array().expect("labels").iter().map(|l| json!({
            "uri":uri,"range":span(document,l["marker"].as_str().expect("marker")),"message":l["message"]
        })).collect::<Vec<_>>();
        json!({"code":d["code"],"message":d["message"],"severity":1,
            "range":span(document,d["marker"].as_str().expect("marker")),
            "labels":labels,"candidates":d["candidates"],"repairHints":0})
    }).collect()
}
pub(crate) fn missing_disk(spec: &Spec, file: &str) -> bool {
    spec.oracle["missingDisk"]
        .as_array()
        .expect("missing disk")
        .iter()
        .any(|f| f == file)
}
