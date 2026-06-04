use vela_common::SourceId;
use vela_common::diagnostic_render::{DiagnosticSource, render_diagnostic};
use vela_syntax::parser::parse_source;

const GENERIC_TYPE_HINT: &str =
    include_str!("../../../tests/fixtures/diagnostics/generic_type_hint.vela");
const GENERIC_TYPE_HINT_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/generic_type_hint.expected");

#[test]
fn parser_generic_type_hint_fixture_renders_span_and_hint() {
    let source = normalized_fixture(GENERIC_TYPE_HINT);
    let parsed = parse_source(SourceId::new(1), &source);

    assert_eq!(parsed.diagnostics.len(), 1);
    let rendered = render_diagnostic(
        &parsed.diagnostics[0],
        [diagnostic_source("generic_type_hint.vela", source)],
    )
    .join("\n");

    assert_rendered_eq(&rendered, GENERIC_TYPE_HINT_EXPECTED);
}

fn diagnostic_source(name: &str, source: String) -> DiagnosticSource {
    DiagnosticSource::new(SourceId::new(1), name, source)
}

fn normalized_fixture(source: &str) -> String {
    source.replace("\r\n", "\n")
}

fn assert_rendered_eq(rendered: &str, expected: &str) {
    assert_eq!(rendered.trim_end(), normalized_fixture(expected).trim_end());
}
