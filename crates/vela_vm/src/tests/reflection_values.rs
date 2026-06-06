use super::*;
use crate::owned_value::OwnedValue;
use crate::value::Value as RuntimeValue;

#[test]
fn compiled_source_reflects_script_record_implements() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Player { level: int }

fn main() {
    let player = Player { level: 7 };
    if reflect::name(reflect::type_of(player)) == "Player" && reflect::implements(player, "Damageable") {
        return reflect::get(player, "level") + reflect::fields(player).len();
    }
    return 0;
}
"#,
    )
    .expect("compile script record reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(OwnedValue::Int(8))
    );
}

#[test]
fn compiled_source_reflect_set_returns_updated_script_record() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Player { level: int, name: string }

fn main() {
    let player = Player { level: 7, name: "hero" };
    let updated = reflect::set(player, "level", 10);
    if reflect::get(player, "level") == 7
        && reflect::get(updated, "level") == 10
        && reflect::name(updated) == "Player"
        && updated.name == "hero" {
        return 1;
    }
    return 0;
}
"#,
    )
    .expect("compile script record reflect set source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(OwnedValue::Int(1))
    );
    assert!(tx.is_empty());
}

#[test]
fn compiled_source_reflect_set_rejects_metadata_records() {
    let program = compile_program_source(
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
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::InvalidTarget)
    ));
    assert!(tx.is_empty());
}

#[test]
fn compiled_source_reflect_get_script_record_unknown_field_reports_schema() {
    let program = compile_program_source(
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
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::UnknownField {
            type_name: "Player".to_owned(),
            field: "leve".to_owned(),
            candidates: vec!["level".to_owned()],
            related: vec![ReflectCandidate::new("level", None)],
        })
    ));
    assert!(tx.is_empty());
}

#[test]
fn compiled_source_reflect_get_script_record_respects_field_permission() {
    let program = compile_program_source(
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
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(registry),
        reflect::permissions::ReflectPolicy::all(),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::FieldPermissionDenied {
            type_name: "Player".to_owned(),
            field: "level".to_owned(),
            permission: "player.level.inspect".to_owned(),
            source_span: None,
        })
    ));
    assert!(tx.is_empty());
}

#[test]
fn heap_execution_reflection_fields_returns_heap_metadata_records() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let fields = reflect::fields(player);
    return fields.len() == 2
        && fields[0].owner == "Player"
        && fields[0].name == "id"
        && fields[1].owner == "Player"
        && fields[1].name == "level";
}
"#,
    )
    .expect("compile reflection fields source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 8192, usize::MAX, usize::MAX);

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        vm.run_program_runtime_with_host_heap_and_budget(
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
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Player { level: int }

fn main() {
    let player = Player { level: 7 };
    if reflect::name(reflect::type_of(player)) == "Player" && reflect::implements(player, "Damageable") {
        return reflect::get(player, "level") + reflect::fields(player).len();
    }
    return 0;
}
"#,
    )
    .expect("compile heap script record reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_reflection_registry()));
    let mut budget = ExecutionBudget::unbounded();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        vm.run_program_with_host_managed_heap_and_budget(
            &program,
            "main",
            &[],
            &mut host,
            &mut budget,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(8)));
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
    if reflect::name(reflect::type_of(player)) == "game::Player" && reflect::implements(player, "game::Damageable") {
        return player.damage() + reflect::fields(player).len();
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
    let program = compile_module_sources(&sources).expect("compile script trait module");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(registry));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "game::main", &[], &mut host),
        Ok(OwnedValue::Int(8))
    );
}

#[test]
fn compiled_source_reflect_call_counts_host_method_mutation() {
    let host_ref = player_ref(3);
    let method = HostMethodId::new(5);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect::call(player, "grant_exp", 20);
    return 1;
}
"#,
    )
    .expect("compile reflection call source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::Int(12));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        vm.run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(1)));
    assert_eq!(tx.mutation_count(), 1);
    assert_eq!(
        adapter.method_calls(),
        &[(HostPath::new(host_ref), method, vec![HostValue::Int(20)])]
    );
}
