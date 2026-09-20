use super::{Document, load, parse_markers};
use serde_json::Value;

pub(crate) fn cases(crlf: bool) -> Vec<(Value, Document, Document)> {
    load("rename-local-shorthand").oracle["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .map(|case| {
            let parse = |field: &str| {
                let text = case[field].as_str().expect("source");
                parse_markers(&if crlf {
                    text.replace('\n', "\r\n")
                } else {
                    text.to_owned()
                })
                .expect("markers")
            };
            (case.clone(), parse("source"), parse("expected"))
        })
        .collect()
}
