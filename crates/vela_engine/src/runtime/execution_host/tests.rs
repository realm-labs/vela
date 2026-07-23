use vela_host::adapter::ScriptStateAdapter;
use vela_host::error::HostErrorKind;
use vela_host::object::ScriptHostObject;
use vela_host::path::HostRef;
use vela_host::slot::HostRefSlots;
use vela_vm::budget::ExecutionBudget;
use vela_vm::heap::ScriptHeap;
use vela_vm::value::Value;

use super::{CallArgRuntime, EXECUTION_HOST_OBJECT_ID_BASE, ExecutionHost, ReentryExecutionHost};
use crate::runtime::{CallArgs, RuntimeExternStateBindings, RuntimeHostArena};

#[test]
fn direct_host_ids_are_allocated_by_the_execution_owner() {
    let shared = vec![1_i64];
    let mut mutable = vec![2_i64];
    let args = CallArgs::new()
        .with_host_ref("shared", &shared)
        .with_host_mut("mutable", &mut mutable);
    let mut extern_states = RuntimeExternStateBindings::new();
    let mut host_arena = RuntimeHostArena::new();
    let mut host_slots = HostRefSlots::new();

    {
        let host = ExecutionHost::new(args, &mut extern_states, &mut host_arena, &mut host_slots);

        assert_eq!(
            host.next_direct_object_id(),
            EXECUTION_HOST_OBJECT_ID_BASE + 2
        );
    }

    let root = HostRef::new(
        shared.host_type_id(),
        vela_common::HostObjectId::new(EXECUTION_HOST_OBJECT_ID_BASE),
        1,
    );
    let handle = host_slots.intern(root);
    assert_eq!(
        handle.generation(),
        1,
        "dropping an unused direct argument must not manufacture and recycle a slot"
    );
}

#[test]
fn nested_execution_uses_one_canonical_generational_slot_namespace() {
    let value = vec![1_i64];
    let args = CallArgs::new().with_host_ref("value", &value);
    let mut extern_states = RuntimeExternStateBindings::new();
    let mut host_arena = RuntimeHostArena::new();
    let mut host_slots = HostRefSlots::new();
    let mut host = ExecutionHost::new(args, &mut extern_states, &mut host_arena, &mut host_slots);
    let root = HostRef::new(
        value.host_type_id(),
        vela_common::HostObjectId::new(EXECUTION_HOST_OBJECT_ID_BASE),
        1,
    );

    let handle = host
        .intern_host_ref(root)
        .expect("root execution should intern canonical host metadata");
    let alias = {
        let child_args = CallArgs::new();
        let mut child = ReentryExecutionHost::new(child_args, &mut host)
            .expect("nested execution should share root host slots");
        let alias = child
            .intern_host_ref(root)
            .expect("nested alias should intern through the root table");
        assert_eq!(
            child
                .resolve_host_ref(alias)
                .expect("nested alias should resolve"),
            root
        );
        alias
    };
    assert_eq!(alias, handle);

    assert_eq!(
        host.release_host_ref(handle)
            .expect("explicit release should return canonical metadata"),
        root
    );
    let error = host
        .resolve_host_ref(alias)
        .expect_err("every copied alias should become stale together");
    assert!(matches!(
        error.kind,
        HostErrorKind::InvalidHostSlot { handle: stale } if stale == alias
    ));

    let replacement = host
        .intern_host_ref(root)
        .expect("released slots should be reusable");
    assert_eq!(replacement.slot(), handle.slot());
    assert_ne!(replacement.generation(), handle.generation());
}

#[test]
fn nested_scope_uses_shared_allocator_and_invalidates_child_ref_on_drop() {
    let root_value = vec![1_i64];
    let child_value = vec![2_i64];
    let args = CallArgs::new().with_host_ref("root", &root_value);
    let mut extern_states = RuntimeExternStateBindings::new();
    let mut host_arena = RuntimeHostArena::new();
    let mut host_slots = HostRefSlots::new();
    let mut host = ExecutionHost::new(args, &mut extern_states, &mut host_arena, &mut host_slots);
    let mut heap = ScriptHeap::default();
    let mut budget = ExecutionBudget::unbounded();
    let program = vela_bytecode::LinkedProgram::new();

    let (child_slot, child_ref) = {
        let child_args = CallArgs::new().with_host_ref("child", &child_value);
        let mut child = ReentryExecutionHost::new(child_args, &mut host)
            .expect("nested scope should allocate its direct binding");
        let values = child
            .resolve_values(
                "child",
                &["child".to_owned()],
                &[false],
                CallArgRuntime::new(1, &program, &mut heap, &mut budget),
            )
            .expect("child binding should resolve");
        let [Value::HostRef(child_ref)] = values.as_slice() else {
            panic!("child binding should resolve to HostRef");
        };
        let child_slot = *child_ref;
        let child_ref = child
            .resolve_host_ref(child_slot)
            .expect("live child slot should resolve");
        (child_slot, child_ref)
    };

    assert_eq!(child_ref.object_id.get(), EXECUTION_HOST_OBJECT_ID_BASE + 1);
    assert_eq!(
        host.next_direct_object_id(),
        EXECUTION_HOST_OBJECT_ID_BASE + 2
    );
    assert!(host.args.direct_binding(child_ref).is_none());
    assert!(matches!(
        host.resolve_host_ref(child_slot),
        Err(error) if matches!(error.kind, HostErrorKind::InvalidHostSlot { .. })
    ));
}
