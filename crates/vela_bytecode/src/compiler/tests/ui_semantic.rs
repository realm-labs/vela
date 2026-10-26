use std::{collections::BTreeSet, path::Path};

use vela_common::Diagnostic;
use vela_common::diagnostic_render::{DiagnosticRenderer, DiagnosticSource};

use super::*;

// The typed contract tests check the actual failure phase, code, message and
// ordered labels before rendering. Physical fixture paths are not source names.
pub(super) fn assert_ui_golden(
    diagnostics: &[Diagnostic],
    source_id: SourceId,
    source: &str,
    logical_name: &str,
    expected: &str,
) {
    assert_eq!(
        diagnostics.len(),
        1,
        "UI case requires exactly one diagnostic"
    );
    for diagnostic in diagnostics {
        let primary = diagnostic.span.expect("UI diagnostic needs a primary span");
        for span in std::iter::once(primary).chain(diagnostic.labels.iter().map(|label| label.span))
        {
            assert_eq!(span.source, source_id);
            let start = usize::try_from(span.start).expect("span start");
            let end = usize::try_from(span.end).expect("span end");
            assert!(
                start < end
                    && end <= source.len()
                    && source.is_char_boundary(start)
                    && source.is_char_boundary(end),
                "invalid UI diagnostic span: {span:?}"
            );
        }
    }
    let renderer =
        DiagnosticRenderer::new([DiagnosticSource::new(source_id, logical_name, source)]);
    let rendered = diagnostics
        .iter()
        .flat_map(|diagnostic| renderer.render(diagnostic))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    // Exact LF output, including the final newline; no trimming/path rewriting.
    assert_eq!(rendered, expected, "UI golden mismatch for {logical_name}");
}

#[test]
fn compiler_ui_inventory_matches_registered_pairs() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ui/semantic");
    let cases = [
        "duplicate_argument",
        "literal_range",
        "missing_argument",
        "negated_const_range",
        "positional_after_named",
        "too_many_arguments",
        "type_contract",
        "unknown_named_argument",
    ];
    let registered = include_str!("diagnostic_contracts.rs");
    let mut expected = BTreeSet::new();
    for stem in cases {
        for extension in ["vela", "expected"] {
            let filename = format!("{stem}.{extension}");
            let reference = format!("../../../tests/fixtures/ui/semantic/{filename}");
            assert_eq!(
                registered.matches(&reference).count(),
                1,
                "pair must be registered exactly once: {filename}"
            );
            let contents = std::fs::read_to_string(directory.join(&filename))
                .expect("nonmissing UTF-8 fixture");
            assert!(!contents.is_empty(), "empty UI fixture: {filename}");
            expected.insert(filename);
        }
    }
    let actual = std::fs::read_dir(directory)
        .expect("UI corpus directory")
        .map(|entry| {
            let entry = entry.expect("UI corpus entry");
            assert!(entry.file_type().expect("file type").is_file());
            entry.file_name().into_string().expect("UTF-8 fixture name")
        })
        .collect::<BTreeSet<_>>();
    assert!(!actual.is_empty());
    assert_eq!(
        actual, expected,
        "missing, orphan or unregistered UI fixture"
    );
}

#[rstest::rstest]
#[case::type_contract("fn grant(amount: i64) { return amount; }\nfn main() { return grant(1); }")]
#[case::literal_range("fn main() { return 127i8; }")]
#[case::negated_const_range("const GOOD = -128i8;\nfn main() { return GOOD; }")]
#[case::named_argument(
    "fn grant(base, amount = 10) { return base + amount; }\nfn main() { return grant(amount = 2, base = 1); }"
)]
#[case::positional_before_named(
    "fn grant(first = 0, second = 10) { return first; }\nfn main() { return grant(3, second = 2); }"
)]
#[case::exact_arity("fn grant(only) { return only; }\nfn main() { return grant(1); }")]
#[case::distinct_arguments(
    "fn grant(base, amount = 8) { return base; }\nfn main() { return grant(41, amount = 2); }"
)]
#[case::required_argument(
    "fn grant(required, optional = 10) { return required; }\nfn main() { return grant(1); }"
)]
fn compiler_ui_positive_controls(#[case] source: &str) {
    let program = compile_test_program(SourceId::new(720), source)
        .expect("valid UI counterpart must compile");
    assert!(program.function("main").is_some());
}

const RANGE_SOURCE: &str = include_str!("../../../tests/fixtures/ui/semantic/literal_range.vela");
const RANGE_EXPECTED: &str =
    include_str!("../../../tests/fixtures/ui/semantic/literal_range.expected");
const RANGE_ID: SourceId = SourceId::new(706);

fn range_diagnostic() -> Diagnostic {
    let mut diagnostics = compile_test_function(RANGE_ID, RANGE_SOURCE, "main")
        .expect_err("invalid literal")
        .into_semantic_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    diagnostics.remove(0)
}

#[test]
#[should_panic(expected = "UI case requires exactly one diagnostic")]
fn compiler_ui_rejects_extra_diagnostic() {
    let diagnostic = range_diagnostic();
    assert_ui_golden(
        &[diagnostic.clone(), diagnostic],
        RANGE_ID,
        RANGE_SOURCE,
        "literal_range.vela",
        RANGE_EXPECTED,
    );
}

#[test]
#[should_panic(expected = "invalid UI diagnostic span")]
fn compiler_ui_rejects_out_of_bounds_span() {
    let mut diagnostic = range_diagnostic();
    diagnostic.span = Some(Span::new(RANGE_ID, 18, 999));
    assert_ui_golden(
        &[diagnostic],
        RANGE_ID,
        RANGE_SOURCE,
        "literal_range.vela",
        RANGE_EXPECTED,
    );
}

#[test]
#[should_panic(expected = "UI golden mismatch")]
fn compiler_ui_rejects_wrong_in_bounds_span() {
    let mut diagnostic = range_diagnostic();
    diagnostic.span = Some(Span::new(RANGE_ID, 17, 23));
    assert_ui_golden(
        &[diagnostic],
        RANGE_ID,
        RANGE_SOURCE,
        "literal_range.vela",
        RANGE_EXPECTED,
    );
}

#[test]
#[should_panic(expected = "UI golden mismatch")]
fn compiler_ui_rejects_wrong_expected_message() {
    assert!(RANGE_EXPECTED.contains("out of range"));
    let wrong = RANGE_EXPECTED.replacen("out of range", "accepted", 1);
    assert_ui_golden(
        &[range_diagnostic()],
        RANGE_ID,
        RANGE_SOURCE,
        "literal_range.vela",
        &wrong,
    );
}
