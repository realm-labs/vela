use std::sync::Arc;

use vela_bytecode::compiler::compile_program_source;
use vela_bytecode::{
    CacheSiteKind, Constant, InstructionOffset, LinkedProgram, Linker, Register,
    UnlinkedCodeObject, UnlinkedInstruction, UnlinkedInstructionKind, UnlinkedProgram,
};
use vela_common::{HostObjectId, HostTypeId, SourceId, Span};
use vela_def::{FieldId, TypeId};
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::resolved::HostMutationOp;
use vela_host::target::HostTargetPlan;
use vela_host::value::HostValue;
use vela_reflect::access::FieldAccess;
use vela_reflect::permissions::{ReflectPermission, ReflectPermissionSet};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey, TypeRegistry};
use vela_vm::budget::ExecutionBudget;
use vela_vm::owned_value::OwnedValue;
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
const HOST_COMPOUND_WRITE_DENIED: &str =
    include_str!("../../../tests/fixtures/diagnostics/host_compound_write_denied.vela");
const HOST_COMPOUND_WRITE_DENIED_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/host_compound_write_denied.expected");
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
fn host_permission_denied_fixture_renders_source_span() {
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let level_field = FieldId::new(1);
    let level_path = HostPath::new(host_ref).field(level_field);
    let source_span = Span::new(SourceId::new(1), 29, 41);

    let mut code = UnlinkedCodeObject::new("main", 2).with_params(vec!["player".to_owned()]);
    let target = code.intern_host_target(HostTargetPlan::new(host_ref.type_id).field(level_field));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    code.push_instruction(
        UnlinkedInstruction::new(UnlinkedInstructionKind::HostRead {
            dst: Register(1),
            root: Register(0),
            target,
            dynamic_args: Vec::new(),
            cache_site,
        })
        .with_span(source_span),
    );
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));

    let mut program = UnlinkedProgram::new();
    program.insert_function(code);

    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(
        level_path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    adapter.deny_diagnostic_path_read(level_path);
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    let linked = link_fixture_program(&program);
    let mut budget = ExecutionBudget::unbounded();
    let error = Vm::new()
        .run_linked_program_with_host_budget_and_caches(
            &linked,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
            &mut budget,
            None,
        )
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
fn host_compound_write_denied_fixture_renders_source_span() {
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let level_field = FieldId::new(1);
    let level_path = HostPath::new(host_ref).field(level_field);
    let source_span = Span::new(SourceId::new(1), 22, 34);

    let mut code = UnlinkedCodeObject::new("main", 2).with_params(vec!["player".to_owned()]);
    let one = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(1)));
    let target = code.intern_host_target(HostTargetPlan::new(host_ref.type_id).field(level_field));
    let mutate_cache = code.push_cache_site(CacheSiteKind::HostPathMutate, InstructionOffset(1));
    let read_cache = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(2));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: one,
        },
    ));
    code.push_instruction(
        UnlinkedInstruction::new(UnlinkedInstructionKind::HostMutate {
            root: Register(0),
            target,
            dynamic_args: Vec::new(),
            op: HostMutationOp::Add,
            rhs: Register(1),
            cache_site: mutate_cache,
        })
        .with_span(source_span),
    );
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostRead {
            dst: Register(1),
            root: Register(0),
            target,
            dynamic_args: Vec::new(),
            cache_site: read_cache,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));

    let mut program = UnlinkedProgram::new();
    program.insert_function(code);

    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(
        level_path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    adapter.deny_diagnostic_path_write(level_path.clone());
    let mut tx = HostAccess::new();
    let error = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        let linked = link_fixture_program(&program);
        let mut budget = ExecutionBudget::unbounded();
        Vm::new()
            .run_linked_program_with_host_budget_and_caches(
                &linked,
                "main",
                &[OwnedValue::HostRef(host_ref)],
                &mut host,
                &mut budget,
                None,
            )
            .expect_err("fixture should fail on immediate host write")
    };

    let rendered = render_diagnostic(
        &error.to_diagnostic(),
        [diagnostic_source(
            "host_compound_write_denied.vela",
            normalized_fixture(HOST_COMPOUND_WRITE_DENIED),
        )],
    )
    .join("\n");

    assert_rendered_eq(&rendered, HOST_COMPOUND_WRITE_DENIED_EXPECTED);
}

#[test]
fn stale_host_ref_fixture_renders_source_span() {
    let stale_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let fresh_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 2);
    let level_field = FieldId::new(1);
    let level_path = HostPath::new(fresh_ref).field(level_field);
    let source_span = Span::new(SourceId::new(1), 29, 41);

    let mut code = UnlinkedCodeObject::new("main", 2).with_params(vec!["player".to_owned()]);
    let target = code.intern_host_target(HostTargetPlan::new(stale_ref.type_id).field(level_field));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    code.push_instruction(
        UnlinkedInstruction::new(UnlinkedInstructionKind::HostRead {
            dst: Register(1),
            root: Register(0),
            target,
            dynamic_args: Vec::new(),
            cache_site,
        })
        .with_span(source_span),
    );
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));

    let mut program = UnlinkedProgram::new();
    program.insert_function(code);

    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(
        level_path,
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    let linked = link_fixture_program(&program);
    let mut budget = ExecutionBudget::unbounded();
    let error = Vm::new()
        .run_linked_program_with_host_budget_and_caches(
            &linked,
            "main",
            &[OwnedValue::HostRef(stale_ref)],
            &mut host,
            &mut budget,
            None,
        )
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
    adapter.insert_diagnostic_path_value(
        level_path,
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(registry),
        ReflectPermissionSet::read_only().with(ReflectPermission::InspectHostPath),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    let linked = link_fixture_program_with_vm(&program, &vm);
    let mut budget = ExecutionBudget::unbounded();
    let error = vm
        .run_linked_program_with_host_budget_and_caches(
            &linked,
            "main",
            &[OwnedValue::HostRef(host_ref)],
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

fn link_fixture_program(program: &UnlinkedProgram) -> LinkedProgram {
    Linker::new()
        .link_program(program)
        .expect("diagnostic fixture program should link")
}

fn link_fixture_program_with_vm(program: &UnlinkedProgram, vm: &Vm) -> LinkedProgram {
    let mut linker = Linker::new();
    for id in vm.native_implementation_ids() {
        linker.add_native_implementation(id);
    }
    linker
        .link_program(program)
        .expect("diagnostic fixture program should link")
}

fn normalized_fixture(source: &str) -> String {
    source.replace("\r\n", "\n")
}

fn assert_rendered_eq(rendered: &str, expected: &str) {
    assert_eq!(rendered.trim_end(), normalized_fixture(expected).trim_end());
}
