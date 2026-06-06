use vela_common::{HostMethodId, HostTypeId};

use crate::{
    error::{HostError, HostErrorKind, HostResult},
    path::HostPath,
    value::HostValue,
};

pub trait ScriptHostObject {
    fn host_type_id(&self) -> HostTypeId;

    fn read_host_path(&self, path: &HostPath) -> HostResult<HostValue>;

    fn write_host_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()> {
        let _ = value;
        Err(HostError {
            kind: HostErrorKind::PermissionDenied {
                path: path.clone(),
                action: "write",
            },
            source_span: None,
        })
    }

    fn remove_host_path(&mut self, path: &HostPath) -> HostResult<()> {
        Err(HostError {
            kind: HostErrorKind::MissingPath { path: path.clone() },
            source_span: None,
        })
    }

    fn call_host_method(
        &mut self,
        path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let _ = args;
        Err(HostError {
            kind: if path.segments.is_empty() {
                HostErrorKind::UnsupportedMethod { method }
            } else {
                HostErrorKind::MissingPath { path: path.clone() }
            },
            source_span: None,
        })
    }
}
