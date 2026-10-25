use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};

use vela_reflect::error::ReflectErrorKind;
use vela_reflect::permissions::{ReflectPermissionSet, ReflectPolicy};
use vela_vm::error::VmErrorKind;

#[test]
fn setting_only_a_budget_keeps_reflection_disabled() {
    let engine = Engine::builder()
        .reflection_lookup_budget(1)
        .build()
        .expect("explicit reflection fixture should initialize");
    assert!(
        engine
            .compile_source("fn main() { return reflect::types(); }")
            .is_err()
    );
}

#[test]
fn explicit_reflection_preserves_budget_in_either_setter_order() {
    let builders = [
        Engine::builder()
            .reflection_lookup_budget(1)
            .reflection_policy(ReflectPolicy::read_only()),
        Engine::builder()
            .reflection_policy(ReflectPolicy::read_only())
            .reflection_lookup_budget(1),
        Engine::builder()
            .reflection_lookup_budget(1)
            .reflection_permissions(ReflectPermissionSet::read_only()),
        Engine::builder()
            .reflection_permissions(ReflectPermissionSet::read_only())
            .reflection_lookup_budget(1),
    ];
    for builder in builders {
        let engine = builder.build().expect("engine fixture should build");
        let program = engine
            .compile_source("fn main() { reflect::types(); return reflect::types(); }")
            .expect("explicit reflection fixture should initialize");
        let mut runtime = Runtime::new_compiled(engine, program)
            .expect("explicit reflection fixture should initialize");
        let error = runtime
            .call(
                "main",
                CallArgs::new(),
                CallOptions::new(10_000, 1024 * 1024, 64),
            )
            .expect_err("second lookup must exhaust the explicit budget");
        assert_eq!(
            error.kind(),
            VmErrorKind::Reflect(ReflectErrorKind::LookupBudgetExceeded { limit: 1 })
        );
    }
}
