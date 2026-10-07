//! Independent complete bracket recovery ancestry.
use super::selection::byte_column;
pub(crate) use super::selection::{document, expected, positions};
use super::{Spec, load};

pub(crate) fn spec(crlf: bool, shifted: bool) -> Spec {
    let mut spec = load("selection-brackets");
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
    for case in spec.oracle["cases"].as_array_mut().expect("90 cases") {
        case["source"] = serde_json::json!(transform(case["source"].as_str().expect("source")));
    }
    let cases = spec.oracle["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 90);
    assert_eq!(
        cases
            .iter()
            .map(|c| c["queries"].as_array().expect("queries").len())
            .sum::<usize>(),
        265
    );
    let first = document(&cases[0]);
    let name = first.markers["member"];
    assert_eq!(
        (
            name.start.line,
            name.start.character,
            name.end.line,
            name.end.character
        ),
        (usize::from(shifted) * 2, 61, usize::from(shifted) * 2, 66)
    );
    assert_eq!(
        (
            byte_column(&first, name.start),
            byte_column(&first, name.end)
        ),
        (65, 70)
    );
    assert_eq!(
        (
            first.markers["item"].start.character,
            first.markers["item"].end.character
        ),
        (8, 70)
    );
    assert_eq!(first.markers["file"].start.byte, 0);
    assert_eq!(first.markers["file"].end.byte, first.text.len());
    spec
}

pub(crate) fn assert_parse<'a>(codes: impl IntoIterator<Item = Option<&'a str>>, policy: &str) {
    let diagnostics = codes.into_iter().collect::<Vec<_>>();
    match policy {
        "quiet" => assert!(diagnostics.is_empty(), "quiet source: {diagnostics:?}"),
        "damaged" => {
            assert!(
                !diagnostics.is_empty(),
                "unclosed source must retain errors"
            );
            assert!(
                diagnostics.iter().all(|d| *d == Some("E_PARSE")),
                "only authored parse damage: {diagnostics:?}"
            );
        }
        other => panic!("unexpected authored parse policy {other}"),
    }
}
