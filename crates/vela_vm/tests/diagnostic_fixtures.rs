use vela_bytecode::compiler::compile_program_source;
use vela_common::SourceId;
use vela_common::diagnostic_render::{DiagnosticSource, render_diagnostic};
use vela_vm::Vm;

const RUNTIME_DIVISION_BY_ZERO: &str =
    include_str!("../../../tests/fixtures/diagnostics/runtime_division_by_zero.vela");
const RUNTIME_DIVISION_BY_ZERO_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/runtime_division_by_zero.expected");

#[test]
fn runtime_division_by_zero_fixture_renders_source_span_and_call_stack() {
    let program = compile_program_source(SourceId::new(1), RUNTIME_DIVISION_BY_ZERO)
        .expect("runtime diagnostic fixture should compile");
    let error = Vm::new()
        .run_program(&program, "main", &[])
        .expect_err("fixture should fail at runtime");

    let rendered = render_diagnostic(
        &error.to_diagnostic(),
        [DiagnosticSource::new(
            SourceId::new(1),
            "runtime_division_by_zero.vela",
            RUNTIME_DIVISION_BY_ZERO,
        )],
    )
    .join("\n");

    assert_eq!(
        rendered.trim_end(),
        RUNTIME_DIVISION_BY_ZERO_EXPECTED.trim_end()
    );
}
