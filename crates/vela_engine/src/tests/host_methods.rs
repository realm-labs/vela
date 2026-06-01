use vela_bytecode::compiler::{compile_program_source, compile_program_source_with_options};
use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, TypeId, VariantId};
use vela_host::mock::MockStateAdapter;
use vela_host::patch::PatchOp;
use vela_host::path::{HostPath, HostRef};
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_reflect::registry::{FieldDesc, MethodDesc, TypeDesc, TypeKey, VariantDesc};
use vela_vm::HostExecution;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::value::Value;

use crate::engine::Engine;
use crate::method::NativeMethodDesc;
use crate::native::{EffectSet, FunctionAccess, TypeHint};
use crate::runtime::{CallOptions, Runtime};

use super::player_type;

#[test]
fn runtime_call_records_host_patches_without_applying() {
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
        .call(
            "main",
            &[Value::HostRef(host_ref)],
            CallOptions::gameplay(),
            &mut adapter,
            &mut tx,
        )
        .expect("runtime call should run");

    assert_eq!(result, Value::String("done".to_owned()));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(12)]
        }
    );
    assert!(
        adapter.method_calls().is_empty(),
        "runtime call must not apply patches"
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
            &[Value::HostRef(host_ref)],
            &mut host
        ),
        Ok(Value::Int(1))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, HostPath::new(host_ref));
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(10)],
        }
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
            &[Value::HostRef(host_ref)],
            &mut host
        ),
        Ok(Value::Int(1))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(host_ref).field(inventory)
    );
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::String("gold".to_owned()), HostValue::Int(20)],
        }
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
            &[Value::HostRef(host_ref)],
            &mut host
        ),
        Ok(Value::Int(5))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, quest_count);
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(1)));
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
            &[Value::HostRef(player), Value::HostRef(monster)],
            &mut host
        ),
        Ok(Value::Int(1))
    );
    assert_eq!(tx.patches().len(), 2);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method: player_method,
            args: vec![HostValue::Int(10)],
        }
    );
    assert_eq!(
        tx.patches()[1].op,
        PatchOp::CallHostMethod {
            method: monster_method,
            args: vec![HostValue::Int(3)],
        }
    );
}

#[test]
fn engine_registers_callable_native_methods_for_host_paths() {
    let method = HostMethodId::new(6);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .grant_permission("player.grant_exp")
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_native_method_fn(
            NativeMethodDesc::new(owner, method, "grant_exp")
                .param("amount", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().require_permission("player.grant_exp"))
                .docs("Grant player experience.")
                .attr("domain", "gameplay")
                .attr("effect", "reward"),
            move |receiver, args, host| {
                let [Value::Int(amount)] = args else {
                    return Ok(Value::Null);
                };
                host.tx.call_method(
                    receiver.clone(),
                    method,
                    vec![HostValue::Int(*amount)],
                    None,
                )?;
                Ok(Value::Null)
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
            &[Value::Int(10)],
            &mut host,
        ),
        Ok(Value::Null)
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(10)],
        }
    );

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
            &[Value::HostRef(host_ref)],
            &mut host
        ),
        Ok(Value::Int(1))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, HostPath::new(host_ref));
}

#[test]
fn engine_registers_typed_callable_native_methods_for_host_paths() {
    let method = HostMethodId::new(8);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .grant_permission("player.grant_exp")
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_typed_native_method_fn::<(i64,), _>(
            NativeMethodDesc::new(owner, method, "typed_grant_exp")
                .param("amount", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().require_permission("player.grant_exp")),
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
            &[Value::Int(15)],
            &mut host,
        ),
        Ok(Value::Int(15))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(15)],
        }
    );
}

#[test]
fn typed_callable_native_method_conversion_errors_before_patch() {
    let method = HostMethodId::new(8);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .grant_permission("player.grant_exp")
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_typed_native_method_fn::<(i64,), _>(
            NativeMethodDesc::new(owner, method, "typed_grant_exp")
                .access(FunctionAccess::public().require_permission("player.grant_exp")),
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
            &[Value::String("bad".to_owned())],
            &mut host,
        ),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch { operation: "int" },
            ..
        })
    ));
    assert!(tx.patches().is_empty());
}

fn typed_grant_exp(
    receiver: &HostPath,
    host: &mut HostExecution<'_>,
    amount: i64,
) -> VmResult<Option<i64>> {
    host.tx.call_method(
        receiver.clone(),
        HostMethodId::new(8),
        vec![HostValue::Int(amount)],
        None,
    )?;
    Ok(Some(amount))
}

#[test]
fn engine_registers_four_arg_typed_callable_native_methods() {
    let method = HostMethodId::new(9);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .grant_permission("player.sum")
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_typed_native_method_fn::<(i64, i64, i64, i64), _>(
            NativeMethodDesc::new(owner, method, "typed_sum4")
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .param("d", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().require_permission("player.sum")),
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
            &[Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)],
            &mut host,
        ),
        Ok(Value::Int(10))
    );
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(10)],
        }
    );
}

#[test]
fn engine_registers_five_arg_typed_callable_native_methods() {
    let method = HostMethodId::new(10);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .grant_permission("player.sum5")
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
                .access(FunctionAccess::public().require_permission("player.sum5")),
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
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
                Value::Int(4),
                Value::Int(5),
            ],
            &mut host,
        ),
        Ok(Value::Int(15))
    );
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(15)],
        }
    );
}

#[test]
fn engine_registers_six_arg_typed_callable_native_methods() {
    let method = HostMethodId::new(11);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .grant_permission("player.sum6")
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
                .access(FunctionAccess::public().require_permission("player.sum6")),
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
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
                Value::Int(4),
                Value::Int(5),
                Value::Int(6),
            ],
            &mut host,
        ),
        Ok(Value::Int(21))
    );
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(21)],
        }
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
            &[Value::HostRef(host_ref)],
            &mut host
        ),
        Ok(Value::Int(12))
    );
    assert!(tx.patches().is_empty());
}
