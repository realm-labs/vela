use vela_host::error::HostResult;
use vela_host::lease::{host_lease_unsupported, host_object_busy};
use vela_host::object::ScriptHostObject;
use vela_host::path::HostRef;

use super::{ExecutionHost, ScopedHostObjectBinding};

impl ExecutionHost<'_, '_> {
    pub(super) fn inspect_scoped_host<T>(
        &self,
        root: HostRef,
        mut inspect: impl FnMut(&dyn ScriptHostObject) -> HostResult<T>,
    ) -> Option<HostResult<T>> {
        let binding = self.scoped_hosts.get(&root)?;
        Some(match &binding.object {
            ScopedHostObjectBinding::Single(object) => object
                .try_read()
                .ok_or_else(|| host_object_busy(root))
                .and_then(|object| inspect(&**object)),
            ScopedHostObjectBinding::Group { object, index } => {
                object.with_dependent(|_, objects| {
                    objects
                        .get(*index)
                        .ok_or_else(|| host_lease_unsupported(root))?
                        .try_read()
                        .ok_or_else(|| host_object_busy(root))
                        .and_then(|object| inspect(&**object))
                })
            }
        })
    }
}
