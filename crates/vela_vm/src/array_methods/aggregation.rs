use std::collections::BTreeMap;

use crate::heap::HeapValue;
use crate::method_runtime::MethodRuntime;
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
        for value in values {
            let mapped = call_unary_callback(&mut runtime, "method sum", callback, value, &[])?;
            total.add_value(&mapped, "method sum")?;
        }
    } else {
        total.add_receiver(receiver, runtime.heap.as_deref(), "method sum")?;
    }
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
    for value in values {
        let protected;
        let protected_values = if runtime.heap.is_some() {
            protected = groups
                .values()
                .flat_map(|values| values.iter().copied())
                .collect::<Vec<_>>();
            protected.as_slice()
        } else {
            &[]
        };
        let key_value = call_unary_callback(
            &mut runtime,
            "method group_by",
            &args[0],
            value,
            protected_values,
        )?;
        let key = group_key(&key_value, runtime.heap.as_deref())?;
        match groups.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(vec![value]);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                entry.get_mut().push(value);
            }
        }
    }
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
            (NumericTotal::Int(total), Value::Scalar(vela_common::ScalarValue::I64(value))) => {
                *total = total
                    .checked_add(*value)
                    .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
            }
            (NumericTotal::Int(total), Value::Scalar(vela_common::ScalarValue::F64(value))) => {
                *self = NumericTotal::Float(*total as f64 + *value);
            }
            (NumericTotal::Float(total), Value::Scalar(vela_common::ScalarValue::I64(value))) => {
                *total += *value as f64;
            }
            (NumericTotal::Float(total), Value::Scalar(vela_common::ScalarValue::F64(value))) => {
                *total += *value;
            }
            _ => return type_error(operation),
        }
        Ok(())
    }

    fn add_runtime_value(&mut self, value: &Value, operation: &'static str) -> VmResult<()> {
        match (&mut *self, value) {
            (NumericTotal::Int(total), Value::Scalar(vela_common::ScalarValue::I64(value))) => {
                *total = total
                    .checked_add(*value)
                    .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
            }
            (NumericTotal::Int(total), Value::Scalar(vela_common::ScalarValue::F64(value))) => {
                *self = NumericTotal::Float(*total as f64 + *value);
            }
            (NumericTotal::Float(total), Value::Scalar(vela_common::ScalarValue::I64(value))) => {
                *total += *value as f64;
            }
            (NumericTotal::Float(total), Value::Scalar(vela_common::ScalarValue::F64(value))) => {
                *total += *value;
            }
            _ => return type_error(operation),
        }
        Ok(())
    }

    fn into_value(self) -> Value {
        match self {
            NumericTotal::Int(value) => Value::Scalar(vela_common::ScalarValue::I64(value)),
            NumericTotal::Float(value) => Value::Scalar(vela_common::ScalarValue::F64(value)),
        }
    }
}

fn group_key(value: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<String> {
    string_value(value, heap, "method group_by").map(str::to_owned)
}
