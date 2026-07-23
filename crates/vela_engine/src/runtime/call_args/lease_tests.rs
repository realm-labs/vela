use std::any::Any;

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
    let mut host = LeaseHost;
    let mut args = CallArgs::new().with_host_mut("host", &mut host);
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
            .map(|slot| slot.entry_index)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert!(!args.direct_host_slots.spilled());
    assert_eq!(next, base + 2);

    let roots = args
        .direct_host_slots
        .iter()
        .map(|slot| match &args.entries[slot.entry_index as usize] {
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
fn mutable_origin_shared_leases_coexist_and_restore_exclusive_access() {
    fn require_send<T: Send>(_: &T) {}

    let mut host = LeaseHost;
    let mut args = CallArgs::new().with_host_mut("host", &mut host);
    let mut next = 1_u64 << 63;
    args.assign_direct_host_refs(&mut next);
    let root = bound_root(&args);

    let first = args
        .take_host_lease(root, HostLeaseKind::Shared)
        .expect("first shared lease should be available");
    let second = args
        .take_host_lease(root, HostLeaseKind::Shared)
        .expect("second shared lease should coexist");
    require_send(&first);
    require_send(&second);
    assert!(!first.is_exclusive());
    assert!(!second.is_exclusive());

    let binding = args
        .direct_binding(root)
        .expect("mutable binding should remain registered");
    let HostArgBinding::Mutable { object } = binding else {
        panic!("binding should have mutable origin");
    };
    assert!(
        object.try_read().is_some(),
        "parent read access remains legal"
    );
    assert!(
        object.try_write().is_none(),
        "shared leases exclude mutation"
    );
    let conflict = match args.take_host_lease(root, HostLeaseKind::Exclusive) {
        Ok(_) => panic!("exclusive acquisition must conflict with shared leases"),
        Err(error) => error,
    };
    assert!(matches!(
        conflict.kind,
        vela_host::error::HostErrorKind::HostObjectBusy { .. }
    ));

    drop(first);
    drop(second);
    let exclusive = args
        .take_host_lease(root, HostLeaseKind::Exclusive)
        .expect("dropping all shared leases should restore exclusive access");
    require_send(&exclusive);
    assert!(exclusive.is_exclusive());
    drop(exclusive);
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
