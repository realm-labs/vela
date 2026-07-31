use std::collections::BTreeSet;
use std::sync::Arc;

use vela_bytecode::Constant;
use vela_host::adapter::ScriptStateAdapter;
use vela_host::error::HostRefLifetimeBoundary;
use vela_host::slot::HostRefSlots;
use vela_host::value::HostValue;

use crate::SmallStorage;
use crate::budget::ExecutionBudget;
use crate::collection_mutation::check_collection_len;
use crate::error::{VmError, VmErrorKind, VmResult};
use crate::heap::{HeapValue, ScriptHeap};
use crate::heap_execution::HeapExecution;
use crate::option_result::std_enum_identity_for_names;
use crate::owned_value::{OwnedClosureValue, OwnedIteratorState, OwnedMapEntry, OwnedValue};
use crate::script_map::ScriptMap;
use crate::script_object::ScriptFields;
use crate::script_set::ScriptSet;
use crate::value::{ClosureValue, Value};

struct HostSlotConversion<'host> {
    storage: HostSlotStorage<'host>,
}

enum HostSlotStorage<'host> {
    None,
    Adapter(&'host mut (dyn ScriptStateAdapter + Send)),
    Slots(&'host mut HostRefSlots),
}

impl HostSlotConversion<'_> {
    fn intern(
        &mut self,
        reference: vela_host::path::HostRef,
    ) -> VmResult<vela_host::path::HostSlotRef> {
        match &mut self.storage {
            HostSlotStorage::None => Err(type_error("host ref requires active slot resolver")),
            HostSlotStorage::Adapter(host) => host.intern_host_ref(reference).map_err(Into::into),
            HostSlotStorage::Slots(slots) => Ok(slots.intern(reference)),
        }
    }
}

#[derive(Clone, Copy)]
enum HostSlotResolver<'host> {
    Adapter(&'host (dyn ScriptStateAdapter + Send)),
    Slots(&'host HostRefSlots),
}

impl HostSlotResolver<'_> {
    fn resolve(self, handle: vela_host::path::HostSlotRef) -> VmResult<vela_host::path::HostRef> {
        match self {
            Self::Adapter(host) => host.resolve_host_ref(handle).map_err(Into::into),
            Self::Slots(slots) => slots
                .resolve(handle)
                .ok_or_else(|| type_error("invalid host slot")),
        }
    }
}

pub fn owned_to_persistent_value(
    value: OwnedValue,
    heap: &mut ScriptHeap,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let mut heap_execution = HeapExecution::new(heap);
    owned_to_value(value, &mut heap_execution, budget, None)
}

pub fn owned_to_persistent_value_with_host(
    value: OwnedValue,
    heap: &mut ScriptHeap,
    budget: Option<&mut ExecutionBudget>,
    host: &mut (dyn ScriptStateAdapter + Send),
) -> VmResult<Value> {
    let mut heap_execution = HeapExecution::new(heap);
    owned_to_value(value, &mut heap_execution, budget, Some(host))
}

pub fn owned_to_persistent_value_with_slots(
    value: OwnedValue,
    heap: &mut ScriptHeap,
    budget: Option<&mut ExecutionBudget>,
    slots: &mut HostRefSlots,
) -> VmResult<Value> {
    let mut heap_execution = HeapExecution::new(heap);
    owned_to_value_with_storage(
        value,
        &mut heap_execution,
        budget,
        None,
        HostSlotStorage::Slots(slots),
    )
}

pub fn owned_to_linked_persistent_value(
    value: OwnedValue,
    program: &vela_bytecode::LinkedProgram,
    heap: &mut ScriptHeap,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let mut heap_execution = HeapExecution::new(heap);
    owned_to_value_with_program(value, &mut heap_execution, budget, Some(program), None)
}

pub fn owned_to_linked_persistent_value_with_host(
    value: OwnedValue,
    program: &vela_bytecode::LinkedProgram,
    heap: &mut ScriptHeap,
    budget: Option<&mut ExecutionBudget>,
    host: &mut (dyn ScriptStateAdapter + Send),
) -> VmResult<Value> {
    let mut heap_execution = HeapExecution::new(heap);
    owned_to_value_with_program(
        value,
        &mut heap_execution,
        budget,
        Some(program),
        Some(host),
    )
}

pub fn owned_to_linked_persistent_value_with_slots(
    value: OwnedValue,
    program: &vela_bytecode::LinkedProgram,
    heap: &mut ScriptHeap,
    budget: Option<&mut ExecutionBudget>,
    slots: &mut HostRefSlots,
) -> VmResult<Value> {
    let mut heap_execution = HeapExecution::new(heap);
    owned_to_value_with_storage(
        value,
        &mut heap_execution,
        budget,
        Some(program),
        HostSlotStorage::Slots(slots),
    )
}

pub(crate) fn owned_to_linked_value(
    value: OwnedValue,
    program: &vela_bytecode::LinkedProgram,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
    host: Option<&mut (dyn ScriptStateAdapter + Send)>,
) -> VmResult<Value> {
    owned_to_value_with_program(value, heap, budget, Some(program), host)
}

pub(crate) fn owned_values_to_linked_values(
    values: &[OwnedValue],
    program: &vela_bytecode::LinkedProgram,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
    host: Option<&mut (dyn ScriptStateAdapter + Send)>,
) -> VmResult<Vec<Value>> {
    let mut slots = HostSlotConversion {
        storage: host.map_or(HostSlotStorage::None, HostSlotStorage::Adapter),
    };
    values
        .iter()
        .cloned()
        .map(|value| {
            owned_to_value_inner(
                value,
                heap,
                budget.as_deref_mut(),
                Some(program),
                &mut slots,
            )
        })
        .collect()
}

pub fn persistent_value_to_owned(value: &Value, heap: &mut ScriptHeap) -> VmResult<OwnedValue> {
    let heap_execution = HeapExecution::new(heap);
    value_to_owned(value, Some(&heap_execution), None)
}

pub fn persistent_value_to_owned_with_host(
    value: &Value,
    heap: &mut ScriptHeap,
    host: &(dyn ScriptStateAdapter + Send),
) -> VmResult<OwnedValue> {
    let heap_execution = HeapExecution::new(heap);
    value_to_owned(value, Some(&heap_execution), Some(host))
}

pub fn persistent_value_to_owned_with_slots(
    value: &Value,
    heap: &mut ScriptHeap,
    slots: &HostRefSlots,
) -> VmResult<OwnedValue> {
    let heap_execution = HeapExecution::new(heap);
    value_to_owned_inner(
        value,
        Some(&heap_execution),
        Some(HostSlotResolver::Slots(slots)),
    )
}

pub fn validate_persistent_value_host_refs(
    value: &Value,
    heap: &ScriptHeap,
    host: &(dyn ScriptStateAdapter + Send),
    boundary: HostRefLifetimeBoundary,
) -> VmResult<()> {
    validate_value_host_refs(value, Some(heap), host, boundary)
}

pub(crate) fn validate_value_host_refs(
    value: &Value,
    heap: Option<&ScriptHeap>,
    host: &(dyn ScriptStateAdapter + Send),
    boundary: HostRefLifetimeBoundary,
) -> VmResult<()> {
    let mut pending = vec![*value];
    let mut visited = BTreeSet::new();
    while let Some(value) = pending.pop() {
        match value {
            Value::HostRef(handle) => {
                let root = host.resolve_host_ref(handle)?;
                host.validate_host_ref_lifetime(root, boundary)?;
            }
            Value::HeapRef(reference) if visited.insert(reference) => {
                let Some(value) = heap.and_then(|heap| heap.get(reference)) else {
                    return Err(type_error("host-ref lifetime validation"));
                };
                match value {
                    HeapValue::String(_) | HeapValue::Bytes(_) | HeapValue::Range(_) => {}
                    HeapValue::Tuple(values) | HeapValue::Array(values) => {
                        pending.extend(values.iter().copied());
                    }
                    HeapValue::Map(values) => {
                        for entry in values.entries() {
                            pending.push(entry.key);
                            pending.push(entry.value);
                        }
                    }
                    HeapValue::Set(values) => pending.extend(values.values().copied()),
                    HeapValue::Record { fields, .. } | HeapValue::Enum { fields, .. } => {
                        pending.extend(fields.values().copied());
                    }
                    HeapValue::Closure(closure) => {
                        pending.extend(closure.captures.as_slice().iter().copied());
                    }
                    HeapValue::Iterator(iterator) => {
                        pending.extend(iterator.protected_values());
                    }
                    HeapValue::PathProxy(proxy) => {
                        host.validate_host_ref_lifetime(proxy.root(), boundary)?;
                    }
                }
            }
            Value::Missing
            | Value::Unit
            | Value::Bool(_)
            | Value::Char(_)
            | Value::I8(_)
            | Value::I16(_)
            | Value::I32(_)
            | Value::I64(_)
            | Value::U8(_)
            | Value::U16(_)
            | Value::U32(_)
            | Value::U64(_)
            | Value::F32(_)
            | Value::F64(_)
            | Value::HeapRef(_) => {}
        }
    }
    Ok(())
}

pub(crate) enum BorrowedReleaseTarget {
    Interned(vela_host::path::HostSlotRef),
    ScopedIterator(vela_host::path::HostRef),
}

pub(crate) fn optional_borrowed_host_ref(
    value: Value,
    heap: Option<&ScriptHeap>,
) -> VmResult<Option<BorrowedReleaseTarget>> {
    match value {
        Value::HostRef(reference) => Ok(Some(BorrowedReleaseTarget::Interned(reference))),
        Value::HeapRef(reference) => {
            if let Some(HeapValue::Iterator(iterator)) = heap.and_then(|heap| heap.get(reference)) {
                return iterator
                    .scoped_host_root()
                    .map(BorrowedReleaseTarget::ScopedIterator)
                    .map(Some)
                    .ok_or_else(|| type_error("release borrowed host lease"));
            }
            let Some(HeapValue::Enum {
                identity: Some(identity),
                fields,
                ..
            }) = heap.and_then(|heap| heap.get(reference))
            else {
                return Err(type_error("release borrowed host lease"));
            };
            let Some((kind, variant)) = crate::option_result::std_enum_tag(*identity) else {
                return Err(type_error("release borrowed host lease"));
            };
            match (kind, variant) {
                (
                    crate::option_result::StdEnumKind::Option,
                    crate::option_result::StdEnumVariant::None,
                ) => Ok(None),
                (
                    crate::option_result::StdEnumKind::Option,
                    crate::option_result::StdEnumVariant::Some,
                ) => match fields.get_slot(0, "0").map(crate::stored_runtime_value) {
                    Some(Value::HostRef(reference)) => {
                        Ok(Some(BorrowedReleaseTarget::Interned(reference)))
                    }
                    _ => Err(type_error("release borrowed host lease")),
                },
                _ => Err(type_error("release borrowed host lease")),
            }
        }
        _ => Err(type_error("release borrowed host lease")),
    }
}

pub(crate) fn value_from_constant(
    constant: &Constant,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    match constant {
        Constant::Unit => Ok(Value::Unit),
        Constant::Bool(value) => Ok(Value::Bool(*value)),
        Constant::Char(value) => Ok(Value::Char(*value)),
        Constant::Scalar(value) => Ok(Value::from_scalar(*value)),
        Constant::String(value) => {
            let Some(heap) = heap else {
                return Err(type_error("constant string"));
            };
            allocate_heap_value(HeapValue::String(value.clone()), heap, budget)
        }
        Constant::Bytes(value) => {
            let Some(heap) = heap else {
                return Err(type_error("constant bytes"));
            };
            allocate_heap_value(HeapValue::Bytes(value.clone()), heap, budget)
        }
        Constant::Array(values) => {
            let Some(mut heap) = heap else {
                return Err(type_error("constant array"));
            };
            let mut budget = budget;
            let values = values
                .iter()
                .map(|value| value_from_constant(value, Some(&mut heap), budget.as_deref_mut()))
                .collect::<VmResult<Vec<_>>>()?;
            allocate_heap_value(HeapValue::Array(values), heap, budget)
        }
        Constant::Map(entries) => {
            let Some(mut heap) = heap else {
                return Err(type_error("constant map"));
            };
            let mut budget = budget;
            let entries = entries
                .iter()
                .map(|(key, value)| {
                    Ok((
                        key.clone(),
                        value_from_constant(value, Some(&mut heap), budget.as_deref_mut())?,
                    ))
                })
                .collect::<VmResult<Vec<_>>>()?;
            let values = script_map_from_string_entries(
                entries,
                heap,
                budget.as_deref_mut(),
                "constant map",
            )?;
            allocate_heap_value(HeapValue::Map(values), heap, budget)
        }
    }
}

pub(crate) fn allocate_heap_value(
    value: HeapValue,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let reference = if let Some(budget) = budget {
        heap.heap.allocate_with_budget(value, budget)?
    } else {
        heap.heap.allocate(value)
    };
    Ok(Value::HeapRef(reference))
}

pub fn allocate_zero_field_record(
    type_name: String,
    type_id: vela_def::TypeId,
    shape_id: vela_common::ShapeId,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    allocate_heap_value(
        HeapValue::Record {
            fields: ScriptFields::from_pairs(&type_name, std::iter::empty()),
            identity: Some(crate::heap::RecordIdentity::new(type_id, shape_id)),
        },
        heap,
        budget,
    )
}

pub(crate) fn store_runtime_value(
    value: &Value,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    store_value_in_heap(*value, heap, budget)
}

pub(crate) fn stored_runtime_value(value: &Value) -> Value {
    *value
}

#[allow(dead_code)]
pub(crate) fn make_string_value(
    value: String,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    allocate_heap_value(HeapValue::String(value), heap, budget)
}

#[allow(dead_code)]
pub(crate) fn make_array_value(
    values: Vec<Value>,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    check_collection_len("array", 0, values.len(), budget.as_deref(), |budget| {
        budget.collection_limits().max_array_len
    })?;
    allocate_heap_value(HeapValue::Array(values), heap, budget)
}

#[allow(dead_code)]
pub(crate) fn make_set_value(
    values: Vec<Value>,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    check_collection_len("set", 0, values.len(), budget.as_deref(), |budget| {
        budget.collection_limits().max_set_len
    })?;
    let values = ScriptSet::from_values(values, Some(&*heap), "set construction")?;
    allocate_heap_value(HeapValue::Set(values), heap, budget)
}

#[allow(dead_code)]
pub(crate) fn make_enum_value(
    enum_name: impl Into<String>,
    variant: impl Into<String>,
    fields: Vec<(String, Value)>,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let enum_name = enum_name.into();
    let variant = variant.into();
    let owner = enum_variant_owner(&enum_name, &variant);
    let identity = std_enum_identity_for_names(&enum_name, &variant);
    allocate_heap_value(
        HeapValue::Enum {
            enum_name,
            variant,
            identity,
            fields: ScriptFields::from_pairs(&owner, fields),
        },
        heap,
        budget,
    )
}

pub(crate) fn enum_variant_owner(enum_name: &str, variant: &str) -> String {
    format!("{enum_name}::{variant}")
}

#[allow(dead_code)]
pub(crate) fn owned_to_value(
    value: OwnedValue,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
    host: Option<&mut (dyn ScriptStateAdapter + Send)>,
) -> VmResult<Value> {
    owned_to_value_with_program(value, heap, budget, None, host)
}

fn owned_to_value_with_program(
    value: OwnedValue,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
    program: Option<&vela_bytecode::LinkedProgram>,
    host: Option<&mut (dyn ScriptStateAdapter + Send)>,
) -> VmResult<Value> {
    owned_to_value_with_storage(
        value,
        heap,
        budget,
        program,
        host.map_or(HostSlotStorage::None, HostSlotStorage::Adapter),
    )
}

fn owned_to_value_with_storage(
    value: OwnedValue,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
    program: Option<&vela_bytecode::LinkedProgram>,
    storage: HostSlotStorage<'_>,
) -> VmResult<Value> {
    let mut slots = HostSlotConversion { storage };
    owned_to_value_inner(value, heap, budget, program, &mut slots)
}

fn owned_to_value_inner(
    value: OwnedValue,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
    program: Option<&vela_bytecode::LinkedProgram>,
    slots: &mut HostSlotConversion<'_>,
) -> VmResult<Value> {
    match value {
        OwnedValue::Unit => Ok(Value::Unit),
        OwnedValue::Bool(value) => Ok(Value::Bool(value)),
        OwnedValue::Char(value) => Ok(Value::Char(value)),
        OwnedValue::Scalar(value) => Ok(Value::from_scalar(value)),
        OwnedValue::Range(value) => {
            allocate_heap_value(HeapValue::Range(value), heap, budget.as_deref_mut())
        }
        OwnedValue::HostRef(value) => Ok(Value::HostRef(slots.intern(value)?)),
        OwnedValue::String(value) => {
            allocate_heap_value(HeapValue::String(value), heap, budget.as_deref_mut())
        }
        OwnedValue::Bytes(value) => {
            allocate_heap_value(HeapValue::Bytes(value), heap, budget.as_deref_mut())
        }
        OwnedValue::Tuple(values) => {
            let values = values
                .into_iter()
                .map(|value| {
                    owned_to_value_inner(value, heap, budget.as_deref_mut(), program, slots)
                })
                .collect::<VmResult<Vec<_>>>()?;
            allocate_heap_value(HeapValue::Tuple(values), heap, budget)
        }
        OwnedValue::Array(values) => {
            let values = values
                .into_iter()
                .map(|value| {
                    owned_to_value_inner(value, heap, budget.as_deref_mut(), program, slots)
                })
                .collect::<VmResult<Vec<_>>>()?;
            allocate_heap_value(HeapValue::Array(values), heap, budget)
        }
        OwnedValue::Set(values) => {
            let values = values
                .into_iter()
                .map(|value| {
                    owned_to_value_inner(value, heap, budget.as_deref_mut(), program, slots)
                })
                .collect::<VmResult<Vec<_>>>()?;
            let values = ScriptSet::from_values(values, Some(&*heap), "owned set")?;
            allocate_heap_value(HeapValue::Set(values), heap, budget)
        }
        OwnedValue::Map(values) => {
            let values = values
                .into_iter()
                .map(|entry| {
                    Ok((
                        owned_to_value_inner(
                            entry.key,
                            heap,
                            budget.as_deref_mut(),
                            program,
                            slots,
                        )?,
                        owned_to_value_inner(
                            entry.value,
                            heap,
                            budget.as_deref_mut(),
                            program,
                            slots,
                        )?,
                    ))
                })
                .collect::<VmResult<Vec<_>>>()?;
            let values = ScriptMap::from_entries(values, Some(&*heap), "owned map")?;
            allocate_heap_value(HeapValue::Map(values), heap, budget)
        }
        OwnedValue::Record { type_name, fields } => {
            let nominal = program.and_then(|program| {
                crate::owned_contract::resolve_nominal_type(&type_name, program)
            });
            let canonical_name =
                nominal.map_or_else(|| type_name.clone(), |ty| ty.runtime_name.clone());
            let identity = nominal.and_then(|ty| {
                ty.shape
                    .map(|shape| crate::heap::RecordIdentity::new(ty.id, shape))
            });
            let fields = fields
                .into_pairs()
                .map(|(key, value)| {
                    Ok((
                        key,
                        owned_to_value_inner(value, heap, budget.as_deref_mut(), program, slots)?,
                    ))
                })
                .collect::<VmResult<Vec<_>>>()?;
            allocate_heap_value(
                HeapValue::Record {
                    fields: ScriptFields::from_pairs(&canonical_name, fields),
                    identity,
                },
                heap,
                budget,
            )
        }
        OwnedValue::Enum {
            enum_name,
            variant,
            fields,
        } => {
            let nominal = program.and_then(|program| {
                crate::owned_contract::resolve_nominal_type(&enum_name, program)
            });
            let canonical_name =
                nominal.map_or_else(|| enum_name.clone(), |ty| ty.runtime_name.clone());
            let owner = enum_variant_owner(&canonical_name, &variant);
            let identity = nominal
                .and_then(|ty| {
                    ty.variants
                        .iter()
                        .find(|candidate| candidate.name == variant)
                        .map(|candidate| crate::heap::EnumIdentity::new(ty.id, candidate.id, None))
                })
                .or_else(|| std_enum_identity_for_names(&canonical_name, &variant));
            let fields = fields
                .into_pairs()
                .map(|(key, value)| {
                    Ok((
                        key,
                        owned_to_value_inner(value, heap, budget.as_deref_mut(), program, slots)?,
                    ))
                })
                .collect::<VmResult<Vec<_>>>()?;
            allocate_heap_value(
                HeapValue::Enum {
                    fields: ScriptFields::from_pairs(&owner, fields),
                    enum_name: canonical_name,
                    variant,
                    identity,
                },
                heap,
                budget,
            )
        }
        OwnedValue::Closure(closure) => {
            let captures = SmallStorage::try_from_slice_map(&closure.captures, 4, |capture| {
                owned_to_value_inner(capture.clone(), heap, budget.as_deref_mut(), program, slots)
            })?;
            allocate_heap_value(
                HeapValue::Closure(ClosureValue {
                    owner: Arc::clone(&closure.owner),
                    function: closure.function,
                    captures,
                }),
                heap,
                budget,
            )
        }
        OwnedValue::Iterator(iterator) => {
            let values = iterator
                .values()
                .iter()
                .cloned()
                .map(|value| {
                    owned_to_value_inner(value, heap, budget.as_deref_mut(), program, slots)
                })
                .collect::<VmResult<Vec<_>>>()?;
            allocate_heap_value(
                HeapValue::Iterator(crate::iteration::IteratorState::from_values_at(
                    values,
                    iterator.next_index(),
                )),
                heap,
                budget,
            )
        }
        OwnedValue::PathProxy(proxy) => {
            allocate_heap_value(HeapValue::PathProxy(proxy), heap, budget)
        }
    }
}

pub(crate) fn value_to_owned(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
    host: Option<&(dyn ScriptStateAdapter + Send)>,
) -> VmResult<OwnedValue> {
    value_to_owned_inner(value, heap, host.map(HostSlotResolver::Adapter))
}

fn value_to_owned_inner(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
    host: Option<HostSlotResolver<'_>>,
) -> VmResult<OwnedValue> {
    if let Some(value) = value.as_scalar() {
        return Ok(OwnedValue::Scalar(value));
    }
    match value {
        Value::Missing => Err(type_error("missing value")),
        Value::Unit => Ok(OwnedValue::Unit),
        Value::Bool(value) => Ok(OwnedValue::Bool(*value)),
        Value::Char(value) => Ok(OwnedValue::Char(*value)),
        Value::HostRef(value) => {
            let host = host.ok_or_else(|| type_error("host ref requires active slot resolver"))?;
            Ok(OwnedValue::HostRef(host.resolve(*value)?))
        }
        Value::HeapRef(reference) => {
            let Some(heap_value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return Err(type_error("heap ref"));
            };
            heap_value_to_owned(heap_value, heap, host)
        }
        _ => unreachable!("scalar values return before owned conversion match"),
    }
}

#[allow(dead_code)]
fn heap_value_to_owned(
    value: &HeapValue,
    heap: Option<&HeapExecution<'_>>,
    host: Option<HostSlotResolver<'_>>,
) -> VmResult<OwnedValue> {
    match value {
        HeapValue::String(value) => Ok(OwnedValue::String(value.clone())),
        HeapValue::Bytes(value) => Ok(OwnedValue::Bytes(value.clone())),
        HeapValue::Range(value) => Ok(OwnedValue::Range(*value)),
        HeapValue::Tuple(values) => values
            .iter()
            .map(|value| value_to_owned_inner(value, heap, host))
            .collect::<VmResult<Vec<_>>>()
            .map(OwnedValue::Tuple),
        HeapValue::Array(values) => values
            .iter()
            .map(|value| value_to_owned_inner(value, heap, host))
            .collect::<VmResult<Vec<_>>>()
            .map(OwnedValue::Array),
        HeapValue::Map(values) => values
            .entries()
            .map(|entry| {
                let key = value_to_owned_inner(&entry.key, heap, host)?;
                Ok(OwnedMapEntry::new(
                    key,
                    value_to_owned_inner(&entry.value, heap, host)?,
                ))
            })
            .collect::<VmResult<Vec<_>>>()
            .map(OwnedValue::Map),
        HeapValue::Set(values) => values
            .values()
            .map(|value| value_to_owned_inner(value, heap, host))
            .collect::<VmResult<Vec<_>>>()
            .map(OwnedValue::Set),
        HeapValue::Record { fields, .. } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), value_to_owned_inner(value, heap, host)?)))
            .collect::<VmResult<Vec<_>>>()
            .map(|converted| OwnedValue::Record {
                type_name: fields.owner_name().to_owned(),
                fields: ScriptFields::from_pairs(fields.owner_name(), converted),
            }),
        HeapValue::Enum {
            enum_name,
            variant,
            fields,
            ..
        } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), value_to_owned_inner(value, heap, host)?)))
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| OwnedValue::Enum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields: ScriptFields::from_pairs(&enum_variant_owner(enum_name, variant), fields),
            }),
        HeapValue::Closure(closure) => closure
            .captures
            .as_slice()
            .iter()
            .map(|capture| value_to_owned_inner(capture, heap, host))
            .collect::<VmResult<Vec<_>>>()
            .map(|captures| {
                OwnedValue::Closure(OwnedClosureValue {
                    owner: Arc::clone(&closure.owner),
                    function: closure.function,
                    captures,
                })
            }),
        HeapValue::Iterator(iterator) => {
            if iterator.is_host_backed() {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "host-backed iterator escape",
                }));
            }
            iterator
                .values()
                .iter()
                .map(|value| value_to_owned_inner(value, heap, host))
                .collect::<VmResult<Vec<_>>>()
                .map(|values| {
                    OwnedValue::Iterator(OwnedIteratorState::from_runtime(iterator, values))
                })
        }
        HeapValue::PathProxy(proxy) => Ok(OwnedValue::PathProxy(proxy.clone())),
    }
}

#[allow(dead_code)]
pub(crate) fn host_to_value(
    value: HostValue,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
    host: &mut crate::HostExecution<'_>,
) -> VmResult<Value> {
    match value {
        HostValue::Unit => Ok(Value::Unit),
        HostValue::Bool(value) => Ok(Value::Bool(value)),
        HostValue::Char(value) => Ok(Value::Char(value)),
        HostValue::Scalar(value) => Ok(Value::from_scalar(value)),
        HostValue::String(value) => allocate_heap_value(HeapValue::String(value), heap, budget),
        HostValue::Bytes(value) => allocate_heap_value(HeapValue::Bytes(value), heap, budget),
        HostValue::HostRef(value) => Ok(Value::HostRef(host.intern_host_ref(value)?)),
    }
}

#[allow(dead_code)]
pub(crate) fn value_to_host(
    value: &Value,
    operation: &'static str,
    heap: Option<&HeapExecution<'_>>,
    host: Option<&crate::HostExecution<'_>>,
) -> VmResult<HostValue> {
    if let Some(value) = value.as_scalar() {
        return Ok(HostValue::Scalar(value));
    }
    match value {
        Value::Unit => Ok(HostValue::Unit),
        Value::Bool(value) => Ok(HostValue::Bool(*value)),
        Value::Char(value) => Ok(HostValue::Char(*value)),
        Value::HostRef(value) => Ok(HostValue::HostRef(
            host.ok_or_else(|| type_error(operation))?
                .resolve_host_ref(*value)?,
        )),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(HostValue::String(value.clone())),
            Some(HeapValue::Bytes(value)) => Ok(HostValue::Bytes(value.clone())),
            Some(
                HeapValue::Array(_)
                | HeapValue::Tuple(_)
                | HeapValue::Map(_)
                | HeapValue::Set(_)
                | HeapValue::Record { .. }
                | HeapValue::Enum { .. },
            ) => Err(type_error(operation)),
            _ => Err(type_error(operation)),
        },
        Value::Missing => Err(type_error(operation)),
        _ => unreachable!("scalar values return before host conversion match"),
    }
}

pub(crate) fn store_value_in_heap_if_needed(
    value: Value,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let Some(heap) = heap else {
        return if matches!(value, Value::Missing) {
            Err(type_error("missing value"))
        } else {
            Ok(value)
        };
    };
    store_value_in_heap(value, heap, budget)
}

fn store_value_in_heap(
    value: Value,
    _heap: &mut HeapExecution<'_>,
    _budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    match value {
        Value::Missing => Err(type_error("missing value")),
        Value::Unit
        | Value::Bool(_)
        | Value::Char(_)
        | Value::I8(_)
        | Value::I16(_)
        | Value::I32(_)
        | Value::I64(_)
        | Value::U8(_)
        | Value::U16(_)
        | Value::U32(_)
        | Value::U64(_)
        | Value::F32(_)
        | Value::F64(_)
        | Value::HostRef(_)
        | Value::HeapRef(_) => Ok(value),
    }
}

fn type_error(operation: &'static str) -> VmError {
    VmError::new(VmErrorKind::TypeMismatch { operation })
}

pub(crate) fn script_map_from_string_entries(
    entries: impl IntoIterator<Item = (String, Value)>,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<ScriptMap> {
    let entries = entries.into_iter();
    let (min_entries, _) = entries.size_hint();
    let mut values = Vec::with_capacity(min_entries);
    for (key, value) in entries {
        let key = allocate_heap_value(HeapValue::String(key), heap, budget.as_deref_mut())?;
        values.push((key, value));
    }
    ScriptMap::from_entries(values, Some(&*heap), operation)
}

#[cfg(test)]
mod tests {
    use vela_host::slot::HostRefSlots;

    use crate::heap::ScriptHeap;

    use super::*;

    #[test]
    fn owned_bytes_round_trip_through_heap_value() {
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let value = owned_to_value(
            OwnedValue::Bytes(vec![0, 1, 255]),
            &mut heap_execution,
            None,
            None,
        )
        .expect("bytes should allocate");

        let Value::HeapRef(reference) = value else {
            panic!("bytes should be heap backed");
        };
        assert_eq!(
            heap_execution.heap.get(reference),
            Some(&HeapValue::Bytes(vec![0, 1, 255]))
        );
        assert_eq!(
            value_to_owned(&value, Some(&heap_execution), None),
            Ok(OwnedValue::Bytes(vec![0, 1, 255]))
        );
    }

    #[test]
    fn bytes_constant_allocates_heap_bytes() {
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let value = value_from_constant(
            &Constant::Bytes(vec![b'a', b'b', b'c']),
            Some(&mut heap_execution),
            None,
        )
        .expect("bytes constant should allocate");

        let Value::HeapRef(reference) = value else {
            panic!("bytes should be heap backed");
        };
        assert_eq!(
            heap_execution.heap.get(reference),
            Some(&HeapValue::Bytes(vec![b'a', b'b', b'c']))
        );
    }

    #[test]
    fn nested_host_ref_aliases_round_trip_through_one_compact_slot() {
        let host_ref = vela_host::path::HostRef::new(
            vela_common::HostTypeId::new(7),
            vela_common::HostObjectId::new(11),
            3,
        );
        let owned = OwnedValue::Array(vec![
            OwnedValue::HostRef(host_ref),
            OwnedValue::HostRef(host_ref),
        ]);
        let mut heap = ScriptHeap::new();
        let mut slots = HostRefSlots::new();

        let value =
            owned_to_persistent_value_with_slots(owned.clone(), &mut heap, None, &mut slots)
                .expect("active slots should admit nested host aliases");

        let Value::HeapRef(reference) = value else {
            panic!("array should be heap backed");
        };
        let handle = {
            let Some(HeapValue::Array(values)) = heap.get(reference) else {
                panic!("array should remain available in the script heap");
            };
            let [Value::HostRef(first), Value::HostRef(second)] = values.as_slice() else {
                panic!("nested host aliases should use compact slot handles");
            };
            assert_eq!(first, second);
            *first
        };
        assert_eq!(slots.len(), 1);
        assert_eq!(
            persistent_value_to_owned_with_slots(&value, &mut heap, &slots),
            Ok(owned)
        );

        slots
            .release(handle)
            .expect("releasing the canonical slot should succeed");
        assert!(
            persistent_value_to_owned_with_slots(&value, &mut heap, &slots).is_err(),
            "every copied alias must fail after its generation is invalidated"
        );
    }
}
