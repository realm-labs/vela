//! Literal folding markers and complete sets; no provider-derived expectations.
use super::{Document, Point, Spec, load, parse_markers};
use serde_json::{Value, json};

pub(crate) fn spec(crlf: bool, shifted: bool) -> Spec {
    let mut spec = load("folding-imports");
    let transform = |source: &str| {
        let source = if shifted {
            format!("// shifted 中😀\n/* second 😀 */\n{source}")
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
    for case in spec.oracle["cases"].as_array_mut().expect("fourteen cases") {
        case["source"] = json!(transform(case["source"].as_str().expect("source")));
    }
    assert_eq!(spec.oracle["cases"].as_array().expect("cases").len(), 14);
    let first = document(&spec.oracle["cases"][0]);
    let imports = first.markers["imports"];
    assert_eq!(
        (
            imports.start.line,
            imports.start.character,
            imports.end.line,
            imports.end.character
        ),
        (
            usize::from(shifted) * 2,
            8,
            1 + usize::from(shifted) * 2,
            37
        )
    );
    assert_eq!(byte_column(&first, imports.start), 12);
    assert_eq!(byte_column(&first, imports.end), 41);
    spec
}

pub(crate) fn document(case: &Value) -> Document {
    parse_markers(case["source"].as_str().expect("source")).expect("authored folding source")
}
pub(crate) fn byte_column(doc: &Document, point: Point) -> usize {
    point.byte
        - doc.text[..point.byte]
            .rfind('\n')
            .map_or(0, |offset| offset + 1)
}
pub(crate) fn expected(case: &Value, protocol: bool) -> Value {
    let doc = document(case);
    Value::Array(case["ranges"].as_array().expect("complete rows").iter().map(|row| {
        let range = doc.markers[row["range"].as_str().expect("range")];
        if protocol {
            json!({"kind":row["kind"],"startLine":range.start.line,"startCharacter":range.start.character,
                "endLine":range.end.line,"endCharacter":range.end.character})
        } else {
            json!({"kind":if row["kind"] == "imports" {"Imports"} else {panic!("authored import kind")},
                "start":{"line":range.start.line,"character":byte_column(&doc,range.start)},
                "end":{"line":range.end.line,"character":byte_column(&doc,range.end)}})
        }
    }).collect())
}
