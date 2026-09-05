use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_host::adapter::ScriptStateAdapter;
use vela_host::lease::HostLeaseKind;
use vela_host::path::HostRef;
use vela_host::slot::HostRefSlots;
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

#[test]
fn custom_adapter_default_does_not_grant_exclusive_receiver_access() {
    let slots = HostRefSlots::default();
    let root = HostRef::new(
        vela_common::HostTypeId::new(1),
        vela_common::HostObjectId::new(1),
        0,
    );
    assert_eq!(slots.host_receiver_access(root), HostLeaseKind::Shared);
}
