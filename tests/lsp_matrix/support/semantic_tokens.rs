//! Independent token coordinates, source slicing and whole-stream oracles.
use serde_json::Value;

use super::{Document, offset_at};

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
            let length = if utf16 {
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
            assert_eq!(result.text, entry["text"].as_str().expect("token text"));
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
