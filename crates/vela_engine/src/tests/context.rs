use vela_bytecode::UnlinkedProgram;
use vela_common::SourceId;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;
use vela_vm::{HostExecution, Vm};

use crate::clock::{TIME_ELAPSED_SINCE_FUNCTION_ID, TIME_NOW_FUNCTION_ID, TIME_TICK_FUNCTION_ID};
use crate::engine::Engine;
use crate::permission::Capability;
use vela_reflect::permissions::ReflectPermissionSet;

fn linked_vm(
    engine: &Engine,
    program: &UnlinkedProgram,
) -> (Vm, std::sync::Arc<vela_bytecode::LinkedArtifact>) {
    let linked = engine
        .link_test_program(program)
        .expect("engine test program should link");
    (engine.into_vm_for_program(program), linked)
}

fn run_linked_program(
    engine: &Engine,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
) -> VmResult<OwnedValue> {
    let (vm, linked) = linked_vm(engine, program);
    vm.run_linked_program(&linked, entry, args)
}

fn run_linked_program_with_host(
    engine: &Engine,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let (vm, linked) = linked_vm(engine, program);
    let mut budget = ExecutionBudget::unbounded();
    vm.run_linked_program_with_host_budget_and_caches(&linked, entry, args, host, &mut budget, None)
}

#[test]
fn engine_time_clock_requires_time_capability() {
    let engine = Engine::builder()
        .with_time_clock(1_700_000_000, 42)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn main() {
    return time::now();
}
"#,
        )
        .expect("program should compile");

    assert!(matches!(
        run_linked_program(&engine, &program, "main", &[]),
        Err(error) if error.kind() == VmErrorKind::PermissionDenied {
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
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn main() {
    return time::elapsed_since(1699999990);
}
"#,
        )
        .expect("program should compile");

    assert!(matches!(
        run_linked_program(&engine, &program, "main", &[]),
        Err(error) if error.kind() == VmErrorKind::PermissionDenied {
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
    let time_program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn main() {
    return time::now() + time::tick();
}
"#,
        )
        .expect("time program should compile");
    assert_eq!(
        run_linked_program(&engine, &time_program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(
            1_700_000_042
        )))
    );

    let random_program = engine
        .compile_source_with_id(
            SourceId::new(2),
            r#"
fn main() {
    return math::random(1, 6);
}
"#,
        )
        .expect("random program should compile");
    assert!(matches!(
        run_linked_program(&engine, &random_program, "main", &[]),
        Err(error) if error.kind() == VmErrorKind::PermissionDenied {
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
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn main() {
    return time::elapsed_since(1699999990) + time::tick();
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(52)))
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
    let program = engine
        .compile_source_with_id(
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

    let mut host = HostExecution { state_values: None };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, "main", &[], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(
            1_700_000_010
        )))
    );
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
    assert_eq!(now.return_type.as_deref(), Some("i64"));
    assert!(now.access.reflect_visible);
    assert!(now.effects.reads_time);
    assert!(now.access.required_permissions().is_empty());
    assert_eq!(tick.id, TIME_TICK_FUNCTION_ID);
    assert_eq!(tick.module.as_deref(), Some("time"));
    assert!(tick.params.is_empty());
    assert_eq!(tick.return_type.as_deref(), Some("i64"));
    assert!(tick.access.reflect_visible);
    assert!(tick.effects.reads_time);
    assert!(tick.access.required_permissions().is_empty());
    assert_eq!(elapsed.id, TIME_ELAPSED_SINCE_FUNCTION_ID);
    assert_eq!(elapsed.module.as_deref(), Some("time"));
    assert_eq!(elapsed.params.len(), 1);
    assert_eq!(elapsed.params[0].name, "start");
    assert_eq!(elapsed.params[0].type_hint.as_deref(), Some("i64"));
    assert_eq!(elapsed.return_type.as_deref(), Some("i64"));
    assert!(elapsed.access.reflect_visible);
    assert!(elapsed.effects.reads_time);
    assert!(elapsed.access.required_permissions().is_empty());
}
