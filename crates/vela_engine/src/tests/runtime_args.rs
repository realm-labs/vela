use std::collections::BTreeMap;

use vela_bytecode::compiler::compile_program_source_with_options;
use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, TypeId};
use vela_host::access::HostAccess;
use vela_host::adapter::ScriptStateAdapter;
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::mock::MockStateAdapter;
use vela_host::object::ScriptHostObject;
use vela_host::path::{HostPath, HostRef, PathSegment};
use vela_host::value::HostValue;
use vela_reflect::registry::{FieldDesc, MethodDesc, TypeDesc, TypeKey};
use vela_vm::error::VmErrorKind;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::runtime::{CallArgs, CallOptions, Runtime};

use super::player_type;

#[derive(Debug, Eq, PartialEq)]
struct DirectPlayer {
    level: i64,
    inventory: BTreeMap<String, i64>,
}

fn direct_player(level: i64) -> DirectPlayer {
    DirectPlayer {
        level,
        inventory: BTreeMap::new(),
    }
}

fn direct_player_type() -> TypeDesc {
    TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
        .host_type(HostTypeId::new(1))
        .field(FieldDesc::new(FieldId::new(1), "level").writable(true))
        .field(FieldDesc::new(FieldId::new(2), "inventory").writable(true))
        .method(MethodDesc::new(HostMethodId::new(10), "grant_exp"))
        .method(MethodDesc::new(HostMethodId::new(11), "add"))
}

fn script_record_field<'value>(
    value: &'value OwnedValue,
    field: &str,
) -> Option<&'value OwnedValue> {
    let OwnedValue::Record { fields, .. } = value else {
        return None;
    };
    fields.get(field)
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
            [PathSegment::Field(field), PathSegment::Key(key)] if *field == FieldId::new(2) => {
                Ok(HostValue::Int(*self.inventory.get(key).unwrap_or(&0)))
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
            ([PathSegment::Field(field), PathSegment::Key(key)], HostValue::Int(count))
                if *field == FieldId::new(2) =>
            {
                self.inventory.insert(key.clone(), count);
                Ok(())
            }
            _ => Err(HostError {
                kind: HostErrorKind::MissingPath { path: path.clone() },
                source_span: None,
            }),
        }
    }

    fn call_host_method(
        &mut self,
        path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        match (path.segments.as_slice(), method, args) {
            ([], method, [HostValue::Int(amount)]) if method == HostMethodId::new(10) => {
                self.level += amount;
                Ok(HostValue::Int(self.level))
            }
            (
                [PathSegment::Field(field)],
                method,
                [HostValue::String(key), HostValue::Int(amount)],
            ) if *field == FieldId::new(2) && method == HostMethodId::new(11) => {
                *self.inventory.entry(key.clone()).or_insert(0) += amount;
                Ok(HostValue::Null)
            }
            _ => Err(HostError {
                kind: HostErrorKind::UnsupportedMethod { method },
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
    let mut tx = HostAccess::new();
    let mut args = CallArgs::new()
        .with_value("amount", 2_i64)
        .with_host_handle("player", player);

    let result = runtime
        .call_args_raw(
            "main",
            &mut args,
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect("runtime call args should run");

    assert_eq!(result, OwnedValue::Int(12));
    assert_eq!(adapter.read_path(&level), Ok(HostValue::Int(11)));
}

#[test]
fn runtime_host_global_decl_reads_and_writes_persistent_host_object() {
    let engine = Engine::builder()
        .register_type(direct_player_type())
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
global state: Player;

fn main() {
    state.level += 2;
    return state.level;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let global = runtime.insert_host_global("main::state", direct_player(9));

    let result = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("runtime call should run");

    assert_eq!(runtime.host_global_ref("main::state"), Some(global));
    assert_eq!(result.into_value(), OwnedValue::Int(11));
}

#[test]
fn runtime_host_global_decl_requires_host_inserted_instance() {
    let engine = Engine::builder()
        .register_type(direct_player_type())
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
global state: Player;

fn main() {
    return state.level;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);

    let error = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect_err("missing global should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::Host(HostErrorKind::MissingGlobal {
            name: "main::state".to_owned()
        })
    );
}

#[test]
fn runtime_script_global_decl_persists_vm_owned_value_and_rust_updates() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
struct ServerState {
    level: Int,
    name: String,
}

global state: ServerState;

fn make_state() {
    return ServerState { level: 5, name: "boot" };
}

fn bump(amount) {
    state.level += amount;
    return state.level;
}

fn read_name() {
    return state.name;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);

    let state = runtime
        .call("make_state", CallArgs::new(), CallOptions::unbounded())
        .expect("factory should run")
        .into_value();
    runtime
        .insert_script_global("main::state", state)
        .expect("script global should insert");

    let first = runtime
        .call(
            "bump",
            CallArgs::from_positional([OwnedValue::Int(2)]),
            CallOptions::unbounded(),
        )
        .expect("first bump should run");
    let second = runtime
        .call(
            "bump",
            CallArgs::from_positional([OwnedValue::Int(3)]),
            CallOptions::unbounded(),
        )
        .expect("second bump should run");

    assert_eq!(first.into_value(), OwnedValue::Int(7));
    assert_eq!(second.into_value(), OwnedValue::Int(10));
    assert_eq!(
        script_record_field(
            &runtime
                .script_global("main::state")
                .expect("script global should materialize")
                .expect("script global should exist"),
            "level",
        ),
        Some(&OwnedValue::Int(10))
    );

    runtime
        .update_script_global("main::state", |value| {
            let OwnedValue::Record { fields, .. } = value else {
                panic!("state should remain a record");
            };
            fields
                .set_existing("level", OwnedValue::Int(40))
                .expect("level field should exist");
        })
        .expect("rust update should replace persistent global");

    let after_rust_update = runtime
        .call(
            "bump",
            CallArgs::from_positional([OwnedValue::Int(1)]),
            CallOptions::unbounded(),
        )
        .expect("bump after rust update should run");
    let name = runtime
        .call("read_name", CallArgs::new(), CallOptions::unbounded())
        .expect("read name should run");

    assert_eq!(after_rust_update.into_value(), OwnedValue::Int(41));
    assert_eq!(name.into_value(), OwnedValue::String("boot".to_owned()));
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
    let mut tx = HostAccess::new();
    let mut args = CallArgs::from_positional([OwnedValue::Int(2), OwnedValue::Int(7)]);

    let result = runtime
        .call_args_raw(
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
    let mut tx = HostAccess::new();
    let mut args = CallArgs::new()
        .with_value("value", 1_i64)
        .with_value("value", 2_i64);

    let error = runtime
        .call_args_raw(
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
    let mut tx = HostAccess::new();
    let mut args = CallArgs::new().with_value("missing", 1_i64);

    let error = runtime
        .call_args_raw(
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
    let mut tx = HostAccess::new();
    let mut args = CallArgs::new().with(1_i64).with_value("value", 2_i64);

    let error = runtime
        .call_args_raw(
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
    let mut player = direct_player(9);
    let output = runtime
        .call(
            "main",
            CallArgs::new()
                .with_host_mut("player", &mut player)
                .with_value("amount", 4_i64),
            CallOptions::unbounded(),
        )
        .expect("runtime direct host args should run");

    assert_eq!(&*output, &OwnedValue::Int(13));
    assert_eq!(player.level, 13);
}

#[test]
fn runtime_call_args_host_mut_writes_string_key_map_path_to_rust_object() {
    let engine = Engine::builder()
        .register_type(direct_player_type())
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player, amount) {
    player.inventory["gold"] += amount;
    return player.inventory["gold"];
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let mut player = direct_player(9);
    player.inventory.insert("gold".to_owned(), 3);

    let output = runtime
        .call(
            "main",
            CallArgs::new()
                .with_host_mut("player", &mut player)
                .with_value("amount", 4_i64),
            CallOptions::unbounded(),
        )
        .expect("runtime direct map path should run");

    assert_eq!(&*output, &OwnedValue::Int(7));
    assert_eq!(player.inventory.get("gold"), Some(&7));
}

#[test]
fn runtime_call_args_host_mut_dispatches_root_and_child_host_methods() {
    let engine = Engine::builder()
        .register_type(direct_player_type())
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    let level = player.grant_exp(2);
    player.inventory.add("gold", 5);
    return level + player.inventory["gold"];
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let mut player = direct_player(9);

    let output = runtime
        .call(
            "main",
            CallArgs::new().with_host_mut("player", &mut player),
            CallOptions::unbounded(),
        )
        .expect("runtime direct host methods should run");

    assert_eq!(&*output, &OwnedValue::Int(16));
    assert_eq!(player.level, 11);
    assert_eq!(player.inventory.get("gold"), Some(&5));
}

#[test]
fn runtime_call_returns_value_like_output() {
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
    let mut player = direct_player(9);

    let output = runtime
        .call(
            "main",
            CallArgs::new()
                .with_host_mut("player", &mut player)
                .with_value("amount", 4_i64),
            CallOptions::unbounded(),
        )
        .expect("runtime direct call should run");

    assert_eq!(&*output, &OwnedValue::Int(13));
    assert_eq!(player.level, 13);
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
    let mut player = direct_player(9);
    let mut args = CallArgs::new().with_host_mut("player", &mut player);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    let report = runtime
        .call_args_raw_at_event_end_safe_point(
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
    let player = direct_player(9);

    let error = runtime
        .call(
            "main",
            CallArgs::new().with_host_ref("player", &player),
            CallOptions::unbounded(),
        )
        .expect_err("read-only direct host args should reject writes");

    assert!(matches!(
        error.kind,
        VmErrorKind::Host(HostErrorKind::PermissionDenied {
            action: "write",
            ..
        })
    ));
    assert_eq!(player.level, 9);
}
