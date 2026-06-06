use vela_bytecode::compiler::{compile_program_source, compile_program_source_with_options};
use vela_common::{HostObjectId, SourceId};
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_vm::HostExecution;
use vela_vm::error::VmErrorKind;
use vela_vm::owned_value::OwnedValue;

use crate::clock::{TIME_ELAPSED_SINCE_FUNCTION_ID, TIME_NOW_FUNCTION_ID, TIME_TICK_FUNCTION_ID};
use crate::context_schema::{
    CONTEXT_EMIT_METHOD_ID, CONTEXT_HOST_TYPE_ID, CONTEXT_LOG_METHOD_ID, CONTEXT_NOW_FIELD_ID,
    CONTEXT_TICK_FIELD_ID, CONTEXT_TYPE_ID, context_host_type_desc,
};
use crate::engine::Engine;
use crate::permission::Capability;
use vela_reflect::permissions::ReflectPermissionSet;

#[test]
fn engine_time_clock_requires_time_capability() {
    let engine = Engine::builder()
        .with_time_clock(1_700_000_000, 42)
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return time::now();
}
"#,
    )
    .expect("program should compile");

    assert!(matches!(
        engine.into_vm().run_program(&program, "main", &[]),
        Err(error) if error.kind == VmErrorKind::PermissionDenied {
            native: "time::now".to_owned(),
            capability: Capability::Time.as_str().to_owned(),
        }
    ));
}

#[test]
fn engine_time_elapsed_since_requires_time_capability() {
    let engine = Engine::builder()
        .with_time_clock(1_700_000_000, 42)
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return time::elapsed_since(1699999990);
}
"#,
    )
    .expect("program should compile");

    assert!(matches!(
        engine.into_vm().run_program(&program, "main", &[]),
        Err(error) if error.kind == VmErrorKind::PermissionDenied {
            native: "time::elapsed_since".to_owned(),
            capability: Capability::Time.as_str().to_owned(),
        }
    ));
}

#[test]
fn explicit_capabilities_allow_time_but_not_random() {
    let engine = Engine::builder()
        .capability(Capability::Time)
        .with_time_clock(1_700_000_000, 42)
        .with_controlled_random(7)
        .build()
        .expect("engine should build");
    let time_program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return time::now() + time::tick();
}
"#,
    )
    .expect("time program should compile");
    assert_eq!(
        engine
            .clone()
            .into_vm()
            .run_program(&time_program, "main", &[]),
        Ok(OwnedValue::Int(1_700_000_042))
    );

    let random_program = compile_program_source(
        SourceId::new(2),
        r#"
fn main() {
    return math::random(1, 6);
}
"#,
    )
    .expect("random program should compile");
    assert!(matches!(
        engine.into_vm().run_program(&random_program, "main", &[]),
        Err(error) if error.kind == VmErrorKind::PermissionDenied {
            native: "math::random".to_owned(),
            capability: Capability::Random.as_str().to_owned(),
        }
    ));
}

#[test]
fn engine_time_clock_returns_configured_values() {
    let engine = Engine::builder()
        .capability(Capability::Time)
        .with_time_clock(1_700_000_000, 42)
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return time::elapsed_since(1699999990) + time::tick();
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(52))
    );
}

#[test]
fn engine_reflect_call_invokes_capability_gated_time_clock_functions() {
    let engine = Engine::builder()
        .capability(Capability::Time)
        .with_time_clock(1_700_000_000, 42)
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let now = reflect::function("time::now");
    let elapsed = reflect::function("time::elapsed_since");
    return reflect::call(now) + reflect::call(elapsed, 1699999990);
}
"#,
    )
    .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Ok(OwnedValue::Int(1_700_000_010))
    );
    assert!(tx.is_empty());
}

#[test]
fn engine_time_clock_registers_metadata() {
    let engine = Engine::builder()
        .with_time_clock(1, 2)
        .build()
        .expect("engine should build");

    let registry = engine.registry();
    let module = registry
        .module_by_name("time")
        .expect("time module metadata");
    assert_eq!(module.docs.as_deref(), Some("Deterministic time helpers."));
    assert_eq!(module.attrs.get("stdlib"), Some("time"));
    assert_eq!(module.attrs.get("domain"), Some("time"));
    assert_eq!(module.exports.len(), 3);
    assert!(
        module
            .exports
            .iter()
            .any(|export| export.name == "time::now")
    );
    assert!(
        module
            .exports
            .iter()
            .any(|export| export.name == "time::tick")
    );
    assert!(
        module
            .exports
            .iter()
            .any(|export| export.name == "time::elapsed_since")
    );

    let now = registry
        .function_by_name("time::now")
        .expect("time.now metadata");
    let tick = registry
        .function_by_name("time::tick")
        .expect("time.tick metadata");
    let elapsed = registry
        .function_by_name("time::elapsed_since")
        .expect("time.elapsed_since metadata");

    assert_eq!(now.id, TIME_NOW_FUNCTION_ID);
    assert_eq!(now.module.as_deref(), Some("time"));
    assert!(now.params.is_empty());
    assert_eq!(now.return_type.as_deref(), Some("int"));
    assert!(now.access.reflect_visible);
    assert!(now.effects.reads_time);
    assert!(now.access.required_permissions().is_empty());
    assert_eq!(tick.id, TIME_TICK_FUNCTION_ID);
    assert_eq!(tick.module.as_deref(), Some("time"));
    assert!(tick.params.is_empty());
    assert_eq!(tick.return_type.as_deref(), Some("int"));
    assert!(tick.access.reflect_visible);
    assert!(tick.effects.reads_time);
    assert!(tick.access.required_permissions().is_empty());
    assert_eq!(elapsed.id, TIME_ELAPSED_SINCE_FUNCTION_ID);
    assert_eq!(elapsed.module.as_deref(), Some("time"));
    assert_eq!(elapsed.params.len(), 1);
    assert_eq!(elapsed.params[0].name, "start");
    assert_eq!(elapsed.params[0].type_hint.as_deref(), Some("int"));
    assert_eq!(elapsed.return_type.as_deref(), Some("int"));
    assert!(elapsed.access.reflect_visible);
    assert!(elapsed.effects.reads_time);
    assert!(elapsed.access.required_permissions().is_empty());
}

#[test]
fn engine_context_host_schema_registers_metadata() {
    let engine = Engine::builder()
        .with_context_host_schema()
        .build()
        .expect("engine should build");
    let direct_desc = context_host_type_desc();
    assert_eq!(direct_desc.key.id, CONTEXT_TYPE_ID);

    let registry = engine.registry();
    let context = registry
        .type_by_name("Context")
        .expect("context type metadata");
    assert_eq!(context.key.id, CONTEXT_TYPE_ID);
    assert_eq!(context.host_type_id, Some(CONTEXT_HOST_TYPE_ID));
    assert_eq!(context.attrs.get("stdlib"), Some("context"));
    assert_eq!(context.attrs.get("domain"), Some("context"));
    assert_eq!(context.fields.len(), 2);
    assert_eq!(context.fields[0].id, CONTEXT_NOW_FIELD_ID);
    assert_eq!(context.fields[0].name, "now");
    assert_eq!(context.fields[0].type_hint.as_deref(), Some("int"));
    assert_eq!(context.fields[0].attrs.get("stdlib"), Some("context"));
    assert_eq!(context.fields[0].attrs.get("domain"), Some("context"));
    assert_eq!(context.fields[1].id, CONTEXT_TICK_FIELD_ID);
    assert_eq!(context.fields[1].name, "tick");
    assert_eq!(context.fields[1].type_hint.as_deref(), Some("int"));
    assert_eq!(context.fields[1].attrs.get("stdlib"), Some("context"));
    assert_eq!(context.fields[1].attrs.get("domain"), Some("context"));

    let emit = context
        .methods
        .iter()
        .find(|method| method.name == "emit")
        .expect("emit method metadata");
    assert_eq!(emit.id, CONTEXT_EMIT_METHOD_ID);
    assert!(emit.effects.emits_events);
    assert!(emit.access.reflect_callable);
    assert_eq!(emit.params[0].name, "event");
    assert_eq!(emit.params[0].type_hint.as_deref(), Some("string"));
    assert_eq!(emit.return_type.as_deref(), Some("null"));
    assert_eq!(emit.attrs.get("stdlib"), Some("context"));
    assert_eq!(emit.attrs.get("domain"), Some("context"));

    let log = context
        .methods
        .iter()
        .find(|method| method.name == "log")
        .expect("log method metadata");
    assert_eq!(log.id, CONTEXT_LOG_METHOD_ID);
    assert!(log.effects.emits_events);
    assert!(log.access.reflect_callable);
    assert_eq!(log.params[0].name, "level");
    assert_eq!(log.params[1].name, "message");
    assert_eq!(log.return_type.as_deref(), Some("null"));
    assert_eq!(log.attrs.get("stdlib"), Some("context"));
    assert_eq!(log.attrs.get("domain"), Some("context"));
}

#[test]
fn engine_context_host_schema_metadata_is_script_reflectable() {
    let engine = Engine::builder()
        .with_context_host_schema()
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    let context = reflect::type_info("Context");
    let fields = reflect::fields(context);
    let methods = reflect::methods(context);
    let emit = reflect::method(context, "emit");
    let log = reflect::method(context, "log");
    return reflect::docs(context) == "Standard host context object for deterministic time, events, and logging."
        && reflect::attr(context, "stdlib") == "context"
        && reflect::attr(context, "domain") == "context"
        && fields.len() == 2
        && fields[0].name == "now"
        && reflect::docs(fields[0]) == "Current deterministic context timestamp."
        && reflect::attr(fields[0], "stdlib") == "context"
        && reflect::attr(fields[0], "domain") == "context"
        && fields[1].name == "tick"
        && reflect::docs(fields[1]) == "Current deterministic context tick."
        && reflect::attr(fields[1], "stdlib") == "context"
        && reflect::attr(fields[1], "domain") == "context"
        && methods.len() == 2
        && emit.owner == "Context"
        && reflect::docs(emit) == "Records an event emission patch for the host safe point."
        && reflect::attr(emit, "stdlib") == "context"
        && reflect::attr(emit, "domain") == "context"
        && log.owner == "Context"
        && reflect::docs(log) == "Records a log patch for the host safe point."
        && reflect::attr(log, "stdlib") == "context"
        && reflect::attr(log, "domain") == "context";
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Ok(OwnedValue::Bool(true))
    );
    assert!(tx.is_empty());
}

#[test]
fn engine_context_host_schema_lowers_patch_tx_workflows() {
    let engine = Engine::builder()
        .with_context_host_schema()
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(ctx) {
    let stamp = ctx.now + ctx.tick;
    ctx.emit(event = "player.level_checked");
    ctx.log(message = "player.level_checked", level = "info", payload = stamp);
    return stamp;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let ctx = HostRef::new(CONTEXT_HOST_TYPE_ID, HostObjectId::new(99), 1);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(
        HostPath::new(ctx).field(CONTEXT_NOW_FIELD_ID),
        HostValue::Int(1_700_000_000),
    );
    adapter.insert_value(
        HostPath::new(ctx).field(CONTEXT_TICK_FIELD_ID),
        HostValue::Int(42),
    );
    adapter.insert_method_return(CONTEXT_EMIT_METHOD_ID, HostValue::Null);
    adapter.insert_method_return(CONTEXT_LOG_METHOD_ID, HostValue::Null);
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(ctx)],
            &mut host
        ),
        Ok(OwnedValue::Int(1_700_000_042))
    );
    assert_eq!(tx.mutation_count(), 2);
    assert_eq!(
        adapter.method_calls(),
        &[
            (
                HostPath::new(ctx),
                CONTEXT_EMIT_METHOD_ID,
                vec![HostValue::String("player.level_checked".to_owned())],
            ),
            (
                HostPath::new(ctx),
                CONTEXT_LOG_METHOD_ID,
                vec![
                    HostValue::String("info".to_owned()),
                    HostValue::String("player.level_checked".to_owned()),
                    HostValue::Int(1_700_000_042),
                ],
            ),
        ]
    );
}
