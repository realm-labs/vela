use vela_bytecode::compiler::compile_program_source_with_options;
use vela_common::{FieldId, HostObjectId, HostTypeId, SourceId, TypeId};
use vela_host::adapter::ScriptStateAdapter;
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::mock::MockStateAdapter;
use vela_host::object::ScriptHostObject;
use vela_host::patch::PatchOp;
use vela_host::path::{HostPath, HostRef, PathSegment};
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_vm::error::VmErrorKind;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::runtime::{CallArgs, CallOptions, Runtime};

use super::player_type;

#[derive(Debug, Eq, PartialEq)]
struct DirectPlayer {
    level: i64,
}

impl ScriptHostObject for DirectPlayer {
    fn host_type_id(&self) -> HostTypeId {
        HostTypeId::new(1)
    }

    fn read_host_path(&self, path: &HostPath) -> HostResult<HostValue> {
        match path.segments.as_slice() {
            [PathSegment::Field(field)] if *field == FieldId::new(1) => {
                Ok(HostValue::Int(self.level))
            }
            _ => Err(HostError {
                kind: HostErrorKind::MissingPath { path: path.clone() },
                source_span: None,
            }),
        }
    }

    fn write_host_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()> {
        match (path.segments.as_slice(), value) {
            ([PathSegment::Field(field)], HostValue::Int(level)) if *field == FieldId::new(1) => {
                self.level = level;
                Ok(())
            }
            _ => Err(HostError {
                kind: HostErrorKind::MissingPath { path: path.clone() },
                source_span: None,
            }),
        }
    }
}

#[test]
fn runtime_call_args_bind_named_values_by_function_params() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player, amount, bonus = 1) {
    player.level += amount;
    return player.level + bonus;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let level = HostPath::new(player).field(super::FieldId::new(1));
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(level.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut args = CallArgs::new()
        .with_value("amount", 2_i64)
        .with_host_handle("player", player);

    let result = runtime
        .call_args(
            "main",
            &mut args,
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect("runtime call args should run");

    assert_eq!(result, OwnedValue::Int(12));
    assert_eq!(adapter.read_path(&level), Ok(HostValue::Int(11)));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(2)));
}

#[test]
fn runtime_call_args_accept_positional_values() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(left, right) {
    return left * 10 + right;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut args = CallArgs::from_positional([OwnedValue::Int(2), OwnedValue::Int(7)]);

    let result = runtime
        .call_args(
            "main",
            &mut args,
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect("runtime call args should run");

    assert_eq!(result, OwnedValue::Int(27));
}

#[test]
fn runtime_call_args_reject_duplicate_named_values() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        "fn main(value) { return value; }",
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut args = CallArgs::new()
        .with_value("value", 1_i64)
        .with_value("value", 2_i64);

    let error = runtime
        .call_args(
            "main",
            &mut args,
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect_err("duplicate named args should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::TypeMismatch {
            operation: "duplicate named call argument"
        }
    );
}

#[test]
fn runtime_call_args_reject_unknown_named_values() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        "fn main(value) { return value; }",
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut args = CallArgs::new().with_value("missing", 1_i64);

    let error = runtime
        .call_args(
            "main",
            &mut args,
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect_err("unknown named args should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::TypeMismatch {
            operation: "unknown named call argument"
        }
    );
}

#[test]
fn runtime_call_args_reject_mixed_modes() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        "fn main(value) { return value; }",
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut args = CallArgs::new().with(1_i64).with_value("value", 2_i64);

    let error = runtime
        .call_args(
            "main",
            &mut args,
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect_err("mixed args should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::TypeMismatch {
            operation: "mixed positional and named call arguments"
        }
    );
}

#[test]
fn runtime_call_args_host_mut_writes_through_to_rust_object() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player, amount) {
    player.level += amount;
    return player.level;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let mut player = DirectPlayer { level: 9 };
    let mut args = CallArgs::new()
        .with_host_mut("player", &mut player)
        .with_value("amount", 4_i64);
    let mut tx = PatchTx::new();

    let result = runtime
        .call_args_direct("main", &mut args, CallOptions::unbounded(), &mut tx)
        .expect("runtime direct host args should run");

    assert_eq!(result, OwnedValue::Int(13));
    drop(args);
    assert_eq!(player.level, 13);
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(4)));
}

#[test]
fn runtime_call_args_safe_point_preserves_direct_host_bindings() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.level += 1;
    return player.level;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let mut player = DirectPlayer { level: 9 };
    let mut args = CallArgs::new().with_host_mut("player", &mut player);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    let report = runtime
        .call_args_at_event_end_safe_point(
            "main",
            &mut args,
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect("runtime safe-point direct host args should run");

    assert_eq!(report.value, OwnedValue::Int(10));
    assert_eq!(report.reload, None);
    drop(args);
    assert_eq!(player.level, 10);
    assert_eq!(tx.patches().len(), 1);
}

#[test]
fn runtime_call_args_host_ref_denies_writes_to_rust_object() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.level += 1;
    return player.level;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let player = DirectPlayer { level: 9 };
    let mut args = CallArgs::new().with_host_ref("player", &player);
    let mut tx = PatchTx::new();

    let error = runtime
        .call_args_direct("main", &mut args, CallOptions::unbounded(), &mut tx)
        .expect_err("read-only direct host args should reject writes");

    assert!(matches!(
        error.kind,
        VmErrorKind::Host(HostErrorKind::PermissionDenied {
            action: "write",
            ..
        })
    ));
    assert_eq!(player.level, 9);
    assert!(tx.patches().is_empty());
}
