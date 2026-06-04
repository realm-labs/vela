use std::collections::BTreeMap;

use crate::method_runtime::MethodRuntime;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

use super::{array_values, call_unary_callback, expect_arity, string_value, type_error};

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
    let values = array_values(receiver, runtime.heap.as_deref(), "method sum")?;
    let mut total = NumericTotal::default();
    if let Some(callback) = args.first() {
        for value in values {
            let mapped = call_unary_callback(&mut runtime, "method sum", callback, value, &[])?;
            total.add_value(&mapped, "method sum")?;
        }
    } else {
        for value in values {
            total.add_value(&value, "method sum")?;
        }
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
    let mut groups = BTreeMap::<String, Value>::new();
    for value in values {
        let protected;
        let protected_values = if runtime.heap.is_some() {
            protected = groups.values().cloned().collect::<Vec<_>>();
            protected.as_slice()
        } else {
            &[]
        };
        let key_value = call_unary_callback(
            &mut runtime,
            "method group_by",
            &args[0],
            value.clone(),
            protected_values,
        )?;
        let key = group_key(&key_value, runtime.heap.as_deref())?;
        match groups.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Value::Array(vec![value]));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let Value::Array(values) = entry.get_mut() else {
                    unreachable!("group_by only stores array group values");
                };
                values.push(value);
            }
        }
    }
    Ok(Value::Map(groups))
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
    fn add_value(&mut self, value: &Value, operation: &'static str) -> VmResult<()> {
        match (&mut *self, value) {
            (NumericTotal::Int(total), Value::Int(value)) => {
                *total = total
                    .checked_add(*value)
                    .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
            }
            (NumericTotal::Int(total), Value::Float(value)) => {
                *self = NumericTotal::Float(*total as f64 + *value);
            }
            (NumericTotal::Float(total), Value::Int(value)) => {
                *total += *value as f64;
            }
            (NumericTotal::Float(total), Value::Float(value)) => {
                *total += *value;
            }
            _ => return type_error(operation),
        }
        Ok(())
    }

    fn into_value(self) -> Value {
        match self {
            NumericTotal::Int(value) => Value::Int(value),
            NumericTotal::Float(value) => Value::Float(value),
        }
    }
}

fn group_key(value: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<String> {
    string_value(value, heap, "method group_by").map(str::to_owned)
}
