use vela_host::path::HostRef;
use vela_host::resolved::ResolvedHostAccess;
use vela_host::target::HostTargetPlan;

use crate::method_runtime::MethodRuntime;
use crate::{Value, VmError, VmErrorKind, VmResult};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct HostArrayCursor {
    // The traversal extent is frozen at creation, while each value is read
    // through the prepared access immediately before it is yielded.
    root: HostRef,
    target: HostTargetPlan,
    access: ResolvedHostAccess,
    next: usize,
    len: usize,
}

impl HostArrayCursor {
    pub(super) fn new(
        root: HostRef,
        target: HostTargetPlan,
        access: ResolvedHostAccess,
        len: usize,
    ) -> Self {
        Self {
            root,
            target,
            access,
            next: 0,
            len,
        }
    }

    pub(super) fn next(
        &mut self,
        runtime: &mut MethodRuntime<'_, '_, '_>,
        operation: &'static str,
    ) -> VmResult<Option<Value>> {
        if self.next >= self.len {
            return Ok(None);
        }
        let index = u32::try_from(self.next)
            .map_err(|_| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
        let host = runtime
            .host
            .as_deref_mut()
            .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
        let value = host.read_index(
            self.root,
            &self.target,
            self.access,
            index,
            runtime.heap.as_deref_mut(),
            runtime.budget.as_deref_mut(),
        )?;
        self.next = self.next.saturating_add(1);
        Ok(Some(value))
    }
}
