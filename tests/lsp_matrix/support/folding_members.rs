//! Independently authored whole member/constructor spans, never provider-derived.
use super::{Document, Point, Spec, load, parse_markers};
use serde_json::{Value, json};

pub(crate) fn spec(crlf: bool, shifted: bool) -> Spec {
    let mut spec = load("folding-members");
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
    for case in spec.oracle["cases"]
        .as_array_mut()
        .expect("thirty-nine cases")
    {
        case["source"] = json!(transform(case["source"].as_str().expect("source")));
    }
    assert_eq!(spec.oracle["cases"].as_array().expect("cases").len(), 39);
    let first = document(&spec.oracle["cases"][0]);
    let range = first.markers["item"];
    assert_eq!(
        (
            range.start.line,
            range.start.character,
            range.end.line,
            range.end.character
        ),
        (
            2 + usize::from(shifted) * 2,
            8,
            8 + usize::from(shifted) * 2,
            1
        )
    );
    assert_eq!(byte_column(&first, range.start), 12);
    assert_eq!(byte_column(&first, range.end), 1);
    spec
}
pub(crate) fn document(case: &Value) -> Document {
    parse_markers(case["source"].as_str().expect("source")).expect("authored members")
}
fn byte_column(doc: &Document, point: Point) -> usize {
    point.byte
        - doc.text[..point.byte]
            .rfind('\n')
            .map_or(0, |offset| offset + 1)
}
pub(crate) fn expected(case: &Value, protocol: bool) -> Value {
    let doc = document(case);
    let rows = case["ranges"].as_array().expect("complete authored rows").iter().map(|row| {
        assert_eq!(row["kind"],"region");
        let range = doc.markers[row["range"].as_str().expect("range")];
        if protocol {
            json!({"kind":"region","startLine":range.start.line,"startCharacter":range.start.character,
                "endLine":range.end.line,"endCharacter":range.end.character})
        } else {
            json!({"kind":"Region","start":{"line":range.start.line,"character":byte_column(&doc,range.start)},
                "end":{"line":range.end.line,"character":byte_column(&doc,range.end)}})
        }
    }).collect();
    Value::Array(rows)
}
