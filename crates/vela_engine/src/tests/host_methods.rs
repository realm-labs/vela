use vela_bytecode::UnlinkedProgram;
use vela_common::{HostMethodId, HostObjectId, HostTypeId, SourceId};
use vela_def::{DefPath, FieldId, TypeId, VariantId};
use vela_host::access::HostAccess;
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::value::HostValue;
use vela_reflect::registry::{
    FieldDesc, HostIndexCapability, MethodDesc, TypeDesc, TypeKey, VariantDesc,
};
use vela_vm::HostExecution;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::args::{HostArgType, TypedHostMut, TypedHostRef};
use crate::engine::Engine;
use crate::host_type::HostTypeSpec;
use crate::method::NativeMethodDesc;
use crate::native::{EffectSet, FunctionAccess, TypeHint};
use crate::permission::Capability;
use crate::runtime::{CallOptions, Runtime};

use super::player_type;

fn run_linked_program_with_host(
    engine: &Engine,
    program: &UnlinkedProgram,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let linked = engine
        .link_program(program)
        .expect("engine host method test program should link");
    let mut budget = ExecutionBudget::unbounded();
    engine
        .into_vm_for_program(program)
        .run_linked_program_with_host_budget_and_caches(
            &linked,
            "main",
            args,
            host,
            &mut budget,
            None,
        )
}

#[test]
fn runtime_call_writes_through_host_method_and_updates_adapter() {
    let method = HostMethodId::new(23);
    let engine = Engine::builder()
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(method, "grant_exp")),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player: Player) {
    player.grant_exp(12);
    return "done";
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    let result = runtime
        .call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect("runtime call should run");

    assert_eq!(result, OwnedValue::String("done".to_owned()));
    assert_eq!(adapter.method_calls().len(), 1);
    assert_eq!(
        adapter.method_calls()[0].diagnostic_path(),
        HostPath::new(host_ref)
    );
    assert_eq!(adapter.method_calls()[0].method, method);
    assert_eq!(
        adapter.method_calls()[0].args,
        vec![HostValue::Scalar(vela_common::ScalarValue::I64(12))]
    );
}

#[test]
fn engine_compiler_options_lower_registered_host_methods() {
    let method = HostMethodId::new(5);
    let engine = Engine::builder()
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(method, "grant_exp")),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player: Player) {
    player.grant_exp(10);
    return 1;
}
"#,
        )
        .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(
            &engine,
            &program,
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn engine_compiler_registry_resolves_registered_host_definitions() {
    let level = FieldId::new(3);
    let method = HostMethodId::new(5);
    let host_type = HostTypeId::new(1);
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(host_type)
                .field(FieldDesc::new(level, "level"))
                .method(MethodDesc::new(method, "grant_exp")),
        )
        .build()
        .expect("engine should build");

    let registry = engine.compiler_registry();
    let player = registry
        .resolve_type(&DefPath::ty("host", std::iter::empty::<&str>(), "Player"))
        .expect("host type should resolve from definition registry");
    let level_def = registry
        .resolve_host_field(player, "level")
        .expect("host field should resolve from definition registry");
    let method_def = registry
        .resolve_host_method(player, "grant_exp")
        .expect("host method should resolve from definition registry");

    assert_eq!(
        registry.type_host_runtime_id(player),
        Some(host_type.get().into())
    );
    assert_ne!(level_def, level);
    assert_eq!(registry.field_host_runtime_id(level_def), Some(level.get()));
    assert_eq!(registry.field_writable(level_def), Some(false));
    assert_eq!(
        registry.host_method_runtime_id(method_def),
        Some(method.get())
    );
}

#[test]
fn engine_compiler_options_lower_registered_host_field_methods() {
    let inventory = FieldId::new(3);
    let method = HostMethodId::new(5);
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .field(FieldDesc::new(inventory, "inventory").type_hint("Inventory")),
        )
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(2), "Inventory"))
                .host_type(HostTypeId::new(2))
                .method(MethodDesc::new(method, "add")),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player: Player) {
    player.inventory.add("gold", 20);
    return 1;
}
"#,
        )
        .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(
            &engine,
            &program,
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn engine_compiler_options_lower_registered_host_variant_fields() {
    let quest_progress = FieldId::new(3);
    let count = FieldId::new(4);
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .field(FieldDesc::new(quest_progress, "quest_progress").type_hint("QuestProgress")),
        )
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(2), "QuestProgress"))
                .host_type(HostTypeId::new(2))
                .variant(
                    VariantDesc::new(VariantId::new(1), "Active")
                        .field(FieldDesc::new(count, "count")),
                ),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player: Player) {
    player.quest_progress.count += 1;
    return player.quest_progress.count;
}
"#,
        )
        .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let quest_count = HostPath::new(host_ref)
        .field(quest_progress)
        .variant_field(count);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(
        quest_count.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(4)),
    );
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(
            &engine,
            &program,
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(5)))
    );
}

#[test]
fn engine_compiler_options_disambiguate_host_methods_by_receiver_type() {
    let player_method = HostMethodId::new(5);
    let monster_method = HostMethodId::new(6);
    let engine = Engine::builder()
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(player_method, "grant_exp")),
        )
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(2), "Monster"))
                .host_type(HostTypeId::new(2))
                .method(MethodDesc::new(monster_method, "grant_exp")),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player: Player, monster: Monster) {
    player.grant_exp(10);
    monster.grant_exp(3);
    return 1;
}
"#,
        )
        .expect("program should compile");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let monster = HostRef::new(HostTypeId::new(2), HostObjectId::new(7), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(
            &engine,
            &program,
            &[OwnedValue::HostRef(player), OwnedValue::HostRef(monster)],
            &mut host,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn engine_registers_callable_native_methods_for_host_paths() {
    let method = HostMethodId::new(6);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_native_method_fn(
            NativeMethodDesc::new(owner, method, "grant_exp")
                .param("amount", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public())
                .docs("Grant player experience.")
                .attr("domain", "gameplay")
                .attr("effect", "reward"),
            move |receiver, args, host| {
                let [OwnedValue::Scalar(vela_common::ScalarValue::I64(amount))] = args else {
                    return Ok(OwnedValue::Null);
                };
                host.access.call_diagnostic_path_method(
                    host.adapter,
                    receiver.clone(),
                    method,
                    vec![HostValue::Scalar(vela_common::ScalarValue::I64(*amount))],
                    None,
                )?;
                Ok(OwnedValue::Null)
            },
        )
        .build()
        .expect("engine should build");
    let registry = engine.registry();
    let reflected_method = registry
        .type_by_name("Player")
        .and_then(|desc| {
            desc.methods
                .iter()
                .find(|method| method.name == "grant_exp")
        })
        .expect("reflected native method metadata");
    assert_eq!(
        reflected_method.docs.as_deref(),
        Some("Grant player experience.")
    );
    assert_eq!(reflected_method.params.len(), 1);
    assert_eq!(reflected_method.params[0].name, "amount");
    assert_eq!(reflected_method.params[0].type_hint.as_deref(), Some("int"));
    assert_eq!(reflected_method.return_type.as_deref(), Some("null"));
    assert_eq!(reflected_method.attrs.get("domain"), Some("gameplay"));
    assert_eq!(reflected_method.attrs.get("effect"), Some("reward"));
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player: Player) {
    player.grant_exp(10);
    return 1;
}
"#,
        )
        .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(host_ref),
            &[OwnedValue::Scalar(vela_common::ScalarValue::I64(10))],
            &mut host,
        ),
        Ok(OwnedValue::Null)
    );

    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };
    assert_eq!(
        run_linked_program_with_host(
            &engine,
            &program,
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn engine_registers_typed_callable_native_methods_for_host_paths() {
    let method = HostMethodId::new(8);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_typed_native_method_fn::<(i64,), _>(
            NativeMethodDesc::new(owner, method, "typed_grant_exp")
                .param("amount", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_grant_exp,
        )
        .build()
        .expect("engine should build");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(host_ref),
            &[OwnedValue::Scalar(vela_common::ScalarValue::I64(15))],
            &mut host,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(15)))
    );
}

#[test]
fn typed_callable_native_method_conversion_errors_before_host_access() {
    let method = HostMethodId::new(8);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_typed_native_method_fn::<(i64,), _>(
            NativeMethodDesc::new(owner, method, "typed_grant_exp")
                .access(FunctionAccess::public()),
            typed_grant_exp,
        )
        .build()
        .expect("engine should build");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert!(matches!(
        engine.call_native_method(
            method,
            &HostPath::new(host_ref),
            &[OwnedValue::String("bad".to_owned())],
            &mut host,
        ),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { operation: "int" })
    ));
}

#[test]
fn typed_callable_native_method_maps_host_result_errors() {
    let method = HostMethodId::new(13);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_typed_native_method_fn::<(bool,), _>(
            NativeMethodDesc::new(owner, method, "typed_require_grant")
                .param("allowed", TypeHint::Bool)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_require_grant,
        )
        .build()
        .expect("engine should build");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let expected_path = HostPath::new(player);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        engine
            .call_native_method(
                method,
                &expected_path,
                &[OwnedValue::Bool(false)],
                &mut host
            )
            .map_err(|error| error.kind()),
        Err(VmErrorKind::Host(HostErrorKind::PermissionDenied {
            path: expected_path,
            action: "call",
        })),
    );
}

#[test]
fn callable_native_method_error_retains_written_mutation() {
    let method = HostMethodId::new(12);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_native_method_fn(
            NativeMethodDesc::new(owner, method, "failing_method")
                .param("amount", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            move |receiver, args, host| {
                let [OwnedValue::Scalar(vela_common::ScalarValue::I64(amount))] = args else {
                    return Ok(OwnedValue::Null);
                };
                host.access.call_diagnostic_path_method(
                    host.adapter,
                    receiver.clone(),
                    method,
                    vec![HostValue::Scalar(vela_common::ScalarValue::I64(*amount))],
                    None,
                )?;
                Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "failing native method",
                }))
            },
        )
        .build()
        .expect("engine should build");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    let error = engine
        .call_native_method(
            method,
            &HostPath::new(host_ref),
            &[OwnedValue::Scalar(vela_common::ScalarValue::I64(15))],
            &mut host,
        )
        .expect_err("native method should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "failing native method",
        }
    );
    assert_eq!(adapter.method_calls().len(), 1);
}

#[test]
fn engine_registers_unified_host_type_spec_with_native_method_and_index_metadata() {
    let method = HostMethodId::new(31);
    let owner = TypeKey::new(TypeId::new(31), "IntIntMap");
    let spec = HostTypeSpec::new(
        TypeDesc::new(owner.clone())
            .host_type(HostTypeId::new(31))
            .index_capability(
                HostIndexCapability::new()
                    .readable(true)
                    .writable(true)
                    .addable(true)
                    .removable(true)
                    .key_type("int")
                    .value_type("int"),
            ),
    )
    .native_method_fn(
        NativeMethodDesc::new(owner, method, "set")
            .param("key", TypeHint::Int)
            .param("value", TypeHint::Int)
            .returns(TypeHint::Null)
            .effects(EffectSet::host_write())
            .access(FunctionAccess::public()),
        move |receiver, args, host| {
            let [
                OwnedValue::Scalar(vela_common::ScalarValue::I64(key)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(value)),
            ] = args
            else {
                return Ok(OwnedValue::Null);
            };
            host.access.write_diagnostic_path(
                host.adapter,
                receiver.clone().key(key.to_string()),
                HostValue::Scalar(vela_common::ScalarValue::I64(*value)),
                None,
            )?;
            Ok(OwnedValue::Null)
        },
    );
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_host_type_spec(spec)
        .build()
        .expect("engine should build");
    let registry = engine.registry();
    let reflected = registry
        .type_by_name("IntIntMap")
        .expect("registered host type");
    let index = reflected.index_capability.as_ref().expect("index metadata");
    assert!(index.readable);
    assert!(index.writable);
    assert!(index.addable);
    assert!(index.removable);
    assert_eq!(index.key_type.as_deref(), Some("int"));
    assert_eq!(index.value_type.as_deref(), Some("int"));
    assert!(reflected.methods.iter().any(|method| method.name == "set"));
    assert!(
        engine
            .compiler_options()
            .host_index_capability("IntIntMap")
            .is_some()
    );

    let host_ref = HostRef::new(HostTypeId::new(31), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_object(host_ref);
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(host_ref),
            &[
                OwnedValue::Scalar(vela_common::ScalarValue::I64(7)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(99))
            ],
            &mut host,
        ),
        Ok(OwnedValue::Null)
    );
    assert_eq!(
        adapter.read_diagnostic_path(&HostPath::new(host_ref).key("7")),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(99)))
    );
}

#[test]
fn typed_callable_native_method_accepts_typed_host_path_arguments() {
    let method = HostMethodId::new(32);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(2), "Inventory")).host_type(HostTypeId::new(2)),
        )
        .register_typed_native_method_fn::<(TypedHostMut<InventoryArg>, i64), _>(
            NativeMethodDesc::new(owner, method, "transfer_to")
                .param("target", TypeHint::PathProxy)
                .param("amount", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_transfer_to,
        )
        .build()
        .expect("engine should build");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let inventory = HostRef::new(HostTypeId::new(2), HostObjectId::new(7), 1);
    let amount_path = HostPath::new(inventory).field(FieldId::new(77));
    let mut adapter = MockStateAdapter::new();
    adapter.insert_object(player);
    adapter.insert_object(inventory);
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(player),
            &[
                OwnedValue::HostRef(inventory),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(20))
            ],
            &mut host,
        ),
        Ok(OwnedValue::Null)
    );
    assert_eq!(
        adapter.read_diagnostic_path(&amount_path),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(20)))
    );
}

#[test]
fn typed_host_argument_rejects_mismatched_host_type() {
    let method = HostMethodId::new(33);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_typed_native_method_fn::<(TypedHostRef<InventoryArg>,), _>(
            NativeMethodDesc::new(owner, method, "inspect_inventory")
                .param("target", TypeHint::PathProxy)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_read())
                .access(FunctionAccess::public()),
            typed_inspect_inventory,
        )
        .build()
        .expect("engine should build");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert!(matches!(
        engine.call_native_method(
            method,
            &HostPath::new(player),
            &[OwnedValue::HostRef(player)],
            &mut host,
        ),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch {
                operation: "typed host ref type"
            })
    ));
}

fn typed_grant_exp(
    receiver: &HostPath,
    host: &mut HostExecution<'_>,
    amount: i64,
) -> VmResult<Option<i64>> {
    host.access.call_diagnostic_path_method(
        host.adapter,
        receiver.clone(),
        HostMethodId::new(8),
        vec![HostValue::Scalar(vela_common::ScalarValue::I64(amount))],
        None,
    )?;
    Ok(Some(amount))
}

fn typed_require_grant(
    receiver: &HostPath,
    _host: &mut HostExecution<'_>,
    allowed: bool,
) -> HostResult<i64> {
    if allowed {
        Ok(13)
    } else {
        Err(HostError {
            kind: HostErrorKind::PermissionDenied {
                path: receiver.clone(),
                action: "call",
            },
            source_span: None,
        })
    }
}

struct InventoryArg;

impl HostArgType for InventoryArg {
    const TYPE_NAME: &'static str = "Inventory";
    const HOST_TYPE_ID: Option<HostTypeId> = Some(HostTypeId::new(2));
}

fn typed_transfer_to(
    _receiver: &HostPath,
    host: &mut HostExecution<'_>,
    target: TypedHostMut<InventoryArg>,
    amount: i64,
) -> VmResult<()> {
    host.access.write_diagnostic_path(
        host.adapter,
        target.into_path().field(FieldId::new(77)),
        HostValue::Scalar(vela_common::ScalarValue::I64(amount)),
        None,
    )?;
    Ok(())
}

fn typed_inspect_inventory(
    _receiver: &HostPath,
    _host: &mut HostExecution<'_>,
    _target: TypedHostRef<InventoryArg>,
) -> VmResult<()> {
    Ok(())
}

#[test]
fn engine_registers_four_arg_typed_callable_native_methods() {
    let method = HostMethodId::new(9);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_typed_native_method_fn::<(i64, i64, i64, i64), _>(
            NativeMethodDesc::new(owner, method, "typed_sum4")
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .param("d", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_sum4,
        )
        .build()
        .expect("engine should build");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(player),
            &[
                OwnedValue::Scalar(vela_common::ScalarValue::I64(1)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(4))
            ],
            &mut host,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
}

#[test]
fn engine_registers_five_arg_typed_callable_native_methods() {
    let method = HostMethodId::new(10);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_typed_native_method_fn::<(i64, i64, i64, i64, i64), _>(
            NativeMethodDesc::new(owner, method, "typed_sum5")
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .param("d", TypeHint::Int)
                .param("e", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_sum5,
        )
        .build()
        .expect("engine should build");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(player),
            &[
                OwnedValue::Scalar(vela_common::ScalarValue::I64(1)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(5)),
            ],
            &mut host,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(15)))
    );
}

#[test]
fn engine_registers_six_arg_typed_callable_native_methods() {
    let method = HostMethodId::new(11);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_typed_native_method_fn::<(i64, i64, i64, i64, i64, i64), _>(
            NativeMethodDesc::new(owner, method, "typed_sum6")
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .param("d", TypeHint::Int)
                .param("e", TypeHint::Int)
                .param("f", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_sum6,
        )
        .build()
        .expect("engine should build");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(player),
            &[
                OwnedValue::Scalar(vela_common::ScalarValue::I64(1)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(5)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(6)),
            ],
            &mut host,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(21)))
    );
}

fn typed_sum4(
    receiver: &HostPath,
    host: &mut HostExecution<'_>,
    a: i64,
    b: i64,
    c: i64,
    d: i64,
) -> VmResult<i64> {
    let total = a + b + c + d;
    host.access.call_diagnostic_path_method(
        host.adapter,
        receiver.clone(),
        HostMethodId::new(9),
        vec![HostValue::Scalar(vela_common::ScalarValue::I64(total))],
        None,
    )?;
    Ok(total)
}

fn typed_sum5(
    receiver: &HostPath,
    host: &mut HostExecution<'_>,
    a: i64,
    b: i64,
    c: i64,
    d: i64,
    e: i64,
) -> VmResult<i64> {
    let total = a + b + c + d + e;
    host.access.call_diagnostic_path_method(
        host.adapter,
        receiver.clone(),
        HostMethodId::new(10),
        vec![HostValue::Scalar(vela_common::ScalarValue::I64(total))],
        None,
    )?;
    Ok(total)
}

#[allow(clippy::too_many_arguments)]
fn typed_sum6(
    receiver: &HostPath,
    host: &mut HostExecution<'_>,
    a: i64,
    b: i64,
    c: i64,
    d: i64,
    e: i64,
    f: i64,
) -> VmResult<i64> {
    let total = a + b + c + d + e + f;
    host.access.call_diagnostic_path_method(
        host.adapter,
        receiver.clone(),
        HostMethodId::new(11),
        vec![HostValue::Scalar(vela_common::ScalarValue::I64(total))],
        None,
    )?;
    Ok(total)
}

#[test]
fn engine_installs_type_registry_for_host_ref_script_impl_dispatch() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return amount + 7;
    }
}

fn main(player: Player) {
    return player.bonus(5);
}
"#,
        )
        .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(
            &engine,
            &program,
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}
