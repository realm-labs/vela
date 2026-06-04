use std::collections::BTreeMap;

use crate::heap::{HeapSlot, HeapValue};
use crate::script_object::ScriptFields;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, value_from_heap_slot};

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) enum ValueView<'a, 'heap> {
    Value(&'a Value),
    Slot(&'a HeapSlot, &'a HeapExecution<'heap>),
}

impl<'a, 'heap> ValueView<'a, 'heap> {
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn to_owned_value(self) -> Value {
        match self {
            Self::Value(value) => value.clone(),
            Self::Slot(slot, heap) => {
                let _ = heap;
                value_from_heap_slot(slot)
            }
        }
    }
}

pub(crate) enum StringView<'a> {
    Value(&'a str),
}

impl<'a> StringView<'a> {
    pub(crate) fn from_value(
        value: &'a Value,
        heap: Option<&'a HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        match value {
            Value::String(value) => Ok(Self::Value(value)),
            Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
                Some(HeapValue::String(value)) => Ok(Self::Value(value)),
                _ => type_error(operation),
            },
            _ => type_error(operation),
        }
    }

    #[must_use]
    pub(crate) fn as_str(&self) -> &'a str {
        match self {
            Self::Value(value) => value,
        }
    }
}

pub(crate) enum ArrayView<'a, 'heap> {
    Values(&'a [Value]),
    Slots(&'a [HeapSlot], &'a HeapExecution<'heap>),
}

impl<'a, 'heap> ArrayView<'a, 'heap> {
    pub(crate) fn from_value(
        value: &'a Value,
        heap: Option<&'a HeapExecution<'heap>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        match value {
            Value::Array(values) => Ok(Self::Values(values)),
            Value::HeapRef(reference) => {
                let Some(heap) = heap else {
                    return type_error(operation);
                };
                match heap.heap.get(*reference) {
                    Some(HeapValue::Array(values)) => Ok(Self::Slots(values, heap)),
                    _ => type_error(operation),
                }
            }
            _ => type_error(operation),
        }
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Values(values) => values.len(),
            Self::Slots(values, _) => values.len(),
        }
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub(crate) fn first_owned(&self) -> Option<Value> {
        match self {
            Self::Values(values) => values.first().cloned(),
            Self::Slots(values, _) => values.first().map(value_from_heap_slot),
        }
    }

    #[must_use]
    pub(crate) fn last_owned(&self) -> Option<Value> {
        match self {
            Self::Values(values) => values.last().cloned(),
            Self::Slots(values, _) => values.last().map(value_from_heap_slot),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn for_each_value(&self, mut visit: impl FnMut(ValueView<'_, 'heap>)) {
        match self {
            Self::Values(values) => {
                for value in *values {
                    visit(ValueView::Value(value));
                }
            }
            Self::Slots(values, heap) => {
                for value in *values {
                    visit(ValueView::Slot(value, heap));
                }
            }
        }
    }

    pub(crate) fn materialize_values(&self) -> Vec<Value> {
        match self {
            Self::Values(values) => values.to_vec(),
            Self::Slots(values, _) => values.iter().map(value_from_heap_slot).collect(),
        }
    }
}

pub(crate) enum SetView<'a, 'heap> {
    Values(&'a [Value]),
    Slots(&'a [HeapSlot], &'a HeapExecution<'heap>),
}

impl<'a, 'heap> SetView<'a, 'heap> {
    pub(crate) fn from_value(
        value: &'a Value,
        heap: Option<&'a HeapExecution<'heap>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        match value {
            Value::Set(values) => Ok(Self::Values(values)),
            Value::HeapRef(reference) => {
                let Some(heap) = heap else {
                    return type_error(operation);
                };
                match heap.heap.get(*reference) {
                    Some(HeapValue::Set(values)) => Ok(Self::Slots(values, heap)),
                    _ => type_error(operation),
                }
            }
            _ => type_error(operation),
        }
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Values(values) => values.len(),
            Self::Slots(values, _) => values.len(),
        }
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[allow(dead_code)]
    pub(crate) fn for_each_value(&self, mut visit: impl FnMut(ValueView<'_, 'heap>)) {
        match self {
            Self::Values(values) => {
                for value in *values {
                    visit(ValueView::Value(value));
                }
            }
            Self::Slots(values, heap) => {
                for value in *values {
                    visit(ValueView::Slot(value, heap));
                }
            }
        }
    }

    pub(crate) fn materialize_values(&self) -> Vec<Value> {
        match self {
            Self::Values(values) => values.to_vec(),
            Self::Slots(values, _) => values.iter().map(value_from_heap_slot).collect(),
        }
    }
}

pub(crate) enum MapView<'a, 'heap> {
    Values(&'a BTreeMap<String, Value>),
    Slots(&'a BTreeMap<String, HeapSlot>, &'a HeapExecution<'heap>),
}

impl<'a, 'heap> MapView<'a, 'heap> {
    pub(crate) fn from_value(
        value: &'a Value,
        heap: Option<&'a HeapExecution<'heap>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        match value {
            Value::Map(values) => Ok(Self::Values(values)),
            Value::HeapRef(reference) => {
                let Some(heap) = heap else {
                    return type_error(operation);
                };
                match heap.heap.get(*reference) {
                    Some(HeapValue::Map(values)) => Ok(Self::Slots(values, heap)),
                    _ => type_error(operation),
                }
            }
            _ => type_error(operation),
        }
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Values(values) => values.len(),
            Self::Slots(values, _) => values.len(),
        }
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub(crate) fn contains_key(&self, key: &str) -> bool {
        match self {
            Self::Values(values) => values.contains_key(key),
            Self::Slots(values, _) => values.contains_key(key),
        }
    }

    #[must_use]
    pub(crate) fn get_owned(&self, key: &str) -> Option<Value> {
        match self {
            Self::Values(values) => values.get(key).cloned(),
            Self::Slots(values, _) => values.get(key).map(value_from_heap_slot),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn for_each_entry(&self, mut visit: impl FnMut(&str, ValueView<'_, 'heap>)) {
        match self {
            Self::Values(values) => {
                for (key, value) in *values {
                    visit(key, ValueView::Value(value));
                }
            }
            Self::Slots(values, heap) => {
                for (key, value) in *values {
                    visit(key, ValueView::Slot(value, heap));
                }
            }
        }
    }

    pub(crate) fn materialize_entries(&self) -> Vec<(String, Value)> {
        match self {
            Self::Values(values) => values
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
            Self::Slots(values, _) => values
                .iter()
                .map(|(key, value)| (key.clone(), value_from_heap_slot(value)))
                .collect(),
        }
    }
}

pub(crate) enum EnumFieldsView<'a, 'heap> {
    Values(&'a ScriptFields<Value>),
    Slots(&'a ScriptFields<HeapSlot>, &'a HeapExecution<'heap>),
}

impl<'a, 'heap> EnumFieldsView<'a, 'heap> {
    #[must_use]
    pub(crate) fn get_owned(&self, field: &str) -> Option<Value> {
        match self {
            Self::Values(fields) => fields.get(field).cloned(),
            Self::Slots(fields, heap) => {
                let _ = heap;
                fields.get(field).map(value_from_heap_slot)
            }
        }
    }
}

pub(crate) struct EnumView<'a, 'heap> {
    pub(crate) enum_name: &'a str,
    pub(crate) variant: &'a str,
    pub(crate) fields: EnumFieldsView<'a, 'heap>,
}

impl<'a, 'heap> EnumView<'a, 'heap> {
    pub(crate) fn from_value(
        value: &'a Value,
        heap: Option<&'a HeapExecution<'heap>>,
    ) -> Option<Self> {
        match value {
            Value::Enum {
                enum_name,
                variant,
                fields,
            } => Some(Self {
                enum_name,
                variant,
                fields: EnumFieldsView::Values(fields),
            }),
            Value::HeapRef(reference) => {
                let heap = heap?;
                match heap.heap.get(*reference)? {
                    HeapValue::Enum {
                        enum_name,
                        variant,
                        fields,
                    } => Some(Self {
                        enum_name,
                        variant,
                        fields: EnumFieldsView::Slots(fields, heap),
                    }),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

pub(crate) enum LengthView<'a> {
    String(&'a str),
    Count(usize),
}

impl LengthView<'_> {
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Self::String(value) => value.is_empty(),
            Self::Count(value) => *value == 0,
        }
    }
}

pub(crate) fn length_view<'a>(
    value: &'a Value,
    heap: Option<&'a HeapExecution<'_>>,
    operation: &'static str,
) -> Option<VmResult<LengthView<'a>>> {
    match value {
        Value::String(value) => Some(Ok(LengthView::String(value))),
        Value::Array(values) | Value::Set(values) => Some(Ok(LengthView::Count(values.len()))),
        Value::Map(values) => Some(Ok(LengthView::Count(values.len()))),
        Value::Record { fields, .. } | Value::Enum { fields, .. } => {
            Some(Ok(LengthView::Count(fields.len())))
        }
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return Some(type_error(operation));
            };
            let result = match heap.heap.get(*reference) {
                Some(HeapValue::String(value)) => Ok(LengthView::String(value)),
                Some(HeapValue::Array(values) | HeapValue::Set(values)) => {
                    Ok(LengthView::Count(values.len()))
                }
                Some(HeapValue::Map(values)) => Ok(LengthView::Count(values.len())),
                Some(HeapValue::Record { fields, .. } | HeapValue::Enum { fields, .. }) => {
                    Ok(LengthView::Count(fields.len()))
                }
                None => type_error(operation),
            };
            Some(result)
        }
        _ => None,
    }
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
