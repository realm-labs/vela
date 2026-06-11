use std::cell::Cell;
use std::collections::BTreeMap;

use vela_common::{HostMethodId, HostObjectId, HostTypeId, SourceId};
use vela_def::{FieldId, TypeId};
use vela_host::access::HostAccess;
use vela_host::adapter::{GlobalBinding, ScriptStateAdapter};
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::mock::MockStateAdapter;
use vela_host::object::ScriptHostObject;
use vela_host::path::{HostPath, HostRef};
use vela_host::resolved::{
    HostAccessOp, HostAccessSpec, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess,
    ResolvedHostAccessKind,
};
use vela_host::target::{HostPathArg, HostPathPart, HostTargetInstance};
use vela_host::value::HostValue;
use vela_reflect::registry::{FieldDesc, MethodDesc, MethodParamDesc, TypeDesc, TypeKey};
use vela_vm::error::VmErrorKind;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::runtime::{
    CallArgs, CallOptions, Runtime, RuntimeImage, SharedRuntime, VelaFunction, VelaMethod,
    VelaValue,
};

use super::player_type;

mod managed_values;

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
    let program = engine
        .compile_source(
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
        )
        .expect("compile runtime image code-view source");
    let mut runtime = Runtime::new(engine, program);

    let value = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("runtime call should execute");

    assert_eq!(
        runtime.value_to_owned(&value),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(16)))
    );
}

#[test]
fn runtime_call_checks_public_entry_parameter_contracts() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(value: i64) {
    return value;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);

    let error = runtime
        .call(
            "main",
            CallArgs::new().with_value("value", "bad"),
            CallOptions::unbounded(),
        )
        .expect_err("runtime host entry should check parameter guards");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "string".to_owned(),
            debug_name: "value".to_owned(),
        }
    );
}

fn direct_player_type() -> TypeDesc {
    TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
        .host_type(HostTypeId::new(1))
        .field(FieldDesc::new(FieldId::new(1), "level").writable(true))
        .field(FieldDesc::new(FieldId::new(2), "inventory").writable(true))
        .method(
            MethodDesc::new(HostMethodId::new(10), "grant_exp")
                .param(MethodParamDesc::new("amount").type_hint("int")),
        )
        .method(
            MethodDesc::new(HostMethodId::new(11), "add")
                .param(MethodParamDesc::new("key").type_hint("string"))
                .param(MethodParamDesc::new("amount").type_hint("int")),
        )
}

#[derive(Default)]
struct CountingGlobalLookupAdapter {
    global_ref_calls: Cell<usize>,
    global_ref_by_slot_calls: Cell<usize>,
}

impl ScriptStateAdapter for CountingGlobalLookupAdapter {
    fn global_ref(&self, global: GlobalBinding<'_>) -> HostResult<HostRef> {
        if global.slot.is_some() {
            self.global_ref_by_slot_calls
                .set(self.global_ref_by_slot_calls.get().saturating_add(1));
        }
        self.global_ref_calls
            .set(self.global_ref_calls.get().saturating_add(1));
        Err(HostError {
            kind: HostErrorKind::MissingGlobal {
                name: global.name.to_owned(),
            },
            source_span: None,
        })
    }

    fn read_host(
        &self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        Err(HostError {
            kind: HostErrorKind::MissingPath {
                path: target.to_diagnostic_path().to_host_path(),
            },
            source_span: None,
        })
    }

    fn write_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        _value: HostValue,
    ) -> HostResult<()> {
        Err(HostError {
            kind: HostErrorKind::MissingPath {
                path: target.to_diagnostic_path().to_host_path(),
            },
            source_span: None,
        })
    }

    fn mutate_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        _op: HostMutationOp,
        _rhs: HostValue,
    ) -> HostResult<()> {
        Err(HostError {
            kind: HostErrorKind::MissingPath {
                path: target.to_diagnostic_path().to_host_path(),
            },
            source_span: None,
        })
    }

    fn remove_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<()> {
        Err(HostError {
            kind: HostErrorKind::MissingPath {
                path: target.to_diagnostic_path().to_host_path(),
            },
            source_span: None,
        })
    }

    fn call_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        _method: HostMethodId,
        _args: &[HostValue],
    ) -> HostResult<HostValue> {
        Err(HostError {
            kind: HostErrorKind::MissingPath {
                path: target.to_diagnostic_path().to_host_path(),
            },
            source_span: None,
        })
    }
}

impl ScriptHostObject for DirectPlayer {
    fn host_type_id(&self) -> HostTypeId {
        HostTypeId::new(1)
    }

    fn resolve_host_target(&self, spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess> {
        let epoch = HostSchemaEpoch::new(0);
        match (spec.op, spec.plan.parts.as_slice()) {
            (
                HostAccessOp::Read | HostAccessOp::Write | HostAccessOp::Mutate(_),
                [HostPathPart::Field(field)],
            ) if *field == FieldId::new(1) => Ok(ResolvedHostAccess::direct_field(0, epoch)),
            (
                HostAccessOp::Read | HostAccessOp::Write | HostAccessOp::Mutate(_),
                [HostPathPart::Field(field), _],
            ) if *field == FieldId::new(2) => Ok(ResolvedHostAccess::direct_field(1, epoch)),
            (HostAccessOp::Read, [HostPathPart::Field(field)]) if *field == FieldId::new(2) => {
                Ok(ResolvedHostAccess::direct_field(1, epoch))
            }
            (HostAccessOp::Call(method), []) if method == HostMethodId::new(10) => {
                Ok(ResolvedHostAccess::direct_method(0, epoch))
            }
            (HostAccessOp::Call(method), [HostPathPart::Field(field)])
                if method == HostMethodId::new(11) && *field == FieldId::new(2) =>
            {
                Ok(ResolvedHostAccess::direct_method(1, epoch))
            }
            _ => Ok(ResolvedHostAccess::generic_target(epoch)),
        }
    }

    fn read_resolved_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        match target.plan.parts.as_slice() {
            [HostPathPart::Field(field)] if *field == FieldId::new(1) => {
                require_direct_field(access, 0)?;
                Ok(HostValue::Scalar(vela_common::ScalarValue::I64(self.level)))
            }
            [HostPathPart::Field(field)] if *field == FieldId::new(2) => {
                require_direct_field(access, 1)?;
                Ok(HostValue::Null)
            }
            [HostPathPart::Field(field), key_part] if *field == FieldId::new(2) => {
                require_direct_field(access, 1)?;
                let key = direct_target_key(target, key_part)?;
                Ok(HostValue::i64(*self.inventory.get(key).unwrap_or(&0)))
            }
            _ => Err(HostError {
                kind: HostErrorKind::MissingPath {
                    path: target.to_diagnostic_path().to_host_path(),
                },
                source_span: None,
            }),
        }
    }

    fn write_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        match (target.plan.parts.as_slice(), value) {
            (
                [HostPathPart::Field(field)],
                HostValue::Scalar(vela_common::ScalarValue::I64(level)),
            ) if *field == FieldId::new(1) => {
                require_direct_field(access, 0)?;
                self.level = level;
                Ok(())
            }
            (
                [HostPathPart::Field(field), key_part],
                HostValue::Scalar(vela_common::ScalarValue::I64(count)),
            ) if *field == FieldId::new(2) => {
                require_direct_field(access, 1)?;
                let key = direct_target_key(target, key_part)?.to_owned();
                self.inventory.insert(key, count);
                Ok(())
            }
            _ => Err(HostError {
                kind: HostErrorKind::MissingPath {
                    path: target.to_diagnostic_path().to_host_path(),
                },
                source_span: None,
            }),
        }
    }

    fn call_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        match (target.plan.parts.as_slice(), method, args) {
            ([], method, [HostValue::Scalar(vela_common::ScalarValue::I64(amount))])
                if method == HostMethodId::new(10) =>
            {
                require_direct_method(access, 0)?;
                self.level += amount;
                Ok(HostValue::Scalar(vela_common::ScalarValue::I64(self.level)))
            }
            (
                [HostPathPart::Field(field)],
                method,
                [
                    HostValue::String(key),
                    HostValue::Scalar(vela_common::ScalarValue::I64(amount)),
                ],
            ) if *field == FieldId::new(2) && method == HostMethodId::new(11) => {
                require_direct_method(access, 1)?;
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

fn require_direct_field(access: ResolvedHostAccess, slot: u32) -> HostResult<()> {
    if access.adapter_kind == ResolvedHostAccessKind::DirectField(slot) {
        Ok(())
    } else {
        Err(invalid_direct_access())
    }
}

fn require_direct_method(access: ResolvedHostAccess, slot: u32) -> HostResult<()> {
    if access.adapter_kind == ResolvedHostAccessKind::DirectMethod(slot) {
        Ok(())
    } else {
        Err(invalid_direct_access())
    }
}

fn invalid_direct_access() -> HostError {
    HostError {
        kind: HostErrorKind::InvalidArgument {
            expected: "resolved direct host access",
        },
        source_span: None,
    }
}

fn direct_target_key<'a>(
    target: HostTargetInstance<'a>,
    part: &'a HostPathPart,
) -> HostResult<&'a str> {
    match part {
        HostPathPart::ConstKey(key) => Ok(key),
        HostPathPart::DynKey { arg } | HostPathPart::DynIndex { arg } => match target.arg(*arg) {
            Some(HostPathArg::Key(key)) => Ok(key),
            Some(HostPathArg::Index(_)) | None => Err(HostError {
                kind: HostErrorKind::MissingPath {
                    path: target.to_diagnostic_path().to_host_path(),
                },
                source_span: None,
            }),
        },
        HostPathPart::Field(_) | HostPathPart::VariantField(_) | HostPathPart::ConstIndex(_) => {
            Err(HostError {
                kind: HostErrorKind::MissingPath {
                    path: target.to_diagnostic_path().to_host_path(),
                },
                source_span: None,
            })
        }
    }
}

#[test]
fn runtime_call_args_bind_named_values_by_function_params() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player: Player, amount, bonus = 1) {
    player.level += amount;
    return player.level + bonus;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let level = HostPath::new(player).field(super::FieldId::new(1));
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(
        level.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
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

    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(12))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&level),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(11)))
    );
}

#[test]
fn runtime_call_args_accept_positional_values() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(left, right) {
    return left * 10 + right;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut args = CallArgs::from_positional([
        OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
        OwnedValue::Scalar(vela_common::ScalarValue::I64(7)),
    ]);

    let result = runtime
        .call_args_raw(
            "main",
            &mut args,
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect("runtime call args should run");

    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(27))
    );
}

#[test]
fn runtime_call_args_reject_duplicate_named_values() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source(SourceId::new(1), "fn main(value) { return value; }")
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
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "duplicate named call argument"
        }
    );
}

#[test]
fn runtime_call_args_reject_unknown_named_values() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source(SourceId::new(1), "fn main(value) { return value; }")
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
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "unknown named call argument"
        }
    );
}

#[test]
fn runtime_call_args_reject_mixed_modes() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source(SourceId::new(1), "fn main(value) { return value; }")
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
        error.kind(),
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
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player: Player, amount) {
    player.level += amount;
    return player.level;
}
"#,
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

    assert_eq!(
        runtime.value_to_owned(&output),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(13)))
    );
    assert_eq!(player.level, 13);
}

#[test]
fn runtime_call_args_host_mut_writes_string_key_map_path_to_rust_object() {
    let engine = Engine::builder()
        .register_type(direct_player_type())
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player: Player, amount) {
    player.inventory["gold"] += amount;
    return player.inventory["gold"];
}
"#,
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

    assert_eq!(
        runtime.value_to_owned(&output),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
    assert_eq!(player.inventory.get("gold"), Some(&7));
}

#[test]
fn runtime_call_args_host_mut_dispatches_root_and_child_host_methods() {
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .field(FieldDesc::new(FieldId::new(1), "level").writable(true))
                .field(
                    FieldDesc::new(FieldId::new(2), "inventory")
                        .writable(true)
                        .type_hint("Inventory"),
                )
                .method(
                    MethodDesc::new(HostMethodId::new(10), "grant_exp")
                        .param(MethodParamDesc::new("amount").type_hint("int")),
                ),
        )
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(2), "Inventory"))
                .host_type(HostTypeId::new(2))
                .method(
                    MethodDesc::new(HostMethodId::new(11), "add")
                        .param(MethodParamDesc::new("key").type_hint("string"))
                        .param(MethodParamDesc::new("amount").type_hint("int")),
                ),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player: Player) {
    let level = player.grant_exp(2);
    player.inventory.add("gold", 5);
    return level + player.inventory["gold"];
}
"#,
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

    assert_eq!(
        runtime.value_to_owned(&output),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(16)))
    );
    assert_eq!(player.level, 11);
    assert_eq!(player.inventory.get("gold"), Some(&5));
}

#[test]
fn runtime_call_returns_runtime_value() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player: Player, amount) {
    player.level += amount;
    return player.level;
}
"#,
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

    assert_eq!(
        runtime.value_to_owned(&output),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(13)))
    );
    assert_eq!(player.level, 13);
}

#[test]
fn runtime_cached_entry_calls_function() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(amount, multiplier = 2) {
    return amount * multiplier;
}
"#,
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
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(14)))
    );
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(21)))
    );
}

#[test]
fn runtime_cached_entry_rejects_function_from_another_runtime() {
    let engine = Engine::builder().build().expect("engine should build");
    let source = r#"
fn main(amount) {
    return amount * 2;
}
"#;
    let program_a = engine
        .compile_source(SourceId::new(1), source)
        .expect("program should compile");
    let program_b = engine
        .compile_source(SourceId::new(2), source)
        .expect("program should compile");
    let runtime_a = Runtime::new(engine.clone(), program_a);
    let mut runtime_b = Runtime::new(engine, program_b);
    let main = runtime_a.entry("main").expect("entry should resolve");

    let error = runtime_b
        .call(
            &main,
            CallArgs::from_positional([OwnedValue::Scalar(vela_common::ScalarValue::I64(7))]),
            CallOptions::unbounded(),
        )
        .expect_err("cached entry from another runtime should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "VelaFunction belongs to another Runtime"
        }
    );
}

#[test]
fn runtime_call_method_on_runtime_value_by_name_and_cached_method() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source(
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
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(17)))
    );
    assert_eq!(
        runtime.value_to_owned(&score_by_cached_method),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(19)))
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
    let program_a = engine
        .compile_source(SourceId::new(1), source)
        .expect("program should compile");
    let program_b = engine
        .compile_source(SourceId::new(2), source)
        .expect("program should compile");
    let mut runtime_a = Runtime::new(engine.clone(), program_a);
    let mut runtime_b = Runtime::new(engine, program_b);
    let reward_a = runtime_a
        .call(
            "make_reward",
            CallArgs::from_positional([OwnedValue::Scalar(vela_common::ScalarValue::I64(7))]),
            CallOptions::unbounded(),
        )
        .expect("first factory should run");
    let reward_b = runtime_b
        .call(
            "make_reward",
            CallArgs::from_positional([OwnedValue::Scalar(vela_common::ScalarValue::I64(11))]),
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
            CallArgs::from_positional([OwnedValue::Scalar(vela_common::ScalarValue::I64(5))]),
            CallOptions::unbounded(),
        )
        .expect_err("cached method from another runtime should fail");

    assert_eq!(
        error.kind(),
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
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player: Player) {
    player.level += 1;
    return player.level;
}
"#,
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

    assert_eq!(
        report.value,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(10))
    );
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
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player: Player) {
    player.level += 1;
    return player.level;
}
"#,
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
        error.kind(),
        VmErrorKind::Host(HostErrorKind::PermissionDenied {
            action: "write",
            ..
        })
    ));
    assert_eq!(player.level, 9);
}
