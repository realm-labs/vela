use std::collections::BTreeMap;

use crate::heap::HeapValue;
use crate::iteration;
use crate::method_runtime::{MethodRuntime, call_callback_with_protected_values};
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

use super::{
    array_values, call_unary_callback, expect_arity, make_array_value, make_map_value,
    string_value, type_error,
};

pub(crate) fn sum(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    if args.len() > 1 {
        return Err(VmError::new(VmErrorKind::ArityMismatch {
            name: "sum".to_owned(),
            expected: 1,
            actual: args.len(),
        }));
    }
    let mut total = NumericTotal::default();
    if let Some(callback) = args.first() {
        let values = array_values(receiver, runtime.heap.as_deref(), "method sum")?;
        iteration::try_for_each_over(values, &mut runtime, "method sum", |runtime, value| {
            let mapped = call_unary_callback(runtime, "method sum", callback, value, &[])?;
            total.add_value(&mapped, "method sum")?;
            Ok(())
        })?;
    } else {
        return sum_values(receiver, runtime.heap.as_deref(), "method sum");
    }
    Ok(total.into_value())
}

pub(crate) fn sum_values(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Value> {
    let mut total = NumericTotal::default();
    total.add_receiver(receiver, heap, operation)?;
    Ok(total.into_value())
}

pub(crate) fn group_by(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("group_by", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method group_by")?;
    let mut groups = BTreeMap::<String, Vec<Value>>::new();
    iteration::try_for_each_over(values, &mut runtime, "method group_by", |runtime, value| {
        let key_value = if runtime.heap.is_some() {
            call_callback_with_protected_values(
                runtime,
                "method group_by",
                &args[0],
                std::slice::from_ref(&value),
                groups.values().flat_map(|values| values.iter()),
            )?
        } else {
            call_unary_callback(runtime, "method group_by", &args[0], value, &[])?
        };
        let key = group_key(&key_value, runtime.heap.as_deref())?;
        match groups.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(vec![value]);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                entry.get_mut().push(value);
            }
        }
        Ok(())
    })?;
    let mut heap_groups = BTreeMap::new();
    for (key, values) in groups {
        let value = make_array_value(
            values,
            &mut runtime.heap,
            &mut runtime.budget,
            "method group_by",
        )?;
        heap_groups.insert(key, value);
    }
    make_map_value(
        heap_groups,
        &mut runtime.heap,
        &mut runtime.budget,
        "method group_by",
    )
}

enum NumericTotal {
    Int(i64),
    Float(f64),
}

impl Default for NumericTotal {
    fn default() -> Self {
        Self::Int(0)
    }
}

impl NumericTotal {
    fn add_receiver(
        &mut self,
        receiver: &Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<()> {
        match receiver {
            Value::HeapRef(reference) => {
                let Some(HeapValue::Array(values)) =
                    heap.and_then(|heap| heap.heap.get(*reference))
                else {
                    return type_error(operation);
                };
                for value in values {
                    self.add_runtime_value(value, operation)?;
                }
                Ok(())
            }
            _ => type_error(operation),
        }
    }

    fn add_value(&mut self, value: &Value, operation: &'static str) -> VmResult<()> {
        match (&mut *self, value) {
            (NumericTotal::Int(total), Value::I64(value)) => {
                *total = total
                    .checked_add(*value)
                    .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
            }
            (NumericTotal::Int(total), Value::F64(value)) => {
                *self = NumericTotal::Float(*total as f64 + *value);
            }
            (NumericTotal::Float(total), Value::I64(value)) => {
                *total += *value as f64;
            }
            (NumericTotal::Float(total), Value::F64(value)) => {
                *total += *value;
            }
            _ => return type_error(operation),
        }
        Ok(())
    }

    fn add_runtime_value(&mut self, value: &Value, operation: &'static str) -> VmResult<()> {
        match (&mut *self, value) {
            (NumericTotal::Int(total), Value::I64(value)) => {
                *total = total
                    .checked_add(*value)
                    .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
            }
            (NumericTotal::Int(total), Value::F64(value)) => {
                *self = NumericTotal::Float(*total as f64 + *value);
            }
            (NumericTotal::Float(total), Value::I64(value)) => {
                *total += *value as f64;
            }
            (NumericTotal::Float(total), Value::F64(value)) => {
                *total += *value;
            }
            _ => return type_error(operation),
        }
        Ok(())
    }

    fn into_value(self) -> Value {
        match self {
            NumericTotal::Int(value) => Value::I64(value),
            NumericTotal::Float(value) => Value::F64(value),
        }
    }
}

fn group_key(value: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<String> {
    string_value(value, heap, "method group_by").map(str::to_owned)
}
