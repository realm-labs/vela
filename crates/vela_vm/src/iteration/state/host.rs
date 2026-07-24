use vela_host::path::HostRef;
use vela_host::resolved::ResolvedHostAccess;
use vela_host::target::HostTargetPlan;

use crate::method_runtime::MethodRuntime;
use crate::{Value, VmError, VmErrorKind, VmResult};

use super::{IteratorCursor, IteratorState};
use crate::iteration::host_array::HostArrayCursor;
use crate::iteration::host_map::{HostMapCursor, HostMapCursorKind};
use crate::iteration::host_set::HostSetCursor;

impl IteratorState {
    pub(crate) fn next_host_map_entry_with_runtime(
        &mut self,
        runtime: &mut MethodRuntime<'_, '_, '_>,
        operation: &'static str,
    ) -> VmResult<Option<(Value, Value)>> {
        if let Some(budget) = runtime.budget.as_deref_mut() {
            budget.charge_execution_units(1)?;
        }
        let IteratorCursor::HostMap(cursor) = &mut self.cursor else {
            return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
        };
        cursor.next_entry(runtime, operation)
    }

    pub(crate) fn from_host_array(
        root: HostRef,
        target: HostTargetPlan,
        access: ResolvedHostAccess,
        len: usize,
    ) -> Self {
        Self {
            cursor: IteratorCursor::HostArray(Box::new(HostArrayCursor::new(
                root, target, access, len,
            ))),
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn from_host_map_values(
        root: HostRef,
        target: HostTargetPlan,
        access: ResolvedHostAccess,
        keys: Vec<Value>,
    ) -> Self {
        Self::from_host_map(root, target, access, keys, HostMapCursorKind::Values)
    }

    pub(crate) fn from_host_map_entries(
        root: HostRef,
        target: HostTargetPlan,
        access: ResolvedHostAccess,
        keys: Vec<Value>,
    ) -> Self {
        Self::from_host_map(root, target, access, keys, HostMapCursorKind::Entries)
    }

    fn from_host_map(
        root: HostRef,
        target: HostTargetPlan,
        access: ResolvedHostAccess,
        keys: Vec<Value>,
        kind: HostMapCursorKind,
    ) -> Self {
        Self {
            cursor: IteratorCursor::HostMap(Box::new(HostMapCursor::new(
                root, target, access, keys, kind,
            ))),
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn from_host_set(
        root: HostRef,
        target: HostTargetPlan,
        access: ResolvedHostAccess,
        values: Vec<Value>,
    ) -> Self {
        Self {
            cursor: IteratorCursor::HostSet(Box::new(HostSetCursor::new(
                root, target, access, values,
            ))),
            item_guards: Vec::new(),
        }
    }
}
