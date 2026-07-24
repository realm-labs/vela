use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use parking_lot::RwLock;
use vela_common::{HostObjectId, HostTypeId, ScalarValue};
use vela_host::access::HostAccess;
use vela_host::adapter::ScriptStateAdapter;
use vela_host::error::{HostErrorKind, HostResult};
use vela_host::lease::{HostLeaseKind, ScopedHostLeaseSlot};
use vela_host::object::ScriptHostObject;
use vela_host::path::HostRef;
use vela_host::protocol::{HostCollectionProjection, HostCollectionSnapshot};
use vela_host::resolved::{HostAccessOp, HostAccessSpec};
use vela_host::slot::HostRefSlots;
use vela_host::target::{HostTargetInstance, HostTargetPlan};
use vela_host::value::HostValue;
use vela_vm::budget::ExecutionBudget;
use vela_vm::heap::ScriptHeap;
use vela_vm::value::Value;

use super::{
    CallArgRuntime, EXECUTION_HOST_OBJECT_ID_BASE, ExecutionHost, ReentryExecutionHost,
    ScopedHostBinding, ScopedHostObjectBinding,
};
use crate::runtime::{CallArgs, RuntimeExternStateBindings, RuntimeHostArena};

struct DenseScopedObject;

impl ScriptHostObject for DenseScopedObject {
    fn host_type_id(&self) -> HostTypeId {
        HostTypeId::new(93)
    }

    fn read_resolved_host(
        &self,
        _access: vela_host::resolved::ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        Ok(HostValue::Unit)
    }
}

#[test]
fn complex_vec_elements_are_scoped_children_of_the_parent_lease() {
    let mut values = vec![vec![10_i64, 20], vec![30]];
    let parent_type = values.host_type_id();
    let child_type = values[0].host_type_id();
    let args = CallArgs::new().with_host_mut("values", &mut values);
    let mut extern_states = RuntimeExternStateBindings::new();
    let mut host_arena = RuntimeHostArena::new();
    let mut host_slots = HostRefSlots::new();
    let mut host = ExecutionHost::new(args, &mut extern_states, &mut host_arena, &mut host_slots);
    let parent = HostRef::new(
        parent_type,
        HostObjectId::new(EXECUTION_HOST_OBJECT_ID_BASE),
        1,
    );
    let element_plan = HostTargetPlan::new(parent.type_id).const_index(0);
    let element_target = HostTargetInstance::new(parent, &element_plan, &[]);
    let element_access = host
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &element_plan))
        .expect("complex element access should resolve from the collection type");
    let access = HostAccess::new();
    let HostValue::HostRef(child) = access
        .read_resolved_scoped(&mut host, element_access, element_target, None)
        .expect("complex element reads should retain a scoped child HostRef")
    else {
        panic!("complex element read should not serialize or expose a Rust reference");
    };
    let alias = child;
    assert_eq!(child.type_id, child_type);
    assert_eq!(
        host.host_receiver_access(child),
        HostLeaseKind::Exclusive,
        "a child of a mutable collection view must preserve exclusive access"
    );

    let child_value_plan = HostTargetPlan::new(child.type_id).const_index(1);
    let child_value_target = HostTargetInstance::new(child, &child_value_plan, &[]);
    let child_read = host
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &child_value_plan))
        .expect("nested child read should resolve through the child type");
    assert_eq!(
        access
            .read_resolved_scoped(&mut host, child_read, child_value_target, None)
            .expect("nested child read should observe the parent element"),
        HostValue::Scalar(ScalarValue::I64(20))
    );
    let child_write = host
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Write, &child_value_plan))
        .expect("nested child write should resolve through the child type");
    let mut access = access;
    access
        .write_resolved(
            &mut host,
            child_write,
            child_value_target,
            HostValue::Scalar(ScalarValue::I64(25)),
            None,
        )
        .expect("nested child write should mutate the original collection element");

    host.release_scoped_host(child)
        .expect("uncontended child should release its parent lease");
    let error = access
        .read_resolved_scoped(&mut host, child_read, child_value_target, None)
        .expect_err("every copied alias must expire with the scoped child");
    assert!(matches!(
        error.kind,
        HostErrorKind::ExpiredBorrowedHostRef { .. }
    ));
    assert_eq!(alias, child);
    drop(host);
    assert_eq!(values[0], vec![10, 25]);
}

#[test]
fn complex_vec_projection_returns_one_releasable_child_per_element() {
    let values = vec![vec![1_i64], vec![2]];
    let parent_type = values.host_type_id();
    let args = CallArgs::new().with_host_ref("values", &values);
    let mut extern_states = RuntimeExternStateBindings::new();
    let mut host_arena = RuntimeHostArena::new();
    let mut host_slots = HostRefSlots::new();
    let mut host = ExecutionHost::new(args, &mut extern_states, &mut host_arena, &mut host_slots);
    let parent = HostRef::new(
        parent_type,
        HostObjectId::new(EXECUTION_HOST_OBJECT_ID_BASE),
        1,
    );
    let plan = HostTargetPlan::new(parent.type_id);
    let target = HostTargetInstance::new(parent, &plan, &[]);
    let resolved = host
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &plan))
        .expect("complex collection projection should resolve");
    let access = HostAccess::new();
    let HostCollectionSnapshot::Items(items) = access
        .snapshot_collection_resolved_scoped(
            &mut host,
            resolved,
            target,
            HostCollectionProjection::Values,
            None,
        )
        .expect("complex collection projection should retain scoped children")
    else {
        panic!("array projection should return items");
    };
    let roots = items
        .into_iter()
        .map(|value| match value {
            HostValue::HostRef(root) => root,
            _ => panic!("complex collection projection must preserve host identity"),
        })
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);
    assert_ne!(roots[0], roots[1]);
    assert!(
        roots
            .iter()
            .all(|root| host.host_receiver_access(*root) == HostLeaseKind::Shared)
    );
    for root in roots {
        host.release_scoped_host(root)
            .expect("each projection child should release independently");
    }
}

#[test]
fn empty_complex_vec_projection_does_not_create_a_scoped_group() {
    let values: Vec<Vec<i64>> = Vec::new();
    let parent_type = values.host_type_id();
    let args = CallArgs::new().with_host_ref("values", &values);
    let mut extern_states = RuntimeExternStateBindings::new();
    let mut host_arena = RuntimeHostArena::new();
    let mut host_slots = HostRefSlots::new();
    let mut host = ExecutionHost::new(args, &mut extern_states, &mut host_arena, &mut host_slots);
    let parent = HostRef::new(
        parent_type,
        HostObjectId::new(EXECUTION_HOST_OBJECT_ID_BASE),
        1,
    );
    let plan = HostTargetPlan::new(parent.type_id);
    let target = HostTargetInstance::new(parent, &plan, &[]);
    let resolved = host
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &plan))
        .expect("empty complex collection projection should resolve");
    let snapshot = HostAccess::new()
        .snapshot_collection_resolved_scoped(
            &mut host,
            resolved,
            target,
            HostCollectionProjection::Values,
            None,
        )
        .expect("empty complex projection should not require a non-empty lease group");
    assert_eq!(snapshot, HostCollectionSnapshot::Items(Vec::new()));
    assert!(host.scoped_hosts.is_empty());
}

#[test]
fn complex_map_values_preserve_key_identity_and_write_through() {
    let mut values = BTreeMap::from([("primary".to_owned(), vec![7_i64])]);
    let parent_type = values.host_type_id();
    let args = CallArgs::new().with_host_mut("values", &mut values);
    let mut extern_states = RuntimeExternStateBindings::new();
    let mut host_arena = RuntimeHostArena::new();
    let mut host_slots = HostRefSlots::new();
    let mut host = ExecutionHost::new(args, &mut extern_states, &mut host_arena, &mut host_slots);
    let parent = HostRef::new(
        parent_type,
        HostObjectId::new(EXECUTION_HOST_OBJECT_ID_BASE),
        1,
    );
    let value_plan = HostTargetPlan::new(parent.type_id).const_key("primary");
    let value_target = HostTargetInstance::new(parent, &value_plan, &[]);
    let value_access = host
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &value_plan))
        .expect("complex map value access should resolve");
    let mut access = HostAccess::new();
    let HostValue::HostRef(child) = access
        .read_resolved_scoped(&mut host, value_access, value_target, None)
        .expect("complex map value should retain a scoped child")
    else {
        panic!("complex map value must preserve host identity");
    };
    let nested_plan = HostTargetPlan::new(child.type_id).const_index(0);
    let nested_target = HostTargetInstance::new(child, &nested_plan, &[]);
    let nested_write = host
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Write, &nested_plan))
        .expect("nested map child write should resolve");
    access
        .write_resolved(
            &mut host,
            nested_write,
            nested_target,
            HostValue::Scalar(ScalarValue::I64(9)),
            None,
        )
        .expect("nested map child write should mutate the keyed value");
    host.release_scoped_host(child)
        .expect("map value child should release");
    drop(host);
    assert_eq!(values["primary"], vec![9]);
}

#[test]
fn complex_hash_map_entry_projection_is_deterministic() {
    let values = HashMap::from([
        ("zeta".to_owned(), vec![2_i64]),
        ("alpha".to_owned(), vec![1_i64]),
    ]);
    let parent_type = values.host_type_id();
    let args = CallArgs::new().with_host_ref("values", &values);
    let mut extern_states = RuntimeExternStateBindings::new();
    let mut host_arena = RuntimeHostArena::new();
    let mut host_slots = HostRefSlots::new();
    let mut host = ExecutionHost::new(args, &mut extern_states, &mut host_arena, &mut host_slots);
    let parent = HostRef::new(
        parent_type,
        HostObjectId::new(EXECUTION_HOST_OBJECT_ID_BASE),
        1,
    );
    let plan = HostTargetPlan::new(parent.type_id);
    let target = HostTargetInstance::new(parent, &plan, &[]);
    let resolved = host
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &plan))
        .expect("complex hash map projection should resolve");
    let HostCollectionSnapshot::Entries(entries) = HostAccess::new()
        .snapshot_collection_resolved_scoped(
            &mut host,
            resolved,
            target,
            HostCollectionProjection::Entries,
            None,
        )
        .expect("complex hash map entries should retain scoped value children")
    else {
        panic!("map entry projection should return entries");
    };
    assert_eq!(
        entries
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>(),
        vec![
            HostValue::String("alpha".to_owned()),
            HostValue::String("zeta".to_owned())
        ]
    );
    for (_, value) in entries {
        let HostValue::HostRef(root) = value else {
            panic!("complex map entry value must preserve host identity");
        };
        host.release_scoped_host(root)
            .expect("projected map child should release");
    }
}

#[test]
fn complex_fixed_array_elements_are_scoped_children() {
    let mut values = [vec![3_i64], vec![4]];
    let parent_type = values.host_type_id();
    let args = CallArgs::new().with_host_mut("values", &mut values);
    let mut extern_states = RuntimeExternStateBindings::new();
    let mut host_arena = RuntimeHostArena::new();
    let mut host_slots = HostRefSlots::new();
    let mut host = ExecutionHost::new(args, &mut extern_states, &mut host_arena, &mut host_slots);
    let parent = HostRef::new(
        parent_type,
        HostObjectId::new(EXECUTION_HOST_OBJECT_ID_BASE),
        1,
    );
    let element_plan = HostTargetPlan::new(parent.type_id).const_index(1);
    let element_target = HostTargetInstance::new(parent, &element_plan, &[]);
    let element_access = host
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &element_plan))
        .expect("complex fixed-array element should resolve");
    let HostValue::HostRef(child) = HostAccess::new()
        .read_resolved_scoped(&mut host, element_access, element_target, None)
        .expect("complex fixed-array element should retain a scoped child")
    else {
        panic!("complex fixed-array element must preserve host identity");
    };
    host.release_scoped_host(child)
        .expect("fixed-array child should release");
}

#[test]
fn complex_slice_elements_are_scoped_children() {
    let mut values = [vec![5_i64]];
    let parent_type = crate::standard::standard_slice_host_type_id::<Vec<i64>>();
    let mut args = CallArgs::new();
    args.push_slice_mut("values", values.as_mut_slice());
    let mut extern_states = RuntimeExternStateBindings::new();
    let mut host_arena = RuntimeHostArena::new();
    let mut host_slots = HostRefSlots::new();
    let mut host = ExecutionHost::new(args, &mut extern_states, &mut host_arena, &mut host_slots);
    let parent = HostRef::new(
        parent_type,
        HostObjectId::new(EXECUTION_HOST_OBJECT_ID_BASE),
        1,
    );
    let outer_plan = HostTargetPlan::new(parent.type_id).const_index(0);
    let outer_target = HostTargetInstance::new(parent, &outer_plan, &[]);
    let outer_access = host
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &outer_plan))
        .expect("complex slice element should resolve");
    let mut access = HostAccess::new();
    let HostValue::HostRef(child) = access
        .read_resolved_scoped(&mut host, outer_access, outer_target, None)
        .expect("complex slice element should retain a scoped child")
    else {
        panic!("complex slice element must preserve host identity");
    };

    let scalar_plan = HostTargetPlan::new(child.type_id).const_index(0);
    let scalar_target = HostTargetInstance::new(child, &scalar_plan, &[]);
    let scalar_write = host
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Write, &scalar_plan))
        .expect("slice child scalar write should resolve");
    access
        .write_resolved(
            &mut host,
            scalar_write,
            scalar_target,
            HostValue::Scalar(ScalarValue::I64(8)),
            None,
        )
        .expect("slice child should write through its parent lease");
    host.release_scoped_host(child)
        .expect("slice child should release after nested use");
    drop(host);
    assert_eq!(values[0], vec![8]);
}

#[test]
fn nested_standard_collection_views_keep_exact_identity_and_release_order() {
    let mut values = [vec![vec![10_i64, 20]]];
    let parent_type = crate::standard::standard_slice_host_type_id::<Vec<Vec<i64>>>();
    let outer_type = crate::standard::standard_collection_host_type_id::<Vec<Vec<i64>>>();
    let inner_type = crate::standard::standard_collection_host_type_id::<Vec<i64>>();
    assert_ne!(parent_type, outer_type);
    assert_ne!(outer_type, inner_type);

    let mut args = CallArgs::new();
    args.push_slice_mut("values", values.as_mut_slice());
    let mut extern_states = RuntimeExternStateBindings::new();
    let mut host_arena = RuntimeHostArena::new();
    let mut host_slots = HostRefSlots::new();
    let mut host = ExecutionHost::new(args, &mut extern_states, &mut host_arena, &mut host_slots);
    let parent = HostRef::new(
        parent_type,
        HostObjectId::new(EXECUTION_HOST_OBJECT_ID_BASE),
        1,
    );
    let outer_plan = HostTargetPlan::new(parent.type_id).const_index(0);
    let outer_target = HostTargetInstance::new(parent, &outer_plan, &[]);
    let outer_access = host
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &outer_plan))
        .expect("outer standard collection access should resolve");
    let mut access = HostAccess::new();
    let HostValue::HostRef(outer) = access
        .read_resolved_scoped(&mut host, outer_access, outer_target, None)
        .expect("outer standard collection should retain a scoped HostRef")
    else {
        panic!("outer standard collection must preserve host identity");
    };
    assert_eq!(outer.type_id, outer_type);

    let inner_plan = HostTargetPlan::new(outer.type_id).const_index(0);
    let inner_target = HostTargetInstance::new(outer, &inner_plan, &[]);
    let inner_access = host
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, &inner_plan))
        .expect("inner standard collection access should resolve");
    let HostValue::HostRef(inner) = access
        .read_resolved_scoped(&mut host, inner_access, inner_target, None)
        .expect("inner standard collection should retain a distinct scoped HostRef")
    else {
        panic!("inner standard collection must preserve host identity");
    };
    assert_eq!(inner.type_id, inner_type);

    let error = host
        .release_scoped_host(outer)
        .expect_err("a parent view cannot release while its child is live");
    assert!(matches!(error.kind, HostErrorKind::BorrowStillInUse { .. }));

    let scalar_plan = HostTargetPlan::new(inner.type_id).const_index(1);
    let scalar_target = HostTargetInstance::new(inner, &scalar_plan, &[]);
    let scalar_write = host
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Write, &scalar_plan))
        .expect("scalar write through the inner view should resolve");
    access
        .write_resolved(
            &mut host,
            scalar_write,
            scalar_target,
            HostValue::Scalar(ScalarValue::I64(25)),
            None,
        )
        .expect("inner standard collection should write through both parent leases");

    host.release_scoped_host(inner)
        .expect("inner view should release first");
    host.release_scoped_host(outer)
        .expect("outer view should release after its child");

    let inner_error = access
        .read_resolved_scoped(&mut host, scalar_write, scalar_target, None)
        .expect_err("released inner aliases must expire");
    assert!(matches!(
        inner_error.kind,
        HostErrorKind::ExpiredBorrowedHostRef { .. }
    ));
    let outer_error = access
        .read_resolved_scoped(&mut host, inner_access, inner_target, None)
        .expect_err("released outer aliases must expire");
    assert!(matches!(
        outer_error.kind,
        HostErrorKind::ExpiredBorrowedHostRef { .. }
    ));

    drop(host);
    assert_eq!(values[0][0], vec![10, 25]);
}

#[test]
fn scoped_hosts_use_dense_generation_checked_identity() {
    let args = CallArgs::new();
    let mut extern_states = RuntimeExternStateBindings::new();
    let mut host_arena = RuntimeHostArena::new();
    let mut host_slots = HostRefSlots::new();
    let mut host = ExecutionHost::new(args, &mut extern_states, &mut host_arena, &mut host_slots);
    let type_id = HostTypeId::new(93);
    let object: ScopedHostLeaseSlot<'_> = Arc::new(RwLock::new(Box::new(DenseScopedObject)));
    let dense_handle = host.scoped_hosts.insert_with(|handle| ScopedHostBinding {
        borrow_lease_id: ExecutionHost::borrow_lease_id(handle),
        type_id,
        access: HostLeaseKind::Shared,
        object: ScopedHostObjectBinding::Single(object),
        activity: Arc::new(()),
    });
    let root = ExecutionHost::scoped_root(dense_handle, type_id);
    let alias = root;
    let borrow_lease_id = host
        .scoped_borrow_lease_id(root)
        .expect("live scoped root should have one borrow-group identity");

    assert!(!host.scoped_hosts.spilled());
    assert_eq!(host.scoped_borrow_lease_id(alias), Some(borrow_lease_id));
    assert_eq!(host.host_receiver_access(root), HostLeaseKind::Shared);
    assert_eq!(
        host.host_receiver_access(HostRef::new(
            HostTypeId::new(94),
            root.object_id,
            root.generation,
        )),
        HostLeaseKind::Exclusive
    );
    assert_eq!(
        host.host_receiver_access(HostRef::new(
            root.type_id,
            root.object_id,
            root.generation + 1,
        )),
        HostLeaseKind::Exclusive
    );

    let compact = host
        .intern_host_ref(root)
        .expect("scoped host should enter the canonical compact table");
    host.release_scoped_host(root)
        .expect("uncontended scoped host should release");
    assert!(host.scoped_hosts.is_empty());
    assert_eq!(host.expired_scoped_hosts.get(&root), Some(&borrow_lease_id));
    assert_eq!(
        host.resolve_host_ref(compact)
            .expect("released compact alias should retain its diagnostic root"),
        root
    );
    assert!(matches!(
        host.take_execution_host_lease(root, HostLeaseKind::Shared),
        Err(error) if matches!(error.kind, HostErrorKind::ExpiredBorrowedHostRef { .. })
    ));

    let replacement: ScopedHostLeaseSlot<'_> = Arc::new(RwLock::new(Box::new(DenseScopedObject)));
    let replacement_handle = host.scoped_hosts.insert_with(|handle| ScopedHostBinding {
        borrow_lease_id: ExecutionHost::borrow_lease_id(handle),
        type_id,
        access: HostLeaseKind::Shared,
        object: ScopedHostObjectBinding::Single(replacement),
        activity: Arc::new(()),
    });
    let replacement_root = ExecutionHost::scoped_root(replacement_handle, type_id);
    let replacement_borrow_lease_id = host
        .scoped_borrow_lease_id(replacement_root)
        .expect("replacement root should have one borrow-group identity");
    assert_eq!(replacement_handle.slot(), dense_handle.slot());
    assert_ne!(replacement_handle.generation(), dense_handle.generation());
    assert_ne!(replacement_root, root);
    assert_ne!(replacement_borrow_lease_id, borrow_lease_id);
    assert!(host.scoped_binding(root).is_none());
    assert!(host.scoped_binding(replacement_root).is_some());
}

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
fn execution_host_lease_guards_stay_inline_for_common_arities() {
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

    let inline = host
        .take_execution_host_leases(&[(root, HostLeaseKind::Shared); 8])
        .expect("common-arity execution-host leases should be acquired");
    assert!(!inline.spilled());
    drop(inline);

    let spilled = host
        .take_execution_host_leases(&[(root, HostLeaseKind::Shared); 9])
        .expect("wide execution-host lease sets should remain supported");
    assert!(spilled.spilled());
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
