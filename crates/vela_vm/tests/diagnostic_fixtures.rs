use std::sync::Arc;

use crate::test_compile_support::compile_test_program;
use vela_bytecode::{Linker, UnlinkedProgram};
use vela_common::SourceId;
use vela_def::{FieldId, TypeId};

use vela_reflect::access::FieldAccess;
use vela_reflect::permissions::ReflectPermissionSet;
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey, TypeRegistry};
use vela_vm::budget::ExecutionBudget;
use vela_vm::owned_value::OwnedValue;
use vela_vm::{HostExecution, Vm};

use vela_common::diagnostic_render::{DiagnosticSource, render_diagnostic};

const RUNTIME_DIVISION_BY_ZERO: &str =
    include_str!("../../../tests/fixtures/diagnostics/runtime_division_by_zero.vela");
const RUNTIME_DIVISION_BY_ZERO_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/runtime_division_by_zero.expected");
const REFLECTION_UNKNOWN_FIELD: &str =
    include_str!("../../../tests/fixtures/diagnostics/reflection_unknown_field.vela");
const REFLECTION_UNKNOWN_FIELD_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/reflection_unknown_field.expected");

#[test]
fn runtime_division_by_zero_fixture_renders_source_span_and_call_stack() {
    let source = normalized_fixture(RUNTIME_DIVISION_BY_ZERO);
    let program = compile_test_program(SourceId::new(1), &source)
        .expect("runtime diagnostic fixture should compile");
    let linked = link_fixture_program(&program);
    let error = Vm::new()
        .run_linked_program(&linked, "main", &[])
        .expect_err("fixture should fail at runtime");

    let rendered = render_diagnostic(
        &error.to_diagnostic(),
        [diagnostic_source("runtime_division_by_zero.vela", source)],
    )
    .join("\n");

    assert_rendered_eq(&rendered, RUNTIME_DIVISION_BY_ZERO_EXPECTED);
}

#[test]
fn reflection_unknown_field_fixture_renders_candidates_and_source_span() {
    let level_field = FieldId::new(1);

    let source = normalized_fixture(REFLECTION_UNKNOWN_FIELD);
    let program = compile_test_program(SourceId::new(1), &source)
        .expect("reflection diagnostic fixture should compile");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .kind(vela_reflect::registry::TypeKind::ScriptStruct)
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

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(registry),
        ReflectPermissionSet::read_only(),
    );
    let mut host = HostExecution { state_values: None };

    let linked = link_fixture_program_with_vm(&program, &vm);
    let mut budget = ExecutionBudget::unbounded();
    let error = vm
        .run_linked_program_with_host_budget_and_caches(
            &linked,
            "main",
            &[OwnedValue::record(
                "Player",
                [("level", OwnedValue::i64(9))],
            )],
            &mut host,
            &mut budget,
            None,
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

fn link_fixture_program(
    program: &UnlinkedProgram,
) -> std::sync::Arc<vela_bytecode::LinkedArtifact> {
    Linker::new()
        .link_test_program(program)
        .expect("diagnostic fixture program should link")
}

fn link_fixture_program_with_vm(
    program: &UnlinkedProgram,
    vm: &Vm,
) -> std::sync::Arc<vela_bytecode::LinkedArtifact> {
    let mut linker = Linker::new();
    for id in vm.native_implementation_ids() {
        linker.add_native_implementation(id);
    }
    linker
        .link_test_program(program)
        .expect("diagnostic fixture program should link")
}

fn normalized_fixture(source: &str) -> String {
    source.replace("\r\n", "\n")
}

fn assert_rendered_eq(rendered: &str, expected: &str) {
    assert_eq!(rendered.trim_end(), normalized_fixture(expected).trim_end());
}
