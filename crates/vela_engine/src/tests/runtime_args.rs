use std::cell::Cell;
use std::collections::BTreeMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use vela_bytecode::compiler::compile_program_source_with_options;
use vela_common::{FieldId, GlobalSlot, HostMethodId, HostObjectId, HostTypeId, SourceId, TypeId};
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
use crate::runtime::{
    CallArgs, CallOptions, Runtime, RuntimeImage, SharedRuntime, VelaFunction, VelaMethod,
    VelaValue,
};

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

#[test]
fn runtime_and_runtime_values_are_send() {
    fn assert_send<T: Send>() {}

    assert_send::<Runtime>();
    assert_send::<SharedRuntime>();
    assert_send::<VelaValue>();
    assert_send::<VelaFunction>();
    assert_send::<VelaMethod>();
}

#[test]
fn runtime_call_executes_program_image_code_view() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn add_bonus(value) {
    return value + 5;
}

fn main() {
    let player = Player { level: 7 };
    return add_bonus(player.bonus(4));
}
"#,
        &engine.compiler_options(),
    )
    .expect("compile runtime image code-view source");
    let mut runtime = Runtime::new(engine, program);

    let value = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("runtime call should execute");

    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::Int(16)));
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

#[derive(Default)]
struct CountingGlobalLookupAdapter {
    global_ref_calls: Cell<usize>,
    global_ref_by_slot_calls: Cell<usize>,
}

impl ScriptStateAdapter for CountingGlobalLookupAdapter {
    fn global_ref(&self, name: &str) -> HostResult<HostRef> {
        self.global_ref_calls
            .set(self.global_ref_calls.get().saturating_add(1));
        Err(HostError {
            kind: HostErrorKind::MissingGlobal {
                name: name.to_owned(),
            },
            source_span: None,
        })
    }

    fn global_ref_by_slot(&self, slot: GlobalSlot, name: &str) -> HostResult<HostRef> {
        self.global_ref_by_slot_calls
            .set(self.global_ref_by_slot_calls.get().saturating_add(1));
        let _ = slot;
        self.global_ref(name)
    }

    fn read_path(&self, path: &HostPath) -> HostResult<HostValue> {
        Err(HostError {
            kind: HostErrorKind::MissingPath { path: path.clone() },
            source_span: None,
        })
    }

    fn write_path(&mut self, path: &HostPath, _value: HostValue) -> HostResult<()> {
        Err(HostError {
            kind: HostErrorKind::MissingPath { path: path.clone() },
            source_span: None,
        })
    }

    fn remove_path(&mut self, path: &HostPath) -> HostResult<()> {
        Err(HostError {
            kind: HostErrorKind::MissingPath { path: path.clone() },
            source_span: None,
        })
    }

    fn call_method(
        &mut self,
        path: &HostPath,
        _method: HostMethodId,
        _args: &[HostValue],
    ) -> HostResult<HostValue> {
        Err(HostError {
            kind: HostErrorKind::MissingPath { path: path.clone() },
            source_span: None,
        })
    }
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
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Int(11)));
}

#[test]
fn runtime_host_global_decl_uses_slotted_lookup_without_fallback_name_lookup() {
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
    assert!(
        program.global_slot("main::state").is_some(),
        "declared global should have a hot-path slot"
    );
    let mut runtime = Runtime::new(engine, program);
    runtime.insert_host_global("main::state", direct_player(9));
    let mut fallback = CountingGlobalLookupAdapter::default();

    let result = runtime
        .call_with_adapter(
            "main",
            CallArgs::new(),
            CallOptions::unbounded(),
            &mut fallback,
        )
        .expect("runtime call should run");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Int(9)));
    assert_eq!(fallback.global_ref_by_slot_calls.get(), 0);
    assert_eq!(fallback.global_ref_calls.get(), 0);
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
        .expect("factory should run");
    runtime
        .insert_global("main::state", state)
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

    assert_eq!(runtime.value_to_owned(&first), Ok(OwnedValue::Int(7)));
    assert_eq!(runtime.value_to_owned(&second), Ok(OwnedValue::Int(10)));
    assert_eq!(
        script_record_field(
            &runtime
                .global("main::state")
                .expect("script global should materialize")
                .expect("script global should exist"),
            "level",
        ),
        Some(&OwnedValue::Int(10))
    );

    runtime
        .update_global("main::state", |value| {
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

    assert_eq!(
        runtime.value_to_owned(&after_rust_update),
        Ok(OwnedValue::Int(41))
    );
    assert_eq!(
        runtime.value_to_owned(&name),
        Ok(OwnedValue::String("boot".to_owned()))
    );
}

#[test]
fn shared_runtime_image_keeps_script_globals_isolated() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
struct ServerState {
    level: Int,
    name: String,
}

global state: ServerState;

fn make_state(level, name) {
    return ServerState { level: level, name: name };
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
    let shared_image = RuntimeImage::new(engine, program).into_shared();
    let mut first = SharedRuntime::from_shared_image(shared_image.clone());
    let mut second = SharedRuntime::from_shared_image(shared_image);

    let first_state = first
        .call(
            "make_state",
            CallArgs::from_positional([OwnedValue::Int(5), OwnedValue::String("first".into())]),
            CallOptions::unbounded(),
        )
        .expect("first factory should run");
    let second_state = second
        .call(
            "make_state",
            CallArgs::from_positional([OwnedValue::Int(40), OwnedValue::String("second".into())]),
            CallOptions::unbounded(),
        )
        .expect("second factory should run");

    first
        .insert_global("main::state", first_state)
        .expect("first script global should insert");
    second
        .insert_global("main::state", second_state)
        .expect("second script global should insert");

    let first_bumped = first
        .call(
            "bump",
            CallArgs::from_positional([OwnedValue::Int(2)]),
            CallOptions::unbounded(),
        )
        .expect("first bump should run");
    let second_bumped = second
        .call(
            "bump",
            CallArgs::from_positional([OwnedValue::Int(3)]),
            CallOptions::unbounded(),
        )
        .expect("second bump should run");
    let first_name = first
        .call("read_name", CallArgs::new(), CallOptions::unbounded())
        .expect("first name should read");
    let second_name = second
        .call("read_name", CallArgs::new(), CallOptions::unbounded())
        .expect("second name should read");

    assert_eq!(first.value_to_owned(&first_bumped), Ok(OwnedValue::Int(7)));
    assert_eq!(
        second.value_to_owned(&second_bumped),
        Ok(OwnedValue::Int(43))
    );
    assert_eq!(
        first.value_to_owned(&first_name),
        Ok(OwnedValue::String("first".to_owned()))
    );
    assert_eq!(
        second.value_to_owned(&second_name),
        Ok(OwnedValue::String("second".to_owned()))
    );
}

#[cfg(feature = "serde")]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct SerdeServerState {
    level: i64,
    name: String,
}

#[cfg(feature = "serde")]
#[test]
fn runtime_insert_global_accepts_serde_struct_with_single_api() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
struct SerdeServerState {
    level: Int,
    name: String,
}

global state: SerdeServerState;

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
    let state = SerdeServerState {
        level: 5,
        name: "serde".to_owned(),
    };

    runtime
        .insert_global("main::state", &state)
        .expect("serde global should insert through unified API");

    let level_value = runtime
        .call(
            "bump",
            CallArgs::from_positional([OwnedValue::Int(4)]),
            CallOptions::unbounded(),
        )
        .expect("bump should run");
    let name_value = runtime
        .call("read_name", CallArgs::new(), CallOptions::unbounded())
        .expect("read name should run");
    let level: i64 = runtime
        .from_value(&level_value)
        .expect("level value should deserialize directly");
    let name: String = runtime
        .from_value(&name_value)
        .expect("name value should deserialize directly");
    let global: SerdeServerState = runtime
        .global_as("main::state")
        .expect("script global should deserialize directly")
        .expect("script global should exist");

    assert_eq!(state.level, 5);
    assert_eq!(level, 9);
    assert_eq!(name, "serde");
    assert_eq!(
        global,
        SerdeServerState {
            level: 9,
            name: "serde".to_owned()
        }
    );
}

#[test]
fn runtime_insert_global_accepts_runtime_managed_value_with_single_api() {
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
    return ServerState { level: 11, name: "runtime" };
}

fn read_level() {
    return state.level;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);

    let state = runtime
        .call("make_state", CallArgs::new(), CallOptions::unbounded())
        .expect("factory should return runtime-managed value");
    runtime
        .insert_global("main::state", state)
        .expect("runtime value should insert through unified API");

    let level = runtime
        .call("read_level", CallArgs::new(), CallOptions::unbounded())
        .expect("read level should run");
    assert_eq!(runtime.value_to_owned(&level), Ok(OwnedValue::Int(11)));
}

#[test]
fn runtime_call_returns_runtime_managed_value_that_can_be_passed_back() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
struct Reward {
    gold: Int,
    xp: Int,
}

fn make_reward() {
    return Reward { gold: 7, xp: 3 };
}

fn reward_score(reward, bonus) {
    return reward.gold + reward.xp + bonus;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);

    let reward = runtime
        .call("make_reward", CallArgs::new(), CallOptions::unbounded())
        .expect("reward should be returned as runtime value");
    let score = runtime
        .call(
            "reward_score",
            CallArgs::new()
                .with_vela_value(reward.clone())
                .with(OwnedValue::Int(5)),
            CallOptions::unbounded(),
        )
        .expect("runtime value should pass back without owned materialization");

    assert_eq!(runtime.value_to_owned(&score), Ok(OwnedValue::Int(15)));
    assert_eq!(
        script_record_field(
            &runtime
                .value_to_owned(&reward)
                .expect("runtime value can materialize on demand"),
            "gold",
        ),
        Some(&OwnedValue::Int(7))
    );
}

#[test]
fn runtime_call_rejects_values_from_another_runtime() {
    let engine = Engine::builder().build().expect("engine should build");
    let source = r#"
struct Reward {
    gold: Int,
}

fn make_reward() {
    return Reward { gold: 7 };
}

fn read_reward(reward) {
    return reward.gold;
}
"#;
    let program_a =
        compile_program_source_with_options(SourceId::new(1), source, &engine.compiler_options())
            .expect("program should compile");
    let program_b =
        compile_program_source_with_options(SourceId::new(2), source, &engine.compiler_options())
            .expect("program should compile");
    let mut runtime_a = Runtime::new(engine.clone(), program_a);
    let mut runtime_b = Runtime::new(engine, program_b);

    let reward = runtime_a
        .call("make_reward", CallArgs::new(), CallOptions::unbounded())
        .expect("runtime value should be created");
    let error = runtime_b
        .call(
            "read_reward",
            CallArgs::new().with_vela_value(reward),
            CallOptions::unbounded(),
        )
        .expect_err("runtime values must not cross runtime heaps");

    assert!(matches!(
        error.kind,
        VmErrorKind::TypeMismatch {
            operation: "VelaValue belongs to another Runtime",
        }
    ));
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

    assert_eq!(runtime.value_to_owned(&output), Ok(OwnedValue::Int(13)));
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

    assert_eq!(runtime.value_to_owned(&output), Ok(OwnedValue::Int(7)));
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

    assert_eq!(runtime.value_to_owned(&output), Ok(OwnedValue::Int(16)));
    assert_eq!(player.level, 11);
    assert_eq!(player.inventory.get("gold"), Some(&5));
}

#[test]
fn runtime_call_returns_runtime_value() {
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

    assert_eq!(runtime.value_to_owned(&output), Ok(OwnedValue::Int(13)));
    assert_eq!(player.level, 13);
}

#[test]
fn runtime_cached_entry_calls_function() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(amount, multiplier = 2) {
    return amount * multiplier;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let main = runtime.entry("main").expect("entry should resolve");

    let first = runtime
        .call(
            &main,
            CallArgs::new().with_value("amount", 7_i64),
            CallOptions::unbounded(),
        )
        .expect("cached entry should run with default args");
    let second = runtime
        .call(
            &main,
            CallArgs::new()
                .with_value("amount", 7_i64)
                .with_value("multiplier", 3_i64),
            CallOptions::unbounded(),
        )
        .expect("cached entry should run with named args");

    assert_eq!(main.name(), "main");
    assert_eq!(runtime.value_to_owned(&first), Ok(OwnedValue::Int(14)));
    assert_eq!(runtime.value_to_owned(&second), Ok(OwnedValue::Int(21)));
}

#[test]
fn runtime_cached_entry_rejects_function_from_another_runtime() {
    let engine = Engine::builder().build().expect("engine should build");
    let source = r#"
fn main(amount) {
    return amount * 2;
}
"#;
    let program_a =
        compile_program_source_with_options(SourceId::new(1), source, &engine.compiler_options())
            .expect("program should compile");
    let program_b =
        compile_program_source_with_options(SourceId::new(2), source, &engine.compiler_options())
            .expect("program should compile");
    let runtime_a = Runtime::new(engine.clone(), program_a);
    let mut runtime_b = Runtime::new(engine, program_b);
    let main = runtime_a.entry("main").expect("entry should resolve");

    let error = runtime_b
        .call(
            &main,
            CallArgs::from_positional([OwnedValue::Int(7)]),
            CallOptions::unbounded(),
        )
        .expect_err("cached entry from another runtime should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::TypeMismatch {
            operation: "VelaFunction belongs to another Runtime"
        }
    );
}

#[test]
fn runtime_call_method_on_runtime_value_by_name_and_cached_method() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn score(self, amount, multiplier = 2) -> Int;
}

struct Reward {
    gold: Int,
}

impl BonusSource for Reward {
    fn score(self, amount, multiplier = 2) -> Int {
        return self.gold + amount * multiplier;
    }
}

fn make_reward() {
    return Reward { gold: 7 };
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let reward = runtime
        .call("make_reward", CallArgs::new(), CallOptions::unbounded())
        .expect("factory should return runtime value");
    let score_by_name = runtime
        .call_method(
            &reward,
            "score",
            CallArgs::new().with_value("amount", 5_i64),
            CallOptions::unbounded(),
        )
        .expect("named value method should run");
    let score_method = runtime
        .method(&reward, "score")
        .expect("method should resolve");
    let score_by_cached_method = runtime
        .call_method(
            &reward,
            &score_method,
            CallArgs::new()
                .with_value("amount", 3_i64)
                .with_value("multiplier", 4_i64),
            CallOptions::unbounded(),
        )
        .expect("cached value method should run");

    assert_eq!(score_method.name(), "score");
    assert_eq!(score_method.receiver_type(), "Reward");
    assert_eq!(
        runtime.value_to_owned(&score_by_name),
        Ok(OwnedValue::Int(17))
    );
    assert_eq!(
        runtime.value_to_owned(&score_by_cached_method),
        Ok(OwnedValue::Int(19))
    );
}

#[test]
fn runtime_cached_method_rejects_method_from_another_runtime() {
    let engine = Engine::builder().build().expect("engine should build");
    let source = r#"
trait BonusSource {
    fn score(self, amount) -> Int;
}

struct Reward {
    gold: Int,
}

impl BonusSource for Reward {
    fn score(self, amount) -> Int {
        return self.gold + amount;
    }
}

fn make_reward(gold) {
    return Reward { gold: gold };
}
"#;
    let program_a =
        compile_program_source_with_options(SourceId::new(1), source, &engine.compiler_options())
            .expect("program should compile");
    let program_b =
        compile_program_source_with_options(SourceId::new(2), source, &engine.compiler_options())
            .expect("program should compile");
    let mut runtime_a = Runtime::new(engine.clone(), program_a);
    let mut runtime_b = Runtime::new(engine, program_b);
    let reward_a = runtime_a
        .call(
            "make_reward",
            CallArgs::from_positional([OwnedValue::Int(7)]),
            CallOptions::unbounded(),
        )
        .expect("first factory should run");
    let reward_b = runtime_b
        .call(
            "make_reward",
            CallArgs::from_positional([OwnedValue::Int(11)]),
            CallOptions::unbounded(),
        )
        .expect("second factory should run");
    let score = runtime_a
        .method(&reward_a, "score")
        .expect("method should resolve");

    let error = runtime_b
        .call_method(
            &reward_b,
            &score,
            CallArgs::from_positional([OwnedValue::Int(5)]),
            CallOptions::unbounded(),
        )
        .expect_err("cached method from another runtime should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::TypeMismatch {
            operation: "VelaMethod belongs to another Runtime"
        }
    );
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
