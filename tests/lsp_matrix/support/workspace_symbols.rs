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
        let location = if row["ownership"] == "Schema" {
            if protocol {json!({"uri":"vela-schema:"})} else {json!({"schema":true})}
        } else {
            let file = row["file"].as_str().expect("file");
            json!({"uri":uri(file),"range":range(&fixture.disk[file],row["range"].as_str().expect("range"),protocol)})
        };
        let mut result = json!({"name":row["name"],"kind":row[if protocol {"protocolKind"} else {"kind"}],
            "location":location});
        if !row["container"].is_null() {result["containerName"] = row["container"].clone();}
        if !row["detail"].is_null() {
            if protocol {result["data"] = json!({"detail":row["detail"]});}
            else {result["detail"] = row["detail"].clone();}
        }
        if !protocol {
            result["identity"] = if row["fileIdentity"] == true {json!(uri(row["file"].as_str().expect("file")))} else {row["identity"].clone()};
            if !row["ownership"].is_null() {result["ownership"] = row["ownership"].clone();}
        }
        result
    }).collect())
}

pub(crate) fn ownership(crlf: bool, shifted: bool) -> (FixtureWorkspace, Value) {
    let mut spec = load("workspace-symbol-ownership");
    for (file, source) in &mut spec.files {
        if file.ends_with(".vela") {
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
    }
    let fixture = FixtureWorkspace::new(&spec).expect("authored workspace symbol ownership");
    let source = &fixture.disk["scripts/source.vela"];
    let widget = source.markers["widget-range"];
    assert_eq!(
        (widget.start.line, widget.start.character),
        (usize::from(shifted) * 2, 10)
    );
    assert_eq!(
        widget.start.byte
            - source.text[..widget.start.byte]
                .rfind('\n')
                .map_or(0, |i| i + 1),
        14
    );
    assert_eq!(
        spec.oracle["symbols"].as_array().expect("whole rows").len(),
        60
    );
    assert_eq!(
        spec.oracle["queries"].as_array().expect("query sets").len(),
        49
    );
    (fixture, spec.oracle)
}

fn range(document: &Document, marker: &str, protocol: bool) -> Value {
    let marker = document.markers[marker];
    let point = |p: Point| json!({"line":p.line,"character":if protocol {p.character} else {p.byte - document.text[..p.byte].rfind('\n').map_or(0,|i|i+1)}});
    json!({"start":point(marker.start),"end":point(marker.end)})
}
