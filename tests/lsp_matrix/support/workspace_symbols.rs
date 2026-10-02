//! Authored result sets and markers only; no workspace-symbol provider calls.
use super::{Document, FixtureWorkspace, Point, load};
use serde_json::{Value, json};

pub(crate) fn fixture(crlf: bool, shifted: bool) -> (FixtureWorkspace, Value) {
    let mut spec = load("workspace-symbol-declarations");
    for source in spec.files.values_mut() {
        if shifted {
            *source = source.replace(
                "[[file:start]]",
                "[[file:start]]// shifted 中😀\n/* extra 😀 */\n",
            );
        }
        if crlf {
            *source = source.replace('\n', "\r\n");
        }
    }
    let fixture = FixtureWorkspace::new(&spec).expect("authored workspace symbols");
    let document = &fixture.disk["scripts/api.vela"];
    let limit = document.markers["limit-range"];
    assert_eq!(
        (limit.start.line, limit.start.character),
        (1 + usize::from(shifted) * 2, 8)
    );
    assert_eq!(
        limit.start.byte - document.text[..limit.start.byte].rfind('\n').expect("line") - 1,
        12
    );
    assert_eq!(
        spec.oracle["symbols"]
            .as_array()
            .expect("all symbols")
            .len(),
        31
    );
    assert_eq!(
        spec.oracle["queries"].as_array().expect("queries").len(),
        32
    );
    (fixture, spec.oracle)
}

pub(crate) fn expected(
    fixture: &FixtureWorkspace,
    authored: &Value,
    ids: &Value,
    protocol: bool,
    uri: &impl Fn(&str) -> String,
) -> Value {
    Value::Array(ids.as_array().expect("authored ordered ids").iter().map(|id| {
        let row = authored["symbols"].as_array().expect("authored rows").iter().find(|row| row["id"] == *id).expect("owned row");
        let file = row["file"].as_str().expect("file");
        let mut result = json!({"name":row["name"],"kind":row[if protocol {"protocolKind"} else {"kind"}],
            "location":{"uri":uri(file),"range":range(&fixture.disk[file],row["range"].as_str().expect("range"),protocol)}});
        if !row["container"].is_null() {result["containerName"] = row["container"].clone();}
        if !row["detail"].is_null() {
            if protocol {result["data"] = json!({"detail":row["detail"]});}
            else {result["detail"] = row["detail"].clone();}
        }
        if !protocol {
            result["identity"] = if row["fileIdentity"] == true {json!(uri(file))} else {row["identity"].clone()};
        }
        result
    }).collect())
}

fn range(document: &Document, marker: &str, protocol: bool) -> Value {
    let marker = document.markers[marker];
    let point = |p: Point| json!({"line":p.line,"character":if protocol {p.character} else {p.byte - document.text[..p.byte].rfind('\n').map_or(0,|i|i+1)}});
    json!({"start":point(marker.start),"end":point(marker.end)})
}
