use vela_bytecode::HostPathSegment;
use vela_common::SymbolInterner;
use vela_host::{HostPath, HostRef};

use crate::{CallFrame, HeapExecution, Value, VmError, VmErrorKind, VmResult, materialize_value};

pub(crate) fn host_path_from_segments(
    root: HostRef,
    segments: &[HostPathSegment],
    frame: &CallFrame,
    heap: Option<&HeapExecution<'_>>,
    symbols: &mut SymbolInterner,
) -> VmResult<HostPath> {
    let mut path = HostPath::new(root);
    for segment in segments {
        path = match segment {
            HostPathSegment::Field(field) => path.field(*field),
            HostPathSegment::VariantField(field) => path.variant_field(*field),
            HostPathSegment::Value(register) => {
                match materialize_value(frame.read(*register)?, heap)? {
                    Value::Int(index) => {
                        let index = u32::try_from(index).map_err(|_| {
                            VmError::new(VmErrorKind::TypeMismatch {
                                operation: "host path index",
                            })
                        })?;
                        path.index(index)
                    }
                    Value::String(key) => path.key(symbols.intern(key)),
                    _ => {
                        return Err(VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host path index",
                        }));
                    }
                }
            }
        };
    }
    Ok(path)
}
