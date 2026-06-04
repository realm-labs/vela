use vela_common::SourceId;
use vela_common::diagnostic_render::{DiagnosticSource, render_diagnostic};
use vela_syntax::parser::parse_source;

const GENERIC_TYPE_HINT: &str =
    include_str!("../../../tests/fixtures/diagnostics/generic_type_hint.vela");
const GENERIC_TYPE_HINT_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/generic_type_hint.expected");

#[test]
fn parser_generic_type_hint_fixture_renders_span_and_hint() {
    let parsed = parse_source(SourceId::new(1), GENERIC_TYPE_HINT);

    assert_eq!(parsed.diagnostics.len(), 1);
    let rendered = render_diagnostic(
        &parsed.diagnostics[0],
        [DiagnosticSource::new(
            SourceId::new(1),
            "generic_type_hint.vela",
            GENERIC_TYPE_HINT,
        )],
    )
    .join("\n");

    assert_eq!(rendered.trim_end(), GENERIC_TYPE_HINT_EXPECTED.trim_end());
}
