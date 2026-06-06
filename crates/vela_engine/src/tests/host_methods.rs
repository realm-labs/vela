use vela_bytecode::compiler::{compile_program_source, compile_program_source_with_options};
use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, TypeId, VariantId};
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_reflect::registry::{FieldDesc, MethodDesc, TypeDesc, TypeKey, VariantDesc};
use vela_vm::HostExecution;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::method::NativeMethodDesc;
use crate::native::{EffectSet, FunctionAccess, TypeHint};
use crate::permission::Capability;
use crate::runtime::{CallOptions, Runtime};

use super::player_type;

#[test]
fn runtime_call_writes_through_host_method_and_counts_mutation() {
    let method = HostMethodId::new(23);
    let engine = Engine::builder()
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(method, "grant_exp")),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.grant_exp(12);
    return "done";
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

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
    assert_eq!(tx.mutation_count(), 1);
    assert_eq!(
        adapter.method_calls(),
        &[(HostPath::new(host_ref), method, vec![HostValue::Int(12)])]
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
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.grant_exp(10);
    return 1;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Int(1))
    );
    assert_eq!(tx.mutation_count(), 1);
}

#[test]
fn engine_compiler_options_lower_registered_host_field_methods() {
    let inventory = FieldId::new(3);
    let method = HostMethodId::new(5);
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .field(FieldDesc::new(inventory, "inventory")),
        )
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(2), "Inventory"))
                .host_type(HostTypeId::new(2))
                .method(MethodDesc::new(method, "add")),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.inventory.add("gold", 20);
    return 1;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Int(1))
    );
    assert_eq!(tx.mutation_count(), 1);
}

#[test]
fn engine_compiler_options_lower_registered_host_variant_fields() {
    let quest_progress = FieldId::new(3);
    let count = FieldId::new(4);
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .field(FieldDesc::new(quest_progress, "quest_progress")),
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
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.quest_progress.count += 1;
    return player.quest_progress.count;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let quest_count = HostPath::new(host_ref)
        .field(quest_progress)
        .variant_field(count);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(quest_count.clone(), HostValue::Int(4));
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Int(5))
    );
    assert_eq!(tx.mutation_count(), 1);
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
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player, monster: Monster) {
    player.grant_exp(10);
    monster.grant_exp(3);
    return 1;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let monster = HostRef::new(HostTypeId::new(2), HostObjectId::new(7), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(player), OwnedValue::HostRef(monster)],
            &mut host
        ),
        Ok(OwnedValue::Int(1))
    );
    assert_eq!(tx.mutation_count(), 2);
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
                let [OwnedValue::Int(amount)] = args else {
                    return Ok(OwnedValue::Null);
                };
                host.tx.call_method(
                    host.adapter,
                    receiver.clone(),
                    method,
                    vec![HostValue::Int(*amount)],
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
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.grant_exp(10);
    return 1;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(host_ref),
            &[OwnedValue::Int(10)],
            &mut host,
        ),
        Ok(OwnedValue::Null)
    );
    assert_eq!(tx.mutation_count(), 1);

    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };
    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Int(1))
    );
    assert_eq!(tx.mutation_count(), 1);
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
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(host_ref),
            &[OwnedValue::Int(15)],
            &mut host,
        ),
        Ok(OwnedValue::Int(15))
    );
    assert_eq!(tx.mutation_count(), 1);
}

#[test]
fn typed_callable_native_method_conversion_errors_before_mutation_counting() {
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
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        engine.call_native_method(
            method,
            &HostPath::new(host_ref),
            &[OwnedValue::String("bad".to_owned())],
            &mut host,
        ),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch { operation: "int" },
            ..
        })
    ));
    assert!(tx.is_empty());
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
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine
            .call_native_method(
                method,
                &expected_path,
                &[OwnedValue::Bool(false)],
                &mut host
            )
            .map_err(|error| error.kind),
        Err(VmErrorKind::Host(HostErrorKind::PermissionDenied {
            path: expected_path,
            action: "call",
        })),
    );
    assert!(tx.is_empty());
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
                let [OwnedValue::Int(amount)] = args else {
                    return Ok(OwnedValue::Null);
                };
                host.tx.call_method(
                    host.adapter,
                    receiver.clone(),
                    method,
                    vec![HostValue::Int(*amount)],
                    None,
                )?;
                Err(VmError {
                    kind: VmErrorKind::TypeMismatch {
                        operation: "failing native method",
                    },
                    source_span: None,
                    call_stack: Default::default(),
                })
            },
        )
        .build()
        .expect("engine should build");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = engine
        .call_native_method(
            method,
            &HostPath::new(host_ref),
            &[OwnedValue::Int(15)],
            &mut host,
        )
        .expect_err("native method should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::TypeMismatch {
            operation: "failing native method",
        }
    );
    assert_eq!(tx.mutation_count(), 1);
    assert_eq!(adapter.method_calls().len(), 1);
}

fn typed_grant_exp(
    receiver: &HostPath,
    host: &mut HostExecution<'_>,
    amount: i64,
) -> VmResult<Option<i64>> {
    host.tx.call_method(
        host.adapter,
        receiver.clone(),
        HostMethodId::new(8),
        vec![HostValue::Int(amount)],
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
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(player),
            &[
                OwnedValue::Int(1),
                OwnedValue::Int(2),
                OwnedValue::Int(3),
                OwnedValue::Int(4)
            ],
            &mut host,
        ),
        Ok(OwnedValue::Int(10))
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
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(player),
            &[
                OwnedValue::Int(1),
                OwnedValue::Int(2),
                OwnedValue::Int(3),
                OwnedValue::Int(4),
                OwnedValue::Int(5),
            ],
            &mut host,
        ),
        Ok(OwnedValue::Int(15))
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
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(player),
            &[
                OwnedValue::Int(1),
                OwnedValue::Int(2),
                OwnedValue::Int(3),
                OwnedValue::Int(4),
                OwnedValue::Int(5),
                OwnedValue::Int(6),
            ],
            &mut host,
        ),
        Ok(OwnedValue::Int(21))
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
    host.tx.call_method(
        host.adapter,
        receiver.clone(),
        HostMethodId::new(9),
        vec![HostValue::Int(total)],
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
    host.tx.call_method(
        host.adapter,
        receiver.clone(),
        HostMethodId::new(10),
        vec![HostValue::Int(total)],
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
    host.tx.call_method(
        host.adapter,
        receiver.clone(),
        HostMethodId::new(11),
        vec![HostValue::Int(total)],
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
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return amount + 7;
    }
}

fn main(player) {
    return player.bonus(5);
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Int(12))
    );
    assert!(tx.is_empty());
}
