use super::*;
use crate::owned_value::OwnedValue;
use crate::value::Value as RuntimeValue;

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
) -> vela_bytecode::compiler::error::CompileResult<UnlinkedProgram> {
    compile_standard_program_source_with_native_functions(source, text, reflection_value_natives())
}

fn compile_reflection_value_module_sources(
    sources: &[ModuleSource],
) -> vela_bytecode::compiler::error::CompileResult<UnlinkedProgram> {
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
    vela_bytecode::compiler::compile_module_sources_with_registry(sources, registry.compile_view())
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

fn exec_reflection_value_runtime(
    vm: &Vm,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[RuntimeValue],
    host: &mut HostExecution<'_>,
    heap: &mut HeapExecution<'_>,
    budget: &mut ExecutionBudget,
) -> VmResult<RuntimeValue> {
    run_linked_test_program_runtime_with_host_heap_and_budget(
        vm, program, entry, args, host, heap, budget,
    )
}

#[test]
fn compiled_source_reflects_script_record_implements() {
    let program = compile_reflection_value_source(
        SourceId::new(1),
        r#"
struct Player { level: int }

fn main() {
    let player = Player { level: 7 };
    let fields = reflect::fields(player);
    if reflect::name(reflect::type_of(player)) == "Player" && reflect::implements(player, "Damageable") {
        if reflect::get(fields[0], "name") == "level" {
            return reflect::get(player, "level") + 1;
        }
    }
    return 0;
}
"#,
    )
    .expect("compile script record reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

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
    return reflect::name(reflect::type_of(|value| value)) == "closure";
}
"#,
    )
    .expect("compile closure reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    let mut registry = script_reflection_registry();
    registry
        .register(TypeDesc::new(TypeKey::new(TypeId::new(201), "closure")).kind(TypeKind::Closure));
    vm.register_reflection_natives(Arc::new(registry));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        exec_reflection_value_program(&vm, &program, "main", &[], &mut host),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn compiled_source_reflect_set_returns_updated_script_record() {
    let program = compile_reflection_value_source(
        SourceId::new(1),
        r#"
struct Player { level: int, name: string }

fn main() {
    let player = Player { level: 7, name: "hero" };
    let updated = reflect::set(player, "level", 10);
    if reflect::get(player, "level") == 7
        && reflect::get(updated, "level") == 10
        && reflect::name(updated) == "Player"
        && reflect::get(updated, "name") == "hero" {
        return 1;
    }
    return 0;
}
"#,
    )
    .expect("compile script record reflect set source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

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
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

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
struct Player { level: int }

fn main() {
    let player = Player { level: 7 };
    return reflect::get(player, "leve");
}
"#,
    )
    .expect("compile script record unknown field source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

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
struct Player { level: int }

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
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(registry),
        reflect::permissions::ReflectPolicy::all(),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

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
fn heap_execution_reflection_fields_returns_heap_metadata_records() {
    let host_ref = player_ref(3);
    let program = compile_reflection_value_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let fields = reflect::fields(player);
    return reflect::get(fields[0], "owner") == "Player"
        && reflect::get(fields[0], "name") == "id"
        && reflect::get(fields[1], "owner") == "Player"
        && reflect::get(fields[1], "name") == "level";
}
"#,
    )
    .expect("compile reflection fields source");
    let mut adapter = host_adapter(
        host_ref,
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 8192, usize::MAX);

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        exec_reflection_value_runtime(
            &vm,
            &program,
            "main",
            &[RuntimeValue::HostRef(host_ref)],
            &mut host,
            &mut heap_execution,
            &mut budget,
        )
    }
    .expect("run heap reflection fields");

    assert_eq!(result, RuntimeValue::Bool(true));
}

#[test]
fn heap_execution_reflects_script_record_implements() {
    let program = compile_reflection_value_source(
        SourceId::new(1),
        r#"
struct Player { level: int }

fn main() {
    let player = Player { level: 7 };
    let fields = reflect::fields(player);
    if reflect::name(reflect::type_of(player)) == "Player" && reflect::implements(player, "Damageable") {
        if reflect::get(fields[0], "name") == "level" {
            return reflect::get(player, "level") + 1;
        }
    }
    return 0;
}
"#,
    )
    .expect("compile heap script record reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_reflection_registry()));
    let mut budget = ExecutionBudget::unbounded();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
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
        ModulePath::from_qualified("game"),
        r#"
trait Damageable {
    fn damage(self) -> int { return self.level; }
}
struct Player { level: int }

impl Damageable for Player {}

pub fn main() {
    let player = Player { level: 7 };
    let fields = reflect::fields(player);
    if reflect::name(reflect::type_of(player)) == "game::Player" && reflect::implements(player, "game::Damageable") {
        if reflect::get(fields[0], "name") == "level" {
            return player.damage() + 1;
        }
    }
    return 0;
}
"#,
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
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(registry));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        exec_reflection_value_program(&vm, &program, "game::main", &[], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(8)))
    );
}

#[test]
fn compiled_source_reflect_call_calls_host_method() {
    let host_ref = player_ref(3);
    let method = HostMethodId::new(5);
    let program = compile_reflection_value_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect::call(player, "grant_exp", 20);
    return 1;
}
"#,
    )
    .expect("compile reflection call source");
    let mut adapter = host_adapter(
        host_ref,
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    adapter.insert_method_return(method, HostValue::Scalar(vela_common::ScalarValue::I64(12)));
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        exec_reflection_value_program(
            &vm,
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(
        result,
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
    assert_eq!(
        adapter.method_calls(),
        &[(
            HostPath::new(host_ref),
            method,
            vec![HostValue::Scalar(vela_common::ScalarValue::I64(20))]
        )]
    );
}
