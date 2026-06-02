use vela_bytecode::compiler::compile_program_source;
use vela_common::SourceId;
use vela_host::mock::MockStateAdapter;
use vela_host::tx::PatchTx;
use vela_reflect::permissions::ReflectPermissionSet;
use vela_vm::HostExecution;
use vela_vm::error::VmErrorKind;
use vela_vm::value::Value;

use crate::engine::Engine;
use crate::random::{CONTROLLED_RANDOM_PERMISSION, MATH_RANDOM_FUNCTION_ID};

#[test]
fn engine_controlled_random_requires_permission() {
    let engine = Engine::builder()
        .with_controlled_random(7)
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return math.random(1, 6);
}
"#,
    )
    .expect("program should compile");

    assert!(matches!(
        engine.into_vm().run_program(&program, "main", &[]),
        Err(error) if error.kind == VmErrorKind::PermissionDenied {
            native: "math.random".to_owned(),
            permission: CONTROLLED_RANDOM_PERMISSION.to_owned(),
        }
    ));
}

#[test]
fn engine_controlled_random_is_seeded_and_bounded() {
    let source = r#"
fn main() {
    let first = math.random(1, 6);
    let second = math.random(10, 12);
    if first >= 1 && first <= 6 && second >= 10 && second <= 12 {
        return first * 100 + second;
    }
    return 0;
}
"#;
    let program = compile_program_source(SourceId::new(1), source).expect("program should compile");
    let first_engine = Engine::builder()
        .grant_permission(CONTROLLED_RANDOM_PERMISSION)
        .with_controlled_random(42)
        .build()
        .expect("first engine should build");
    let second_engine = Engine::builder()
        .grant_permission(CONTROLLED_RANDOM_PERMISSION)
        .with_controlled_random(42)
        .build()
        .expect("second engine should build");

    let first = first_engine
        .into_vm()
        .run_program(&program, "main", &[])
        .expect("first random run should succeed");
    let second = second_engine
        .into_vm()
        .run_program(&program, "main", &[])
        .expect("second random run should succeed");

    assert_eq!(first, second);
    assert_ne!(first, Value::Int(0));
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
        .function_by_name("math.random")
        .expect("math.random metadata");
    assert_eq!(math.exports.len(), 1);
    assert!(
        math.exports
            .iter()
            .any(|export| export.name == "math.random")
    );
    assert_eq!(function.id, MATH_RANDOM_FUNCTION_ID);
    assert_eq!(function.module.as_deref(), Some("math"));
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[0].type_hint.as_deref(), Some("int"));
    assert_eq!(function.params[1].type_hint.as_deref(), Some("int"));
    assert_eq!(function.return_type.as_deref(), Some("int"));
    assert_eq!(function.attrs.get("stdlib"), Some("math"));
    assert_eq!(
        function.access.required_permissions(),
        &[CONTROLLED_RANDOM_PERMISSION.to_owned()]
    );
    assert!(function.access.reflect_callable);
}

#[test]
fn engine_controlled_random_extends_standard_math_metadata() {
    let engine = Engine::builder()
        .with_standard_natives()
        .with_controlled_random(1)
        .grant_permission(CONTROLLED_RANDOM_PERMISSION)
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");

    let registry = engine.registry();
    let math = registry.module_by_name("math").expect("math module");
    let random = registry
        .function_by_name("math.random")
        .expect("math.random metadata");
    let max = registry.function_by_name("math.max").expect("math.max");

    assert_eq!(math.exports.len(), 15);
    assert!(math.exports.iter().any(|export| export.name == "math.max"));
    assert!(
        math.exports
            .iter()
            .any(|export| export.name == "math.random")
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
    assert_eq!(
        random.access.required_permissions(),
        &[CONTROLLED_RANDOM_PERMISSION.to_owned()]
    );
    assert!(random.access.reflect_visible);
    assert!(random.access.reflect_callable);
    assert!(!random.effects.reads_host);
    assert!(!random.effects.writes_host);
    assert!(!random.effects.emits_events);

    let program = compile_program_source(
        SourceId::new(2),
        r#"
fn main() {
    let math_exports = reflect.exports("math");
    let random = reflect.function("math.random");
    let max = reflect.function("math.max");
    let required = reflect.required_permissions(random);
    let effects = reflect.effects(random);
    return math_exports.len() == 15
        && math_exports.contains("math.max")
        && math_exports.contains("math.random")
        && reflect.docs(max) == "Returns the larger numeric value."
        && reflect.docs(random) == "Returns a deterministic seeded integer in the inclusive range."
        && required.len() == 1
        && required[0] == "std.random"
        && !effects.reads_host
        && !effects.writes_host
        && !effects.emits_events;
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
        Ok(Value::Bool(true))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_controlled_random_reflect_call_is_seeded_and_bounded() {
    let source = r#"
fn main() {
    let random = reflect.function("math.random");
    let first = reflect.call(random, 1, 6);
    let second = reflect.call(random, 10, 12);
    if first >= 1 && first <= 6 && second >= 10 && second <= 12 {
        return first * 100 + second;
    }
    return 0;
}
"#;
    let program = compile_program_source(SourceId::new(1), source).expect("program should compile");
    let first_engine = Engine::builder()
        .grant_permission(CONTROLLED_RANDOM_PERMISSION)
        .reflection_permissions(ReflectPermissionSet::all())
        .with_controlled_random(42)
        .build()
        .expect("first engine should build");
    let second_engine = Engine::builder()
        .grant_permission(CONTROLLED_RANDOM_PERMISSION)
        .reflection_permissions(ReflectPermissionSet::all())
        .with_controlled_random(42)
        .build()
        .expect("second engine should build");
    let mut first_adapter = MockStateAdapter::new();
    let mut first_tx = PatchTx::new();
    let mut first_host = HostExecution {
        adapter: &mut first_adapter,
        tx: &mut first_tx,
    };
    let mut second_adapter = MockStateAdapter::new();
    let mut second_tx = PatchTx::new();
    let mut second_host = HostExecution {
        adapter: &mut second_adapter,
        tx: &mut second_tx,
    };

    let first = first_engine
        .into_vm()
        .run_program_with_host(&program, "main", &[], &mut first_host)
        .expect("first reflected random run should succeed");
    let second = second_engine
        .into_vm()
        .run_program_with_host(&program, "main", &[], &mut second_host)
        .expect("second reflected random run should succeed");

    assert_eq!(first, second);
    assert_ne!(first, Value::Int(0));
    assert!(first_tx.patches().is_empty());
    assert!(second_tx.patches().is_empty());
}
