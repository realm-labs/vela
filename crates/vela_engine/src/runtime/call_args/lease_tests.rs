use std::any::Any;
use std::cell::Cell;

use vela_common::HostTypeId;
use vela_host::lease::HostLeaseKind;
use vela_host::object::ScriptHostObject;
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;

use super::{CallArg, CallArgs, HostArgBinding, HostRef};

struct LeaseHost;

impl ScriptHostObject for LeaseHost {
    fn host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0x1EA5E)
    }

    fn lease_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    fn lease_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn read_resolved_host(
        &self,
        _access: vela_host::resolved::ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
    ) -> vela_host::error::HostResult<HostValue> {
        Ok(HostValue::Unit)
    }
}

struct NonSyncLeaseHost(Cell<i64>);

impl ScriptHostObject for NonSyncLeaseHost {
    fn host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0x5EED)
    }

    fn lease_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    fn lease_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn read_resolved_host(
        &self,
        _access: vela_host::resolved::ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
    ) -> vela_host::error::HostResult<HostValue> {
        Ok(HostValue::i64(self.0.get()))
    }
}

struct BorrowedOpaqueHost<'a> {
    value: &'a mut i64,
}

impl ScriptHostObject for BorrowedOpaqueHost<'_> {
    fn host_type_id(&self) -> HostTypeId {
        HostTypeId::new(45)
    }

    fn read_resolved_host(
        &self,
        _access: vela_host::resolved::ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
    ) -> vela_host::error::HostResult<HostValue> {
        Ok(HostValue::i64(*self.value))
    }
}

#[test]
fn multi_lease_acquisition_rolls_back_on_conflict() {
    let mut host = LeaseHost;
    let mut args = CallArgs::new().with_host_mut("host", &mut host);
    let mut next = 1_u64 << 63;
    args.assign_direct_host_refs(&mut next);
    let root = bound_root(&args);

    let error = match args.take_host_leases(&[
        (root, HostLeaseKind::Exclusive),
        (root, HostLeaseKind::Exclusive),
    ]) {
        Ok(_) => panic!("duplicate exclusive request should fail atomically"),
        Err(error) => error,
    };
    assert!(matches!(
        error.kind,
        vela_host::error::HostErrorKind::HostObjectBusy { .. }
    ));

    let lease = args
        .take_host_lease(root, HostLeaseKind::Exclusive)
        .expect("failed acquisition should restore the first lease");
    drop(lease);
}

#[test]
fn acquired_lease_guards_stay_inline_for_common_arities() {
    let host = LeaseHost;
    let mut args = CallArgs::new().with_host_ref("host", &host);
    let mut next = 1_u64 << 63;
    args.assign_direct_host_refs(&mut next);
    let root = bound_root(&args);

    let inline = args
        .take_host_leases(&[(root, HostLeaseKind::Shared); 8])
        .expect("common-arity shared leases should be acquired");
    assert!(!inline.spilled());
    drop(inline);

    let spilled = args
        .take_host_leases(&[(root, HostLeaseKind::Shared); 9])
        .expect("wide shared lease sets should still be supported");
    assert!(spilled.spilled());
}

#[test]
fn direct_host_bindings_use_dense_slots_across_mixed_arguments() {
    let first = LeaseHost;
    let second = LeaseHost;
    let mut args = CallArgs::new();
    args.push(1_i64)
        .push_host_ref("first", &first)
        .push(2_i64)
        .push_host_ref("second", &second);
    let base = 1_u64 << 63;
    let mut next = base;
    args.assign_direct_host_refs(&mut next);

    assert_eq!(
        args.direct_host_slots
            .iter()
            .map(|(_, slot)| slot.entry_index)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert!(!args.direct_host_slots.spilled());
    assert_eq!(next, base + 2);

    let roots = args
        .direct_host_slots
        .iter()
        .map(|(_, slot)| match &args.entries[slot.entry_index as usize] {
            CallArg::NamedHost {
                host_ref: Some(root),
                ..
            } => *root,
            _ => panic!("dense direct-host slot should point at a bound host argument"),
        })
        .collect::<Vec<_>>();
    assert!(args.direct_binding(roots[0]).is_some());
    assert!(args.direct_binding(roots[1]).is_some());

    let wrong_type = HostRef::new(HostTypeId::new(9), roots[0].object_id, roots[0].generation);
    let wrong_generation = HostRef::new(roots[0].type_id, roots[0].object_id, 2);
    assert!(args.direct_binding(wrong_type).is_none());
    assert!(args.direct_binding(wrong_generation).is_none());
}

#[test]
fn preassigned_reborrow_uses_its_child_binding_without_reacquiring_parent() {
    let mut host = LeaseHost;
    let root = HostRef::new(host.host_type_id(), vela_common::HostObjectId::new(41), 7);
    let mut args = CallArgs::new();
    args.push_reborrowed_host_mut("host", root, &mut host);
    let mut next = 1_u64 << 63;
    args.assign_direct_host_refs(&mut next);

    assert_eq!(next, 1_u64 << 63);
    assert!(args.direct_binding(root).is_some());
    assert_eq!(args.direct_host_refs().count(), 0);
    let lease = args
        .take_host_lease(root, HostLeaseKind::Exclusive)
        .expect("a nested reborrow should lease its child binding");
    assert!(lease.is_exclusive());
}

#[test]
fn mutable_origin_uses_one_exclusive_root_for_every_receiver_view() {
    fn require_send<T: Send>(_: &T) {}

    let mut host = NonSyncLeaseHost(Cell::new(7));
    let mut args = CallArgs::new().with_host_mut("host", &mut host);
    let mut next = 1_u64 << 63;
    args.assign_direct_host_refs(&mut next);
    let root = bound_root(&args);

    let first = args
        .take_host_lease(root, HostLeaseKind::Shared)
        .expect("a shared receiver view should acquire the exclusive mutable root");
    require_send(&first);
    assert!(first.is_exclusive());

    let binding = args
        .direct_binding(root)
        .expect("mutable binding should remain registered");
    let HostArgBinding::Mutable { object } = binding else {
        panic!("binding should have mutable origin");
    };
    assert!(
        object.try_lock().is_none(),
        "the mutable root remains exclusively leased"
    );
    let conflict = match args.take_host_lease(root, HostLeaseKind::Exclusive) {
        Ok(_) => panic!("a second acquisition must conflict with the exclusive root"),
        Err(error) => error,
    };
    assert!(matches!(
        conflict.kind,
        vela_host::error::HostErrorKind::HostObjectBusy { .. }
    ));

    drop(first);
    let exclusive = args
        .take_host_lease(root, HostLeaseKind::Exclusive)
        .expect("dropping the receiver view should restore exclusive access");
    require_send(&exclusive);
    assert!(exclusive.is_exclusive());
    drop(exclusive);
}

#[test]
fn mutable_origin_leases_non_static_opaque_objects_without_any() {
    let mut value = 7_i64;
    let mut host = BorrowedOpaqueHost { value: &mut value };
    let mut args = CallArgs::new().with_host_mut("host", &mut host);
    let mut next = 1_u64 << 63;
    args.assign_direct_host_refs(&mut next);
    let root = bound_root(&args);

    let lease = args
        .take_host_lease(root, HostLeaseKind::Shared)
        .expect("opaque mutable origin uses its exclusive root lease");
    assert!(lease.is_exclusive());
    assert!(lease.object().lease_any().is_none());
    drop(lease);

    let lease = args
        .take_host_lease(root, HostLeaseKind::Exclusive)
        .expect("opaque mutable origin remains available after release");
    assert!(lease.is_exclusive());
}

#[test]
fn mixed_mutable_origin_acquisition_rolls_back_shared_state() {
    let mut host = LeaseHost;
    let mut args = CallArgs::new().with_host_mut("host", &mut host);
    let mut next = 1_u64 << 63;
    args.assign_direct_host_refs(&mut next);
    let root = bound_root(&args);

    let error = match args.take_host_leases(&[
        (root, HostLeaseKind::Shared),
        (root, HostLeaseKind::Exclusive),
    ]) {
        Ok(_) => panic!("shared followed by exclusive should fail atomically"),
        Err(error) => error,
    };
    assert!(matches!(
        error.kind,
        vela_host::error::HostErrorKind::HostObjectBusy { .. }
    ));

    let exclusive = args
        .take_host_lease(root, HostLeaseKind::Exclusive)
        .expect("rollback should restore the available state");
    drop(exclusive);
}

fn bound_root(args: &CallArgs<'_>) -> HostRef {
    match &args.entries[0] {
        CallArg::NamedHost {
            host_ref: Some(root),
            ..
        } => *root,
        _ => panic!("direct host argument should have an identity"),
    }
}
