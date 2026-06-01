use vela_bytecode::compiler::compile_program_source;
use vela_common::SourceId;
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
    let function = registry
        .function_by_name("math.random")
        .expect("math.random metadata");
    assert_eq!(function.id, MATH_RANDOM_FUNCTION_ID);
    assert_eq!(function.module.as_deref(), Some("math"));
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[0].type_hint.as_deref(), Some("int"));
    assert_eq!(function.params[1].type_hint.as_deref(), Some("int"));
    assert_eq!(function.return_type.as_deref(), Some("int"));
    assert_eq!(
        function.access.required_permissions(),
        &[CONTROLLED_RANDOM_PERMISSION.to_owned()]
    );
}
