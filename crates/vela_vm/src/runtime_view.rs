use std::collections::BTreeMap;

use crate::heap::HeapValue;
use crate::script_object::ScriptFields;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) enum ValueView<'a, 'heap> {
    Value(&'a Value),
    #[allow(dead_code)]
    _Marker(std::marker::PhantomData<&'heap ()>),
}

impl<'a, 'heap> ValueView<'a, 'heap> {
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn to_owned_value(self) -> Value {
        match self {
            Self::Value(value) => *value,
            Self::_Marker(_) => unreachable!("marker variant is never constructed"),
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
    Values(&'a [Value], &'a HeapExecution<'heap>),
}

impl<'a, 'heap> ArrayView<'a, 'heap> {
    pub(crate) fn from_value(
        value: &'a Value,
        heap: Option<&'a HeapExecution<'heap>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        match value {
            Value::HeapRef(reference) => {
                let Some(heap) = heap else {
                    return type_error(operation);
                };
                match heap.heap.get(*reference) {
                    Some(HeapValue::Array(values)) => Ok(Self::Values(values, heap)),
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
            Self::Values(values, _) => values.len(),
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
            Self::Values(values, _) => values.first().copied(),
        }
    }

    #[must_use]
    pub(crate) fn last_owned(&self) -> Option<Value> {
        match self {
            Self::Values(values, _) => values.last().copied(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn for_each_value(&self, mut visit: impl FnMut(ValueView<'_, 'heap>)) {
        match self {
            Self::Values(values, _) => {
                for value in *values {
                    visit(ValueView::Value(value));
                }
            }
        }
    }

    pub(crate) fn materialize_values(&self) -> Vec<Value> {
        match self {
            Self::Values(values, _) => values.to_vec(),
        }
    }
}

pub(crate) enum SetView<'a, 'heap> {
    Values(&'a [Value], &'a HeapExecution<'heap>),
}

impl<'a, 'heap> SetView<'a, 'heap> {
    pub(crate) fn from_value(
        value: &'a Value,
        heap: Option<&'a HeapExecution<'heap>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        match value {
            Value::HeapRef(reference) => {
                let Some(heap) = heap else {
                    return type_error(operation);
                };
                match heap.heap.get(*reference) {
                    Some(HeapValue::Set(values)) => Ok(Self::Values(values, heap)),
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
            Self::Values(values, _) => values.len(),
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
            Self::Values(values, _) => {
                for value in *values {
                    visit(ValueView::Value(value));
                }
            }
        }
    }

    pub(crate) fn materialize_values(&self) -> Vec<Value> {
        match self {
            Self::Values(values, _) => values.to_vec(),
        }
    }
}

pub(crate) enum MapView<'a, 'heap> {
    Values(&'a BTreeMap<String, Value>, &'a HeapExecution<'heap>),
}

impl<'a, 'heap> MapView<'a, 'heap> {
    pub(crate) fn from_value(
        value: &'a Value,
        heap: Option<&'a HeapExecution<'heap>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        match value {
            Value::HeapRef(reference) => {
                let Some(heap) = heap else {
                    return type_error(operation);
                };
                match heap.heap.get(*reference) {
                    Some(HeapValue::Map(values)) => Ok(Self::Values(values, heap)),
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
            Self::Values(values, _) => values.len(),
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
            Self::Values(values, _) => values.contains_key(key),
        }
    }

    #[must_use]
    pub(crate) fn get_owned(&self, key: &str) -> Option<Value> {
        match self {
            Self::Values(values, _) => values.get(key).copied(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn for_each_entry(&self, mut visit: impl FnMut(&str, ValueView<'_, 'heap>)) {
        match self {
            Self::Values(values, _) => {
                for (key, value) in *values {
                    visit(key, ValueView::Value(value));
                }
            }
        }
    }

    pub(crate) fn materialize_entries(&self) -> Vec<(String, Value)> {
        match self {
            Self::Values(values, _) => values.iter().map(|(key, value)| (key.clone(), *value)).collect(),
        }
    }
}

pub(crate) enum EnumFieldsView<'a, 'heap> {
    Values(&'a ScriptFields<Value>, &'a HeapExecution<'heap>),
}

impl<'a, 'heap> EnumFieldsView<'a, 'heap> {
    #[must_use]
    pub(crate) fn get_owned(&self, field: &str) -> Option<Value> {
        match self {
            Self::Values(fields, _) => fields.get(field).copied(),
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
                        fields: EnumFieldsView::Values(fields, heap),
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
                _ => type_error(operation),
            };
            Some(result)
        }
        _ => None,
    }
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
