//! Bounded materialization of script graphs as owned trees.

use std::sync::Arc;

use vela_host::protocol::HostCollectionKey;
use vela_host::target::{HostPathArgOwned, HostPathPart, HostTargetPlan};

use crate::error::{VmError, VmErrorKind, VmResult};
use crate::heap::{GcRef, HeapValue};
use crate::heap_execution::HeapExecution;
use crate::heap_values::HostSlotResolver;
use crate::owned_value::{OwnedClosureValue, OwnedIteratorState, OwnedMapEntry, OwnedValue};
use crate::script_object::ScriptFields;
use crate::value::Value;

// These bounds also make recursive destruction of partial/successful trees safe.
const MAX_DEPTH: usize = 64;
const MAX_VALUES: usize = 65_536;
const MAX_BYTES: usize = 16 * 1024 * 1024;

pub(crate) fn export(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
    host: Option<HostSlotResolver<'_>>,
) -> VmResult<OwnedValue> {
    Export {
        path: Vec::new(),
        values_left: MAX_VALUES,
        bytes_left: MAX_BYTES,
    }
    .value(value, heap, host)
}

struct Export {
    path: Vec<GcRef>,
    values_left: usize,
    bytes_left: usize,
}

fn limit(resource: &'static str, limit: usize) -> VmError {
    VmError::new(VmErrorKind::OwnedValueLimitExceeded { resource, limit })
}

impl Export {
    fn bytes(&mut self, bytes: usize) -> VmResult<()> {
        self.bytes_left = self
            .bytes_left
            .checked_sub(bytes)
            .ok_or_else(|| limit("bytes", MAX_BYTES))?;
        Ok(())
    }

    fn string(&mut self, value: &str) -> VmResult<String> {
        self.bytes(value.len())?;
        Ok(value.to_owned())
    }

    // Charge backing storage before reserving it, including children not yet
    // visited. Every alias is charged again because the output is a tree.
    fn storage<T>(&mut self, count: usize, children: usize) -> VmResult<Vec<T>> {
        if children > self.values_left {
            return Err(limit("values", MAX_VALUES));
        }
        self.bytes(count.saturating_mul(size_of::<T>()))?;
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| {
            VmError::new(VmErrorKind::AllocationFailed {
                operation: "owned value export",
            })
        })?;
        Ok(values)
    }

    fn sequence<'a>(
        &mut self,
        values: impl Iterator<Item = &'a Value>,
        len: usize,
        heap: Option<&HeapExecution<'_>>,
        host: Option<HostSlotResolver<'_>>,
    ) -> VmResult<Vec<OwnedValue>> {
        let mut result = self.storage(len, len)?;
        for value in values {
            result.push(self.value(value, heap, host)?);
        }
        Ok(result)
    }

    fn value(
        &mut self,
        value: &Value,
        heap: Option<&HeapExecution<'_>>,
        host: Option<HostSlotResolver<'_>>,
    ) -> VmResult<OwnedValue> {
        self.values_left = self
            .values_left
            .checked_sub(1)
            .ok_or_else(|| limit("values", MAX_VALUES))?;
        if let Some(value) = value.as_scalar() {
            return Ok(OwnedValue::Scalar(value));
        }
        match value {
            Value::Unit => Ok(OwnedValue::Unit),
            Value::Bool(value) => Ok(OwnedValue::Bool(*value)),
            Value::Char(value) => Ok(OwnedValue::Char(*value)),
            Value::HostRef(value) => host
                .ok_or_else(|| type_error("host ref requires active slot resolver"))?
                .resolve(*value)
                .map(OwnedValue::HostRef),
            Value::HeapRef(reference) => {
                if self.path.contains(reference) {
                    return Err(VmError::new(VmErrorKind::OwnedValueCycle));
                }
                if self.path.len() >= MAX_DEPTH {
                    return Err(limit("depth", MAX_DEPTH));
                }
                let value = heap
                    .and_then(|heap| heap.heap.get(*reference))
                    .ok_or_else(|| type_error("heap ref"))?;
                self.path.push(*reference);
                let result = self.object(value, heap, host);
                self.path.pop();
                result
            }
            Value::Missing => Err(type_error("missing value")),
            _ => unreachable!("scalar values handled above"),
        }
    }

    fn object(
        &mut self,
        value: &HeapValue,
        heap: Option<&HeapExecution<'_>>,
        host: Option<HostSlotResolver<'_>>,
    ) -> VmResult<OwnedValue> {
        match value {
            HeapValue::String(value) => self.string(value).map(OwnedValue::String),
            HeapValue::Bytes(value) => {
                self.bytes(value.len())?;
                Ok(OwnedValue::Bytes(value.clone()))
            }
            HeapValue::Range(value) => Ok(OwnedValue::Range(*value)),
            HeapValue::Tuple(values) => self
                .sequence(values.iter(), values.len(), heap, host)
                .map(OwnedValue::Tuple),
            HeapValue::Array(values) => self
                .sequence(values.iter(), values.len(), heap, host)
                .map(OwnedValue::Array),
            HeapValue::Set(values) => self
                .sequence(values.values(), values.len(), heap, host)
                .map(OwnedValue::Set),
            HeapValue::Map(values) => {
                let mut result = self.storage(values.len(), values.len().saturating_mul(2))?;
                for entry in values.entries() {
                    result.push(OwnedMapEntry::new(
                        self.value(&entry.key, heap, host)?,
                        self.value(&entry.value, heap, host)?,
                    ));
                }
                Ok(OwnedValue::Map(result))
            }
            HeapValue::Record { fields, .. } => {
                let type_name = self.string(fields.owner_name())?;
                let values = self.sequence(fields.values(), fields.len(), heap, host)?;
                Ok(OwnedValue::Record {
                    type_name,
                    fields: ScriptFields::from_shape(Arc::clone(fields.shape()), values),
                })
            }
            HeapValue::Enum {
                enum_name,
                variant,
                fields,
                ..
            } => {
                let enum_name = self.string(enum_name)?;
                let variant = self.string(variant)?;
                let values = self.sequence(fields.values(), fields.len(), heap, host)?;
                Ok(OwnedValue::Enum {
                    enum_name,
                    variant,
                    fields: ScriptFields::from_shape(Arc::clone(fields.shape()), values),
                })
            }
            HeapValue::Closure(closure) => {
                let values = closure.captures.as_slice();
                let captures = self.sequence(values.iter(), values.len(), heap, host)?;
                Ok(OwnedValue::Closure(OwnedClosureValue {
                    owner: Arc::clone(&closure.owner),
                    function: closure.function,
                    captures,
                }))
            }
            HeapValue::Iterator(iterator) => {
                if iterator.is_host_backed() {
                    return Err(type_error("host-backed iterator escape"));
                }
                let values = iterator.values();
                let values = self.sequence(values.iter(), values.len(), heap, host)?;
                Ok(OwnedValue::Iterator(OwnedIteratorState::from_runtime(
                    iterator, values,
                )))
            }
            HeapValue::PathProxy(proxy) => {
                self.bytes(size_of::<HostTargetPlan>())?;
                self.bytes(
                    proxy
                        .target()
                        .parts
                        .len()
                        .saturating_mul(size_of::<HostPathPart>()),
                )?;
                self.bytes(
                    proxy
                        .args()
                        .len()
                        .saturating_mul(size_of::<HostPathArgOwned>()),
                )?;
                for part in proxy.target().parts.as_slice() {
                    if let HostPathPart::ConstKey(key) = part {
                        self.bytes(key.len())?;
                    }
                }
                for arg in proxy.args() {
                    match arg {
                        HostPathArgOwned::Key(HostCollectionKey::String(key)) => {
                            self.bytes(key.len())?
                        }
                        HostPathArgOwned::Key(HostCollectionKey::Bytes(key)) => {
                            self.bytes(key.len())?
                        }
                        _ => {}
                    }
                }
                Ok(OwnedValue::PathProxy(proxy.clone()))
            }
        }
    }
}

fn type_error(operation: &'static str) -> VmError {
    VmError::new(VmErrorKind::TypeMismatch { operation })
}

#[cfg(test)]
mod tests;
