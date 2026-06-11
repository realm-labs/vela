use vela_bytecode::UnlinkedProgram;
use vela_common::{HostObjectId, HostTypeId, SourceId};
use vela_def::{FieldId, TypeId};
use vela_host::access::HostAccess;
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::value::HostValue;
use vela_reflect::registry::TypeKey;
use vela_vm::HostExecution;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::context::NativeCallContext;
use crate::engine::Engine;
use crate::native::{EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint};
use crate::permission::Capability;

fn run_linked_program_with_host(
    engine: &Engine,
    program: &UnlinkedProgram,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let linked = engine
        .link_program(program)
        .expect("engine typed host test program should link");
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
fn engine_registers_typed_host_native_functions() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_typed_host_native_fn::<(HostRef, i64), _>(
            NativeFunctionDesc::new("game::typed_host_set_level", NativeFunctionId::new(106))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_host_set_level,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player) {
    game::typed_host_set_level(player, 19);
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
        Ok(OwnedValue::Int(1)),
    );
}

#[test]
fn typed_host_native_conversion_errors_before_host_write() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_typed_host_native_fn::<(HostRef, i64), _>(
            NativeFunctionDesc::new("game::typed_host_set_level", NativeFunctionId::new(107))
                .access(FunctionAccess::public()),
            typed_host_set_level,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    game::typed_host_set_level("not a host", 19);
    return 1;
}
"#,
        )
        .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert!(matches!(
        run_linked_program_with_host(&engine, &program, &[], &mut host),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch {
                operation: "host ref",
            })
    ));
}

#[test]
fn typed_host_native_maps_host_result_errors() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_typed_host_native_fn::<(HostRef, bool), _>(
            NativeFunctionDesc::new("game::typed_host_require_write", NativeFunctionId::new(247))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("allowed", TypeHint::Bool)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_host_require_write,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return game::typed_host_require_write(player, false);
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
        )
        .map_err(|error| error.kind()),
        Err(VmErrorKind::Host(HostErrorKind::PermissionDenied {
            path: HostPath::new(host_ref),
            action: "write",
        })),
    );
}

fn typed_host_set_level(host: &mut HostExecution<'_>, player: HostRef, level: i64) -> VmResult<()> {
    host.access.write_diagnostic_path(
        host.adapter,
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(())
}

fn typed_host_require_write(
    _host: &mut HostExecution<'_>,
    player: HostRef,
    allowed: bool,
) -> HostResult<i64> {
    if allowed {
        Ok(13)
    } else {
        Err(HostError {
            kind: HostErrorKind::PermissionDenied {
                path: HostPath::new(player),
                action: "write",
            },
            source_span: None,
        })
    }
}

#[test]
fn engine_registers_four_arg_typed_host_native_functions() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_typed_host_native_fn::<(HostRef, i64, i64, i64), _>(
            NativeFunctionDesc::new("game::typed_host_sum_level", NativeFunctionId::new(222))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_host_sum_level,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return game::typed_host_sum_level(player, 2, 3, 4);
}
"#,
        )
        .expect("program should compile");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[OwnedValue::HostRef(player)], &mut host),
        Ok(OwnedValue::Int(9)),
    );
}

#[test]
fn engine_registers_five_arg_typed_host_native_functions() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_typed_host_native_fn::<(HostRef, i64, i64, i64, i64), _>(
            NativeFunctionDesc::new("game::typed_host_sum5_level", NativeFunctionId::new(230))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .param("d", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_host_sum5_level,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return game::typed_host_sum5_level(player, 2, 3, 4, 5);
}
"#,
        )
        .expect("program should compile");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[OwnedValue::HostRef(player)], &mut host),
        Ok(OwnedValue::Int(14)),
    );
}

#[test]
fn engine_registers_six_arg_typed_host_native_functions() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_typed_host_native_fn::<(HostRef, i64, i64, i64, i64, i64), _>(
            NativeFunctionDesc::new("game::typed_host_sum6_level", NativeFunctionId::new(238))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .param("d", TypeHint::Int)
                .param("e", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_host_sum6_level,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return game::typed_host_sum6_level(player, 2, 3, 4, 5, 6);
}
"#,
        )
        .expect("program should compile");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[OwnedValue::HostRef(player)], &mut host),
        Ok(OwnedValue::Int(20)),
    );
}

fn typed_host_sum_level(
    host: &mut HostExecution<'_>,
    player: HostRef,
    a: i64,
    b: i64,
    c: i64,
) -> VmResult<i64> {
    let level = a + b + c;
    host.access.write_diagnostic_path(
        host.adapter,
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(level)
}

fn typed_host_sum5_level(
    host: &mut HostExecution<'_>,
    player: HostRef,
    a: i64,
    b: i64,
    c: i64,
    d: i64,
) -> VmResult<i64> {
    let level = a + b + c + d;
    host.access.write_diagnostic_path(
        host.adapter,
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(level)
}

fn typed_host_sum6_level(
    host: &mut HostExecution<'_>,
    player: HostRef,
    a: i64,
    b: i64,
    c: i64,
    d: i64,
    e: i64,
) -> VmResult<i64> {
    let level = a + b + c + d + e;
    host.access.write_diagnostic_path(
        host.adapter,
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(level)
}

#[test]
fn engine_registers_typed_context_host_native_functions() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_typed_context_host_native_fn::<(HostRef, i64), _>(
            NativeFunctionDesc::new("game::typed_set_level", NativeFunctionId::new(104))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Bool)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_set_level,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return game::typed_set_level(player, 17);
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
        Ok(OwnedValue::Bool(true)),
    );
}

#[test]
fn typed_context_host_native_conversion_errors_before_host_write() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_typed_context_host_native_fn::<(HostRef, i64), _>(
            NativeFunctionDesc::new("game::typed_set_level", NativeFunctionId::new(105))
                .access(FunctionAccess::public()),
            typed_set_level,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    game::typed_set_level("not a host", 17);
    return 1;
}
"#,
        )
        .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert!(matches!(
        run_linked_program_with_host(&engine, &program, &[], &mut host),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch {
                operation: "host ref",
            })
    ));
}

#[test]
fn typed_context_host_native_maps_host_result_errors() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_typed_context_host_native_fn::<(HostRef, bool), _>(
            NativeFunctionDesc::new(
                "game::typed_context_require_write",
                NativeFunctionId::new(248),
            )
            .param(
                "player",
                TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
            )
            .param("allowed", TypeHint::Bool)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_write())
            .access(FunctionAccess::public()),
            typed_context_require_write,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return game::typed_context_require_write(player, false);
}
"#,
        )
        .expect("program should compile");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[OwnedValue::HostRef(player)], &mut host)
            .map_err(|error| error.kind()),
        Err(VmErrorKind::Host(HostErrorKind::PermissionDenied {
            path: HostPath::new(player),
            action: "write",
        })),
    );
}

fn typed_set_level(
    ctx: &mut NativeCallContext<'_, '_>,
    player: HostRef,
    level: i64,
) -> VmResult<bool> {
    ctx.charge_instructions(10)?;
    let has_permission = ctx.has_capability(Capability::HostWrite);
    ctx.set_path(
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(has_permission)
}

fn typed_context_require_write(
    ctx: &mut NativeCallContext<'_, '_>,
    player: HostRef,
    allowed: bool,
) -> HostResult<i64> {
    if allowed && ctx.has_capability(Capability::HostWrite) {
        Ok(21)
    } else {
        Err(HostError {
            kind: HostErrorKind::PermissionDenied {
                path: HostPath::new(player),
                action: "write",
            },
            source_span: None,
        })
    }
}

#[test]
fn engine_registers_four_arg_typed_context_host_native_functions() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_typed_context_host_native_fn::<(HostRef, i64, i64, i64), _>(
            NativeFunctionDesc::new("game::typed_context_sum_level", NativeFunctionId::new(223))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_context_sum_level,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return game::typed_context_sum_level(player, 5, 6, 7);
}
"#,
        )
        .expect("program should compile");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[OwnedValue::HostRef(player)], &mut host),
        Ok(OwnedValue::Int(18)),
    );
}

#[test]
fn engine_registers_five_arg_typed_context_host_native_functions() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_typed_context_host_native_fn::<(HostRef, i64, i64, i64, i64), _>(
            NativeFunctionDesc::new("game::typed_context_sum5_level", NativeFunctionId::new(231))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .param("d", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_context_sum5_level,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return game::typed_context_sum5_level(player, 5, 6, 7, 8);
}
"#,
        )
        .expect("program should compile");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[OwnedValue::HostRef(player)], &mut host),
        Ok(OwnedValue::Int(26)),
    );
}

#[test]
fn engine_registers_six_arg_typed_context_host_native_functions() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_typed_context_host_native_fn::<(HostRef, i64, i64, i64, i64, i64), _>(
            NativeFunctionDesc::new("game::typed_context_sum6_level", NativeFunctionId::new(239))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .param("d", TypeHint::Int)
                .param("e", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            typed_context_sum6_level,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return game::typed_context_sum6_level(player, 5, 6, 7, 8, 9);
}
"#,
        )
        .expect("program should compile");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[OwnedValue::HostRef(player)], &mut host),
        Ok(OwnedValue::Int(35)),
    );
}

fn typed_context_sum_level(
    ctx: &mut NativeCallContext<'_, '_>,
    player: HostRef,
    a: i64,
    b: i64,
    c: i64,
) -> VmResult<i64> {
    ctx.charge_instructions(1)?;
    let level = a + b + c;
    ctx.set_path(
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(level)
}

fn typed_context_sum5_level(
    ctx: &mut NativeCallContext<'_, '_>,
    player: HostRef,
    a: i64,
    b: i64,
    c: i64,
    d: i64,
) -> VmResult<i64> {
    ctx.charge_instructions(1)?;
    let level = a + b + c + d;
    ctx.set_path(
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(level)
}

fn typed_context_sum6_level(
    ctx: &mut NativeCallContext<'_, '_>,
    player: HostRef,
    a: i64,
    b: i64,
    c: i64,
    d: i64,
    e: i64,
) -> VmResult<i64> {
    ctx.charge_instructions(1)?;
    let level = a + b + c + d + e;
    ctx.set_path(
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(level)
}
