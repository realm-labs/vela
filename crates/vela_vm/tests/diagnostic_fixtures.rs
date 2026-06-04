use vela_bytecode::compiler::compile_program_source;
use vela_bytecode::{CodeObject, Instruction, InstructionKind, Program, Register};
use vela_common::diagnostic_render::{DiagnosticSource, render_diagnostic};
use vela_common::{FieldId, HostObjectId, HostTypeId, SourceId, Span};
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_vm::value::Value;
use vela_vm::{HostExecution, Vm};

const RUNTIME_DIVISION_BY_ZERO: &str =
    include_str!("../../../tests/fixtures/diagnostics/runtime_division_by_zero.vela");
const RUNTIME_DIVISION_BY_ZERO_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/runtime_division_by_zero.expected");
const HOST_PERMISSION_DENIED: &str =
    include_str!("../../../tests/fixtures/diagnostics/host_permission_denied.vela");
const HOST_PERMISSION_DENIED_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/host_permission_denied.expected");

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

#[test]
fn host_permission_denied_fixture_renders_source_span() {
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let level_field = FieldId::new(1);
    let level_path = HostPath::new(host_ref).field(level_field);
    let source_span = Span::new(SourceId::new(1), 29, 41);

    let mut code = CodeObject::new("main", 2).with_params(vec!["player".to_owned()]);
    code.push_instruction(
        Instruction::new(InstructionKind::GetHostField {
            dst: Register(1),
            root: Register(0),
            field: level_field,
        })
        .with_span(source_span),
    );
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));

    let mut program = Program::new();
    program.insert_function(code);

    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(level_path.clone(), HostValue::Int(9));
    adapter.deny_read(level_path);
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = Vm::new()
        .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
        .expect_err("fixture should fail at the host boundary");

    let rendered = render_diagnostic(
        &error.to_diagnostic(),
        [DiagnosticSource::new(
            SourceId::new(1),
            "host_permission_denied.vela",
            HOST_PERMISSION_DENIED,
        )],
    )
    .join("\n");

    assert_eq!(
        rendered.trim_end(),
        HOST_PERMISSION_DENIED_EXPECTED.trim_end()
    );
}
