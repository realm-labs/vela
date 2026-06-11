use vela_bytecode::UnlinkedProgram;
use vela_common::SourceId;
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_reflect::permissions::ReflectPermissionSet;
use vela_vm::HostExecution;
use vela_vm::Vm;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::permission::Capability;
use crate::random::MATH_RANDOM_FUNCTION_ID;

fn linked_vm(engine: &Engine, program: &UnlinkedProgram) -> (Vm, vela_bytecode::LinkedProgram) {
    let linked = engine
        .link_program(program)
        .expect("engine random test program should link");
    (engine.into_vm_for_program(program), linked)
}

fn run_linked_program(engine: &Engine, program: &UnlinkedProgram) -> VmResult<OwnedValue> {
    let (vm, linked) = linked_vm(engine, program);
    vm.run_linked_program(&linked, "main", &[])
}

fn run_linked_program_with_host(
    engine: &Engine,
    program: &UnlinkedProgram,
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let (vm, linked) = linked_vm(engine, program);
    let mut budget = ExecutionBudget::unbounded();
    vm.run_linked_program_with_host_budget_and_caches(&linked, "main", &[], host, &mut budget, None)
}

#[test]
fn engine_controlled_random_requires_permission() {
    let engine = Engine::builder()
        .with_controlled_random(7)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return math::random(1, 6);
}
"#,
        )
        .expect("program should compile");

    assert!(matches!(
        run_linked_program(&engine, &program),
        Err(error) if error.kind() == VmErrorKind::PermissionDenied {
            native: "math::random".to_owned(),
            capability: Capability::Random.as_str().to_owned(),
        }
    ));
}

#[test]
fn engine_controlled_random_is_seeded_and_bounded() {
    let source = r#"
fn main() {
    let first = math::random(1, 6);
    let second = math::random(10, 12);
    if first >= 1 && first <= 6 && second >= 10 && second <= 12 {
        return first * 100 + second;
    }
    return 0;
}
"#;
    let first_engine = Engine::builder()
        .capability(Capability::Random)
        .with_controlled_random(42)
        .build()
        .expect("first engine should build");
    let second_engine = Engine::builder()
        .capability(Capability::Random)
        .with_controlled_random(42)
        .build()
        .expect("second engine should build");
    let program = first_engine
        .compile_source(SourceId::new(1), source)
        .expect("program should compile");

    let first =
        run_linked_program(&first_engine, &program).expect("first random run should succeed");
    let second =
        run_linked_program(&second_engine, &program).expect("second random run should succeed");

    assert_eq!(first, second);
    assert_ne!(first, OwnedValue::Scalar(vela_common::ScalarValue::I64(0)));
}

#[test]
fn engine_controlled_random_registers_metadata() {
    let engine = Engine::builder()
        .with_controlled_random(1)
        .build()
        .expect("engine should build");

    let registry = engine.registry();
    let math = registry.module_by_name("math").expect("math module");
    let function = registry
        .function_by_name("math::random")
        .expect("math::random metadata");
    assert_eq!(math.exports.len(), 1);
    assert!(
        math.exports
            .iter()
            .any(|export| export.name == "math::random")
    );
    assert_eq!(function.id, MATH_RANDOM_FUNCTION_ID);
    assert_eq!(function.module.as_deref(), Some("math"));
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[0].type_hint.as_deref(), Some("i64"));
    assert_eq!(function.params[1].type_hint.as_deref(), Some("i64"));
    assert_eq!(function.return_type.as_deref(), Some("i64"));
    assert_eq!(function.attrs.get("stdlib"), Some("math"));
    assert!(function.access.required_permissions().is_empty());
    assert!(function.effects.uses_random);
    assert!(function.access.reflect_callable);
}

#[test]
fn engine_controlled_random_extends_standard_math_metadata() {
    let engine = Engine::builder()
        .with_standard_natives()
        .with_controlled_random(1)
        .capability(Capability::Random)
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");

    let registry = engine.registry();
    let math = registry.module_by_name("math").expect("math module");
    let random = registry
        .function_by_name("math::random")
        .expect("math::random metadata");
    let max = registry.function_by_name("math::max").expect("math::max");

    assert_eq!(math.exports.len(), 15);
    assert!(math.exports.iter().any(|export| export.name == "math::max"));
    assert!(
        math.exports
            .iter()
            .any(|export| export.name == "math::random")
    );
    assert_eq!(
        math.docs.as_deref(),
        Some("Deterministic math standard-library helpers.")
    );
    assert_eq!(
        max.docs.as_deref(),
        Some("Returns the larger numeric value.")
    );
    assert_eq!(
        random.docs.as_deref(),
        Some("Returns a deterministic seeded integer in the inclusive range.")
    );
    assert!(random.access.required_permissions().is_empty());
    assert!(random.access.reflect_visible);
    assert!(random.access.reflect_callable);
    assert!(!random.effects.reads_host);
    assert!(!random.effects.writes_host);
    assert!(!random.effects.emits_events);
    assert!(random.effects.uses_random);

    let program = engine
        .compile_source(
            SourceId::new(2),
            r#"
fn main() {
    let math_exports = reflect::exports("math");
    let random = reflect::function("math::random");
    let max = reflect::function("math::max");
    let required = reflect::required_permissions(random);
    let effects = reflect::effects(random);
    return math_exports.len() == 15
        && math_exports.contains("math::max")
        && math_exports.contains("math::random")
        && reflect::docs(max) == "Returns the larger numeric value."
        && reflect::docs(random) == "Returns a deterministic seeded integer in the inclusive range."
        && required.len() == 0
        && !effects.reads_host
        && !effects.writes_host
        && !effects.emits_events
        && effects.uses_random;
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

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &mut host),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn engine_controlled_random_reflect_call_is_seeded_and_bounded() {
    let source = r#"
fn main() {
    let random = reflect::function("math::random");
    let first = reflect::call(random, 1, 6);
    let second = reflect::call(random, 10, 12);
    if first >= 1 && first <= 6 && second >= 10 && second <= 12 {
        return first * 100 + second;
    }
    return 0;
}
"#;
    let first_engine = Engine::builder()
        .capability(Capability::Random)
        .reflection_permissions(ReflectPermissionSet::all())
        .with_controlled_random(42)
        .build()
        .expect("first engine should build");
    let second_engine = Engine::builder()
        .capability(Capability::Random)
        .reflection_permissions(ReflectPermissionSet::all())
        .with_controlled_random(42)
        .build()
        .expect("second engine should build");
    let program = first_engine
        .compile_source(SourceId::new(1), source)
        .expect("program should compile");
    let mut first_adapter = MockStateAdapter::new();
    let mut first_tx = HostAccess::new();
    let mut first_host = HostExecution {
        adapter: &mut first_adapter,
        access: &mut first_tx,
        script_globals: None,
    };
    let mut second_adapter = MockStateAdapter::new();
    let mut second_tx = HostAccess::new();
    let mut second_host = HostExecution {
        adapter: &mut second_adapter,
        access: &mut second_tx,
        script_globals: None,
    };

    let first = run_linked_program_with_host(&first_engine, &program, &mut first_host)
        .expect("first reflected random run should succeed");
    let second = run_linked_program_with_host(&second_engine, &program, &mut second_host)
        .expect("second reflected random run should succeed");

    assert_eq!(first, second);
    assert_ne!(first, OwnedValue::Scalar(vela_common::ScalarValue::I64(0)));
}
