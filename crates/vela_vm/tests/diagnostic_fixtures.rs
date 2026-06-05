use std::sync::Arc;

use vela_bytecode::compiler::compile_program_source;
use vela_bytecode::{CodeObject, Constant, Instruction, InstructionKind, Program, Register};
use vela_common::{FieldId, HostObjectId, HostTypeId, SourceId, Span, TypeId};
use vela_host::adapter::ScriptStateAdapter;
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_reflect::access::FieldAccess;
use vela_reflect::permissions::{ReflectPermission, ReflectPermissionSet};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey, TypeRegistry};
use vela_vm::error::VmError;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;
use vela_vm::{HostExecution, Vm};

use vela_common::diagnostic_render::{DiagnosticSource, render_diagnostic};

const RUNTIME_DIVISION_BY_ZERO: &str =
    include_str!("../../../tests/fixtures/diagnostics/runtime_division_by_zero.vela");
const RUNTIME_DIVISION_BY_ZERO_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/runtime_division_by_zero.expected");
const HOST_PERMISSION_DENIED: &str =
    include_str!("../../../tests/fixtures/diagnostics/host_permission_denied.vela");
const HOST_PERMISSION_DENIED_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/host_permission_denied.expected");
const HOST_PATCH_CONFLICT: &str =
    include_str!("../../../tests/fixtures/diagnostics/host_patch_conflict.vela");
const HOST_PATCH_CONFLICT_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/host_patch_conflict.expected");
const STALE_HOST_REF: &str =
    include_str!("../../../tests/fixtures/diagnostics/stale_host_ref.vela");
const STALE_HOST_REF_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/stale_host_ref.expected");
const REFLECTION_UNKNOWN_FIELD: &str =
    include_str!("../../../tests/fixtures/diagnostics/reflection_unknown_field.vela");
const REFLECTION_UNKNOWN_FIELD_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/reflection_unknown_field.expected");

#[test]
fn runtime_division_by_zero_fixture_renders_source_span_and_call_stack() {
    let source = normalized_fixture(RUNTIME_DIVISION_BY_ZERO);
    let program = compile_program_source(SourceId::new(1), &source)
        .expect("runtime diagnostic fixture should compile");
    let error = Vm::new()
        .run_program_runtime(&program, "main", &[])
        .expect_err("fixture should fail at runtime");

    let rendered = render_diagnostic(
        &error.to_diagnostic(),
        [diagnostic_source("runtime_division_by_zero.vela", source)],
    )
    .join("\n");

    assert_rendered_eq(&rendered, RUNTIME_DIVISION_BY_ZERO_EXPECTED);
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
        .run_program_runtime_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
        .expect_err("fixture should fail at the host boundary");

    let rendered = render_diagnostic(
        &error.to_diagnostic(),
        [diagnostic_source(
            "host_permission_denied.vela",
            normalized_fixture(HOST_PERMISSION_DENIED),
        )],
    )
    .join("\n");

    assert_rendered_eq(&rendered, HOST_PERMISSION_DENIED_EXPECTED);
}

#[test]
fn host_patch_conflict_fixture_renders_apply_source_span() {
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let level_field = FieldId::new(1);
    let level_path = HostPath::new(host_ref).field(level_field);
    let source_span = Span::new(SourceId::new(1), 22, 34);

    let mut code = CodeObject::new("main", 2).with_params(vec!["player".to_owned()]);
    let one = code.push_constant(Constant::Int(1));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: one,
    }));
    code.push_instruction(
        Instruction::new(InstructionKind::AddHostField {
            root: Register(0),
            field: level_field,
            rhs: Register(1),
        })
        .with_span(source_span),
    );
    code.push_instruction(Instruction::new(InstructionKind::GetHostField {
        dst: Register(1),
        root: Register(0),
        field: level_field,
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));

    let mut program = Program::new();
    program.insert_function(code);

    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(level_path.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();
    {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new()
            .run_program_runtime_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
            .expect("fixture should record a host patch");
    }
    adapter
        .write_path(&level_path, HostValue::Int(12))
        .expect("simulate host state changing before apply");
    let error = tx
        .apply(&mut adapter)
        .map_err(VmError::from)
        .expect_err("changed host base value should conflict");

    let rendered = render_diagnostic(
        &error.to_diagnostic(),
        [diagnostic_source(
            "host_patch_conflict.vela",
            normalized_fixture(HOST_PATCH_CONFLICT),
        )],
    )
    .join("\n");

    assert_rendered_eq(&rendered, HOST_PATCH_CONFLICT_EXPECTED);
}

#[test]
fn stale_host_ref_fixture_renders_source_span() {
    let stale_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let fresh_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 2);
    let level_field = FieldId::new(1);
    let level_path = HostPath::new(fresh_ref).field(level_field);
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
    adapter.insert_value(level_path, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = Vm::new()
        .run_program_runtime_with_host(&program, "main", &[Value::HostRef(stale_ref)], &mut host)
        .expect_err("fixture should fail on stale host ref generation");

    let rendered = render_diagnostic(
        &error.to_diagnostic(),
        [diagnostic_source(
            "stale_host_ref.vela",
            normalized_fixture(STALE_HOST_REF),
        )],
    )
    .join("\n");

    assert_rendered_eq(&rendered, STALE_HOST_REF_EXPECTED);
}

#[test]
fn reflection_unknown_field_fixture_renders_candidates_and_source_span() {
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let level_field = FieldId::new(1);
    let level_path = HostPath::new(host_ref).field(level_field);

    let source = normalized_fixture(REFLECTION_UNKNOWN_FIELD);
    let program = compile_program_source(SourceId::new(1), &source)
        .expect("reflection diagnostic fixture should compile");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .field(FieldDesc::new(level_field, "level"))
            .field(
                FieldDesc::new(FieldId::new(2), "lever")
                    .access(FieldAccess::new().reflect_readable(false)),
            )
            .field(
                FieldDesc::new(FieldId::new(3), "leves")
                    .access(FieldAccess::new().require_permission("player.admin.inspect")),
            ),
    );

    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(level_path, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(registry),
        ReflectPermissionSet::read_only().with(ReflectPermission::InspectHostPath),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = vm
        .run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
        .expect_err("fixture should fail during reflection lookup");

    let rendered = render_diagnostic(
        &error.to_diagnostic(),
        [diagnostic_source("reflection_unknown_field.vela", source)],
    )
    .join("\n");

    assert_rendered_eq(&rendered, REFLECTION_UNKNOWN_FIELD_EXPECTED);
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
