//! Independent token coordinates, source slicing and whole-stream oracles.
use serde_json::Value;

use super::{Document, Spec, offset_at, parse_markers};

#[derive(Clone)]
pub(crate) struct Phase<'a> {
    pub document: Document,
    pub tokens: &'a Value,
    pub diagnostics: &'a Value,
}

pub(crate) fn phases(spec: &Spec, crlf: bool) -> Vec<Phase<'_>> {
    let document = |text: &str| {
        parse_markers(&if crlf {
            text.replace('\n', "\r\n")
        } else {
            text.to_owned()
        })
        .expect("phase markers")
    };
    let positive = Phase {
        document: document(&spec.files["scripts/main.vela"]),
        tokens: &spec.oracle["positive"],
        diagnostics: &spec.oracle["diagnostics"],
    };
    let mut phases = vec![positive.clone()];
    let cases: Vec<_> = spec.oracle["negative"]["cases"].as_array().map_or_else(
        || vec![&spec.oracle["negative"]],
        |cases| cases.iter().collect(),
    );
    for case in cases {
        phases.push(Phase {
            document: document(case["source"].as_str().expect("negative source")),
            tokens: &case["tokens"],
            diagnostics: &case["diagnostics"],
        });
        phases.push(positive.clone());
    }
    phases
}

pub(crate) fn assert_recovery_diagnostics(
    document: &Document,
    actual: &[Value],
    expected: &Value,
    utf16: bool,
) {
    if expected.is_null() {
        return;
    }
    if expected["clean"] == true {
        assert!(actual.is_empty(), "repaired diagnostics: {actual:?}");
    }
    if let Some(parse_errors) = expected["parseErrors"].as_bool() {
        assert_eq!(
            actual.iter().any(|item| item["code"] == "E_PARSE"),
            parse_errors,
            "parser recovery diagnostics for {}: {actual:?}",
            expected["phase"]
        );
    }
    for code in expected["codes"].as_array().into_iter().flatten() {
        assert!(
            actual.iter().any(|item| item["code"] == *code),
            "{code} for {}: {actual:?}",
            expected["phase"]
        );
    }
    for candidate in expected["candidates"].as_array().into_iter().flatten() {
        let marker = document.markers[candidate["marker"].as_str().expect("diagnostic marker")];
        let point = |point: super::Point| {
            serde_json::json!({"line":point.line,"character":if utf16 {
                point.character
            } else {
                point.byte - document.text[..point.byte].rfind('\n').map_or(0, |offset| offset + 1)
            }})
        };
        let range = serde_json::json!({"start":point(marker.start),"end":point(marker.end)});
        let matching: Vec<_> = actual
            .iter()
            .filter(|item| item["code"] == candidate["code"] && item["range"] == range)
            .collect();
        assert_eq!(matching.len(), 1, "diagnostic at {range}: {actual:?}");
        assert_eq!(matching[0]["candidates"], candidate["replacements"]);
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct Row {
    pub line: usize,
    pub column: usize,
    pub length: usize,
    pub text: String,
    pub kind: String,
    pub modifiers: Vec<String>,
}

pub(crate) fn row(
    document: &Document,
    coordinates: (usize, usize, usize),
    kind: &str,
    modifiers: Vec<String>,
    utf16: bool,
) -> Row {
    let (line, column, length) = coordinates;
    assert!(length > 0, "tokens must be nonempty");
    let text = if utf16 {
        let start = offset_at(&document.text, line, column).expect("UTF-16 start boundary");
        let end = offset_at(&document.text, line, column + length).expect("UTF-16 end boundary");
        &document.text[start..end]
    } else {
        document
            .text
            .split('\n')
            .nth(line)
            .expect("token line")
            .trim_end_matches('\r')
            .get(column..column + length)
            .expect("byte boundaries inside line")
    };
    assert!(!text.contains(['\r', '\n']), "a token cannot span lines");
    Row {
        line,
        column,
        length,
        text: text.to_owned(),
        kind: kind.to_owned(),
        modifiers,
    }
}

pub(crate) fn expected(document: &Document, oracle: &Value, utf16: bool) -> Vec<Row> {
    oracle
        .as_array()
        .expect("complete token oracle")
        .iter()
        .map(|entry| {
            let text = entry["text"].as_str().expect("token text");
            let marker = document.markers[entry["marker"].as_str().expect("marker")];
            assert_eq!(marker.start.line, marker.end.line);
            let line_start = document.text[..marker.start.byte]
                .rfind('\n')
                .map_or(0, |index| index + 1);
            let column = if utf16 {
                marker.start.character
            } else {
                marker.start.byte - line_start
            };
            let length = if marker.start == marker.end {
                if utf16 {
                    text.encode_utf16().count()
                } else {
                    text.len()
                }
            } else if utf16 {
                marker.end.character - marker.start.character
            } else {
                marker.end.byte - marker.start.byte
            };
            let result = row(
                document,
                (marker.start.line, column, length),
                entry["type"].as_str().expect("type"),
                entry["modifiers"]
                    .as_array()
                    .expect("modifiers")
                    .iter()
                    .map(|modifier| modifier.as_str().expect("modifier").to_owned())
                    .collect(),
                utf16,
            );
            assert_eq!(result.text, text);
            result
        })
        .collect()
}

pub(crate) fn assert_stream(actual: &[Row], expected: &[Row]) {
    for pair in actual.windows(2) {
        assert!(
            pair[0].line < pair[1].line
                || (pair[0].line == pair[1].line
                    && pair[0].column + pair[0].length <= pair[1].column),
            "tokens must be ordered and nonoverlapping: {pair:?}"
        );
    }
    assert_eq!(actual.len(), expected.len(), "complete token count");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual, expected, "complete token stream at token {index}");
    }
}

#[test]
fn cursor_token_oracles_pin_literal_brackets_and_unicode_lengths() {
    let document =
        super::parse_markers("/* 文😀 */ [[open]][ [[word]]文😀 [[close:start]]][[close:end]]\r\n")
            .expect("cursor beside literal bracket");
    let oracle = serde_json::json!([
        {"marker":"open","text":"[","type":"bracket","modifiers":[]},
        {"marker":"word","text":"文😀","type":"string","modifiers":[]},
        {"marker":"close","text":"]","type":"bracket","modifiers":[]}
    ]);
    for (utf16, wanted) in [
        (false, [(14, 1), (16, 7), (24, 1)]),
        (true, [(10, 1), (12, 3), (16, 1)]),
    ] {
        let rows = expected(&document, &oracle, utf16);
        assert_eq!(
            rows.iter()
                .map(|row| (row.column, row.length))
                .collect::<Vec<_>>(),
            wanted
        );
        assert!(rows.iter().all(|row| row.line == 0));
    }
}
