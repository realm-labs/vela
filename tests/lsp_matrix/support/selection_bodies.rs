//! Independent complete function and method body ancestry.
use super::selection::byte_column;
pub(crate) use super::selection::{document, expected, positions};
use super::{Spec, load};

pub(crate) fn spec(crlf: bool, shifted: bool) -> Spec {
    let mut spec = load("selection-bodies");
    let transform = |source: &str| {
        let source = if shifted {
            source.replacen(
                "[[file:start]]",
                "[[file:start]]// shifted 中😀\n/* second 😀 */\n",
                1,
            )
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
    for case in spec.oracle["cases"].as_array_mut().expect("59 cases") {
        case["source"] = serde_json::json!(transform(case["source"].as_str().expect("source")));
    }
    let cases = spec.oracle["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 59);
    assert_eq!(
        cases
            .iter()
            .map(|c| c["queries"].as_array().expect("queries").len())
            .sum::<usize>(),
        180
    );
    let first = document(&cases[0]);
    let name = first.markers["binding"];
    assert_eq!(
        (
            name.start.line,
            name.start.character,
            name.end.line,
            name.end.character
        ),
        (usize::from(shifted) * 2, 23, usize::from(shifted) * 2, 28)
    );
    assert_eq!(
        (
            byte_column(&first, name.start),
            byte_column(&first, name.end)
        ),
        (27, 32)
    );
    assert_eq!(
        (
            first.markers["item"].start.character,
            first.markers["item"].end.character
        ),
        (8, 73)
    );
    assert_eq!(first.markers["file"].start.byte, 0);
    assert_eq!(first.markers["file"].end.byte, first.text.len());
    spec
}
