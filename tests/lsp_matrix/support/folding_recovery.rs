//! Independently authored whole recovery spans, never provider-derived.
use super::{Document, Point, Spec, load, parse_markers};
use serde_json::{Value, json};

pub(crate) fn spec(crlf: bool, shifted: bool) -> Spec {
    let mut spec = load("folding-recovery");
    let transform = |source: &str| {
        let source = if shifted {
            if source.starts_with("#!") {
                let (first, rest) = source.split_once('\n').unwrap_or((source, ""));
                format!("{first}\n// shifted 中😀\n/* second 😀 */\n{rest}")
            } else {
                format!("// shifted 中😀\n/* second 😀 */\n{source}")
            }
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
        .expect("fifty-four cases")
    {
        case["source"] = json!(transform(case["source"].as_str().expect("source")));
    }
    assert_eq!(spec.oracle["cases"].as_array().expect("cases").len(), 54);
    let first = document(&spec.oracle["cases"][0]);
    let range = first.markers["before"];
    assert_eq!(
        (
            range.start.line,
            range.start.character,
            range.end.line,
            range.end.character
        ),
        (usize::from(shifted) * 2, 8, 2 + usize::from(shifted) * 2, 1)
    );
    assert_eq!(byte_column(&first, range.start), 12);
    assert_eq!(byte_column(&first, range.end), 1);
    spec
}
pub(crate) fn document(case: &Value) -> Document {
    parse_markers(case["source"].as_str().expect("source")).expect("authored trivia")
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
        let kind = match row["kind"].as_str().expect("kind") {
            "region" => if protocol { "region" } else { "Region" },
            "imports" => if protocol { "imports" } else { "Imports" },
            other => panic!("unexpected authored kind {other}"),
        };
        let range = doc.markers[row["range"].as_str().expect("range")];
        if protocol {
            json!({"kind":kind,"startLine":range.start.line,"startCharacter":range.start.character,
                "endLine":range.end.line,"endCharacter":range.end.character})
        } else {
            json!({"kind":kind,"start":{"line":range.start.line,"character":byte_column(&doc,range.start)},
                "end":{"line":range.end.line,"character":byte_column(&doc,range.end)}})
        }
    }).collect();
    Value::Array(rows)
}

pub(crate) fn states(spec: &Spec) -> Vec<Value> {
    let cases = spec.oracle["cases"].as_array().expect("54 cases");
    cases
        .iter()
        .flat_map(|case| [case.clone(), cases[0].clone(), case.clone()])
        .collect()
}
pub(crate) fn assert_syntax(tree: &vela_syntax::ast::SyntaxSourceFile, case: &Value) {
    use vela_syntax::ast::AstNode;
    let doc = document(case);
    for row in case["syntaxRegions"].as_array().into_iter().flatten() {
        let region = doc.markers[row["range"].as_str().expect("syntax range")];
        let actual = tree
            .syntax()
            .descendants()
            .filter(|node| {
                u32::from(node.text_range().start()) as usize == region.start.byte
                    && u32::from(node.text_range().end()) as usize == region.end.byte
            })
            .map(|node| format!("{:?}", node.kind()))
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec![row["kind"].as_str().expect("authored CST kind")]
        );
    }
}
