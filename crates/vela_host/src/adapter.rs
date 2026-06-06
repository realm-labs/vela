use vela_common::HostMethodId;

use crate::{
    error::{HostError, HostErrorKind, HostResult},
    path::{HostPath, HostRef},
    value::HostValue,
};

pub trait ScriptStateAdapter {
    fn global_ref(&self, name: &str) -> HostResult<HostRef> {
        Err(HostError {
            kind: HostErrorKind::MissingGlobal {
                name: name.to_owned(),
            },
            source_span: None,
        })
    }

    fn read_path(&self, path: &HostPath) -> HostResult<HostValue>;

    fn write_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()>;

    fn remove_path(&mut self, path: &HostPath) -> HostResult<()>;

    fn call_method(
        &mut self,
        path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue>;
}
