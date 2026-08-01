use vela_bytecode::Register;
use vela_host::protocol::{HostCollectionKey, HostCollectionKeyRef};
use vela_host::target::{HostPathArg, HostPathPart, HostTargetPlan};

use crate::heap::HeapValue;
use crate::{CallFrame, HeapExecution, HostExecution, Value, VmError, VmErrorKind, VmResult};

pub(crate) struct RuntimeCollectionIndex(HostCollectionKey);

impl RuntimeCollectionIndex {
    pub(crate) fn target(
        &self,
        root_type: vela_common::HostTypeId,
    ) -> (HostTargetPlan, HostPathArg<'_>) {
        (
            HostTargetPlan::new(root_type).dyn_key(0),
            HostPathArg::Key(self.0.as_ref()),
        )
    }
}

pub(crate) fn runtime_collection_key(
    index: &Value,
    heap: Option<&HeapExecution<'_>>,
    host: Option<&HostExecution<'_>>,
    operation: &'static str,
) -> VmResult<HostCollectionKey> {
    let key = match index {
        Value::Bool(value) => HostCollectionKey::Bool(*value),
        Value::Char(value) => HostCollectionKey::Char(*value),
        Value::I8(value) => HostCollectionKey::I8(*value),
        Value::I16(value) => HostCollectionKey::I16(*value),
        Value::I32(value) => HostCollectionKey::I32(*value),
        Value::I64(value) => HostCollectionKey::I64(*value),
        Value::U8(value) => HostCollectionKey::U8(*value),
        Value::U16(value) => HostCollectionKey::U16(*value),
        Value::U32(value) => HostCollectionKey::U32(*value),
        Value::U64(value) => HostCollectionKey::U64(*value),
        Value::HostRef(value) => HostCollectionKey::HostRef(
            host.ok_or_else(super::host_access::missing_host_context)?
                .resolve_host_ref(*value)?,
        ),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(key)) => HostCollectionKey::String(key.clone()),
            Some(HeapValue::Bytes(key)) => HostCollectionKey::Bytes(key.clone()),
            _ => return Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        },
        Value::Missing | Value::Unit | Value::F32(_) | Value::F64(_) => {
            return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
        }
    };
    Ok(key)
}

pub(crate) fn runtime_collection_index(
    index: &Value,
    heap: Option<&HeapExecution<'_>>,
    host: Option<&HostExecution<'_>>,
    operation: &'static str,
) -> VmResult<RuntimeCollectionIndex> {
    runtime_collection_key(index, heap, host, operation).map(RuntimeCollectionIndex)
}

pub(crate) enum MaterializedHostArgs<'a> {
    Empty,
    Values(Vec<HostPathArg<'a>>),
}

impl<'a> MaterializedHostArgs<'a> {
    pub(crate) fn as_slice(&'a self) -> &'a [HostPathArg<'a>] {
        match self {
            Self::Empty => &[],
            Self::Values(args) => args,
        }
    }
}

pub(crate) fn materialize_host_args<'a>(
    frame: &CallFrame,
    target: &HostTargetPlan,
    registers: &[Register],
    heap: Option<&'a HeapExecution<'a>>,
    operation: &'static str,
) -> VmResult<MaterializedHostArgs<'a>> {
    if registers.is_empty() {
        return Ok(MaterializedHostArgs::Empty);
    }
    registers
        .iter()
        .enumerate()
        .map(|(argument, register)| {
            let is_key = target.parts.as_slice().iter().any(|part| {
                matches!(part, HostPathPart::DynKey { arg } if usize::from(*arg) == argument)
            });
            host_arg_from_value(&frame.read(*register)?, heap, operation, is_key)
        })
        .collect::<VmResult<Vec<_>>>()
        .map(MaterializedHostArgs::Values)
}

fn host_arg_from_value<'a>(
    value: &Value,
    heap: Option<&'a HeapExecution<'a>>,
    operation: &'static str,
    is_key: bool,
) -> VmResult<HostPathArg<'a>> {
    if is_key {
        let key = match value {
            Value::Bool(value) => HostCollectionKeyRef::Bool(*value),
            Value::Char(value) => HostCollectionKeyRef::Char(*value),
            Value::I8(value) => HostCollectionKeyRef::I8(*value),
            Value::I16(value) => HostCollectionKeyRef::I16(*value),
            Value::I32(value) => HostCollectionKeyRef::I32(*value),
            Value::I64(value) => HostCollectionKeyRef::I64(*value),
            Value::U8(value) => HostCollectionKeyRef::U8(*value),
            Value::U16(value) => HostCollectionKeyRef::U16(*value),
            Value::U32(value) => HostCollectionKeyRef::U32(*value),
            Value::U64(value) => HostCollectionKeyRef::U64(*value),
            Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
                Some(HeapValue::String(value)) => HostCollectionKeyRef::String(value.as_str()),
                Some(HeapValue::Bytes(value)) => HostCollectionKeyRef::Bytes(value.as_slice()),
                _ => return Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
            },
            Value::HostRef(_) | Value::Missing | Value::Unit | Value::F32(_) | Value::F64(_) => {
                return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
            }
        };
        return Ok(HostPathArg::Key(key));
    }
    match value {
        Value::I64(index) => {
            let index = u32::try_from(*index).map_err(|_| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "host path index",
                })
            })?;
            Ok(HostPathArg::Index(index))
        }
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(HostPathArg::Key(HostCollectionKeyRef::String(
                value.as_str(),
            ))),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        },
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}
