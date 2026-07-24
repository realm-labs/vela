use vela_host::path::HostRef;
use vela_host::resolved::ResolvedHostAccess;
use vela_host::target::HostTargetPlan;

use crate::method_runtime::{HostIteratorTarget, MethodRuntime};
use crate::{Value, VmError, VmErrorKind, VmResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HostMapCursorKind {
    Values,
    Entries,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct HostMapCursor {
    // Keys establish deterministic traversal order and a frozen extent. Map
    // values remain live and are read through the prepared key target.
    root: HostRef,
    target: HostTargetPlan,
    access: ResolvedHostAccess,
    keys: Vec<Value>,
    next: usize,
    kind: HostMapCursorKind,
}

impl HostMapCursor {
    pub(super) fn new(
        root: HostRef,
        target: HostTargetPlan,
        access: ResolvedHostAccess,
        keys: Vec<Value>,
        kind: HostMapCursorKind,
    ) -> Self {
        Self {
            root,
            target,
            access,
            keys,
            next: 0,
            kind,
        }
    }

    pub(super) fn next(
        &mut self,
        runtime: &mut MethodRuntime<'_, '_, '_>,
        operation: &'static str,
    ) -> VmResult<Option<Value>> {
        let Some(key) = self.keys.get(self.next).copied() else {
            return Ok(None);
        };
        let host = runtime
            .host
            .as_deref_mut()
            .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
        let value = host.read_key(
            HostIteratorTarget {
                root: self.root,
                plan: &self.target,
                access: self.access,
            },
            &key,
            runtime.heap.as_deref_mut(),
            runtime.budget.as_deref_mut(),
            operation,
        )?;
        let value = match self.kind {
            HostMapCursorKind::Values => value,
            HostMapCursorKind::Entries => {
                crate::map_methods::map_entry(key, value, &mut runtime.heap, &mut runtime.budget)?
            }
        };
        self.next = self.next.saturating_add(1);
        Ok(Some(value))
    }

    pub(super) fn keys(&self) -> &[Value] {
        &self.keys
    }

    pub(super) fn keys_mut(&mut self) -> &mut [Value] {
        &mut self.keys
    }
}
