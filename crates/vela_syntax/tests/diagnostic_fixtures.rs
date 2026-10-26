use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use vela_common::diagnostic_render::{DiagnosticRenderer, DiagnosticSource, render_diagnostic};
use vela_common::{Diagnostic, SourceId, Span};
use vela_syntax::ast::AstNode;
use vela_syntax::parse::parse_source_with_id;

const GENERIC_TYPE_HINT: &str =
    include_str!("../../../tests/fixtures/diagnostics/generic_type_hint.vela");
const GENERIC_TYPE_HINT_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/generic_type_hint.expected");

#[test]
fn parser_generic_type_hint_fixture_renders_span_and_hint() {
    // Preserve this existing shared fixture's local CRLF/trailing-whitespace policy.
    let source = normalized_fixture(GENERIC_TYPE_HINT);
    let parsed = parse_source_with_id(SourceId::new(1), &source);
    let span = Span::new(SourceId::new(1), 17, 18);
    let expected = Diagnostic::error(
        "only builtin container, Option, and Result type hints support type arguments",
    )
    .with_code("syntax::generic_type_hint")
    .with_span(span)
    .with_label(
        span,
        "use a builtin parameterized type hint or remove these type arguments",
    )
    .with_repair(
        "Remove unsupported type arguments",
        Span::new(SourceId::new(1), 17, 22),
        "",
    );
    assert_diagnostics(&source, parsed.diagnostics(), &[expected]);
    assert_eq!(parsed.syntax_node().to_string(), source);
    let rendered = render_diagnostic(
        &parsed.diagnostics()[0],
        [DiagnosticSource::new(
            SourceId::new(1),
            "generic_type_hint.vela",
            source,
        )],
    )
    .join("\n");

    assert_rendered_eq(&rendered, GENERIC_TYPE_HINT_EXPECTED);
}

#[rstest::rstest]
fn parser_ui_diagnostics(#[files("tests/fixtures/ui/parser/*.vela")] path: PathBuf) {
    let source = std::fs::read_to_string(&path).expect("read UTF-8 parser source");
    let expected =
        std::fs::read_to_string(path.with_extension("expected")).expect("read UTF-8 parser golden");
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("case stem");
    let parsed = parse_source_with_id(SourceId::new(1), &source);
    assert_eq!(parsed.syntax_node().to_string(), source, "lossless {stem}");
    assert_diagnostics(
        &source,
        parsed.diagnostics(),
        &expected_parser_diagnostics(stem),
    );

    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .expect("logical name");
    let renderer = DiagnosticRenderer::new([DiagnosticSource::new(SourceId::new(1), name, source)]);
    let mut rendered = parsed
        .diagnostics()
        .iter()
        .flat_map(|diagnostic| renderer.render(diagnostic))
        .collect::<Vec<_>>()
        .join("\n");
    // New UI-P goldens compare every byte, including exactly one final LF.
    rendered.push('\n');
    assert_eq!(
        rendered, expected,
        "complete parser diagnostic output for {stem}"
    );
}

fn expected_parser_diagnostics(stem: &str) -> Vec<Diagnostic> {
    let source = SourceId::new(1);
    match stem {
        "missing_function_name" => vec![
            Diagnostic::error("expected function name")
                .with_code("E_PARSE")
                .with_span(Span::new(source, 3, 4)),
        ],
        "recovered_declaration" => vec![
            Diagnostic::error("expected const initializer")
                .with_code("E_PARSE")
                .with_span(Span::new(source, 0, 21)),
        ],
        "removed_async" => [Span::new(source, 4, 9), Span::new(source, 28, 33)]
            .into_iter()
            .map(|span| {
                Diagnostic::error("script async and await are no longer supported")
                    .with_code("syntax::removed_async")
                    .with_span(span)
            })
            .collect(),
        _ => panic!("parser fixture has no typed diagnostic expectations: {stem}"),
    }
}

fn assert_diagnostics(source: &str, actual: &[Diagnostic], expected: &[Diagnostic]) {
    assert!(
        !expected.is_empty(),
        "error fixture must expect diagnostics"
    );
    assert_eq!(actual.len(), expected.len(), "diagnostic count");
    for diagnostic in actual {
        let spans = std::iter::once(diagnostic.span.expect("primary diagnostic span"))
            .chain(diagnostic.labels.iter().map(|label| label.span))
            .chain(diagnostic.repairs.iter().map(|repair| repair.span));
        for span in spans {
            assert_eq!(span.source, SourceId::new(1));
            assert!(
                span.start <= span.end && span.end as usize <= source.len(),
                "{span:?}"
            );
            assert!(source.is_char_boundary(span.start as usize));
            assert!(source.is_char_boundary(span.end as usize));
        }
    }
    // Full typed equality pins ordering, severity, code, message, spans, labels and repairs.
    assert_eq!(actual, expected, "typed parser diagnostics");
}

#[test]
fn parser_ui_fixture_inventory() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ui/parser");
    let mut sources = BTreeSet::new();
    let mut goldens = BTreeSet::new();
    for entry in std::fs::read_dir(directory).expect("read parser corpus") {
        let entry = entry.expect("read parser fixture entry");
        assert!(entry.file_type().expect("fixture file type").is_file());
        let path = entry.path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("UTF-8 case stem");
        assert!(!stem.is_empty(), "empty parser case stem");
        match path.extension().and_then(|s| s.to_str()) {
            Some("vela") => {
                sources.insert(stem.to_owned());
            }
            Some("expected") => {
                goldens.insert(stem.to_owned());
            }
            _ => panic!("unexpected parser fixture: {}", path.display()),
        }
    }
    assert!(!sources.is_empty(), "empty parser corpus");
    assert_eq!(sources, goldens, "missing/orphan parser goldens");
    for stem in sources {
        assert!(!expected_parser_diagnostics(&stem).is_empty());
    }
    // This named shared case is intentionally outside the owner-local discovery glob.
    let shared = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/diagnostics");
    for name in ["generic_type_hint.vela", "generic_type_hint.expected"] {
        assert!(
            shared.join(name).is_file(),
            "missing shared parser fixture: {name}"
        );
    }
}

#[rstest::rstest]
#[case::missing_function_name("fn named() {}\nfn grant(amount: i64) -> i64 { return amount; }\n", &["named", "grant"], &[])]
#[case::recovered_declaration("const Broken: i64 = 1;\nfn after() { return 1; }\n", &["after"], &["Broken"])]
#[case::removed_async("pub fn run() { load(); }\nfn after() { return 1; }\n", &["run", "after"], &[])]
#[case::builtin_type_hint("fn bad(xs: Array<i64>) { return xs; }", &["bad"], &[])]
fn parser_ui_valid_controls(
    #[case] source: &str,
    #[case] names: &[&str],
    #[case] const_names: &[&str],
) {
    let parsed = parse_source_with_id(SourceId::new(1), source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    assert_eq!(parsed.syntax_node().to_string(), source);
    assert_eq!(
        parsed
            .tree()
            .functions()
            .map(|function| function.name_text().expect("function name"))
            .collect::<Vec<_>>(),
        names
    );
    assert!(
        parsed
            .tree()
            .functions()
            .all(|function| function.body().is_some())
    );
    assert_eq!(
        parsed
            .tree()
            .consts()
            .map(|item| item.name_text().expect("const name"))
            .collect::<Vec<_>>(),
        const_names
    );
    assert!(parsed.tree().consts().all(|item| item.value().is_some()));
}

#[test]
fn parser_ui_recovery_preserves_declarations_and_healthy_neighbor() {
    let source = include_str!("fixtures/ui/parser/recovered_declaration.vela");
    let parsed = parse_source_with_id(SourceId::new(1), source);
    assert_diagnostics(
        source,
        parsed.diagnostics(),
        &expected_parser_diagnostics("recovered_declaration"),
    );
    let tree = parsed.tree();
    assert_eq!(tree.items().count(), 2);
    let item = tree.consts().next().expect("recovered const");
    assert_eq!(item.name_text().as_deref(), Some("Broken"));
    assert!(item.value().is_none());
    assert_eq!(
        item.type_hint()
            .expect("retained type")
            .syntax()
            .to_string(),
        "i64"
    );
    let function = tree.functions().next().expect("healthy neighbor");
    assert_eq!(function.name_text().as_deref(), Some("after"));
    assert_eq!(
        function.body().expect("neighbor body").syntax().to_string(),
        "{ return 1; }"
    );
}

fn normalized_fixture(source: &str) -> String {
    source.replace("\r\n", "\n")
}

fn assert_rendered_eq(rendered: &str, expected: &str) {
    assert_eq!(rendered.trim_end(), normalized_fixture(expected).trim_end());
}
