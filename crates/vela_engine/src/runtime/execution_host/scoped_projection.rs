use std::cell::RefCell;

use vela_host::adapter::{ScopedHostReturn, ScopedHostReturnGroup, ScriptStateAdapter};
use vela_host::error::{HostError, HostResult};
use vela_host::lease::{HostLeaseKind, try_scoped_host_cell, try_scoped_host_group_cell};
use vela_host::object::ScopedHostCollectionDependents;
use vela_host::path::HostRef;
use vela_host::protocol::{HostCollectionProjection, HostCollectionSnapshot};
use vela_host::resolved::ResolvedHostAccess;
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;

use super::ExecutionHost;

enum ScopedProjectionError {
    Missing,
    Host(HostError),
}

impl From<HostError> for ScopedProjectionError {
    fn from(error: HostError) -> Self {
        Self::Host(error)
    }
}

enum ScopedProjectionShape {
    Items,
    Entries(Vec<HostValue>),
}

impl<'state, 'host> ExecutionHost<'state, 'host> {
    pub(super) fn retain_scoped_element(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<Option<HostRef>> {
        if self.extern_states.binding(target.root).is_some()
            || (!self.expired_scoped_hosts.contains_key(&target.root)
                && !self.host_arena.contains(target.root)
                && self.scoped_binding(target.root).is_none()
                && self.args.direct_binding(target.root).is_none())
        {
            return Ok(None);
        }
        let kind = self.host_receiver_access(target.root);
        let parent = self.take_execution_host_lease(target.root, kind)?;
        let cell = try_scoped_host_cell(parent, |lease| {
            let child = match kind {
                HostLeaseKind::Shared => {
                    lease.object().borrow_resolved_host_shared(access, target)?
                }
                HostLeaseKind::Exclusive => lease
                    .object_mut()
                    .expect("an exclusive parent lease exposes mutable access")
                    .borrow_resolved_host_exclusive(access, target)?,
            };
            child.ok_or(ScopedProjectionError::Missing)
        });
        match cell {
            Ok(object) => Ok(Some(self.retain_scoped_host(ScopedHostReturn {
                object,
                access: kind,
            }))),
            Err(ScopedProjectionError::Missing) => Ok(None),
            Err(ScopedProjectionError::Host(error)) => Err(error),
        }
    }

    pub(super) fn retain_scoped_collection(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: HostCollectionProjection,
    ) -> HostResult<Option<HostCollectionSnapshot>> {
        if self.extern_states.binding(target.root).is_some()
            || (!self.expired_scoped_hosts.contains_key(&target.root)
                && !self.host_arena.contains(target.root)
                && self.scoped_binding(target.root).is_none()
                && self.args.direct_binding(target.root).is_none())
        {
            return Ok(None);
        }
        let kind = self.host_receiver_access(target.root);
        let parent = self.take_execution_host_lease(target.root, kind)?;
        let shape = RefCell::new(None);
        let object = try_scoped_host_group_cell(parent, |lease| {
            let dependents = match kind {
                HostLeaseKind::Shared => lease
                    .object()
                    .borrow_collection_resolved_host_shared(access, target, projection)?,
                HostLeaseKind::Exclusive => lease
                    .object_mut()
                    .expect("an exclusive parent lease exposes mutable access")
                    .borrow_collection_resolved_host_exclusive(access, target, projection)?,
            }
            .ok_or(ScopedProjectionError::Missing)?;
            Ok(match dependents {
                ScopedHostCollectionDependents::Items(children) => {
                    shape.replace(Some(ScopedProjectionShape::Items));
                    children
                }
                ScopedHostCollectionDependents::Entries(entries) => {
                    let (keys, children): (Vec<_>, Vec<_>) = entries.into_iter().unzip();
                    shape.replace(Some(ScopedProjectionShape::Entries(keys)));
                    children
                }
            })
        });
        let object = match object {
            Ok(object) => object,
            Err(ScopedProjectionError::Missing) => return Ok(None),
            Err(ScopedProjectionError::Host(error)) => return Err(error),
        };
        if object.is_empty() {
            return Ok(Some(
                match shape
                    .into_inner()
                    .expect("an empty scoped collection records its output shape")
                {
                    ScopedProjectionShape::Items => HostCollectionSnapshot::Items(Vec::new()),
                    ScopedProjectionShape::Entries(_) => {
                        HostCollectionSnapshot::Entries(Vec::new())
                    }
                },
            ));
        }
        let accesses = vec![kind; object.len()];
        let roots = self.retain_scoped_host_group(ScopedHostReturnGroup { object, accesses })?;
        let snapshot = match shape
            .into_inner()
            .expect("a scoped collection projection records its output shape")
        {
            ScopedProjectionShape::Items => {
                HostCollectionSnapshot::Items(roots.into_iter().map(HostValue::HostRef).collect())
            }
            ScopedProjectionShape::Entries(keys) => HostCollectionSnapshot::Entries(
                keys.into_iter()
                    .zip(roots)
                    .map(|(key, root)| (key, HostValue::HostRef(root)))
                    .collect(),
            ),
        };
        Ok(Some(snapshot))
    }
}
