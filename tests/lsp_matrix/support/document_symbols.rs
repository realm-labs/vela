//! Independently authored symbol trees and source markers; no provider calls.
use super::{Document, FixtureWorkspace, Point, load};
use serde_json::{Value, json};

pub(crate) fn fixture(crlf: bool) -> (FixtureWorkspace, Value) {
    let mut spec = load("document-symbol-declarations");
    if crlf {
        for source in spec.files.values_mut() {
            *source = source.replace('\n', "\r\n");
        }
    }
    let fixture = FixtureWorkspace::new(&spec).expect("symbol fixture");
    // One leading space + #[doc(" (7), 中😀 (3 UTF-16, 7 bytes),
    // ` read misleading name")] fn ` (28): real name starts at 39/43.
    let doc = &fixture.disk["scripts/api.vela"];
    let name = doc.markers["required-name"];
    assert_eq!(
        (name.start.line, name.start.character, name.end.character),
        (27, 39, 43)
    );
    assert_eq!(
        name.start.byte - doc.text[..name.start.byte].rfind('\n').expect("line") - 1,
        43
    );
    assert_eq!(&doc.text[name.start.byte..name.end.byte], "read");
    (fixture, spec.oracle)
}

pub(crate) fn recovery(crlf: bool) -> (FixtureWorkspace, Value) {
    let mut spec = load("document-symbol-recovery");
    if crlf {
        for source in spec.files.values_mut() {
            *source = source.replace('\n', "\r\n");
        }
        for case in spec.oracle["cases"].as_array_mut().expect("cases") {
            case["source"] = json!(
                case["source"]
                    .as_str()
                    .expect("source")
                    .replace('\n', "\r\n")
            );
        }
    }
    let fixture = FixtureWorkspace::new(&spec).expect("recovery fixture");
    let doc = &fixture.disk["scripts/main.vela"];
    let selected = doc.markers["before-name"];
    assert_eq!(
        (
            selected.start.line,
            selected.start.character,
            selected.end.character
        ),
        (0, 13, 19)
    );
    assert_eq!(selected.start.byte, 17);
    assert_eq!(&doc.text[selected.start.byte..selected.end.byte], "before");
    (fixture, spec.oracle)
}

pub(crate) fn ownership(crlf: bool) -> (FixtureWorkspace, Value) {
    let mut spec = load("document-symbol-ownership");
    if crlf {
        for source in spec.files.values_mut() {
            *source = source.replace('\n', "\r\n");
        }
    }
    let fixture = FixtureWorkspace::new(&spec).expect("ownership fixture");
    let doc = &fixture.disk["scripts/source.vela"];
    let name = doc.markers["widget-name"];
    assert_eq!(
        (name.start.line, name.start.character, name.end.character),
        (0, 21, 27)
    );
    assert_eq!((name.start.byte, name.end.byte), (25, 31));
    assert_eq!(&doc.text[name.start.byte..name.end.byte], "Widget");
    (fixture, spec.oracle)
}

pub(crate) fn expected(document: &Document, rows: &Value, protocol: bool) -> Value {
    Value::Array(rows.as_array().expect("authored tree").iter().map(|row| {
        let mut result = json!({
            "name":row["name"], "kind":row[if protocol { "protocolKind" } else { "kind" }],
            "range":range(document, row["range"].as_str().expect("range marker"), protocol),
            "selectionRange":range(document, row["selection"].as_str().expect("name marker"), protocol)
        });
        if !row["detail"].is_null() { result["detail"] = row["detail"].clone(); }
        if !protocol { result["identity"] = row["identity"].clone(); }
        if !row["children"].as_array().expect("children").is_empty() {
            result["children"] = expected(document, &row["children"], protocol);
        }
        result
    }).collect())
}

fn range(document: &Document, marker: &str, protocol: bool) -> Value {
    let marker = document.markers[marker];
    let point = |p: Point| {
        let character = if protocol {
            p.character
        } else {
            p.byte - document.text[..p.byte].rfind('\n').map_or(0, |i| i + 1)
        };
        json!({"line":p.line,"character":character})
    };
    json!({"start":point(marker.start),"end":point(marker.end)})
}

pub(crate) fn assert_ancestry(rows: &Value, parent: Option<&Value>) -> usize {
    let position = |p: &Value| {
        (
            p["line"].as_u64().expect("line"),
            p["character"].as_u64().expect("column"),
        )
    };
    rows.as_array()
        .expect("tree")
        .iter()
        .map(|row| {
            let range = &row["range"];
            let selected = &row["selectionRange"];
            assert!(position(&range["start"]) <= position(&selected["start"]));
            assert!(position(&selected["start"]) < position(&selected["end"]));
            assert!(position(&selected["end"]) <= position(&range["end"]));
            if let Some(parent) = parent {
                assert!(position(&parent["start"]) <= position(&range["start"]));
                assert!(position(&range["end"]) <= position(&parent["end"]));
            }
            1 + row
                .get("children")
                .map_or(0, |children| assert_ancestry(children, Some(range)))
        })
        .sum()
}
