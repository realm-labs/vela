use vela_host::path::HostRef;
use vela_host::resolved::ResolvedHostAccess;
use vela_host::target::HostTargetPlan;

use crate::method_runtime::{HostIteratorTarget, MethodRuntime};
use crate::{Value, VmError, VmErrorKind, VmResult};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct HostSetCursor {
    // Values establish deterministic traversal order and a frozen extent.
    // Membership is checked through the prepared key target before each yield.
    root: HostRef,
    target: HostTargetPlan,
    access: ResolvedHostAccess,
    values: Vec<Value>,
    next: usize,
}

impl HostSetCursor {
    pub(super) const fn root(&self) -> HostRef {
        self.root
    }

    pub(super) fn new(
        root: HostRef,
        target: HostTargetPlan,
        access: ResolvedHostAccess,
        values: Vec<Value>,
    ) -> Self {
        Self {
            root,
            target,
            access,
            values,
            next: 0,
        }
    }

    pub(super) fn next(
        &mut self,
        runtime: &mut MethodRuntime<'_, '_, '_>,
        operation: &'static str,
    ) -> VmResult<Option<Value>> {
        let Some(value) = self.values.get(self.next).copied() else {
            return Ok(None);
        };
        let host = runtime
            .host
            .as_deref_mut()
            .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
        let present = host.read_key(
            HostIteratorTarget {
                root: self.root,
                plan: &self.target,
                access: self.access,
            },
            &value,
            runtime.heap.as_deref_mut(),
            runtime.budget.as_deref_mut(),
            operation,
        )?;
        if present != Value::Bool(true) {
            return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
        }
        self.next = self.next.saturating_add(1);
        Ok(Some(value))
    }

    pub(super) fn values(&self) -> &[Value] {
        &self.values
    }

    pub(super) fn values_mut(&mut self) -> &mut [Value] {
        &mut self.values
    }
}
