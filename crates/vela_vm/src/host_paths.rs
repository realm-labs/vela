use vela_bytecode::HostPathSegment;
use vela_common::{FieldId, SymbolInterner};
use vela_host::path::{HostPath, HostRef};

use crate::owned_value::OwnedValue;
use crate::{CallFrame, HeapExecution, VmError, VmErrorKind, VmResult, value_to_owned};

pub(crate) fn host_path_from_segments(
    root: HostRef,
    segments: &[HostPathSegment],
    frame: &CallFrame,
    heap: Option<&HeapExecution<'_>>,
    _symbols: &mut SymbolInterner,
) -> VmResult<HostPath> {
    if let Some(path) = static_host_path_from_segments(root, segments) {
        return Ok(path);
    }

    let mut path = HostPath::with_segment_capacity(root, segments.len());
    for segment in segments {
        path = match segment {
            HostPathSegment::Field(field) => path.field(*field),
            HostPathSegment::VariantField(field) => path.variant_field(*field),
            HostPathSegment::Value(register) => {
                match value_to_owned(frame.read(*register)?, heap)? {
                    OwnedValue::Int(index) => {
                        let index = u32::try_from(index).map_err(|_| {
                            VmError::new(VmErrorKind::TypeMismatch {
                                operation: "host path index",
                            })
                        })?;
                        path.index(index)
                    }
                    OwnedValue::String(key) => path.key(key),
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

pub(crate) fn host_field_path(root: HostRef, field: FieldId) -> HostPath {
    HostPath::with_segment_capacity(root, 1).field(field)
}

fn static_host_path_from_segments(root: HostRef, segments: &[HostPathSegment]) -> Option<HostPath> {
    let mut path = HostPath::with_segment_capacity(root, segments.len());
    for segment in segments {
        path = match segment {
            HostPathSegment::Field(field) => path.field(*field),
            HostPathSegment::VariantField(field) => path.variant_field(*field),
            HostPathSegment::Value(_) => return None,
        };
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use vela_bytecode::{HostPathSegment, Register};
    use vela_common::{FieldId, HostObjectId, HostTypeId};
    use vela_host::path::{HostRef, PathSegment};

    use super::*;

    fn host_ref() -> HostRef {
        HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3)
    }

    #[test]
    fn static_host_path_segments_materialize_without_runtime_values() {
        let path = static_host_path_from_segments(
            host_ref(),
            &[
                HostPathSegment::Field(FieldId::new(2)),
                HostPathSegment::VariantField(FieldId::new(5)),
            ],
        )
        .expect("static host path");

        assert_eq!(
            path.segments,
            vec![
                PathSegment::Field(FieldId::new(2)),
                PathSegment::VariantField(FieldId::new(5))
            ]
        );
    }

    #[test]
    fn static_host_path_segments_reject_dynamic_values() {
        assert_eq!(
            static_host_path_from_segments(
                host_ref(),
                &[
                    HostPathSegment::Field(FieldId::new(2)),
                    HostPathSegment::Value(Register(3))
                ],
            ),
            None
        );
    }

    #[test]
    fn host_field_path_materializes_single_field_path() {
        let path = host_field_path(host_ref(), FieldId::new(9));

        assert_eq!(path.segments, vec![PathSegment::Field(FieldId::new(9))]);
    }
}
