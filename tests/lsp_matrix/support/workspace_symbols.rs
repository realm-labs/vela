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

pub(crate) fn type_positions(crlf: bool, shifted: bool) -> (FixtureWorkspace, Value) {
    let mut spec = load("workspace-symbol-type-positions");
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
    let fixture = FixtureWorkspace::new(&spec).expect("authored workspace symbol type positions");
    let source = &fixture.disk["scripts/types.vela"];
    let unit = source.markers["unit-range"];
    assert_eq!(
        (unit.start.line, unit.start.character),
        (2 + usize::from(shifted) * 2, 8)
    );
    assert_eq!(
        unit.start.byte - source.text[..unit.start.byte].rfind('\n').expect("line") - 1,
        12
    );
    assert_eq!(
        spec.oracle["cases"]
            .as_array()
            .expect("type partitions")
            .len(),
        47
    );
    assert_eq!(
        spec.oracle["symbols"].as_array().expect("whole rows").len(),
        78
    );
    assert_eq!(
        spec.oracle["queries"].as_array().expect("query sets").len(),
        80
    );
    (fixture, spec.oracle)
}

pub(crate) fn recovery(crlf: bool, shifted: bool) -> (FixtureWorkspace, Value) {
    let mut spec = load("workspace-symbol-recovery");
    let transform = |text: &str| {
        let text = if shifted {
            // Shebangs retain their first-line grammar position.
            if text.starts_with("[[file:start]]#!") {
                text.replacen('\n', "\n// shifted 中😀\n/* extra 😀 */\n", 1)
            } else {
                text.replace(
                    "[[file:start]]",
                    "[[file:start]]// shifted 中😀\n/* extra 😀 */\n",
                )
            }
        } else {
            text.to_owned()
        };
        if crlf {
            text.replace('\n', "\r\n")
        } else {
            text
        }
    };
    for (file, source) in &mut spec.files {
        if file.ends_with(".vela") {
            *source = transform(source);
        }
    }
    for case in spec.oracle["cases"].as_array_mut().expect("cases") {
        case["source"] = Value::String(transform(case["source"].as_str().expect("source")));
    }
    let fixture = FixtureWorkspace::new(&spec).expect("workspace symbol recovery");
    let before = fixture.disk["scripts/main.vela"].markers["before-name"];
    assert_eq!(
        (
            before.start.line,
            before.start.character,
            before.end.character
        ),
        (usize::from(shifted) * 2, 13, 19)
    );
    assert_eq!(
        &fixture.disk["scripts/main.vela"].text[before.start.byte..before.end.byte],
        "before"
    );
    assert_eq!(spec.oracle["cases"].as_array().expect("cases").len(), 56);
    assert_eq!(
        spec.oracle["queries"].as_array().expect("queries").len(),
        19
    );
    (fixture, spec.oracle)
}

pub(crate) fn lifecycle(crlf: bool, shifted: bool) -> super::Spec {
    let mut spec = load("workspace-symbol-lifecycle");
    let transform = |text: &str| {
        let text = if shifted {
            text.replace(
                "[[file:start]]",
                "[[file:start]]// shifted 中😀\n/* extra 😀 */\n",
            )
        } else {
            text.to_owned()
        };
        if crlf {
            text.replace('\n', "\r\n")
        } else {
            text
        }
    };
    for (file, source) in &mut spec.files {
        if file.ends_with(".vela") {
            *source = transform(source);
        }
    }
    for variant in spec.oracle["variants"]
        .as_object_mut()
        .expect("variants")
        .values_mut()
    {
        variant["source"] = Value::String(transform(variant["source"].as_str().expect("source")));
    }
    let doc = super::document_symbol_lifecycle::document(&spec, "main-base");
    let name = doc.markers["main-fn-name"];
    assert_eq!(
        (name.start.line, name.start.character, name.end.character),
        (1 + usize::from(shifted) * 2, 13, 16)
    );
    assert_eq!(
        name.start.byte - doc.text[..name.start.byte].rfind('\n').expect("line") - 1,
        17
    );
    assert_eq!(spec.oracle["phases"].as_array().expect("phases").len(), 19);
    spec
}

fn range(document: &Document, marker: &str, protocol: bool) -> Value {
    let marker = document.markers[marker];
    let point = |p: Point| json!({"line":p.line,"character":if protocol {p.character} else {p.byte - document.text[..p.byte].rfind('\n').map_or(0,|i|i+1)}});
    json!({"start":point(marker.start),"end":point(marker.end)})
}
