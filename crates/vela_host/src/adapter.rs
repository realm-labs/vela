use vela_common::HostMethodId;

use crate::{HostPath, HostResult, HostValue, Patch};

pub trait ScriptStateAdapter {
    fn read_path(&self, path: &HostPath) -> HostResult<HostValue>;

    fn write_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()>;

    fn call_method(
        &mut self,
        path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue>;

    fn preview_method_return(
        &self,
        path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let _ = (path, method, args);
        Ok(HostValue::Null)
    }

    fn validate_patch(&self, patch: &Patch) -> HostResult<()>;

    fn apply_patch(&mut self, patch: Patch) -> HostResult<()>;

    fn apply_patches(&mut self, patches: Vec<Patch>) -> HostResult<()> {
        for patch in &patches {
            self.validate_patch(patch)
                .map_err(|error| error.with_source_span_if_absent(patch.source_span))?;
        }
        for patch in patches {
            let source_span = patch.source_span;
            self.apply_patch(patch)
                .map_err(|error| error.with_source_span_if_absent(source_span))?;
        }
        Ok(())
    }
}
