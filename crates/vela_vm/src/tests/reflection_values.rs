use super::*;
use crate::owned_value::OwnedValue;
use vela_reflect::value::{ReflectMapEntry, ReflectValue};

fn reflection_value_natives() -> &'static [&'static str] {
    &[
        "reflect::call",
        "reflect::fields",
        "reflect::get",
        "reflect::implements",
        "reflect::name",
        "reflect::set",
        "reflect::type_info",
        "reflect::type_of",
    ]
}

fn compile_reflection_value_source(
    source: SourceId,
    text: &str,
) -> vela_bytecode::compiler::error::CompileResult<vela_bytecode::compiler::CompiledProgram> {
    compile_standard_program_source_with_native_functions(source, text, reflection_value_natives())
}

fn compile_reflection_value_module_sources(
    sources: &[ModuleSource],
) -> vela_bytecode::compiler::error::CompileResult<vela_bytecode::compiler::CompiledProgram> {
    let mut registry = vela_stdlib::standard_registry().expect("standard registry should build");
    for native in reflection_value_natives() {
        let mut segments = native.split("::").collect::<Vec<_>>();
        let function = segments.pop().unwrap_or(native);
        registry
            .register_function(vela_registry::FunctionDef::new(
                vela_def::DefPath::function("host", segments, function),
                vela_registry::FunctionSignature::default(),
            ))
            .expect("test native function should register");
    }
    crate::test_support::compile_test_modules_with_registry(sources, registry.compile_view())
}

fn exec_reflection_value_program(
    vm: &Vm,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_test_program_with_host_budget(vm, program, entry, args, host, &mut budget)
}

fn exec_reflection_value_program_with_budget(
    vm: &Vm,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
    budget: &mut ExecutionBudget,
) -> VmResult<OwnedValue> {
    run_linked_test_program_with_host_budget(vm, program, entry, args, host, budget)
}

#[test]
fn reflection_execution_unit_limit_has_a_stable_edge() {
    let program = compile_reflection_value_source(
        SourceId::new(90),
        "fn main() { return reflect::name(reflect::type_info(\"Player\")); }",
    )
    .expect("compile reflection execution-unit fixture");
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_reflection_registry()));

    let mut host = HostExecution { state_values: None };
    let mut exact = ExecutionBudget::new(3, usize::MAX, usize::MAX);
    assert_eq!(
        exec_reflection_value_program_with_budget(
            &vm,
            &program,
            "main",
            &[],
            &mut host,
            &mut exact,
        ),
        Ok(OwnedValue::String("Player".to_owned()))
    );
    assert_eq!(exact.execution_units_consumed(), 3);

    let mut short = ExecutionBudget::new(2, usize::MAX, usize::MAX);
    let error = exec_reflection_value_program_with_budget(
        &vm,
        &program,
        "main",
        &[],
        &mut host,
        &mut short,
    )
    .expect_err("one fewer reflection execution unit traps at the second boundary");
    assert_eq!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::ExecutionUnits,
            limit: 2,
        }
    );
}

#[test]
fn reflect_conversion_preserves_owned_map_key_values() {
    let value = OwnedValue::map([
        (
            OwnedValue::Scalar(vela_common::ScalarValue::I64(1)),
            OwnedValue::from("one"),
        ),
        (OwnedValue::Bytes(vec![1, 2, 3]), OwnedValue::from("bytes")),
    ]);

    let reflected = value_to_reflect(&value, "test reflect map").expect("reflect owned map");

    assert_eq!(
        reflected,
        ReflectValue::Map(vec![
            ReflectMapEntry::new(
                ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(1))),
                ReflectValue::Host(HostValue::String("one".to_owned())),
            ),
            ReflectMapEntry::new(
                ReflectValue::Host(HostValue::Bytes(vec![1, 2, 3])),
                ReflectValue::Host(HostValue::String("bytes".to_owned())),
            ),
        ])
    );
    assert_eq!(value_from_reflect(reflected), Ok(value));
}

#[test]
fn runtime_reflection_preserves_map_key_values() {
    let value = OwnedValue::map([(1_i64, "one"), (2_i64, "two")]);
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let runtime =
        owned_to_value(value.clone(), &mut heap_execution, None).expect("materialize map");

    let reflected = runtime_value_to_reflect(&runtime, &heap_execution, "test runtime reflect map")
        .expect("reflect runtime map");

    assert_eq!(
        reflected,
        ReflectValue::Map(vec![
            ReflectMapEntry::new(
                ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(1))),
                ReflectValue::Host(HostValue::String("one".to_owned())),
            ),
            ReflectMapEntry::new(
                ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(2))),
                ReflectValue::Host(HostValue::String("two".to_owned())),
            ),
        ])
    );
    assert_eq!(value_from_reflect(reflected), Ok(value));
}

#[test]
fn compiled_source_reflects_script_record_implements() {
    let program = compile_reflection_value_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/script_trait.vela"),
    )
    .expect("compile script record reflection source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_value_program(&vm, &program, "main", &[], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(8)))
    );
}

#[test]
fn linked_reflect_type_of_classifies_closure_without_owned_materialization() {
    let program = compile_reflection_value_source(
        SourceId::new(1),
        r#"
fn main() {
    let missing_type = ReflectType { name: "", kind: "missing" };
    let closure_type = reflect::type_of(|value| value).unwrap_or(missing_type);
    return reflect::name(closure_type) == "closure";
}
"#,
    )
    .expect("compile closure reflection source");

    let mut vm = Vm::new();
    let mut registry = script_reflection_registry();
    registry
        .register(TypeDesc::new(TypeKey::new(TypeId::new(201), "closure")).kind(TypeKind::Closure));
    vm.register_reflection_natives(Arc::new(registry));
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_value_program(&vm, &program, "main", &[], &mut host),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn compiled_source_reflect_set_returns_updated_script_record() {
    let program = compile_reflection_value_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/updated_record.vela"),
    )
    .expect("compile script record reflect set source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_value_program(&vm, &program, "main", &[], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn compiled_source_reflect_set_rejects_metadata_records() {
    let program = compile_reflection_value_source(
        SourceId::new(1),
        r#"
fn main() {
    let player_type = reflect::type_info("Player");
    return reflect::set(player_type, "name", "Monster");
}
"#,
    )
    .expect("compile metadata reflect set source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert!(matches!(
        exec_reflection_value_program(&vm, &program, "main", &[], &mut host),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::InvalidTarget)
    ));
}

#[test]
fn compiled_source_reflect_get_script_record_unknown_field_reports_schema() {
    let program = compile_reflection_value_source(
        SourceId::new(1),
        r#"
struct Player { level: i64 }

fn main() {
    let player = Player { level: 7 };
    return reflect::get(player, "leve");
}
"#,
    )
    .expect("compile script record unknown field source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert!(matches!(
        exec_reflection_value_program(&vm, &program, "main", &[], &mut host),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::UnknownField {
            type_name: "Player".to_owned(),
            field: "leve".to_owned(),
            candidates: vec!["level".to_owned()],
            related: vec![ReflectCandidate::new("level", None)],
        })
    ));
}

#[test]
fn compiled_source_reflect_get_script_record_respects_field_permission() {
    let program = compile_reflection_value_source(
        SourceId::new(1),
        r#"
struct Player { level: i64 }

fn main() {
    let player = Player { level: 7 };
    return reflect::get(player, "level");
}
"#,
    )
    .expect("compile script record field permission source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(200), "Player"))
            .kind(TypeKind::ScriptStruct)
            .field(
                FieldDesc::new(FieldId::new(2), "level")
                    .access(FieldAccess::new().require_permission("player.level.inspect")),
            ),
    );

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(registry),
        reflect::permissions::ReflectPolicy::all(),
    );
    let mut host = HostExecution { state_values: None };

    assert!(matches!(
        exec_reflection_value_program(&vm, &program, "main", &[], &mut host),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::FieldPermissionDenied {
            type_name: "Player".to_owned(),
            field: "level".to_owned(),
            permission: "player.level.inspect".to_owned(),
            source_span: None,
        })
    ));
}

#[test]
fn heap_execution_reflects_script_record_implements() {
    let program = compile_reflection_value_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/script_trait.vela"),
    )
    .expect("compile heap script record reflection source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_reflection_registry()));
    let mut budget = ExecutionBudget::unbounded();

    let result = {
        let mut host = HostExecution { state_values: None };
        exec_reflection_value_program_with_budget(
            &vm,
            &program,
            "main",
            &[],
            &mut host,
            &mut budget,
        )
    };

    assert_eq!(
        result,
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(8)))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn compiled_module_reflects_registered_script_trait_impls() {
    let sources = [ModuleSource::new(
        SourceId::new(1),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("game"),
        include_str!("../../tests/fixtures/reflection/registered_script_trait.vela"),
    )];
    let mut graph = ModuleGraph::new();
    for source in &sources {
        graph.add_source(source.clone());
    }
    graph.resolve_imports();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let mut registry = TypeRegistry::new();
    registry.register_script_types(&graph);
    let program =
        compile_reflection_value_module_sources(&sources).expect("compile script trait module");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(registry));
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_value_program(&vm, &program, "game::main", &[], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(8)))
    );
}
