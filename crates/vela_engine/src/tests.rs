use vela_bytecode::compiler::compile_program_source;
use vela_common::{FieldId, HostObjectId, HostTypeId, SourceId, TypeId};
use vela_host::{HostPath, HostRef, HostValue, MockStateAdapter, PatchOp, PatchTx};
use vela_reflect::{FieldDesc, TypeDesc, TypeKey};
use vela_vm::{HostExecution, Value};

use crate::{
    EffectSet, Engine, EngineErrorKind, FunctionAccess, NativeFunctionDesc, NativeFunctionId,
    TypeHint,
};

#[test]
fn engine_installs_registered_native_functions_into_vm() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.add", NativeFunctionId::new(1))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::pure())
                .access(FunctionAccess::public())
                .docs("Adds two integers."),
            |args| {
                let [Value::Int(lhs), Value::Int(rhs)] = args else {
                    return Ok(Value::Null);
                };
                Ok(Value::Int(lhs + rhs))
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game.add(2, 3);
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(5))
    );
}

#[test]
fn engine_installs_registered_host_native_functions_into_vm() {
    let engine = Engine::builder()
        .register_host_native_fn(
            NativeFunctionDesc::new("game.set_level", NativeFunctionId::new(2))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().require_permission("player.write")),
            |args, host| {
                let [Value::HostRef(player), Value::Int(level)] = args else {
                    return Ok(Value::Null);
                };
                host.tx.set_path(
                    HostPath::new(*player).field(FieldId::new(1)),
                    HostValue::Int(*level),
                    None,
                )?;
                Ok(Value::Null)
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    game.set_level(player, 9);
    return 1;
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
        Ok(Value::Int(1))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(host_ref).field(FieldId::new(1))
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(9)));
}

#[test]
fn engine_rejects_duplicate_native_function_ids() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.first", NativeFunctionId::new(10)),
            |_| Ok(Value::Null),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game.second", NativeFunctionId::new(10)),
            |_| Ok(Value::Null),
        )
        .build();

    assert!(matches!(
        result.map(|_| ()),
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionId { id: 10 }
    ));
}

#[test]
fn engine_rejects_duplicate_names_across_host_and_pure_natives() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.same", NativeFunctionId::new(10)),
            |_| Ok(Value::Null),
        )
        .register_host_native_fn(
            NativeFunctionDesc::new("game.same", NativeFunctionId::new(11)),
            |_, _| Ok(Value::Null),
        )
        .build();

    assert!(matches!(
        result.map(|_| ()),
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionName {
            name: "game.same".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_duplicate_type_names() {
    let result = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_type(player_type(TypeId::new(2), HostTypeId::new(2)))
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateTypeName {
            name: "Player".to_owned()
        }
    ));
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

fn player_type(type_id: TypeId, host_type_id: HostTypeId) -> TypeDesc {
    TypeDesc::new(TypeKey::new(type_id, "Player"))
        .host_type(host_type_id)
        .field(FieldDesc::new(FieldId::new(1), "level").writable(true))
}
